#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if command -v node >/dev/null 2>&1; then
  node --check frontend/settings.js
  node --check frontend/floating.js
else
  echo "WARN: node 不存在，跳过 JavaScript 语法检查"
fi

if command -v python3 >/dev/null 2>&1; then
  python3 -m json.tool src-tauri/tauri.conf.json >/dev/null
  python3 -m json.tool src-tauri/capabilities/default.json >/dev/null
  python3 - <<'PYTOML'
import tomllib
with open("src-tauri/Cargo.toml", "rb") as f:
    tomllib.load(f)
PYTOML
else
  echo "WARN: python3 不存在，跳过 JSON/TOML 语法检查"
fi

# Backend module boundaries: main.rs should only assemble the application.
for module in config probe runtime macos_window tray commands; do
  test -f "src-tauri/src/${module}.rs"
  grep -q "mod ${module};" src-tauri/src/main.rs
done
test "$(wc -l < src-tauri/src/main.rs | tr -d ' ')" -le 180

grep -q 'pub(crate) struct AppConfig' src-tauri/src/config.rs
grep -q 'validate_config' src-tauri/src/config.rs
grep -q 'endpoint_key' src-tauri/src/config.rs

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
grep -q 'target-update.*table' frontend/settings.js
grep -q 'prefers-color-scheme: dark' frontend/floating.css
grep -q 'font-variant-numeric: tabular-nums' frontend/floating.css
grep -q 'status-dot' frontend/floating.css
grep -q '@keyframes glassEnter' frontend/floating.css
grep -q '@keyframes glassSheenDrift' frontend/floating.css
grep -q 'prefers-reduced-motion' frontend/floating.css
grep -q -- '--pointer-x' frontend/floating.js
grep -q -- '--glass-border-alpha' frontend/floating.css
grep -q 'padding: 0;' frontend/floating.css
grep -q 'config.uiVersion = 7' frontend/settings.js

test -x script/build_and_run.sh
test -f .codex/environments/environment.toml

echo "Static checks passed."
