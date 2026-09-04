# TCP Latency V0.4 Changelog

## 定位

V0.4 是 **macOS Apple 风格悬浮 HUD 重构版**。TCP 探测、IPv4/IPv6 fallback、P95、Stale、恢复通知、多目标并行等 V0.3 能力继续保留，本轮主要重构桌面悬浮窗和相关配置。

## 新增与优化

### 1. Apple 风格悬浮 HUD

- 标准卡片视觉尺寸约 `220 × 92`
- 24px 大圆角
- SF 系统字体优先
- 半透明中性毛玻璃背景
- 24px blur + 160% saturation
- 轻边缘高光和柔和阴影
- Light / Dark 自动跟随 macOS
- 主数字和目标名称重新分层

### 2. 状态表达更克制

异常不再整块修改背景，只改变：

- 主延迟数值颜色
- 右上角 6px 状态点

支持：

- Normal
- Warning
- High
- Critical
- Timeout
- Refused
- Offline
- DNS Timeout / Error
- Stale
- Paused
- Disabled
- Starting

### 3. 数字稳定

使用 `tabular-nums` / `tnum`，延迟从 `8 → 18 → 108` 时减少横向跳动。

趋势箭头从延迟数字字符串中拆分为单独元素，避免布局抖动。

### 4. 三种窗体尺寸

- Compact：约 `170 × 68`
- Standard：约 `220 × 92`
- Large：约 `260 × 108`

Rust 后端会同步调整 Tauri 原生窗口尺寸，不只是缩放网页内容。

### 5. 可选悬浮信息

新增设置：

- 显示目标名称
- 显示状态点
- 显示趋势箭头
- 窗体尺寸
- 毛玻璃透明度
- 主数字字号

默认保持极简：显示目标、显示状态点、隐藏趋势箭头。

### 6. V0.3 → V0.4 配置迁移

旧配置首次加载时自动升级到 V0.4 UI 默认：

```text
floatingOpacity = 0.82
floatingFontSize = 42
floatingSize = standard
floatingShowStatusDot = true
floatingShowTrend = false
uiVersion = 4
```

不会影响已有监测目标、阈值、通知配置。

## 未改变

- TCP Connect RTT 定义不变
- DNS 耗时仍与 TCP RTT 分离
- 多目标仍并行探测
- IPv4/IPv6 fallback 不变
- 60 秒历史、P95、Jitter、Failure Rate 不变
- 鼠标穿透、登录启动、系统通知继续保留
