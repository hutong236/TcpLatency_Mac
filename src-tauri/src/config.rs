use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::PathBuf};

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
pub(crate) struct TargetConfig {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(default = "default_interval_ms")]
    pub(crate) interval_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub(crate) timeout_ms: u64,
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_address_family")]
    pub(crate) address_family: String,
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
pub(crate) struct ThresholdConfig {
    pub(crate) warning_ms: f64,
    pub(crate) high_ms: f64,
    pub(crate) critical_ms: f64,
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
pub(crate) struct AppConfig {
    pub(crate) targets: Vec<TargetConfig>,
    pub(crate) active_target_id: String,
    pub(crate) show_floating: bool,
    pub(crate) autostart: bool,
    pub(crate) thresholds: ThresholdConfig,
    pub(crate) mouse_passthrough: bool,
    pub(crate) notifications_enabled: bool,
    #[serde(default = "default_notify_high_count")]
    pub(crate) notify_consecutive_high: u32,
    #[serde(default = "default_notify_failure_count")]
    pub(crate) notify_consecutive_failure: u32,
    #[serde(default = "default_notification_cooldown_sec")]
    pub(crate) notification_cooldown_sec: u64,
    #[serde(default = "default_true")]
    pub(crate) notify_recovery: bool,
    #[serde(default = "default_true")]
    pub(crate) floating_show_target: bool,
    #[serde(default = "default_floating_opacity")]
    pub(crate) floating_opacity: f64,
    #[serde(default = "default_floating_font_size")]
    pub(crate) floating_font_size: u32,
    #[serde(default = "default_floating_size")]
    pub(crate) floating_size: String,
    #[serde(default = "default_true")]
    pub(crate) floating_show_status_dot: bool,
    #[serde(default)]
    pub(crate) floating_show_trend: bool,
    #[serde(default)]
    pub(crate) ui_version: u32,
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
    pub(crate) fn active_target(&self) -> Option<&TargetConfig> {
        self.targets
            .iter()
            .find(|target| target.id == self.active_target_id)
            .or_else(|| self.targets.iter().find(|target| target.enabled))
            .or_else(|| self.targets.first())
    }
}

pub(crate) fn endpoint_key(target: &TargetConfig) -> String {
    format!(
        "{}:{}:{}",
        target.host.trim().to_ascii_lowercase(),
        target.port,
        target.address_family.trim().to_ascii_lowercase()
    )
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TcpLatency")
        .join("config.json")
}

pub(crate) fn migrate_config(mut config: AppConfig) -> AppConfig {
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

pub(crate) fn load_config() -> AppConfig {
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

pub(crate) fn persist_config(config: &AppConfig) -> Result<(), String> {
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

pub(crate) fn validate_config(mut config: AppConfig) -> Result<AppConfig, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_gets_current_defaults() {
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
}
