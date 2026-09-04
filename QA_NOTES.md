# TCP Latency V0.4 QA Notes

## 已完成静态检查

- `frontend/floating.js`：Node 语法检查通过
- `frontend/settings.js`：Node 语法检查通过
- `src-tauri/tauri.conf.json`：JSON 解析通过
- `src-tauri/capabilities/default.json`：JSON 解析通过
- `src-tauri/Cargo.toml`：TOML 解析通过
- macOS 图标文件完整性检查通过
- V0.4 配置字段存在性检查通过
- Light / Dark CSS 规则检查通过
- `tabular-nums` 数字稳定规则检查通过
- 原生窗口尺寸同步函数存在性检查通过

## V0.4 关键代码检查点

- `floatingSize`: compact / standard / large
- `floatingShowStatusDot`
- `floatingShowTrend`
- `floatingOpacity`: 0.70 ~ 1.00
- `floatingFontSize`: 30 ~ 52
- `uiVersion = 4`
- V0.3 配置首次加载自动迁移并落盘
- `apply_floating_window_size()` 在启动和保存配置时执行
- `set_ignore_cursor_events()` 鼠标穿透继续保留

## 当前环境限制

当前生成环境没有 `cargo` / `rustc`，因此没有实际执行：

```bash
cargo check
cargo test
cargo tauri dev
```

请在 macOS 上执行：

```bash
cd TcpLatency_Mac_V0.4
./scripts/static-check.sh
cd src-tauri
cargo check
cargo test
cargo tauri dev
```

如果出现 Rust/Tauri 编译错误，以 macOS 实际编译结果为准继续修正。
