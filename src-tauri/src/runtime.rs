use crate::{
    config::{endpoint_key, AppConfig, TargetConfig},
    probe::{tcp_probe, ProbeResult},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

pub(crate) const TRAY_ID: &str = "latency-tray";
const HISTORY_WINDOW_MS: u128 = 60_000;
const MAX_HISTORY_POINTS: usize = 600;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeSnapshot {
    pub(crate) target_id: String,
    pub(crate) target_name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) enabled: bool,
    pub(crate) current_ms: Option<f64>,
    pub(crate) average_ms: Option<f64>,
    pub(crate) min_ms: Option<f64>,
    pub(crate) max_ms: Option<f64>,
    pub(crate) jitter_ms: Option<f64>,
    pub(crate) p95_ms: Option<f64>,
    pub(crate) failure_percent: f64,
    pub(crate) sample_count: usize,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    pub(crate) paused: bool,
    pub(crate) timestamp_ms: u128,
    pub(crate) sample_age_ms: u128,
    pub(crate) dns_ms: Option<f64>,
    pub(crate) resolved_address: Option<String>,
}

impl ProbeSnapshot {
    fn for_target(target: &TargetConfig) -> Self {
        Self {
            target_id: target.id.clone(),
            target_name: target.name.clone(),
            host: target.host.clone(),
            port: target.port,
            enabled: target.enabled,
            current_ms: None,
            average_ms: None,
            min_ms: None,
            max_ms: None,
            jitter_ms: None,
            p95_ms: None,
            failure_percent: 0.0,
            sample_count: 0,
            status: if target.enabled { "starting" } else { "disabled" }.into(),
            error: None,
            paused: false,
            timestamp_ms: now_millis(),
            sample_age_ms: 0,
            dns_ms: None,
            resolved_address: None,
        }
    }

    fn empty() -> Self {
        Self::for_target(&TargetConfig::default())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryPoint {
    timestamp_ms: u128,
    latency_ms: Option<f64>,
    status: String,
}

#[derive(Debug, Clone)]
struct Sample {
    timestamp_ms: u128,
    latency_ms: Option<f64>,
    status: String,
}

struct TargetRuntime {
    samples: VecDeque<Sample>,
    snapshot: ProbeSnapshot,
    last_probe_started: Option<Instant>,
    consecutive_high: u32,
    consecutive_failure: u32,
    last_notification_ms: u128,
    incident: Option<String>,
    endpoint_key: String,
}

impl TargetRuntime {
    fn new(target: &TargetConfig) -> Self {
        Self {
            samples: VecDeque::with_capacity(128),
            snapshot: ProbeSnapshot::for_target(target),
            last_probe_started: None,
            consecutive_high: 0,
            consecutive_failure: 0,
            last_notification_ms: 0,
            incident: None,
            endpoint_key: endpoint_key(target),
        }
    }
}

pub(crate) struct SharedState {
    pub(crate) config: RwLock<AppConfig>,
    pub(crate) paused: AtomicBool,
    runtimes: Mutex<HashMap<String, TargetRuntime>>,
    inflight: Mutex<HashMap<String, u64>>,
    pub(crate) generation: AtomicU64,
    pub(crate) scheduler_notify: Notify,
}

impl SharedState {
    pub(crate) fn new(config: AppConfig) -> Self {
        let mut runtimes = HashMap::new();
        for target in &config.targets {
            runtimes.insert(target.id.clone(), TargetRuntime::new(target));
        }
        Self {
            config: RwLock::new(config),
            paused: AtomicBool::new(false),
            runtimes: Mutex::new(runtimes),
            inflight: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(1),
            scheduler_notify: Notify::new(),
        }
    }

    pub(crate) fn reconcile_targets(&self, config: &AppConfig) {
        if let Ok(mut runtimes) = self.runtimes.lock() {
            let target_ids: HashSet<&str> = config.targets.iter().map(|t| t.id.as_str()).collect();
            runtimes.retain(|id, _| target_ids.contains(id.as_str()));

            for target in &config.targets {
                let key = endpoint_key(target);
                let replace = runtimes
                    .get(&target.id)
                    .map(|runtime| runtime.endpoint_key != key)
                    .unwrap_or(false);
                if replace {
                    runtimes.insert(target.id.clone(), TargetRuntime::new(target));
                    continue;
                }

                let runtime = runtimes
                    .entry(target.id.clone())
                    .or_insert_with(|| TargetRuntime::new(target));
                let was_enabled = runtime.snapshot.enabled;
                runtime.snapshot.target_name = target.name.clone();
                runtime.snapshot.host = target.host.clone();
                runtime.snapshot.port = target.port;
                runtime.snapshot.enabled = target.enabled;
                runtime.endpoint_key = key;
                if target.enabled && !was_enabled {
                    runtime.last_probe_started = None;
                    runtime.snapshot.status = "starting".into();
                }
                if !target.enabled {
                    runtime.snapshot.status = "disabled".into();
                    runtime.snapshot.current_ms = None;
                    runtime.snapshot.error = None;
                    runtime.incident = None;
                }
            }
        }
    }
}

#[derive(Debug)]
struct AlertRequest {
    title: String,
    body: String,
}

fn stale_after_ms(target: &TargetConfig) -> u128 {
    (target.interval_ms.saturating_mul(3).saturating_add(target.timeout_ms)).max(5_000) as u128
}

fn apply_freshness(target: &TargetConfig, mut snapshot: ProbeSnapshot, paused: bool) -> ProbeSnapshot {
    snapshot.paused = paused;
    snapshot.sample_age_ms = now_millis().saturating_sub(snapshot.timestamp_ms);
    if paused && target.enabled {
        snapshot.status = "paused".into();
        return snapshot;
    }
    if target.enabled
        && snapshot.status != "starting"
        && snapshot.status != "disabled"
        && snapshot.sample_age_ms > stale_after_ms(target)
    {
        snapshot.status = "stale".into();
        snapshot.current_ms = None;
        snapshot.error = Some(format!(
            "超过 {} ms 未收到新的探测结果",
            stale_after_ms(target)
        ));
    }
    snapshot
}

pub(crate) fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn prune_history(samples: &mut VecDeque<Sample>, now: u128) {
    while samples
        .front()
        .map(|sample| now.saturating_sub(sample.timestamp_ms) > HISTORY_WINDOW_MS)
        .unwrap_or(false)
    {
        samples.pop_front();
    }
    while samples.len() > MAX_HISTORY_POINTS {
        samples.pop_front();
    }
}

fn stats(
    samples: &VecDeque<Sample>,
) -> (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    f64,
    usize,
) {
    if samples.is_empty() {
        return (None, None, None, None, None, 0.0, 0);
    }

    let mut successful: Vec<f64> = samples.iter().filter_map(|sample| sample.latency_ms).collect();
    let failures = samples.len().saturating_sub(successful.len());
    let failure_percent = failures as f64 / samples.len() as f64 * 100.0;

    if successful.is_empty() {
        return (None, None, None, None, None, failure_percent, samples.len());
    }

    let sum: f64 = successful.iter().sum();
    let average = sum / successful.len() as f64;
    let min = successful.iter().copied().fold(f64::INFINITY, f64::min);
    let max = successful
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    let jitter = if successful.len() >= 2 {
        let jitter_sum: f64 = successful.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        Some(jitter_sum / (successful.len() - 1) as f64)
    } else {
        None
    };

    successful.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = (((successful.len() as f64) * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(successful.len() - 1);
    let p95 = Some(successful[index]);

    (
        Some(average),
        Some(min),
        Some(max),
        jitter,
        p95,
        failure_percent,
        samples.len(),
    )
}

pub(crate) fn snapshot_for_active(state: &SharedState) -> ProbeSnapshot {
    let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
    let paused = state.paused.load(Ordering::Relaxed);
    let Some(target) = config.active_target() else {
        return ProbeSnapshot::empty();
    };

    let snapshot = state
        .runtimes
        .lock()
        .ok()
        .and_then(|runtimes| runtimes.get(&target.id).map(|runtime| runtime.snapshot.clone()))
        .unwrap_or_else(|| ProbeSnapshot::for_target(target));
    apply_freshness(target, snapshot, paused)
}

pub(crate) fn all_snapshots(state: &SharedState) -> Vec<ProbeSnapshot> {
    let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
    let paused = state.paused.load(Ordering::Relaxed);
    let runtimes = state.runtimes.lock().ok();

    config
        .targets
        .iter()
        .map(|target| {
            let snapshot = runtimes
                .as_ref()
                .and_then(|map| map.get(&target.id).map(|runtime| runtime.snapshot.clone()))
                .unwrap_or_else(|| ProbeSnapshot::for_target(target));
            apply_freshness(target, snapshot, paused)
        })
        .collect()
}

pub(crate) fn history_for_target(state: &SharedState, target_id: &str) -> Vec<HistoryPoint> {
    let now = now_millis();
    state
        .runtimes
        .lock()
        .ok()
        .and_then(|mut runtimes| {
            let runtime = runtimes.get_mut(target_id)?;
            prune_history(&mut runtime.samples, now);
            Some(
                runtime
                    .samples
                    .iter()
                    .map(|sample| HistoryPoint {
                        timestamp_ms: sample.timestamp_ms,
                        latency_ms: sample.latency_ms,
                        status: sample.status.clone(),
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn tray_title(snapshot: &ProbeSnapshot) -> String {
    if snapshot.paused {
        return "Paused".into();
    }
    if !snapshot.enabled {
        return "Disabled".into();
    }
    if let Some(ms) = snapshot.current_ms {
        return format!("{} ms", ms.round() as u64);
    }
    match snapshot.status.as_str() {
        "timeout" => "Timeout".into(),
        "refused" => "Refused".into(),
        "offline" => "Offline".into(),
        "dns_timeout" => "DNS Timeout".into(),
        "dns_error" => "DNS Error".into(),
        "stale" => "Stale".into(),
        "disabled" => "Disabled".into(),
        _ => "-- ms".into(),
    }
}

pub(crate) fn emit_active_snapshot(app: &AppHandle, state: &SharedState) {
    let snapshot = snapshot_for_active(state);
    let _ = app.emit("latency-update", snapshot.clone());
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(Some(tray_title(&snapshot)));
        let endpoint = snapshot.resolved_address.as_deref().unwrap_or("unresolved");
        let tooltip = format!(
            "{} · {}:{} · {} · P95 {} · 失败 {:.1}%",
            snapshot.target_name,
            snapshot.host,
            snapshot.port,
            endpoint,
            snapshot
                .p95_ms
                .map(|v| format!("{v:.0}ms"))
                .unwrap_or_else(|| "--".into()),
            snapshot.failure_percent
        );
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn emit_target_snapshot(app: &AppHandle, state: &SharedState, snapshot: &ProbeSnapshot) {
    let _ = app.emit("target-update", snapshot.clone());
    let active_id = state
        .config
        .read()
        .ok()
        .map(|c| c.active_target_id.clone())
        .unwrap_or_default();
    if active_id == snapshot.target_id {
        emit_active_snapshot(app, state);
    }
}

fn send_notification(app: &AppHandle, alert: AlertRequest) {
    let _ = app
        .notification()
        .builder()
        .title(alert.title)
        .body(alert.body)
        .show();
}

fn complete_probe(
    state: &SharedState,
    target: &TargetConfig,
    config: &AppConfig,
    result: ProbeResult,
) -> (ProbeSnapshot, Option<AlertRequest>) {
    let now = now_millis();
    let latency = result.latency_ms;
    let status = result.status.clone();

    // Keep the global runtime lock only for the mutation/copy step. Sorting for
    // P95 and the rest of the statistics happens after the lock is released so
    // another target can update/read its state concurrently.
    let samples = {
        let Ok(mut runtimes) = state.runtimes.lock() else {
            let mut snapshot = ProbeSnapshot::for_target(target);
            snapshot.status = "offline".into();
            snapshot.error = Some("runtime lock failed".into());
            return (snapshot, None);
        };
        let runtime = runtimes
            .entry(target.id.clone())
            .or_insert_with(|| TargetRuntime::new(target));
        runtime.samples.push_back(Sample {
            timestamp_ms: now,
            latency_ms: latency,
            status: status.clone(),
        });
        prune_history(&mut runtime.samples, now);
        runtime.samples.clone()
    };

    let (avg, min, max, jitter, p95, failure, count) = stats(&samples);

    let Ok(mut runtimes) = state.runtimes.lock() else {
        let mut snapshot = ProbeSnapshot::for_target(target);
        snapshot.status = "offline".into();
        snapshot.error = Some("runtime lock failed".into());
        return (snapshot, None);
    };
    let runtime = runtimes
        .entry(target.id.clone())
        .or_insert_with(|| TargetRuntime::new(target));

    runtime.snapshot = ProbeSnapshot {
        target_id: target.id.clone(),
        target_name: target.name.clone(),
        host: target.host.clone(),
        port: target.port,
        enabled: target.enabled,
        current_ms: latency,
        average_ms: avg,
        min_ms: min,
        max_ms: max,
        jitter_ms: jitter,
        p95_ms: p95,
        failure_percent: failure,
        sample_count: count,
        status: status.clone(),
        error: result.error.clone(),
        paused: false,
        timestamp_ms: now,
        sample_age_ms: 0,
        dns_ms: result.dns_ms,
        resolved_address: result.resolved_address.clone(),
    };

    let previous_incident = runtime.incident.clone();
    if latency.is_none() {
        runtime.consecutive_failure = runtime.consecutive_failure.saturating_add(1);
        runtime.consecutive_high = 0;
    } else {
        runtime.consecutive_failure = 0;
        if latency.unwrap_or_default() >= config.thresholds.high_ms {
            runtime.consecutive_high = runtime.consecutive_high.saturating_add(1);
        } else {
            runtime.consecutive_high = 0;
        }
    }

    let recovered_kind = match previous_incident.as_deref() {
        Some("failure") if latency.is_some() => Some("failure"),
        Some("high") if latency.map(|ms| ms < config.thresholds.high_ms).unwrap_or(false) => Some("high"),
        _ => None,
    };
    if recovered_kind.is_some() {
        runtime.incident = None;
    }

    let recovery = if config.notifications_enabled && config.notify_recovery {
        match recovered_kind {
            Some("failure") => Some(AlertRequest {
                title: format!("{} 已恢复", target.name),
                body: format!(
                    "{}:{} 已恢复可达，当前 {:.0} ms",
                    target.host,
                    target.port,
                    latency.unwrap_or_default()
                ),
            }),
            Some("high") => Some(AlertRequest {
                title: format!("{} 延迟恢复", target.name),
                body: format!(
                    "当前 {:.0} ms，已回落到 {:.0} ms 阈值以下",
                    latency.unwrap_or_default(),
                    config.thresholds.high_ms
                ),
            }),
            _ => None,
        }
    } else {
        None
    };

    if recovery.is_some() {
        return (runtime.snapshot.clone(), recovery);
    }

    let cooldown_ms = config.notification_cooldown_sec as u128 * 1000;
    let can_notify = config.notifications_enabled
        && now.saturating_sub(runtime.last_notification_ms) >= cooldown_ms;

    let alert = if can_notify && runtime.consecutive_failure >= config.notify_consecutive_failure {
        runtime.last_notification_ms = now;
        runtime.incident = Some("failure".into());
        Some(AlertRequest {
            title: format!("{} 不可达", target.name),
            body: format!(
                "{}:{} 已连续 {} 次 TCP 探测失败（{}）",
                target.host, target.port, runtime.consecutive_failure, status
            ),
        })
    } else if can_notify && runtime.consecutive_high >= config.notify_consecutive_high {
        runtime.last_notification_ms = now;
        runtime.incident = Some("high".into());
        Some(AlertRequest {
            title: format!("{} 延迟异常", target.name),
            body: format!(
                "当前 {:.0} ms，已连续 {} 次高于 {:.0} ms",
                latency.unwrap_or_default(),
                runtime.consecutive_high,
                config.thresholds.high_ms
            ),
        })
    } else {
        None
    };

    (runtime.snapshot.clone(), alert)
}

fn claim_probe(state: &SharedState, target: &TargetConfig, now: Instant, generation: u64) -> bool {
    if !target.enabled {
        return false;
    }

    let Ok(mut inflight) = state.inflight.lock() else {
        return false;
    };
    if inflight.get(&target.id).copied() == Some(generation) {
        return false;
    }

    let Ok(mut runtimes) = state.runtimes.lock() else {
        return false;
    };
    let runtime = runtimes
        .entry(target.id.clone())
        .or_insert_with(|| TargetRuntime::new(target));

    let due = runtime
        .last_probe_started
        .map(|started| {
            now.saturating_duration_since(started) >= Duration::from_millis(target.interval_ms)
        })
        .unwrap_or(true);
    if !due {
        return false;
    }

    runtime.last_probe_started = Some(now);
    inflight.insert(target.id.clone(), generation);
    true
}

fn next_probe_delay(
    state: &SharedState,
    config: &AppConfig,
    now: Instant,
    generation: u64,
) -> Option<Duration> {
    let inflight = state.inflight.lock().ok()?;
    let runtimes = state.runtimes.lock().ok()?;

    config
        .targets
        .iter()
        .filter(|target| target.enabled)
        .filter(|target| inflight.get(&target.id).copied() != Some(generation))
        .map(|target| {
            let interval = Duration::from_millis(target.interval_ms);
            runtimes
                .get(&target.id)
                .and_then(|runtime| runtime.last_probe_started)
                .map(|started| interval.saturating_sub(now.saturating_duration_since(started)))
                .unwrap_or(Duration::ZERO)
        })
        .min()
}

fn clear_inflight(state: &SharedState, target_id: &str, generation: u64) {
    let cleared = if let Ok(mut inflight) = state.inflight.lock() {
        if inflight.get(target_id).copied() == Some(generation) {
            inflight.remove(target_id);
            true
        } else {
            false
        }
    } else {
        false
    };

    if cleared {
        state.scheduler_notify.notify_one();
    }
}

pub(crate) async fn probe_scheduler(app: AppHandle, state: Arc<SharedState>) {
    loop {
        if state.paused.load(Ordering::Relaxed) {
            state.scheduler_notify.notified().await;
            continue;
        }

        let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
        let generation = state.generation.load(Ordering::Relaxed);
        let now = Instant::now();

        for target in config.targets.iter().filter(|target| target.enabled) {
            if !claim_probe(&state, target, now, generation) {
                continue;
            }

            let app_handle = app.clone();
            let loop_state = state.clone();
            let target = target.clone();
            tauri::async_runtime::spawn(async move {
                let result = tcp_probe(&target).await;
                if loop_state.generation.load(Ordering::Relaxed) != generation {
                    clear_inflight(&loop_state, &target.id, generation);
                    return;
                }
                let current_config = loop_state.config.read().map(|c| c.clone()).unwrap_or_default();
                let (snapshot, alert) = complete_probe(&loop_state, &target, &current_config, result);
                clear_inflight(&loop_state, &target.id, generation);
                emit_target_snapshot(&app_handle, &loop_state, &snapshot);
                if let Some(alert) = alert {
                    send_notification(&app_handle, alert);
                }
            });
        }

        let wait = next_probe_delay(&state, &config, Instant::now(), generation)
            .unwrap_or(Duration::from_secs(60));
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = state.scheduler_notify.notified() => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ts: u128, latency: Option<f64>, status: &str) -> Sample {
        Sample {
            timestamp_ms: ts,
            latency_ms: latency,
            status: status.to_string(),
        }
    }

    #[test]
    fn stats_include_failure_rate_and_jitter() {
        let samples = VecDeque::from(vec![
            sample(1, Some(10.0), "ok"),
            sample(2, Some(20.0), "ok"),
            sample(3, None, "timeout"),
            sample(4, Some(30.0), "ok"),
        ]);
        let (avg, min, max, jitter, p95, failure, count) = stats(&samples);
        assert_eq!(count, 4);
        assert_eq!(avg, Some(20.0));
        assert_eq!(min, Some(10.0));
        assert_eq!(max, Some(30.0));
        assert_eq!(jitter, Some(10.0));
        assert_eq!(p95, Some(30.0));
        assert_eq!(failure, 25.0);
    }

    #[test]
    fn stale_threshold_has_safe_floor() {
        let mut target = TargetConfig::default();
        target.interval_ms = 200;
        target.timeout_ms = 100;
        assert_eq!(stale_after_ms(&target), 5_000);
    }

    #[test]
    fn scheduler_deadline_is_immediate_before_first_probe() {
        let config = AppConfig::default();
        let state = SharedState::new(config.clone());
        let generation = state.generation.load(Ordering::Relaxed);
        let delay = next_probe_delay(&state, &config, Instant::now(), generation);
        assert_eq!(delay, Some(Duration::ZERO));
    }

    #[test]
    fn claimed_probe_waits_for_completion_before_rescheduling() {
        let config = AppConfig::default();
        let state = SharedState::new(config.clone());
        let target = config.targets[0].clone();
        let generation = state.generation.load(Ordering::Relaxed);

        assert!(claim_probe(&state, &target, Instant::now(), generation));
        assert_eq!(next_probe_delay(&state, &config, Instant::now(), generation), None);

        clear_inflight(&state, &target.id, generation);
        let delay = next_probe_delay(&state, &config, Instant::now(), generation).unwrap();
        assert!(delay > Duration::ZERO);
        assert!(delay <= Duration::from_millis(target.interval_ms));
    }
}
