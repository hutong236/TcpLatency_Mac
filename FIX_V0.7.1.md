# V0.7.1 macOS 编译修复

## 修复内容

修复 macOS AppKit bridge 编译错误：

```text
error[E0425]: cannot find value `YES` in this scope
```

问题出现在 `configure_native_floating_window()` 和 `configure_native_settings_window()`。
V0.7 使用了 `setHasShadow_(YES)`、`setMovable_(YES)`、`setCanHide_(YES)`，但对应函数的局部 import 只导入了 `NO`。

V0.7.1 已补充：

```rust
use cocoa::base::{id, nil, NO, YES};
use cocoa::base::{id, NO, YES};
```

同时版本号更新到 `0.7.1`，静态检查增加 YES import 校验。

## 关于 40 个 warning

这些 warning 来自 `cocoa 0.26.1` 已弃用 API，**不是本次构建失败原因**。当前先保持最小改动，避免在修复设置窗口的同时引入 objc2 大范围迁移风险。后续版本可单独把 AppKit bridge 从 `cocoa` 迁移到 `objc2` / `objc2-app-kit`。

## Mac 验证

```bash
./scripts/static-check.sh
cd src-tauri
cargo check
cargo test
cd ..
./script/build_and_run.sh --verify
```
