#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if command -v node >/dev/null 2>&1; then
  node --check frontend/settings.js
  node --check frontend/floating.js
  node --check frontend/traffic-settings.js
  node --check frontend/traffic-floating.js
  node --check frontend/cat-floating.js
else
  echo "WARN: node 不存在，跳过 JavaScript 语法检查"
fi

if command -v python3 >/dev/null 2>&1; then
  python3 -m json.tool src-tauri/tauri.conf.json >/dev/null
  python3 -m json.tool src-tauri/capabilities/default.json >/dev/null
  python3 - <<'PYTOML'
import tomllib
with open("src-tauri/Cargo.toml", "rb") as f:
    cargo = tomllib.load(f)
assert cargo["package"]["version"] == "0.12.0"
PYTOML
else
  echo "WARN: python3 不存在，跳过 JSON/TOML 语法检查"
fi

grep -q '"version": "0.12.0"' src-tauri/tauri.conf.json
grep -q '"shadow": false' src-tauri/tauri.conf.json

# Backend module boundaries: main.rs should only assemble the application.
for module in config probe runtime macos_window tray commands traffic; do
  test -f "src-tauri/src/${module}.rs"
  grep -q "mod ${module};" src-tauri/src/main.rs
done
test "$(wc -l < src-tauri/src/main.rs | tr -d ' ')" -le 190

grep -q 'pub(crate) struct AppConfig' src-tauri/src/config.rs
grep -q 'validate_config' src-tauri/src/config.rs
grep -q 'endpoint_key' src-tauri/src/config.rs
grep -q 'floating_show_traffic' src-tauri/src/config.rs
grep -q 'traffic_interface' src-tauri/src/config.rs

grep -q 'network_cat_enabled' src-tauri/src/config.rs
grep -q 'network_cat_animation_enabled' src-tauri/src/config.rs
grep -q 'network_cat_theme' src-tauri/src/config.rs
grep -q 'network_cat_custom_asset' src-tauri/src/config.rs
grep -q 'config.ui_version = 8' src-tauri/src/config.rs
grep -q 'custom_cat_requires_supported_asset' src-tauri/src/config.rs

grep -q 'load_cat_asset' src-tauri/src/commands.rs
grep -q 'save_cat_asset' src-tauri/src/commands.rs
grep -q 'MAX_CAT_ASSET_BYTES' src-tauri/src/commands.rs
grep -q 'commands::load_cat_asset' src-tauri/src/main.rs
grep -q 'commands::save_cat_asset' src-tauri/src/main.rs

grep -q 'DNS_CACHE_TTL' src-tauri/src/probe.rs
grep -q 'DNS_CACHE_MAX_ENTRIES' src-tauri/src/probe.rs
grep -q 'invalidate_cached_addresses' src-tauri/src/probe.rs
grep -q 'tcp_probe' src-tauri/src/probe.rs

grep -q 'scheduler_notify: Notify' src-tauri/src/runtime.rs
grep -q 'next_probe_delay' src-tauri/src/runtime.rs
grep -q 'tokio::select!' src-tauri/src/runtime.rs
grep -q 'runtime.samples.clone()' src-tauri/src/runtime.rs
grep -q 'p95_ms' src-tauri/src/runtime.rs
grep -q 'sample_age_ms' src-tauri/src/runtime.rs
! grep -q 'SCHEDULER_TICK_MS' src-tauri/src/runtime.rs
! grep -q 'sleep(Duration::from_millis(250))' src-tauri/src/runtime.rs

grep -q 'traffic_sampler' src-tauri/src/traffic.rs
grep -q 'traffic-update' src-tauri/src/traffic.rs
grep -q 'getifaddrs' src-tauri/src/traffic.rs

grep -q 'set_ignore_cursor_events' src-tauri/src/macos_window.rs
grep -q 'Effect::UnderWindowBackground' src-tauri/src/macos_window.rs
grep -q 'configure_native_floating_window' src-tauri/src/macos_window.rs
grep -q 'configure_native_settings_window' src-tauri/src/macos_window.rs
grep -q 'activate_settings_window_native' src-tauri/src/macos_window.rs
grep -q 'makeKeyAndOrderFront(None)' src-tauri/src/macos_window.rs
grep -q 'NSWindowCollectionBehavior::CanJoinAllSpaces' src-tauri/src/macos_window.rs
grep -q 'ns_window.setHasShadow(false);' src-tauri/src/macos_window.rs
! grep -q 'ns_window.setHasShadow(true);' src-tauri/src/macos_window.rs
! grep -R -q 'cocoa::' src-tauri/src
! grep -R -q 'activateIgnoringOtherApps' src-tauri/src

grep -q 'build_tray' src-tauri/src/tray.rs
grep -q 'show_settings_window' src-tauri/src/tray.rs
grep -q 'get_history' src-tauri/src/commands.rs
grep -q 'test_target' src-tauri/src/commands.rs
grep -q 'notify_recovery' src-tauri/src/config.rs
grep -q '"sync"' src-tauri/Cargo.toml
grep -q 'objc2-app-kit = "0.3.2"' src-tauri/Cargo.toml
! grep -q 'cocoa = ' src-tauri/Cargo.toml

for icon in \
  src-tauri/icons/icon.png \
  src-tauri/icons/32x32.png \
  src-tauri/icons/128x128.png \
  src-tauri/icons/128x128@2x.png \
  src-tauri/icons/icon.icns; do
  test -f "$icon"
done

grep -q 'floatingShowStatusDot' frontend/settings.js
grep -q 'floatingShowTrend' frontend/settings.js
grep -q 'floatingSize' frontend/settings.js
grep -q 'queueTargetRows' frontend/settings.js
grep -q 'requestAnimationFrame' frontend/settings.js
grep -q 'restartAnimationClass' frontend/floating.js
grep -q 'latestSnapshot' frontend/floating.js
! grep -q 'offsetWidth' frontend/floating.js
grep -q 'traffic-update' frontend/traffic-floating.js
grep -q 'trafficInterface' frontend/traffic-settings.js
grep -q 'prefers-color-scheme: dark' frontend/floating.css
grep -q 'font-variant-numeric: tabular-nums' frontend/floating.css
grep -q 'status-dot' frontend/floating.css
grep -q '@keyframes glassEnter' frontend/floating.css
grep -q '@keyframes glassSheenDrift' frontend/floating.css
grep -q 'prefers-reduced-motion' frontend/floating.css
grep -q -- '--pointer-x' frontend/floating.js
grep -q -- '--glass-border-alpha' frontend/floating.css
grep -q 'padding: 0;' frontend/floating.css

# Network Cat 2.0: settings, themes, fused status and custom assets.
test -f frontend/cat.css
test -f frontend/cat-floating.js
grep -q 'cat.css' frontend/index.html
grep -q 'cat-floating.js' frontend/index.html
grep -q 'id="networkCat"' frontend/index.html
grep -q 'latency-update' frontend/cat-floating.js
grep -q 'traffic-update' frontend/cat-floating.js
grep -q 'combinedHealth' frontend/cat-floating.js
grep -q 'pixelCatMarkup' frontend/cat-floating.js
grep -q 'cyberCatMarkup' frontend/cat-floating.js
grep -q 'renderCustomTheme' frontend/cat-floating.js
grep -q 'load_cat_asset' frontend/cat-floating.js
grep -q 'data-theme="pixel"' frontend/cat.css
grep -q 'data-theme="cyber"' frontend/cat.css
grep -q 'cat-static' frontend/cat.css
grep -q 'prefers-reduced-motion' frontend/cat.css
grep -q 'networkCatEnabled' frontend/traffic-settings.js
grep -q 'networkCatAnimationEnabled' frontend/traffic-settings.js
grep -q 'networkCatTheme' frontend/traffic-settings.js
grep -q 'networkCatFile' frontend/traffic-settings.js
grep -q 'save_cat_asset' frontend/traffic-settings.js
grep -q 'config.uiVersion = 8' frontend/traffic-settings.js

test -x script/build_and_run.sh
test -f .codex/environments/environment.toml
test -f CHANGELOG_V0.9.0.md
test -f CHANGELOG_V0.11.0.md

echo "Static checks passed."
