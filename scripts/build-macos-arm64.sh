#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "未找到 Rust/Cargo。请先安装 Rust: https://rustup.rs"
  exit 1
fi

if ! rustup target list --installed | grep -q '^aarch64-apple-darwin$'; then
  rustup target add aarch64-apple-darwin
fi

if ! cargo tauri --version >/dev/null 2>&1; then
  cargo install tauri-cli --version '^2' --locked
fi

cargo tauri build --target aarch64-apple-darwin

echo
echo "构建完成。产物通常位于："
echo "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/"
echo "src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/"
