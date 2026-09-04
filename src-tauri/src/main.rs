use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    window::{Effect, EffectState, EffectsBuilder},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_window_state::StateFlags;
use tokio::net::{lookup_host, TcpStream};

const TRAY_ID: &str = "latency-tray";
const HISTORY_WINDOW_MS: u128 = 60_000;
const MAX_HISTORY_POINTS: usize = 600;
const SCHEDULER_TICK_MS: u64 = 100;

fn default_true() -> bool {
    true
}
fn default_interval_ms() -> u64 {
    1000
}
fn default_timeout_ms() -> u64 {
    2000
}
fn default_notify_high_count() -> u32 {
    3
}
fn default_notify_failure_count() -> u32 {
    3
}
fn default_notification_cooldown_sec() -> u64 {
    300
}
fn default_address_family() -> String {
    "auto".into()
}
fn default_floating_opacity() -> f64 {
    0.82
}
fn default_floating_font_size() -> u32 {
    42
}
fn default_floating_size() -> String {
    "standard".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct TargetConfig {
    id: String,
    name: String,
    host: String,
    port: u16,
    #[serde(default = "default_interval_ms")]
    interval_ms: u64,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_address_family")]
    address_family: String,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            id: "internet".into(),
            name: "Internet".into(),
            host: "1.1.1.1".into(),
            port: 443,
            interval_ms: default_interval_ms(),
            timeout_ms: default_timeout_ms(),
            enabled: true,
            address_family: default_address_family(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ThresholdConfig {
    warning_ms: f64,
    high_ms: f64,
    critical_ms: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            warning_ms: 50.0,
            high_ms: 100.0,
            critical_ms: 200.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AppConfig {
    targets: Vec<TargetConfig>,
    active_target_id: String,
    show_floating: bool,
    autostart: bool,
    thresholds: ThresholdConfig,
    mouse_passthrough: bool,
    notifications_enabled: bool,
    #[serde(default = "default_notify_high_count")]
    notify_consecutive_high: u32,
    #[serde(default = "default_notify_failure_count")]
    notify_consecutive_failure: u32,
    #[serde(default = "default_notification_cooldown_sec")]
    notification_cooldown_sec: u64,
    #[serde(default = "default_true")]
    notify_recovery: bool,
    #[serde(default = "default_true")]
    floating_show_target: bool,
    #[serde(default = "default_floating_opacity")]
    floating_opacity: f64,
    #[serde(default = "default_floating_font_size")]
    floating_font_size: u32,
    #[serde(default = "default_floating_size")]
    floating_size: String,
    #[serde(default = "default_true")]
    floating_show_status_dot: bool,
    #[serde(default)]
    floating_show_trend: bool,
    #[serde(default)]
    ui_version: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            targets: vec![TargetConfig::default()],
            active_target_id: "internet".into(),
            show_floating: true,
            autostart: false,
            thresholds: ThresholdConfig::default(),
            mouse_passthrough: false,
            notifications_enabled: false,
            notify_consecutive_high: default_notify_high_count(),
            notify_consecutive_failure: default_notify_failure_count(),
            notification_cooldown_sec: default_notification_cooldown_sec(),
            notify_recovery: true,
            floating_show_target: true,
            floating_opacity: default_floating_opacity(),
            floating_font_size: default_floating_font_size(),
            floating_size: default_floating_size(),
            floating_show_status_dot: true,
            floating_show_trend: false,
            ui_version: 7,
        }
    }
}

impl AppConfig {
    fn active_target(&self) -> Option<&TargetConfig> {
        self.targets
            .iter()
            .find(|target| target.id == self.active_target_id)
            .or_else(|| self.targets.iter().find(|target| target.enabled))
            .or_else(|| self.targets.first())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeSnapshot {
    target_id: String,
    target_name: String,
    host: String,
    port: u16,
    enabled: bool,
    current_ms: Option<f64>,
    average_ms: Option<f64>,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
    jitter_ms: Option<f64>,
    p95_ms: Option<f64>,
    failure_percent: f64,
    sample_count: usize,
    status: String,
    error: Option<String>,
    paused: bool,
    timestamp_ms: u128,
    sample_age_ms: u128,
    dns_ms: Option<f64>,
    resolved_address: Option<String>,
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
struct HistoryPoint {
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

struct SharedState {
    config: RwLock<AppConfig>,
    paused: AtomicBool,
    runtimes: Mutex<HashMap<String, TargetRuntime>>,
    inflight: Mutex<HashMap<String, u64>>,
    generation: AtomicU64,
}

impl SharedState {
    fn new(config: AppConfig) -> Self {
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
        }
    }

    fn reconcile_targets(&self, config: &AppConfig) {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeResult {
    latency_ms: Option<f64>,
    dns_ms: Option<f64>,
    resolved_address: Option<String>,
    attempted_addresses: Vec<String>,
    status: String,
    error: Option<String>,
}

#[derive(Debug)]
struct AlertRequest {
    title: String,
    body: String,
}

fn endpoint_key(target: &TargetConfig) -> String {
    format!(
        "{}:{}:{}",
        target.host.trim().to_ascii_lowercase(),
        target.port,
        target.address_family.trim().to_ascii_lowercase()
    )
}

fn stale_after_ms(target: &TargetConfig) -> u128 {
    (target.interval_ms.saturating_mul(3).saturating_add(target.timeout_ms))
        .max(5_000) as u128
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

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TcpLatency")
        .join("config.json")
}

fn migrate_config(mut config: AppConfig) -> AppConfig {
    // V0.4 introduced the Apple-style HUD defaults. Keep that one-time migration
    // for very old configs, then mark V0.6 without overwriting user choices.
    if config.ui_version < 4 {
        config.floating_opacity = default_floating_opacity();
        config.floating_font_size = default_floating_font_size();
        config.floating_size = default_floating_size();
        config.floating_show_status_dot = true;
        config.floating_show_trend = false;
    }
    if config.ui_version < 7 {
        config.ui_version = 7;
    }
    config
}

fn load_config() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(path) {
        Ok(raw) => {
            let parsed: AppConfig = serde_json::from_str(&raw).unwrap_or_default();
            let needs_migration = parsed.ui_version < 7;
            let validated = validate_config(migrate_config(parsed)).unwrap_or_default();
            if needs_migration {
                let _ = persist_config(&validated);
            }
            validated
        }
        Err(_) => AppConfig::default(),
    }
}

fn persist_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let body = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {e}"))?;
    fs::write(path, body).map_err(|e| format!("写入配置失败: {e}"))
}

fn valid_target_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn validate_config(mut config: AppConfig) -> Result<AppConfig, String> {
    if config.targets.is_empty() {
        return Err("至少需要配置一个监测目标".into());
    }

    let mut ids = HashSet::new();
    for target in &mut config.targets {
        target.name = target.name.trim().to_string();
        target.host = target.host.trim().to_string();
        target.id = target.id.trim().to_string();

        if !valid_target_id(&target.id) {
            return Err(format!("{} 的目标 ID 只能包含字母、数字、-、_", target.name));
        }
        if !ids.insert(target.id.clone()) {
            return Err(format!("目标 ID 重复: {}", target.id));
        }
        if target.name.is_empty() || target.host.is_empty() {
            return Err("目标名称和 Host 不能为空".into());
        }
        if target.port == 0 {
            return Err(format!("{} 的端口无效", target.name));
        }
        if !(200..=60_000).contains(&target.interval_ms) {
            return Err(format!("{} 的检测间隔必须在 200~60000ms 之间", target.name));
        }
        if !(100..=30_000).contains(&target.timeout_ms) {
            return Err(format!("{} 的超时时间必须在 100~30000ms 之间", target.name));
        }
        target.address_family = target.address_family.trim().to_ascii_lowercase();
        if !matches!(target.address_family.as_str(), "auto" | "ipv4" | "ipv6") {
            return Err(format!("{} 的地址族必须是 auto / ipv4 / ipv6", target.name));
        }
    }

    if !config
        .targets
        .iter()
        .any(|target| target.id == config.active_target_id)
    {
        config.active_target_id = config.targets[0].id.clone();
    }

    let t = &config.thresholds;
    if t.warning_ms <= 0.0 || t.high_ms <= t.warning_ms || t.critical_ms <= t.high_ms {
        return Err("延迟阈值必须满足 0 < Warning < High < Critical".into());
    }
    if !(1..=30).contains(&config.notify_consecutive_high) {
        return Err("高延迟连续次数必须在 1~30 之间".into());
    }
    if !(1..=30).contains(&config.notify_consecutive_failure) {
        return Err("失败连续次数必须在 1~30 之间".into());
    }
    if !(10..=86_400).contains(&config.notification_cooldown_sec) {
        return Err("通知冷却时间必须在 10~86400 秒之间".into());
    }
    if !(0.70..=1.0).contains(&config.floating_opacity) {
        return Err("悬浮窗透明度必须在 0.70~1.00 之间".into());
    }
    if !(30..=52).contains(&config.floating_font_size) {
        return Err("悬浮窗字体大小必须在 30~52 之间".into());
    }
    config.floating_size = config.floating_size.trim().to_ascii_lowercase();
    if !matches!(config.floating_size.as_str(), "compact" | "standard" | "large") {
        return Err("悬浮窗尺寸必须是 compact / standard / large".into());
    }
    config.ui_version = 7;

    Ok(config)
}

async fn tcp_probe(target: &TargetConfig) -> ProbeResult {
    let total_timeout = Duration::from_millis(target.timeout_ms);
    let total_started = Instant::now();
    let dns_started = Instant::now();

    let lookup = tokio::time::timeout(total_timeout, lookup_host((target.host.as_str(), target.port))).await;
    let mut addrs: Vec<SocketAddr> = match lookup {
        Err(_) => {
            return ProbeResult {
                latency_ms: None,
                dns_ms: None,
                resolved_address: None,
                attempted_addresses: vec![],
                status: "dns_timeout".into(),
                error: Some("DNS resolve timeout".into()),
            }
        }
        Ok(Err(err)) => {
            return ProbeResult {
                latency_ms: None,
                dns_ms: Some(dns_started.elapsed().as_secs_f64() * 1000.0),
                resolved_address: None,
                attempted_addresses: vec![],
                status: "dns_error".into(),
                error: Some(format!("DNS: {err}")),
            }
        }
        Ok(Ok(iter)) => iter.collect(),
    };

    let dns_ms = dns_started.elapsed().as_secs_f64() * 1000.0;
    addrs.sort_by_key(|addr| if addr.is_ipv4() { 0 } else { 1 });
    addrs.dedup();
    addrs.retain(|addr| match target.address_family.as_str() {
        "ipv4" => addr.is_ipv4(),
        "ipv6" => addr.is_ipv6(),
        _ => true,
    });

    let attempted_addresses: Vec<String> = addrs.iter().map(ToString::to_string).collect();
    if addrs.is_empty() {
        return ProbeResult {
            latency_ms: None,
            dns_ms: Some(dns_ms),
            resolved_address: None,
            attempted_addresses,
            status: "dns_error".into(),
            error: Some(format!("DNS 未返回 {} 可用地址", target.address_family)),
        };
    }

    let mut last_status = "offline".to_string();
    let mut last_error = Some("TCP connect failed".to_string());
    let mut last_address = None;

    for addr in addrs {
        let elapsed_total = total_started.elapsed();
        if elapsed_total >= total_timeout {
            last_status = "timeout".into();
            last_error = Some("TCP connect timeout".into());
            break;
        }
        let remaining = total_timeout.saturating_sub(elapsed_total);
        let connect_started = Instant::now();
        match tokio::time::timeout(remaining, TcpStream::connect(addr)).await {
            Err(_) => {
                last_status = "timeout".into();
                last_error = Some(format!("TCP connect timeout: {addr}"));
                last_address = Some(addr.to_string());
            }
            Ok(Ok(stream)) => {
                let latency_ms = connect_started.elapsed().as_secs_f64() * 1000.0;
                drop(stream);
                return ProbeResult {
                    latency_ms: Some(latency_ms),
                    dns_ms: Some(dns_ms),
                    resolved_address: Some(addr.to_string()),
                    attempted_addresses,
                    status: "ok".into(),
                    error: None,
                };
            }
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
                last_status = "refused".into();
                last_error = Some(format!("TCP connection refused: {addr}"));
                last_address = Some(addr.to_string());
            }
            Ok(Err(err)) => {
                last_status = "offline".into();
                last_error = Some(format!("{addr}: {err}"));
                last_address = Some(addr.to_string());
            }
        }
    }

    ProbeResult {
        latency_ms: None,
        dns_ms: Some(dns_ms),
        resolved_address: last_address,
        attempted_addresses,
        status: last_status,
        error: last_error,
    }
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

fn stats(samples: &VecDeque<Sample>) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, f64, usize) {
    if samples.is_empty() {
        return (None, None, None, None, None, 0.0, 0);
    }

    let successful: Vec<f64> = samples.iter().filter_map(|sample| sample.latency_ms).collect();
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
        let diffs: Vec<f64> = successful.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        Some(diffs.iter().sum::<f64>() / diffs.len() as f64)
    } else {
        None
    };

    let mut sorted = successful.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = (((sorted.len() as f64) * 0.95).ceil() as usize).saturating_sub(1).min(sorted.len() - 1);
    let p95 = Some(sorted[index]);

    (Some(average), Some(min), Some(max), jitter, p95, failure_percent, samples.len())
}

fn snapshot_for_active(state: &SharedState) -> ProbeSnapshot {
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

fn all_snapshots(state: &SharedState) -> Vec<ProbeSnapshot> {
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

fn emit_active_snapshot(app: &AppHandle, state: &SharedState) {
    let snapshot = snapshot_for_active(state);
    let _ = app.emit("latency-update", snapshot.clone());
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(Some(tray_title(&snapshot)));
        let endpoint = snapshot
            .resolved_address
            .as_deref()
            .unwrap_or("unresolved");
        let tooltip = format!(
            "{} · {}:{} · {} · P95 {} · 失败 {:.1}%",
            snapshot.target_name,
            snapshot.host,
            snapshot.port,
            endpoint,
            snapshot.p95_ms.map(|v| format!("{v:.0}ms")).unwrap_or_else(|| "--".into()),
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

    let (avg, min, max, jitter, p95, failure, count) = stats(&runtime.samples);
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

fn should_schedule_probe(state: &SharedState, target: &TargetConfig, now: Instant) -> bool {
    if !target.enabled {
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
        .map(|started| now.duration_since(started) >= Duration::from_millis(target.interval_ms))
        .unwrap_or(true);
    if due {
        runtime.last_probe_started = Some(now);
    }
    due
}

fn mark_inflight(state: &SharedState, target_id: &str, generation: u64) -> bool {
    let Ok(mut inflight) = state.inflight.lock() else {
        return false;
    };
    match inflight.get(target_id) {
        Some(current) if *current == generation => false,
        _ => {
            inflight.insert(target_id.to_string(), generation);
            true
        }
    }
}

fn clear_inflight(state: &SharedState, target_id: &str, generation: u64) {
    if let Ok(mut inflight) = state.inflight.lock() {
        if inflight.get(target_id).copied() == Some(generation) {
            inflight.remove(target_id);
        }
    }
}

async fn probe_scheduler(app: AppHandle, state: Arc<SharedState>) {
    let mut last_paused = false;

    loop {
        let paused = state.paused.load(Ordering::Relaxed);
        if paused {
            if !last_paused {
                emit_active_snapshot(&app, &state);
                let _ = app.emit("targets-update", all_snapshots(&state));
            }
            last_paused = true;
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }

        if last_paused {
            emit_active_snapshot(&app, &state);
            last_paused = false;
        }

        let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
        let generation = state.generation.load(Ordering::Relaxed);
        let now = Instant::now();

        for target in config.targets.iter().filter(|target| target.enabled) {
            if !should_schedule_probe(&state, target, now)
                || !mark_inflight(&state, &target.id, generation)
            {
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

        tokio::time::sleep(Duration::from_millis(SCHEDULER_TICK_MS)).await;
    }
}


#[cfg(target_os = "macos")]
fn native_ns_window_ptr(
    app: &AppHandle,
    label: &str,
    missing_message: &str,
) -> Result<std::ptr::NonNull<objc2_app_kit::NSWindow>, String> {
    use objc2_app_kit::NSWindow;

    let Some(window) = app.get_webview_window(label) else {
        return Err(missing_message.into());
    };

    let ptr = window
        .ns_window()
        .map_err(|e| format!("获取 macOS 原生窗口句柄失败: {e}"))? as *mut NSWindow;

    // Tauri retains/owns this NSWindow. Keep only a non-owning pointer here;
    // each bridge function borrows it for the duration of that operation.
    std::ptr::NonNull::new(ptr).ok_or_else(|| "macOS 原生窗口句柄为空".to_string())
}

#[cfg(target_os = "macos")]
fn configure_native_floating_window(app: &AppHandle) -> Result<(), String> {
    use objc2_app_kit::{
        NSColor, NSFloatingWindowLevel, NSWindowCollectionBehavior,
    };

    let ns_window_ptr = native_ns_window_ptr(app, "main", "未找到悬浮窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };

    // AppKit owns window behavior; the webview owns content and micro-motion.
    // Keep native shadow OFF because an NSWindow shadow follows the transparent
    // rectangular window bounds and produces square artifacts around the card.
    ns_window.setOpaque(false);
    let clear = NSColor::clearColor();
    ns_window.setBackgroundColor(Some(&clear));
    ns_window.setHasShadow(false);
    ns_window.setHidesOnDeactivate(false);
    ns_window.setCanHide(false);
    ns_window.setMovable(true);
    ns_window.setMovableByWindowBackground(true);
    ns_window.setLevel(NSFloatingWindowLevel);

    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_native_floating_window(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_native_settings_window(app: &AppHandle) -> Result<(), String> {
    let ns_window_ptr = native_ns_window_ptr(app, "settings", "未找到设置窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };

    // Tauri keeps this native window alive; close requests are intercepted and
    // converted into hide, so reopening can reliably reuse the same instance.
    unsafe {
        ns_window.setReleasedWhenClosed(false);
    }
    ns_window.setHidesOnDeactivate(false);
    ns_window.setCanHide(true);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_native_settings_window(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_settings_window_native(app: &AppHandle) -> Result<(), String> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let ns_window_ptr = native_ns_window_ptr(app, "settings", "未找到设置窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };

    // macOS 14 deprecated the old "activateIgnoringOtherApps" option. Use the
    // modern running-application activation API, then make this concrete
    // settings window key/front. Tauri's portable set_focus() is still called
    // by show_settings_window() before this AppKit finalization step.
    let running_app = NSRunningApplication::currentApplication();
    let _ = running_app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    ns_window.makeKeyAndOrderFront(None);
    ns_window.orderFrontRegardless();

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn activate_settings_window_native(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

fn set_floating_visibility(app: &AppHandle, visible: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if visible {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }
}

fn floating_window_dimensions(size: &str) -> (f64, f64) {
    match size {
        "compact" => (178.0, 76.0),
        "large" => (268.0, 116.0),
        _ => (228.0, 100.0),
    }
}

fn apply_floating_window_size(app: &AppHandle, size: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };
    let (width, height) = floating_window_dimensions(size);
    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| format!("设置悬浮窗尺寸失败: {e}"))
}

fn floating_effect_radius(size: &str) -> f64 {
    match size {
        "compact" => 20.0,
        "large" => 28.0,
        _ => 24.0,
    }
}

#[cfg(target_os = "macos")]
fn apply_floating_window_effect(app: &AppHandle, size: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };

    // V0.6 uses a neutral under-window material for the real desktop blur.
    // The visible tint/border/highlight stays in CSS so the running app can
    // match the bundled HTML preview much more closely than HudWindow does.
    window
        .set_effects(
            EffectsBuilder::new()
                .effect(Effect::UnderWindowBackground)
                .state(EffectState::Active)
                .radius(floating_effect_radius(size))
                .build(),
        )
        .map_err(|e| format!("设置 macOS 磨砂玻璃效果失败: {e}"))
}

#[cfg(not(target_os = "macos"))]
fn apply_floating_window_effect(_app: &AppHandle, _size: &str) -> Result<(), String> {
    Ok(())
}

fn set_mouse_passthrough_native(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| format!("设置鼠标穿透失败: {e}"))
}

fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

fn build_tray_menu(app: &AppHandle, state: &Arc<SharedState>) -> tauri::Result<Menu<tauri::Wry>> {
    let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();

    for target in &config.targets {
        let mark = if target.id == config.active_target_id { "✓ " } else { "  " };
        let disabled = if target.enabled { "" } else { " [停用]" };
        let label = format!("{}{} — {}:{}{}", mark, target.name, target.host, target.port, disabled);
        items.push(MenuItem::with_id(
            app,
            format!("target::{}", target.id),
            label,
            true,
            None::<&str>,
        )?);
    }

    let separator1 = PredefinedMenuItem::separator(app)?;
    let floating_item = MenuItem::with_id(app, "toggle-floating", "显示/隐藏悬浮窗", true, None::<&str>)?;
    let passthrough_label = if config.mouse_passthrough {
        "✓ 悬浮窗鼠标穿透"
    } else {
        "  悬浮窗鼠标穿透"
    };
    let passthrough_item = MenuItem::with_id(app, "toggle-passthrough", passthrough_label, true, None::<&str>)?;
    let pause_label = if state.paused.load(Ordering::Relaxed) {
        "恢复监测"
    } else {
        "暂停监测"
    };
    let pause_item = MenuItem::with_id(app, "toggle-pause", pause_label, true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>).collect();
    refs.push(&separator1);
    refs.push(&floating_item);
    refs.push(&passthrough_item);
    refs.push(&pause_item);
    refs.push(&separator2);
    refs.push(&settings_item);
    refs.push(&quit_item);

    Menu::with_items(app, &refs)
}

fn refresh_tray_menu(app: &AppHandle, state: &Arc<SharedState>) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_tray_menu(app, state)) {
        let _ = tray.set_menu(Some(menu));
    }
}

#[tauri::command]
fn get_config(state: State<'_, Arc<SharedState>>) -> AppConfig {
    state.config.read().map(|c| c.clone()).unwrap_or_default()
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<SharedState>>) -> ProbeSnapshot {
    snapshot_for_active(state.inner().as_ref())
}

#[tauri::command]
fn get_all_snapshots(state: State<'_, Arc<SharedState>>) -> Vec<ProbeSnapshot> {
    all_snapshots(state.inner().as_ref())
}

#[tauri::command]
fn get_history(state: State<'_, Arc<SharedState>>, target_id: String) -> Vec<HistoryPoint> {
    let now = now_millis();
    state
        .runtimes
        .lock()
        .ok()
        .and_then(|mut runtimes| {
            let runtime = runtimes.get_mut(&target_id)?;
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

#[tauri::command]
async fn test_target(mut target: TargetConfig) -> Result<ProbeResult, String> {
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
fn save_config(
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
fn set_paused(app: AppHandle, state: State<'_, Arc<SharedState>>, paused: bool) -> bool {
    state.paused.store(paused, Ordering::Relaxed);
    refresh_tray_menu(&app, state.inner());
    emit_active_snapshot(&app, state.inner().as_ref());
    let _ = app.emit("targets-update", all_snapshots(state.inner().as_ref()));
    paused
}

#[tauri::command]
fn is_paused(state: State<'_, Arc<SharedState>>) -> bool {
    state.paused.load(Ordering::Relaxed)
}

#[tauri::command]
fn toggle_floating(app: AppHandle, state: State<'_, Arc<SharedState>>) -> Result<bool, String> {
    let mut config = state.config.write().map_err(|_| "配置锁异常".to_string())?;
    config.show_floating = !config.show_floating;
    persist_config(&config)?;
    set_floating_visibility(&app, config.show_floating);
    let _ = app.emit("config-update", config.clone());
    Ok(config.show_floating)
}

#[tauri::command]
fn set_mouse_passthrough(
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
fn set_active_target(
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

fn show_settings_window(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("settings") else {
        return Err("未找到设置窗口".into());
    };

    // The app lives as an Accessory most of the time. Promote it only while
    // Settings is visible, then let AppKit explicitly make the reusable native
    // settings window key and front. This is more reliable than z-order hacks.
    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(tauri::ActivationPolicy::Regular)
            .map_err(|e| format!("切换 macOS 激活策略失败: {e}"))?;
        // While Settings is open, behave like a normal foreground Mac app.
        // This avoids fighting AppKit activation with a hidden Dock identity.
        let _ = app.set_dock_visibility(true);
    }

    if window.is_minimized().unwrap_or(false) {
        window
            .unminimize()
            .map_err(|e| format!("恢复设置窗口失败: {e}"))?;
    }

    window
        .show()
        .map_err(|e| format!("显示设置窗口失败: {e}"))?;

    // Keep the portable Tauri focus call as a first pass, then use AppKit as
    // the authoritative foreground activation path on macOS.
    let _ = window.set_focus();
    activate_settings_window_native(app)?;
    Ok(())
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

fn build_tray(app: &mut tauri::App, state: Arc<SharedState>) -> tauri::Result<()> {
    let menu = build_tray_menu(app.handle(), &state)?;
    let menu_state = state.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .title("-- ms")
        .tooltip("TCP Latency")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            if let Some(target_id) = id.strip_prefix("target::") {
                if let Ok(mut config) = menu_state.config.write() {
                    if config.targets.iter().any(|target| target.id == target_id) {
                        config.active_target_id = target_id.to_string();
                        let _ = persist_config(&config);
                        let emitted = config.clone();
                        drop(config);
                        refresh_tray_menu(app, &menu_state);
                        emit_active_snapshot(app, &menu_state);
                        let _ = app.emit("config-update", emitted);
                    }
                }
                return;
            }

            match id {
                "toggle-floating" => {
                    if let Ok(mut config) = menu_state.config.write() {
                        config.show_floating = !config.show_floating;
                        let _ = persist_config(&config);
                        set_floating_visibility(app, config.show_floating);
                        let _ = app.emit("config-update", config.clone());
                    }
                }
                "toggle-passthrough" => {
                    if let Ok(mut config) = menu_state.config.write() {
                        config.mouse_passthrough = !config.mouse_passthrough;
                        let _ = set_mouse_passthrough_native(app, config.mouse_passthrough);
                        let _ = persist_config(&config);
                        let _ = app.emit("config-update", config.clone());
                        drop(config);
                        refresh_tray_menu(app, &menu_state);
                    }
                }
                "toggle-pause" => {
                    let paused = menu_state.paused.load(Ordering::Relaxed);
                    menu_state.paused.store(!paused, Ordering::Relaxed);
                    emit_active_snapshot(app, &menu_state);
                    let _ = app.emit("targets-update", all_snapshots(&menu_state));
                    refresh_tray_menu(app, &menu_state);
                }
                "settings" => {
                    if let Err(err) = show_settings_window(app) {
                        eprintln!("[ui] 打开设置失败: {err}");
                        let _ = app.emit("app-error", err);
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

fn main() {
    let config = load_config();
    let state = Arc::new(SharedState::new(config.clone()));
    let setup_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .with_filter(|label| label == "main")
                .build(),
        )
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_snapshot,
            get_all_snapshots,
            get_history,
            test_target,
            save_config,
            set_paused,
            is_paused,
            toggle_floating,
            set_mouse_passthrough,
            set_active_target,
            show_settings
        ])
        .on_window_event(|window, event| {
            if window.label() != "settings" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                {
                    let app = window.app_handle();
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    let _ = app.set_dock_visibility(false);
                }
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            build_tray(app, setup_state.clone())?;
            configure_native_floating_window(app.handle())
                .map_err(|e| format!("初始化 macOS 悬浮窗口失败: {e}"))?;
            configure_native_settings_window(app.handle())
                .map_err(|e| format!("初始化 macOS 设置窗口失败: {e}"))?;
            let _ = apply_floating_window_size(app.handle(), &config.floating_size);
            let _ = apply_floating_window_effect(app.handle(), &config.floating_size);
            set_floating_visibility(app.handle(), config.show_floating);
            let _ = set_mouse_passthrough_native(app.handle(), config.mouse_passthrough);

            if config.autostart {
                let _ = app.handle().autolaunch().enable();
            }
            if config.notifications_enabled {
                let _ = app.handle().notification().request_permission();
            }

            emit_active_snapshot(app.handle(), &setup_state);
            let _ = app.emit("targets-update", all_snapshots(&setup_state));

            let app_handle = app.handle().clone();
            let loop_state = setup_state.clone();
            tauri::async_runtime::spawn(async move {
                probe_scheduler(app_handle, loop_state).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running TCP Latency");
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
    fn v01_config_gets_v02_defaults() {
        let raw = r#"{
          "targets":[{"id":"test","name":"Test","host":"127.0.0.1","port":443,"intervalMs":1000,"timeoutMs":2000}],
          "activeTargetId":"test",
          "showFloating":true,
          "autostart":false,
          "thresholds":{"warningMs":50,"highMs":100,"criticalMs":200}
        }"#;
        let config: AppConfig = migrate_config(serde_json::from_str(raw).unwrap());
        assert!(!config.mouse_passthrough);
        assert!(!config.notifications_enabled);
        assert!(config.targets[0].enabled);
        assert_eq!(config.targets[0].address_family, "auto");
        assert_eq!(config.notify_consecutive_high, 3);
        assert_eq!(config.notify_consecutive_failure, 3);
        assert!(config.notify_recovery);
        assert!(config.floating_show_target);
        assert_eq!(config.floating_font_size, 42);
        assert_eq!(config.floating_size, "standard");
        assert!(config.floating_show_status_dot);
        assert!(!config.floating_show_trend);
        assert_eq!(config.ui_version, 7);
    }

    #[test]
    fn duplicate_target_ids_are_rejected() {
        let mut config = AppConfig::default();
        let mut duplicate = config.targets[0].clone();
        duplicate.name = "Duplicate".into();
        config.targets.push(duplicate);
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn endpoint_key_changes_when_host_or_family_changes() {
        let mut target = TargetConfig::default();
        let original = endpoint_key(&target);
        target.host = "example.com".into();
        assert_ne!(original, endpoint_key(&target));
        let changed_host = endpoint_key(&target);
        target.address_family = "ipv6".into();
        assert_ne!(changed_host, endpoint_key(&target));
    }

    #[test]
    fn stale_threshold_has_safe_floor() {
        let mut target = TargetConfig::default();
        target.interval_ms = 200;
        target.timeout_ms = 100;
        assert_eq!(stale_after_ms(&target), 5_000);
    }
}
