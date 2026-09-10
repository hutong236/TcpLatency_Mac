# TCP Latency V0.9.0 — Live Network Traffic HUD

## 目标

V0.9.0 在现有 TCP Connect 延迟监测基础上增加本机实时网络流量显示，让用户在一个轻量 HUD 中同时判断“网络延迟是否异常”和“当前链路是否繁忙”。

## 新功能

- 悬浮 HUD 增加实时下载 / 上传速率：`↓ Download  ↑ Upload`。
- 新增独立 `traffic.rs` 流量采集模块，不改变现有 TCP probe、DNS cache、P95、Jitter、Failure Rate 语义。
- macOS 使用原生接口计数器读取 RX/TX bytes，每 1 秒计算实时速率，不抓包、不需要管理员权限。
- 默认 `auto` 跟随默认路由接口，避免简单汇总 VPN 与物理接口造成重复统计。
- 支持手动指定接口，例如 `en0`、`utun3`。
- 接口切换、长时间睡眠/唤醒、counter reset 或异常尖峰时自动重建采样 baseline。
- 设置页增加“显示网络流量”和“流量接口”配置。

## UI

- 延时继续作为第一视觉层级。
- 流量作为第二视觉层级显示在延时数字下方。
- Standard 窗口继续保持 228×100，不因新增流量指标扩大窗口。
- Compact / Standard / Large 三种尺寸继续兼容。

## 性能与边界

- 流量采样默认每 1 秒一次。
- 关闭流量显示时不持续读取接口计数器。
- 流量统计表示当前选定网络接口的本机 RX/TX 速率，不是某个 TCP 目标独占的流量。
- 当前仍采用 ad-hoc macOS 签名，尚未接入 Developer ID + notarization。

## 版本

- Cargo package: `0.9.0`
- Tauri app: `0.9.0`
- Git tag: `v0.9.0`
