# V0.3.1 编译修复

修复 Tauri `generate_context!()` 因缺失 `src-tauri/icons/icon.png` 导致的编译错误。

新增：
- `src-tauri/icons/icon.png`
- `src-tauri/icons/32x32.png`
- `src-tauri/icons/128x128.png`
- `src-tauri/icons/128x128@2x.png`
- `src-tauri/icons/icon.icns`

同时：
- 在 `tauri.conf.json` 中显式声明 bundle icon。
- `scripts/static-check.sh` 增加图标存在性检查，避免以后再次漏包。
