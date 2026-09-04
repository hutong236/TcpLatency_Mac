# V0.6 QA Notes

## 已完成静态检查

- `frontend/floating.js`：Node syntax PASS
- `frontend/settings.js`：Node syntax PASS
- `tauri.conf.json`：JSON PASS
- `capabilities/default.json`：JSON PASS
- `Cargo.toml`：TOML PASS
- 图标存在性：PASS
- `show_settings_window()`：存在
- macOS `ActivationPolicy::Regular -> Accessory` 设置窗生命周期：存在
- 设置窗 `CloseRequested -> hide()`：存在
- `Effect::UnderWindowBackground`：存在
- HTML 预览与运行浮窗共用 `frontend/floating.css`：存在
- 228×100 Standard 原生窗口尺寸：存在
- pointer highlight `requestAnimationFrame` 节流：存在
- `prefers-reduced-motion` / `prefers-reduced-transparency`：存在

## 仍需在真实 Mac 验证

当前生成环境不是 macOS，且没有 Rust/Cargo，因此下面两项需要在目标 Mac 上执行：

```bash
cd src-tauri
cargo check
cargo test
cargo tauri dev
```

重点验收：

1. 菜单栏 `设置…` 首次点击可立即置前。
2. 关闭设置窗口后再次点击仍可立即打开。
3. 悬浮窗双击设置与菜单栏使用同一路径。
4. Light / Dark 下磨砂材质与 `TcpLatency_Mac_V0.6_Glass_Motion_Preview.html` 的前景 tint、边框、字体、几何尺寸一致。
5. macOS “降低透明度”打开后 UI 可读。
