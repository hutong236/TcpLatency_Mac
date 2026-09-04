#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="TCP Latency"
PROCESS_NAME="tcp-latency"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_DIR="$ROOT_DIR/src-tauri"
APP_BUNDLE="$TAURI_DIR/target/debug/bundle/macos/$APP_NAME.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: this build/run entrypoint must be executed on macOS." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: cargo was not found. Install the Rust toolchain first." >&2
  exit 1
fi

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "ERROR: cargo-tauri was not found. Install Tauri CLI v2 first:" >&2
  echo "  cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

pkill -x "$PROCESS_NAME" >/dev/null 2>&1 || true
pkill -x "$APP_NAME" >/dev/null 2>&1 || true

cd "$TAURI_DIR"
cargo tauri build --debug --bundles app

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "ERROR: expected app bundle not found: $APP_BUNDLE" >&2
  exit 1
fi

INFO_PLIST="$APP_BUNDLE/Contents/Info.plist"
if command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  BUNDLE_EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$INFO_PLIST" 2>/dev/null || true)"
else
  BUNDLE_EXECUTABLE=""
fi
BUNDLE_EXECUTABLE="${BUNDLE_EXECUTABLE:-$PROCESS_NAME}"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/$BUNDLE_EXECUTABLE"

open_app() {
  /usr/bin/open -n "$APP_BUNDLE"
}

case "$MODE" in
  run)
    open_app
    ;;
  --debug|debug)
    exec lldb -- "$APP_BINARY"
    ;;
  --logs|logs)
    open_app
    exec /usr/bin/log stream --info --style compact \
      --predicate "process == \"$BUNDLE_EXECUTABLE\" OR process == \"$PROCESS_NAME\""
    ;;
  --telemetry|telemetry)
    open_app
    exec /usr/bin/log stream --info --style compact \
      --predicate "process == \"$BUNDLE_EXECUTABLE\" OR process == \"$PROCESS_NAME\""
    ;;
  --verify|verify)
    open_app
    sleep 1
    if pgrep -x "$BUNDLE_EXECUTABLE" >/dev/null 2>&1 || pgrep -x "$PROCESS_NAME" >/dev/null 2>&1; then
      echo "OK: $APP_NAME is running."
    else
      echo "ERROR: $APP_NAME did not remain running after launch." >&2
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac
