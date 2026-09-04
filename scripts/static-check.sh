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
  echo "WARN: python3 不存在，跳过 JSON 语法检查"
fi

grep -q 'set_ignore_cursor_events' src-tauri/src/main.rs
grep -q 'tauri_plugin_notification' src-tauri/src/main.rs
grep -q 'probe_scheduler' src-tauri/src/main.rs
grep -q 'get_history' src-tauri/src/main.rs
grep -q 'target-update' src-tauri/src/main.rs
grep -q 'address_family' src-tauri/src/main.rs
grep -q 'p95_ms' src-tauri/src/main.rs
grep -q 'sample_age_ms' src-tauri/src/main.rs
grep -q 'test_target' src-tauri/src/main.rs
grep -q 'notify_recovery' src-tauri/src/main.rs
grep -q 'generation: AtomicU64' src-tauri/src/main.rs

for icon in \
  src-tauri/icons/icon.png \
  src-tauri/icons/32x32.png \
  src-tauri/icons/128x128.png \
  src-tauri/icons/128x128@2x.png \
  src-tauri/icons/icon.icns; do
  test -f "$icon"
done

grep -q 'floating_size' src-tauri/src/main.rs
grep -q 'apply_floating_window_size' src-tauri/src/main.rs
grep -q 'ui_version: 7' src-tauri/src/main.rs
grep -q 'floatingShowStatusDot' frontend/settings.js
grep -q 'floatingShowTrend' frontend/settings.js
grep -q 'floatingSize' frontend/settings.js
grep -q 'prefers-color-scheme: dark' frontend/floating.css
grep -q 'font-variant-numeric: tabular-nums' frontend/floating.css
grep -q 'status-dot' frontend/floating.css

grep -q 'Effect::UnderWindowBackground' src-tauri/src/main.rs
grep -q 'apply_floating_window_effect' src-tauri/src/main.rs
grep -q '@keyframes glassEnter' frontend/floating.css
grep -q '@keyframes glassSheenDrift' frontend/floating.css
grep -q 'prefers-reduced-motion' frontend/floating.css
grep -q -- '--pointer-x' frontend/floating.js

grep -q 'show_settings_window' src-tauri/src/main.rs
grep -q 'ActivationPolicy::Regular' src-tauri/src/main.rs
grep -q 'CloseRequested' src-tauri/src/main.rs
grep -q -- '--glass-border-alpha' frontend/floating.css
grep -q 'padding: 0;' frontend/floating.css
grep -q 'config.uiVersion = 7' frontend/settings.js

grep -q 'configure_native_floating_window' src-tauri/src/main.rs
grep -q 'configure_native_settings_window' src-tauri/src/main.rs
grep -q 'activate_settings_window_native' src-tauri/src/main.rs
grep -q 'makeKeyAndOrderFront(None)' src-tauri/src/main.rs
grep -q 'NSWindowCollectionBehavior::CanJoinAllSpaces' src-tauri/src/main.rs
grep -q 'objc2-app-kit = "0.3.2"' src-tauri/Cargo.toml
! grep -q 'cocoa = ' src-tauri/Cargo.toml
! grep -q 'cocoa::' src-tauri/src/main.rs
! grep -q 'activateIgnoringOtherApps' src-tauri/src/main.rs
grep -q 'ns_window.setHasShadow(false);' src-tauri/src/main.rs
! grep -q 'ns_window.setHasShadow(true);' src-tauri/src/main.rs
test -x script/build_and_run.sh
test -f .codex/environments/environment.toml

echo "Static checks passed."

