# TCP Latency V0.7 — Native Window Enhancement

## 目标

V0.7 不改变 TCP 探测核心，重点解决两个 macOS 原生体验问题：

1. 菜单栏点击“设置…”后窗口必须可靠成为前台 Key Window。
2. 悬浮 HUD 的窗口行为交给 AppKit，WebView 只负责内容与微动效。

## 设置窗口修复

- 设置窗口显示时临时切换为 `ActivationPolicy::Regular`。
- 设置窗口显示期间允许 Dock 身份出现，避免“Regular + 隐藏 Dock”互相打架。
- 使用 AppKit `NSApplication.activateIgnoringOtherApps_()` 激活应用。
- 使用 `NSWindow.makeKeyAndOrderFront_()` + `orderFrontRegardless()` 确保设置窗体真正置前。
- 设置窗口关闭时仍然只 Hide，不 Destroy，并恢复 Accessory 菜单栏模式。
- 菜单栏“设置…”失败时不再静默吞错误，会输出 `[ui] 打开设置失败: ...`。

## 原生悬浮 HUD

新增窄 AppKit bridge：

- `NSWindow` 透明背景。
- 原生窗口阴影。
- 整窗背景可拖动。
- `NSFloatingWindowLevel`。
- `CanJoinAllSpaces`。
- `FullScreenAuxiliary`。
- `IgnoresCycle`，避免 HUD 进入普通窗口切换循环。
- 应用失焦时 HUD 保持显示。

## 磨砂玻璃修正

V0.6 在真实 App 中同时存在 AppKit 原生 blur 与 WebKit `backdrop-filter: blur()`，部分 macOS 环境下会产生“双重模糊”，看起来比 HTML 预览更奶白。

V0.7：

- 真实 Tauri 运行时由 AppKit `UnderWindowBackground` 提供桌面 blur。
- WebKit 运行时只保留 saturate / contrast、tint、border、highlight 和 motion。
- standalone HTML 预览继续使用 CSS blur 模拟原生材质。
- 字体、颜色、边框、状态、动画仍共用 `frontend/floating.css`。

## Build macOS Apps 工作流

新增：

- `script/build_and_run.sh`
- `.codex/environments/environment.toml`

`script/build_and_run.sh` 支持：

```bash
./script/build_and_run.sh
./script/build_and_run.sh --verify
./script/build_and_run.sh --logs
./script/build_and_run.sh --debug
```

脚本使用 `cargo tauri build --debug --bundles app` 构建真实 `.app`，再通过 macOS `open -n` 启动，避免用裸 Rust 可执行文件验证 GUI 窗口行为。
