# QA Notes V0.8

## 已完成静态检查

- `scripts/static-check.sh`: PASS
- Shell syntax (`bash -n`): PASS
- `tauri.conf.json`: PASS
- `capabilities/default.json`: PASS
- `Cargo.toml` TOML parse: PASS
- `frontend/floating.js`: PASS
- `frontend/settings.js`: PASS
- 源码中无 `cocoa::`: PASS
- `Cargo.toml` 中无 `cocoa =`: PASS
- 源码中无 `activateIgnoringOtherApps`: PASS
- 原生 `NSWindow` shadow 仍保持关闭: PASS

## 当前环境限制

当前生成环境没有 `cargo/rustc` 和 macOS SDK，因此无法在这里真正编译 `cfg(target_os = "macos")` 的 objc2 AppKit 代码。

API 已按 `objc2-app-kit 0.3.2` 的公开接口迁移：

- `NSWindow::setOpaque(bool)`
- `NSWindow::setBackgroundColor(...)`
- `NSWindow::setHasShadow(bool)`
- `NSWindow::setMovable(bool)`
- `NSWindow::setMovableByWindowBackground(bool)`
- `NSWindow::setLevel(NSFloatingWindowLevel)`
- `NSWindow::setCollectionBehavior(...)`
- `NSWindow::makeKeyAndOrderFront(None)`
- `NSWindow::orderFrontRegardless()`
- `NSRunningApplication::activateWithOptions(...)`

## Mac 验证命令

```bash
./scripts/static-check.sh
cd src-tauri
cargo check
cargo test
cd ..
./script/build_and_run.sh --verify
```

如果仍有 warning，请优先贴 `tcp-latency` 自身的 warning；第三方依赖 warning 可单独评估。
