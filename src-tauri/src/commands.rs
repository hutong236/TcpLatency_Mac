use crate::{
    config::{persist_config, validate_config, AppConfig, TargetConfig},
    macos_window::{
        apply_floating_window_effect, apply_floating_window_size, set_floating_visibility,
        set_mouse_passthrough_native, show_settings_window,
    },
    probe::{tcp_probe, ProbeResult},
    runtime::{
        all_snapshots, emit_active_snapshot, history_for_target, snapshot_for_active, HistoryPoint,
        ProbeSnapshot, SharedState,
    },
    tray::refresh_tray_menu,
};
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub(crate) fn get_config(state: State<'_, Arc<SharedState>>) -> AppConfig {
    state.config.read().map(|c| c.clone()).unwrap_or_default()
}

#[tauri::command]
pub(crate) fn get_snapshot(state: State<'_, Arc<SharedState>>) -> ProbeSnapshot {
    snapshot_for_active(state.inner().as_ref())
}

#[tauri::command]
pub(crate) fn get_all_snapshots(state: State<'_, Arc<SharedState>>) -> Vec<ProbeSnapshot> {
    all_snapshots(state.inner().as_ref())
}

#[tauri::command]
pub(crate) fn get_history(
    state: State<'_, Arc<SharedState>>,
    target_id: String,
) -> Vec<HistoryPoint> {
    history_for_target(state.inner().as_ref(), &target_id)
}

#[tauri::command]
pub(crate) async fn test_target(mut target: TargetConfig) -> Result<ProbeResult, String> {
    target.name = target.name.trim().to_string();
    target.host = target.host.trim().to_string();
    target.address_family = target.address_family.trim().to_ascii_lowercase();
    if target.host.is_empty() {
        return Err("Host / IP 不能为空".into());
    }
    if target.port == 0 {
        return Err("TCP Port 无效".into());
    }
    if !(100..=30_000).contains(&target.timeout_ms) {
        return Err("连接超时必须在 100~30000ms 之间".into());
    }
    if !matches!(target.address_family.as_str(), "auto" | "ipv4" | "ipv6") {
        return Err("地址族必须是 auto / ipv4 / ipv6".into());
    }
    Ok(tcp_probe(&target).await)
}

#[tauri::command]
pub(crate) fn save_config(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let config = validate_config(config)?;
    sync_autostart(&app, config.autostart)?;
    if config.notifications_enabled {
        let _ = app.notification().request_permission();
    }
    persist_config(&config)?;

    if let Ok(mut current) = state.config.write() {
        *current = config.clone();
    }
    state.generation.fetch_add(1, Ordering::Relaxed);
    state.reconcile_targets(&config);
    state.scheduler_notify.notify_one();

    apply_floating_window_size(&app, &config.floating_size)?;
    apply_floating_window_effect(&app, &config.floating_size)?;
    set_floating_visibility(&app, config.show_floating);
    set_mouse_passthrough_native(&app, config.mouse_passthrough)?;
    refresh_tray_menu(&app, state.inner());
    emit_active_snapshot(&app, state.inner().as_ref());
    let _ = app.emit("config-update", config.clone());
    let _ = app.emit("targets-update", all_snapshots(state.inner().as_ref()));
    Ok(config)
}

#[tauri::command]
pub(crate) fn set_paused(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    paused: bool,
) -> bool {
    state.paused.store(paused, Ordering::Relaxed);
    state.scheduler_notify.notify_one();
    refresh_tray_menu(&app, state.inner());
    emit_active_snapshot(&app, state.inner().as_ref());
    let _ = app.emit("targets-update", all_snapshots(state.inner().as_ref()));
    paused
}

#[tauri::command]
pub(crate) fn is_paused(state: State<'_, Arc<SharedState>>) -> bool {
    state.paused.load(Ordering::Relaxed)
}

#[tauri::command]
pub(crate) fn toggle_floating(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
) -> Result<bool, String> {
    let mut config = state.config.write().map_err(|_| "配置锁异常".to_string())?;
    config.show_floating = !config.show_floating;
    persist_config(&config)?;
    set_floating_visibility(&app, config.show_floating);
    let _ = app.emit("config-update", config.clone());
    Ok(config.show_floating)
}

#[tauri::command]
pub(crate) fn set_mouse_passthrough(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    enabled: bool,
) -> Result<bool, String> {
    set_mouse_passthrough_native(&app, enabled)?;
    let mut config = state.config.write().map_err(|_| "配置锁异常".to_string())?;
    config.mouse_passthrough = enabled;
    persist_config(&config)?;
    refresh_tray_menu(&app, state.inner());
    let _ = app.emit("config-update", config.clone());
    Ok(enabled)
}

#[tauri::command]
pub(crate) fn set_active_target(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    target_id: String,
) -> Result<AppConfig, String> {
    let mut config = state.config.write().map_err(|_| "配置锁异常".to_string())?;
    if !config.targets.iter().any(|target| target.id == target_id) {
        return Err("目标不存在".into());
    }
    config.active_target_id = target_id;
    persist_config(&config)?;
    let result = config.clone();
    drop(config);

    refresh_tray_menu(&app, state.inner());
    emit_active_snapshot(&app, state.inner().as_ref());
    let _ = app.emit("config-update", result.clone());
    Ok(result)
}

#[tauri::command]
pub(crate) fn show_settings(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}
