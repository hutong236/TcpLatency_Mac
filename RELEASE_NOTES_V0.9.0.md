# TCP Latency v0.9.0

本版本新增本机实时网络流量 HUD：延时继续作为第一视觉层级，下方显示当前接口下载/上传速率。

- 新增 macOS 原生 RX/TX 接口计数采集，每 1 秒更新。
- 默认 Auto 跟随默认路由接口，降低 VPN + 物理接口重复统计风险。
- 支持手动指定 `en0` / `utun*` 等接口。
- 接口切换、睡眠唤醒、counter reset 时自动重建 baseline。
- 设置页新增流量显示和接口选择。
- 保持现有 TCP probe、DNS cache、P95、Jitter、Failure Rate 与告警逻辑不变。

构建产物为 Universal macOS `.app.zip` 与 `.dmg`，包含 arm64 + x86_64。当前采用 ad-hoc 签名，尚未进行 Developer ID notarization。
