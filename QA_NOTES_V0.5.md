# V0.5 QA Notes

## 已完成静态检查

- `node --check frontend/floating.js`：PASS
- `tauri.conf.json` JSON 解析：PASS
- V0.4 图标完整性检查：PASS
- `Effect::HudWindow` 原生窗口效果引用：PASS
- `apply_floating_window_effect()` 初始化与保存配置调用：PASS
- `glassEnter` / `glassSheenDrift` 动效：PASS
- Pointer-reactive glass highlight：PASS
- `prefers-reduced-motion`：PASS

## 仍需在真实 macOS 验证

当前执行环境没有 Rust/macOS 工具链，因此尚未实际执行 `cargo check` / `cargo test` / `cargo tauri dev`。

建议在 Apple Silicon Mac 上执行：

```bash
./scripts/static-check.sh
cd src-tauri
cargo check
cargo test
cd ..
cargo tauri dev
```

重点目视确认：

1. `HudWindow` 材质能够透出桌面/后方窗口并产生真实磨砂效果；
2. 拖动时没有明显卡顿；
3. Light/Dark 两种模式文字对比度合适；
4. 鼠标穿透后反光 Hover 不残留；
5. 系统开启“减少动态效果”后，持续高光和状态脉冲停止。
