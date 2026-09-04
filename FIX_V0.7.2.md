# V0.7.2 — 悬浮窗底部方形阴影修复

## 问题

V0.7.1 同时启用了 macOS 原生 `NSWindow` shadow 和 CSS 圆角卡片 shadow。
透明 `NSWindow` 的原生阴影按窗口的矩形边界生成，不会跟随 WebView 中卡片的 CSS 圆角，因此在左下角、右下角会看到方形/直角阴影残影。

## 修复

- `NSWindow::setHasShadow_(YES)` 改为 `NO`。
- 保留 `.floating` 的 CSS `box-shadow`，它会正确跟随 `border-radius`。
- 将 CSS 外阴影由 `0 14px 36px` 收紧为 `0 10px 28px`。
- 浅色模式阴影透明度从 `0.13` 降为 `0.095`。
- 深色模式阴影透明度从 `0.38` 降为 `0.28`。

结果：保留柔和的 Apple 风格悬浮层次，但不再在窗口矩形边界的左右下角出现阴影块。
