use crate::{
    event_store::{recent_events, hourly_history, IncidentEvent, HourlyAggregate},
    battery::BatteryStatus,
    config::{persist_config, validate_config, AppConfig, TargetConfig},
    macos_window::{
        apply_floating_window_effect, apply_floating_window_size, set_floating_visibility,
        set_mouse_passthrough_native, show_settings_window,
    },
    probe::{probe_target, ProbeResult},
    runtime::{
        all_snapshots, emit_active_snapshot, history_for_target, snapshot_for_active, HistoryPoint,
        ProbeSnapshot, SharedState,
    },
    tray::refresh_tray_menu,
};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}, sync::{atomic::Ordering, Arc}};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

const MAX_CAT_ASSET_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatAsset {
    mime_type: String,
    file_name: String,
    data: Vec<u8>,
}

fn cat_asset_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "svg" => Some("image/svg+xml"),
        "png" | "apng" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn cat_asset_store_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TcpLatency")
        .join("cat-assets")
}

fn safe_cat_asset_name(file_name: &str) -> Result<String, String> {
    let name = Path::new(file_name.trim())
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "自定义网络猫素材文件名无效".to_string())?;
    if name.is_empty() || name.len() > 180 {
        return Err("自定义网络猫素材文件名无效".into());
    }
    if cat_asset_mime(Path::new(name)).is_none() {
        return Err("自定义网络猫素材仅支持 SVG / PNG / APNG / WebP / GIF".into());
    }
    let sanitized: String = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') { ch } else { '_' })
        .collect();
    Ok(sanitized)
}

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
pub(crate) fn get_battery(state: State<'_, Arc<SharedState>>) -> BatteryStatus {
    state.battery.lock().map(|s| s.clone()).unwrap_or_default()
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
pub(crate) async fn get_hourly_history(target_id: String, limit: u32) -> Result<Vec<HourlyAggregate>, String> {
    tokio::task::spawn_blocking(move || hourly_history(target_id, limit))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn get_incident_events(limit: u32) -> Result<Vec<IncidentEvent>, String> {
    tokio::task::spawn_blocking(move || recent_events(limit)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn load_cat_asset(path: String) -> Result<CatAsset, String> {
    let path = Path::new(path.trim());
    let mime = cat_asset_mime(path)
        .ok_or_else(|| "自定义网络猫素材仅支持 SVG / PNG / APNG / WebP / GIF".to_string())?;
    let metadata = fs::metadata(path).map_err(|e| format!("读取自定义网络猫素材失败: {e}"))?;
    if !metadata.is_file() {
        return Err("自定义网络猫素材必须是普通文件".into());
    }
    if metadata.len() > MAX_CAT_ASSET_BYTES {
        return Err("自定义网络猫素材不能超过 8MB".into());
    }
    let data = fs::read(path).map_err(|e| format!("读取自定义网络猫素材失败: {e}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("custom-cat")
        .to_string();
    Ok(CatAsset {
        mime_type: mime.into(),
        file_name,
        data,
    })
}

#[tauri::command]
pub(crate) fn save_cat_asset(file_name: String, data: Vec<u8>) -> Result<String, String> {
    if data.is_empty() {
        return Err("自定义网络猫素材为空".into());
    }
    if data.len() as u64 > MAX_CAT_ASSET_BYTES {
        return Err("自定义网络猫素材不能超过 8MB".into());
    }
    let name = safe_cat_asset_name(&file_name)?;
    let dir = cat_asset_store_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建网络猫素材目录失败: {e}"))?;
    let path = dir.join(name);
    fs::write(&path, data).map_err(|e| format!("保存网络猫素材失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) async fn test_target(mut target: TargetConfig) -> Result<ProbeResult, String> {
    target.name = target.name.trim().to_string();
    target.host = target.host.trim().to_string();
    target.address_family = target.address_family.trim().to_ascii_lowercase();
    target.probe_mode = target.probe_mode.trim().to_ascii_lowercase();
    target.http_path = target.http_path.trim().to_string();
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
    if !matches!(target.probe_mode.as_str(), "tcp" | "ssh" | "http" | "https") {
        return Err("探测协议必须是 tcp / ssh / http / https".into());
    }
    if !target.http_path.starts_with('/') || target.http_path.len() > 512 ||
        target.http_path.chars().any(char::is_control) {
        return Err("HTTP 路径必须以 / 开头且不能包含控制字符".into());
    }
    if target.host.chars().any(|c| c.is_control() || c.is_whitespace()) ||
       target.host.contains('/') || target.host.contains('@') {
        return Err("Host 必须是主机名或 IP，不能填写完整 URL".into());
    }
    Ok(probe_target(&target).await)
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IcmpDiagnostic {
    status: String,
    latency_ms: Option<f64>,
    detail: String,
}

/// Independent one-shot ICMP comparison. This is not TCP RTT, and firewalls
/// may block ICMP even when the application endpoint is healthy.
#[tauri::command]
pub(crate) async fn test_icmp(target: TargetConfig) -> Result<IcmpDiagnostic, String> {
    let host = target.host.trim();
    if host.is_empty() || host.len() > 253 ||
        host.starts_with('-') ||
        host.chars().any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_'))) {
        return Err("ICMP 目标必须是合法主机名或 IP 地址".into());
    }
    if !matches!(target.address_family.as_str(), "auto" | "ipv4" | "ipv6") {
        return Err("地址族必须是 auto / ipv4 / ipv6".into());
    }
    #[cfg(target_os = "macos")]
    {
        let mut ping = tokio::process::Command::new("/sbin/ping");
        if target.address_family == "ipv6" {
            ping = tokio::process::Command::new("/sbin/ping6");
        }
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            ping.args(["-n", "-c", "1", "-W", "2000", host])
                .kill_on_drop(true).output(),
        ).await;
        match output {
            Err(_) => Ok(IcmpDiagnostic { status: "timeout".into(), latency_ms: None, detail: "ICMP 命令超时；这不代表 TCP 不可达".into() }),
            Ok(Err(err)) => Ok(IcmpDiagnostic { status: "unavailable".into(), latency_ms: None, detail: format!("无法执行 ICMP: {err}") }),
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let sample = stdout.lines().find_map(|line| {
                    let part = line.split("time=").nth(1)?;
                    part.split_whitespace().next()?.parse::<f64>().ok()
                });
                let status = if output.status.success() && sample.is_some() { "ok" } else { "blocked_or_failed" };
                Ok(IcmpDiagnostic {
                    status: status.into(), latency_ms: sample,
                    detail: if status == "ok" { "收到 ICMP Echo Reply".into() }
                        else { "未收到 ICMP Echo Reply；可能被防火墙过滤或目标不可达".into() },
                })
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(IcmpDiagnostic { status: "unavailable".into(), latency_ms: None, detail: "ICMP 对照检测目前仅支持 macOS".into() })
    }
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RouteDiagnostic {
    destination: String,
    interface: Option<String>,
    gateway: Option<String>,
    flags: Option<String>,
    note: String,
}

#[tauri::command]
pub(crate) async fn diagnose_route(target: TargetConfig) -> Result<RouteDiagnostic, String> {
    let host = target.host.trim();
    if host.is_empty() || host.len() > 253 || host.starts_with('-')
        || host.chars().any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_'))) {
        return Err("目标主机名或 IP 格式无效".into());
    }
    #[cfg(target_os = "macos")]
    {
        let lookup = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tokio::net::lookup_host((host, target.port.max(1))),
        ).await.map_err(|_| "DNS 解析超时".to_string())?
            .map_err(|e| format!("DNS 解析失败: {e}"))?;
        let ip = lookup.into_iter().find(|a| match target.address_family.as_str() {
            "ipv4" => a.is_ipv4(),
            "ipv6" => a.is_ipv6(),
            _ => true,
        }).ok_or_else(|| "没有与地址族匹配的解析地址".to_string())?.ip();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::process::Command::new("/sbin/route")
                .args(["-n", "get", &ip.to_string()])
                .kill_on_drop(true).output(),
        ).await.map_err(|_| "系统路由查询超时".to_string())?
            .map_err(|e| format!("系统路由查询失败: {e}"))?;
        if !output.status.success() {
            return Err(format!("系统路由查询失败: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let output_text = String::from_utf8_lossy(&output.stdout);
        let field = |key: &str| output_text.lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .map(|v| v.trim().to_string());
        let interface = field("interface:");
        let note = if interface.as_deref().is_some_and(|i| i.starts_with("utun")) {
            "目标路由经过 utun；可能受 VPN/TUN 接管，但路由接口无法证明 TCP 应答来自最终源站。"
        } else {
            "这是当前系统路由选择结果，并非逐跳追踪；无法单凭接口证明端到端路径。"
        };
        Ok(RouteDiagnostic { destination: ip.to_string(), interface, gateway: field("gateway:"),
            flags: field("flags:"), note: note.into() })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("路由诊断目前仅支持 macOS".into())
    }
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
    state.battery_notify.notify_one();

    apply_floating_window_size(&app, &config.floating_size)?;
    apply_floating_window_effect(
        &app,
        &config.floating_size,
        &config.floating_background_mode,
    )?;
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
