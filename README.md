# TCP Latency for macOS

轻量级 macOS TCP 延迟监测工具，支持多目标 TCP connect 探测、菜单栏实时延迟、磨砂玻璃悬浮 HUD、历史统计、阈值通知和诊断测试。

## V0.8.1

V0.8.1 重点优化长期常驻性能与项目结构：

- 探测调度器改为事件驱动 + 最近 deadline，不再固定 100ms 轮询。
- 暂停状态真正休眠，配置/恢复/探测完成通过事件唤醒。
- 30 秒 DNS TTL 缓存（最多 64 个 endpoint），失败地址可提前失效。
- P95 / jitter 统计减少临时分配，并把排序移出 runtime 全局锁。
- 后端拆分为 `config / probe / runtime / macos_window / tray / commands` 六个模块，`main.rs` 仅负责应用组装。
- 设置页多目标更新按 animation frame 合并；悬浮 HUD 跳过无效 DOM/class 更新并移除强制 reflow。
- PR / main push 自动运行静态检查和 Rust tests。
- 主悬浮窗同时在 Tauri 配置和 AppKit 层关闭原生矩形 shadow。

详细变更见 `CHANGELOG_V0.8.1.md`。

## 功能

- 多目标 TCP connect 延迟监测
- IPv4 / IPv6 / Auto 地址族选择
- 当前延迟、平均值、最小值、最大值、P95、Jitter、失败率
- 最近 60 秒历史曲线
- DNS 解析耗时与最终连接地址展示
- Warning / High / Critical 延迟阈值
- 连续高延迟 / 连续失败通知与恢复通知
- macOS 菜单栏目标切换、暂停、悬浮窗显示/隐藏、鼠标穿透
- 原生 AppKit 窗口行为与 `UnderWindowBackground` 磨砂效果
- 设置窗口复用与现代 macOS activation API
- Universal macOS Release 工作流（arm64 + x86_64）

## 开发

### 静态检查

```bash
./scripts/static-check.sh
```

### Rust 测试

```bash
cd src-tauri
cargo test
```

### 本地开发

```bash
./scripts/dev-macos.sh
```

或使用项目统一运行入口：

```bash
./script/build_and_run.sh
```

## Release

版本号需要同时保持一致：

- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

推送 `v*` Tag 后，GitHub Actions 会构建 Universal `.app` 和 `.dmg`、生成 SHA256，并发布到 GitHub Release。

## 架构

```text
frontend/
  floating.js       # 悬浮 HUD 渲染与交互
  settings.js       # 设置、目标列表、历史图

src-tauri/src/
  main.rs            # 应用组装/启动
  config.rs          # 配置、迁移、校验、持久化
  probe.rs           # DNS 缓存 + TCP connect
  runtime.rs         # runtime、统计、告警、scheduler
  macos_window.rs    # AppKit/Tauri 窗口桥接
  tray.rs            # 菜单栏
  commands.rs        # Tauri commands
```

## License

MIT
