# TCP Latency V0.8 — objc2 AppKit Migration

## 目标

V0.7.x 已能正常进入 AppKit 原生窗口层，但使用 `cocoa 0.26.1` 会在新工具链上产生大量 deprecated warning。V0.8 将这部分桥接迁移到 `objc2-app-kit 0.3.2`。

## 变更

- 移除 `cocoa = "0.26.1"`。
- 新增 `objc2-app-kit = "0.3.2"`。
- `NSWindow` 属性全部改为 objc2 的 bool / 强类型 API。
- 使用 `NSFloatingWindowLevel`，不再硬编码 level 3。
- `NSWindowCollectionBehavior` 改用 `CanJoinAllSpaces | FullScreenAuxiliary | IgnoresCycle`。
- 继续关闭 NSWindow 原生矩形 shadow，仅保留 CSS 圆角 shadow。
- 设置窗口复用逻辑继续保留：`setReleasedWhenClosed(false)`。
- 设置窗口激活改为 `NSRunningApplication::activateWithOptions(ActivateAllWindows)`，不再调用 macOS 14 已弃用的 `activateIgnoringOtherApps`。
- 增加 `native_ns_window()`，把 Tauri 原生句柄转换集中在单一边界。
- 静态检查新增：禁止重新引入 `cocoa::` 和 `activateIgnoringOtherApps`。

## 兼容

TCP 探测、菜单栏、磨砂 UI、窗口尺寸、目标配置和 V0.7.2 阴影修复逻辑不变。
