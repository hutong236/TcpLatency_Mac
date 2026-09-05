# TCP Latency V0.8.1 — Runtime & Architecture Optimization

## 目标

V0.8.1 聚焦常驻性能、后端可维护性和设置页刷新效率，不改变现有 TCP 延迟语义、通知规则、AppKit 磨砂悬浮窗交互和多目标能力。

## 常驻性能

- 探测 scheduler 从固定 100ms polling 改为 `Notify + 最近 deadline` 的事件驱动调度。
- 暂停状态不再每 250ms 唤醒，恢复/配置变化时通过事件唤醒。
- 配置保存、暂停/恢复、探测完成会主动触发重新调度。
- 同 generation 的目标只允许一个 in-flight 探测。
- P95/jitter 统计减少临时 Vec 分配和重复 clone。
- 统计排序移出全局 runtime 锁，缩短多目标场景下的锁占用时间。

## DNS

- 增加进程内 DNS TTL 缓存，默认 TTL 30 秒。
- 最多保存 64 个 endpoint，避免长期运行时无界增长。
- endpoint key 包含 host、port、address family，IPv4/IPv6 切换不会复用错误缓存。
- 缓存地址出现 timeout/offline 时立即失效，下次探测重新解析；Connection Refused 保留缓存。

## 前端

- 悬浮 HUD 配置样式只在配置变化时重新计算，不再每次 latency snapshot 重写 CSS 变量。
- 数值和视觉状态未变化时跳过无效 DOM/class 更新。
- 使用 `requestAnimationFrame` 重启动画，移除 `offsetWidth` 强制同步 reflow。
- 设置页 `target-update` 与 `latency-update` 分工，避免活动目标重复重绘。
- 多目标表格更新按 animation frame 合并，连续事件只进行一次完整表格刷新。

## 后端结构

原单文件 `main.rs` 已按职责拆分：

- `config.rs`：配置模型、迁移、校验、持久化。
- `probe.rs`：DNS 解析缓存与 TCP connect 探测。
- `runtime.rs`：快照、历史、统计、告警状态和 scheduler。
- `macos_window.rs`：AppKit/Tauri 原生窗口桥接与磨砂效果。
- `tray.rs`：菜单栏和托盘交互。
- `commands.rs`：Tauri command API。
- `main.rs`：仅保留应用组装、插件注册、窗口生命周期和启动逻辑。

## UI / macOS

- Tauri 主悬浮窗配置直接设置 `shadow: false`，与 AppKit `setHasShadow(false)` 双重保证，减少启动瞬间矩形窗口阴影的机会。
- 保留 objc2 AppKit 窗口实现、全空间显示、设置窗口复用和现代 activation API。

## 工程质量

- PR/main push 自动执行静态检查和 `cargo test`。
- 静态检查验证模块边界，并限制 `main.rs` 只承担应用组装职责。
- 增加 scheduler、统计、配置迁移、目标 ID、endpoint key 和地址族过滤测试。
- 明确禁止重新引入固定 100ms/250ms scheduler polling。

## 版本

- Cargo package: `0.8.1`
- Tauri app: `0.8.1`
