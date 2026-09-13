use crate::runtime::{now_millis, SharedState};
use serde::Serialize;
use std::{
    process::Command,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const ROUTE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const MAX_REASONABLE_BYTES_PER_SEC: f64 = 2_500_000_000.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrafficSnapshot {
    enabled: bool,
    interface_name: Option<String>,
    download_bytes_per_sec: f64,
    upload_bytes_per_sec: f64,
    status: String,
    timestamp_ms: u128,
}

impl TrafficSnapshot {
    fn disabled() -> Self {
        Self {
            enabled: false,
            interface_name: None,
            download_bytes_per_sec: 0.0,
            upload_bytes_per_sec: 0.0,
            status: "disabled".into(),
            timestamp_ms: now_millis(),
        }
    }

    fn unavailable(interface_name: Option<String>) -> Self {
        Self {
            enabled: true,
            interface_name,
            download_bytes_per_sec: 0.0,
            upload_bytes_per_sec: 0.0,
            status: "unavailable".into(),
            timestamp_ms: now_millis(),
        }
    }
}

#[derive(Debug, Clone)]
struct InterfaceCounters {
    name: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Debug)]
struct PreviousSample {
    interface_name: String,
    rx_bytes: u64,
    tx_bytes: u64,
    sampled_at: Instant,
}

#[derive(Debug, Default)]
struct TrafficRuntime {
    previous: Option<PreviousSample>,
    route_interface: Option<String>,
    route_checked_at: Option<Instant>,
    was_enabled: bool,
}

fn eligible_interface(name: &str) -> bool {
    !matches!(name, "lo0" | "awdl0" | "llw0")
}

#[cfg(target_os = "macos")]
fn read_interface_counters() -> Vec<InterfaceCounters> {
    use std::{ffi::CStr, ptr};

    let mut head: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 || head.is_null() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut cursor = head;
    while !cursor.is_null() {
        let ifa = unsafe { &*cursor };
        let addr = ifa.ifa_addr;
        let is_up = (ifa.ifa_flags as i32 & libc::IFF_UP) != 0;

        if is_up && !addr.is_null() && unsafe { (*addr).sa_family as i32 } == libc::AF_LINK {
            let name = unsafe { CStr::from_ptr(ifa.ifa_name) }
                .to_string_lossy()
                .into_owned();
            if eligible_interface(&name) && !ifa.ifa_data.is_null() {
                let data = unsafe { &*(ifa.ifa_data as *const libc::if_data) };
                result.push(InterfaceCounters {
                    name,
                    rx_bytes: data.ifi_ibytes as u64,
                    tx_bytes: data.ifi_obytes as u64,
                });
            }
        }

        cursor = ifa.ifa_next;
    }

    unsafe { libc::freeifaddrs(head) };
    result
}

#[cfg(not(target_os = "macos"))]
fn read_interface_counters() -> Vec<InterfaceCounters> {
    Vec::new()
}

fn fallback_interface(counters: &[InterfaceCounters]) -> Option<String> {
    counters
        .iter()
        .find(|item| item.name == "en0")
        .or_else(|| counters.iter().find(|item| item.name.starts_with("en")))
        .or_else(|| counters.iter().find(|item| item.name.starts_with("utun")))
        .or_else(|| counters.first())
        .map(|item| item.name.clone())
}

#[cfg(target_os = "macos")]
fn read_default_route_interface_blocking() -> Option<String> {
    let output = Command::new("/sbin/route")
        .args(["-n", "get", "default"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("interface:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[cfg(not(target_os = "macos"))]
fn read_default_route_interface_blocking() -> Option<String> {
    None
}

async fn read_default_route_interface() -> Option<String> {
    tokio::task::spawn_blocking(read_default_route_interface_blocking)
        .await
        .ok()
        .flatten()
}

fn counter_delta(current: u64, previous: u64) -> u64 {
    if current >= previous {
        current - previous
    } else {
        // macOS getifaddrs exposes legacy if_data counters. On systems where
        // those counters are 32-bit, account for one wrap between 1s samples.
        (u32::MAX as u64 + 1).saturating_sub(previous) + current
    }
}

fn calculate_snapshot(
    runtime: &mut TrafficRuntime,
    counter: &InterfaceCounters,
) -> TrafficSnapshot {
    let now = Instant::now();
    let timestamp_ms = now_millis();

    let Some(previous) = runtime.previous.take() else {
        runtime.previous = Some(PreviousSample {
            interface_name: counter.name.clone(),
            rx_bytes: counter.rx_bytes,
            tx_bytes: counter.tx_bytes,
            sampled_at: now,
        });
        return TrafficSnapshot {
            enabled: true,
            interface_name: Some(counter.name.clone()),
            download_bytes_per_sec: 0.0,
            upload_bytes_per_sec: 0.0,
            status: "starting".into(),
            timestamp_ms,
        };
    };

    let elapsed = now.saturating_duration_since(previous.sampled_at).as_secs_f64();
    let same_interface = previous.interface_name == counter.name;
    let sane_interval = (0.25..=5.0).contains(&elapsed);

    let (mut download, mut upload, status) = if same_interface && sane_interval {
        (
            counter_delta(counter.rx_bytes, previous.rx_bytes) as f64 / elapsed,
            counter_delta(counter.tx_bytes, previous.tx_bytes) as f64 / elapsed,
            "ok",
        )
    } else {
        (0.0, 0.0, "starting")
    };

    // A counter reset or unexpected ABI interpretation should never paint a
    // multi-GB/s spike into the HUD. Re-baseline instead.
    if download > MAX_REASONABLE_BYTES_PER_SEC || upload > MAX_REASONABLE_BYTES_PER_SEC {
        download = 0.0;
        upload = 0.0;
    }

    runtime.previous = Some(PreviousSample {
        interface_name: counter.name.clone(),
        rx_bytes: counter.rx_bytes,
        tx_bytes: counter.tx_bytes,
        sampled_at: now,
    });

    TrafficSnapshot {
        enabled: true,
        interface_name: Some(counter.name.clone()),
        download_bytes_per_sec: download,
        upload_bytes_per_sec: upload,
        status: status.into(),
        timestamp_ms,
    }
}

async fn selected_interface(
    configured: &str,
    counters: &[InterfaceCounters],
    runtime: &mut TrafficRuntime,
) -> Option<String> {
    if configured != "auto" {
        return counters
            .iter()
            .find(|item| item.name == configured)
            .map(|item| item.name.clone());
    }

    let route_stale = runtime
        .route_checked_at
        .map(|checked| checked.elapsed() >= ROUTE_REFRESH_INTERVAL)
        .unwrap_or(true);

    if route_stale {
        runtime.route_interface = read_default_route_interface().await;
        runtime.route_checked_at = Some(Instant::now());
    }

    runtime
        .route_interface
        .as_ref()
        .filter(|name| counters.iter().any(|item| &item.name == *name))
        .cloned()
        .or_else(|| fallback_interface(counters))
}

fn emit_snapshot(app: &AppHandle, snapshot: TrafficSnapshot) {
    let _ = app.emit("traffic-update", snapshot);
}

pub(crate) async fn traffic_sampler(app: AppHandle, state: Arc<SharedState>) {
    let mut runtime = TrafficRuntime::default();

    loop {
        let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
        if !config.floating_show_traffic || state.battery_paused.load(Ordering::Relaxed) {
            runtime.previous = None;
            runtime.route_checked_at = None;
            if runtime.was_enabled {
                emit_snapshot(&app, TrafficSnapshot::disabled());
            }
            runtime.was_enabled = false;
            tokio::time::sleep(SAMPLE_INTERVAL).await;
            continue;
        }

        runtime.was_enabled = true;
        let counters = read_interface_counters();
        let configured = config.traffic_interface.trim().to_ascii_lowercase();
        let selected = selected_interface(&configured, &counters, &mut runtime).await;

        let snapshot = if let Some(interface_name) = selected {
            if let Some(counter) = counters.iter().find(|item| item.name == interface_name) {
                calculate_snapshot(&mut runtime, counter)
            } else {
                runtime.previous = None;
                TrafficSnapshot::unavailable(Some(interface_name))
            }
        } else {
            runtime.previous = None;
            TrafficSnapshot::unavailable(if configured == "auto" {
                None
            } else {
                Some(configured)
            })
        };

        emit_snapshot(&app, snapshot);
        tokio::time::sleep(SAMPLE_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_wrap_is_handled() {
        assert_eq!(counter_delta(10, u32::MAX as u64 - 5), 16);
    }

    #[test]
    fn fallback_prefers_en0() {
        let counters = vec![
            InterfaceCounters { name: "utun2".into(), rx_bytes: 0, tx_bytes: 0 },
            InterfaceCounters { name: "en0".into(), rx_bytes: 0, tx_bytes: 0 },
        ];
        assert_eq!(fallback_interface(&counters).as_deref(), Some("en0"));
    }
}
