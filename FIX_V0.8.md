# V0.8 — Cocoa Deprecated Warnings 清理

用户在 macOS 上构建 V0.7.2 时已经没有 E0425 编译错误，剩余输出主要是 `cocoa` crate 的 deprecated warnings。

本版本把三个 AppKit bridge 函数迁移到 `objc2-app-kit 0.3.2`，并去掉旧的 `activateIgnoringOtherApps` 调用。

建议验证：

```bash
./scripts/static-check.sh
cd src-tauri
cargo check
cargo test
cd ..
./script/build_and_run.sh --verify
```

重点确认：
1. 构建输出不再出现 `use the objc2-app-kit crate instead` 这一批 Cocoa warning。
2. 菜单栏“设置…”仍然可以稳定置前。
3. 悬浮窗仍然跨 Space、可拖动、无左下/右下矩形阴影残影。
