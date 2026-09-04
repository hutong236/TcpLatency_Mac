#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "未找到 Rust/Cargo。请先安装 Rust: https://rustup.rs"
  exit 1
fi
if ! cargo tauri --version >/dev/null 2>&1; then
  cargo install tauri-cli --version '^2' --locked
fi
cargo tauri dev
