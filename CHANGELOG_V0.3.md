# V0.3 Changelog

## 稳定性修复

- 修改 Host / Port / Address Family 后自动重建目标 Runtime，旧统计不再混入新目标。
- 增加运行时 config generation，配置变更前启动的异步 Probe 结果不会回写新配置。
- inflight 从简单 Set 升级为 `target_id -> generation`，避免旧任务清理掉新任务的 inflight 标记。
- 增加 Stale 判定，不再无限展示旧延迟值。

## 网络探测增强

- 新增 `addressFamily`: `auto / ipv4 / ipv6`。
- DNS 返回多个地址时支持 fallback。
- Auto 模式 IPv4 优先，再尝试其他地址。
- DNS + TCP fallback 共用 Timeout 总预算。
- 增加最后一次 DNS 耗时与实际 resolved address。
- 新增设置页“立即测试”，不保存即可执行一次探测。

## 指标增强

- 新增 P95。
- `lossPercent` 更名为 `failurePercent`，避免把 TCP Probe Failure 误称为丢包。
- 新增 sample age，支持 Stale 展示。

## 通知增强

- 新增恢复通知。
- 不可达恢复后提示当前 RTT。
- 高延迟恢复后提示已回落至 High 阈值以下。

## 悬浮窗增强

- 可隐藏目标名称，只显示延迟数字。
- 可调整透明度。
- 可调整数字字号。
