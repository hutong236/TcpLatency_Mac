use crate::{
    runtime::{all_snapshots, emit_active_snapshot, now_millis, SharedState},
    tray::refresh_tray_menu,
};
use serde::Serialize;
use std::{
    process::Command,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

const CHECK_INTERVAL: Duration = Duration::from_secs(15);
// 低电量条件带恢复回滞：低于阈值触发，回升到阈值 +5% 才解除，避免临界电量反复启停。
const LOW_RESUME_HYSTERESIS_PERCENT: u8 = 5;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BatteryStatus {
    pub(crate) available: bool,
    pub(crate) on_ac: bool,
    pub(crate) level_percent: Option<u8>,
    pub(crate) guard_paused: bool,
    pub(crate) reason: Option<String>,
    pub(crate) timestamp_ms: u128,
}

/// `pmset -g batt` 输出样例：
/// Now drawing from 'AC Power'
///  -InternalBattery-0 (id=36896867)\t80%; AC attached; not charging present: true
///
/// 返回 (是否外接电源, 电量百分比)；台式机无电池行时电量为 None。
fn parse_battery_output(output: &str) -> Option<(bool, Option<u8>)> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    let on_ac = !trimmed.contains("'Battery Power'");
    let level = trimmed.lines().find_map(|line| {
        let (before_percent, _) = line.split_once('%')?;
        let token = before_percent
            .rsplit(char::is_whitespace)
            .next()?
            .trim();
        token.parse::<u8>().ok()
    });
    Some((on_ac, level))
}

#[cfg(target_os = "macos")]
fn read_battery_blocking() -> Option<(bool, Option<u8>)> {
    let output = Command::new("/usr/bin/pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_battery_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
fn read_battery_blocking() -> Option<(bool, Option<u8>)> {
    None
}

async fn read_battery() -> Option<(bool, Option<u8>)> {
    tokio::task::spawn_blocking(read_battery_blocking)
        .await
        .ok()
        .flatten()
}

#[derive(Debug, Clone, Copy)]
struct GuardInput {
    pause_on_battery: bool,
    pause_low_enabled: bool,
    threshold_percent: u8,
    on_ac: bool,
    level_percent: Option<u8>,
}

/// 返回 (是否应暂停, 低电量条件是否激活)。电量未知或处于回滞区间时
/// 低电量条件维持上一状态，避免读数抖动导致启停。
fn evaluate(input: GuardInput, low_active: bool) -> (bool, bool) {
    let on_battery_active = input.pause_on_battery && !input.on_ac;
    let low_active = if !input.pause_low_enabled {
        false
    } else if input
        .level_percent
        .is_some_and(|level| level < input.threshold_percent)
    {
        true
    } else if input.level_percent.is_some_and(|level| {
        level >= input
            .threshold_percent
            .saturating_add(LOW_RESUME_HYSTERESIS_PERCENT)
    }) {
        false
    } else {
        low_active
    };
    (on_battery_active || low_active, low_active)
}

fn guard_reason(input: &GuardInput, low_active: bool) -> Option<String> {
    let on_battery = input.pause_on_battery && !input.on_ac;
    match (on_battery, low_active) {
        (true, true) => Some("on_battery+low_battery".into()),
        (true, false) => Some("on_battery".into()),
        (false, true) => Some("low_battery".into()),
        (false, false) => None,
    }
}

fn notify_transition(app: &AppHandle, paused: bool, input: &GuardInput) {
    let builder = app.notification().builder();
    let _ = if paused {
        let on_battery = input.pause_on_battery && !input.on_ac;
        let body = if on_battery {
            "已切换到电池供电，延迟与流量监测已自动暂停".to_string()
        } else {
            format!(
                "电量低于 {}%，延迟与流量监测已自动暂停",
                input.threshold_percent
            )
        };
        builder.title("TCP Latency 省电暂停").body(body).show()
    } else {
        builder
            .title("TCP Latency 已恢复")
            .body("供电条件恢复，监测已自动继续")
            .show()
    };
}

pub(crate) async fn battery_guard(app: AppHandle, state: Arc<SharedState>) {
    let mut low_active = false;

    loop {
        let config = state.config.read().map(|c| c.clone()).unwrap_or_default();

        let Some((on_ac, level)) = read_battery().await else {
            // 读取失败时维持现有暂停状态，只刷新可用性标记。
            let guard_paused = state.battery_paused.load(Ordering::Relaxed);
            if let Ok(mut cached) = state.battery.lock() {
                cached.available = false;
                cached.guard_paused = guard_paused;
                cached.timestamp_ms = now_millis();
                let snapshot = cached.clone();
                drop(cached);
                let _ = app.emit("battery-update", snapshot);
            }
            sleep_until_next_check(&state).await;
            continue;
        };

        let input = GuardInput {
            pause_on_battery: config.battery_pause_on_battery,
            pause_low_enabled: config.battery_pause_low_enabled,
            threshold_percent: config
                .battery_low_threshold_percent
                .min(u8::MAX as u32) as u8,
            on_ac,
            level_percent: level,
        };
        let (should_pause, next_low) = evaluate(input, low_active);
        low_active = next_low;

        let status = BatteryStatus {
            available: true,
            on_ac,
            level_percent: level,
            guard_paused: should_pause,
            reason: guard_reason(&input, next_low),
            timestamp_ms: now_millis(),
        };
        if let Ok(mut cached) = state.battery.lock() {
            *cached = status.clone();
        }
        let _ = app.emit("battery-update", status);

        let was_paused = state
            .battery_paused
            .swap(should_pause, Ordering::Relaxed);
        if was_paused != should_pause {
            state.scheduler_notify.notify_one();
            emit_active_snapshot(&app, state.as_ref());
            let _ = app.emit("targets-update", all_snapshots(state.as_ref()));
            refresh_tray_menu(&app, &state);
            if config.notifications_enabled {
                notify_transition(&app, should_pause, &input);
            }
        }

        sleep_until_next_check(&state).await;
    }
}

async fn sleep_until_next_check(state: &Arc<SharedState>) {
    tokio::select! {
        _ = tokio::time::sleep(CHECK_INTERVAL) => {}
        _ = state.battery_notify.notified() => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AC_OUTPUT: &str = concat!(
        "Now drawing from 'AC Power'\n",
        " -InternalBattery-0 (id=36896867)\t80%; AC attached; not charging present: true\n",
    );
    const BATTERY_OUTPUT: &str = concat!(
        "Now drawing from 'Battery Power'\n",
        " -InternalBattery-0 (id=36896867)\t45%; discharging; 2:11 remaining present: true\n",
    );
    const DESKTOP_OUTPUT: &str = "Now drawing from 'AC Power'\n";

    #[test]
    fn parses_ac_power_with_level() {
        let (on_ac, level) = parse_battery_output(AC_OUTPUT).unwrap();
        assert!(on_ac);
        assert_eq!(level, Some(80));
    }

    #[test]
    fn parses_battery_power_discharging() {
        let (on_ac, level) = parse_battery_output(BATTERY_OUTPUT).unwrap();
        assert!(!on_ac);
        assert_eq!(level, Some(45));
    }

    #[test]
    fn parses_desktop_without_battery_line() {
        let (on_ac, level) = parse_battery_output(DESKTOP_OUTPUT).unwrap();
        assert!(on_ac);
        assert_eq!(level, None);
    }

    #[test]
    fn blank_output_is_rejected() {
        assert!(parse_battery_output("   \n").is_none());
        assert!(parse_battery_output("").is_none());
    }

    fn input(on_ac: bool, level: Option<u8>) -> GuardInput {
        GuardInput {
            pause_on_battery: true,
            pause_low_enabled: true,
            threshold_percent: 20,
            on_ac,
            level_percent: level,
        }
    }

    #[test]
    fn on_battery_condition_triggers_and_clears() {
        let (paused, _) = evaluate(input(false, Some(80)), false);
        assert!(paused);
        let (paused, _) = evaluate(input(true, Some(80)), false);
        assert!(!paused);
    }

    #[test]
    fn low_battery_condition_uses_hysteresis() {
        let (paused, low) = evaluate(input(true, Some(19)), false);
        assert!(paused);
        assert!(low);

        let (paused, low) = evaluate(input(true, Some(21)), low);
        assert!(paused);
        assert!(low);

        let (paused, low) = evaluate(input(true, Some(25)), low);
        assert!(!paused);
        assert!(!low);
    }

    #[test]
    fn disabled_switches_never_pause() {
        let off = GuardInput {
            pause_on_battery: false,
            pause_low_enabled: false,
            threshold_percent: 20,
            on_ac: false,
            level_percent: Some(3),
        };
        assert_eq!(evaluate(off, true), (false, false));
    }

    #[test]
    fn unknown_level_keeps_previous_low_state() {
        let (_, low) = evaluate(input(true, Some(15)), false);
        assert!(low);
        let (paused, low) = evaluate(input(true, None), low);
        assert!(paused);
        assert!(low);
    }

    #[test]
    fn reason_combines_active_conditions() {
        let both = input(false, Some(15));
        assert_eq!(
            guard_reason(&both, true).as_deref(),
            Some("on_battery+low_battery")
        );
        let neither = input(true, Some(80));
        assert_eq!(guard_reason(&neither, false), None);
    }
}
