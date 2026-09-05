> V0.8: macOS AppKit bridge 已从 deprecated `cocoa 0.26.1` 迁移到 `objc2-app-kit 0.3.2`，并移除 macOS 14 已弃用的 `activateIgnoringOtherApps`。详见 `CHANGELOG_V0.8.md`。

> V0.7.2 hotfix: 关闭透明 NSWindow 的矩形原生阴影，只保留跟随圆角的 CSS 阴影，修复悬浮窗左下/右下角方形阴影残影。详见 `FIX_V0.7.2.md`。

> V0.7.1 hotfix: 修复 macOS AppKit bridge 中 `YES` 未导入导致的 E0425 编译失败。详见 `FIX_V0.7.1.md`。

# TCP Latency for macOS V0.6

使用 **Rust + Tauri 2** 实现的 macOS 菜单栏 TCP Connect 延迟监测工具。V0.6 在现有多目标 TCP 探测能力上，重点修复设置窗口无法置前的问题，并让真实 macOS 磨砂悬浮窗与 HTML 预览使用同一套可控视觉参数。

## V0.6 重点

- Apple 风格半透明毛玻璃 HUD
- Light / Dark 自动跟随 macOS
- Standard 原生窗口 228×100，24px 圆角
- Compact / Standard / Large 三种原生窗体尺寸
- SF 系统字体与 tabular numbers
- 数值、`ms`、趋势、目标名称独立布局，减少刷新抖动
- 状态点可选；异常只改变数字和状态点，不整块染红
- 目标名称自动省略，不撑开窗口
- Timeout / Offline / Stale 等状态使用固定大小文字，不破坏布局
- Hover 不再缩放窗口，只做玻璃反光增强，避免边缘裁切
- 支持 `prefers-reduced-motion`
- 鼠标穿透、拖动、双击设置继续保留

## 默认 UI 参数

```text
Size                 Standard
Window/Card          228 × 100
Corner Radius        24 px
Latency Font         42 px
Unit Font            17 px
Target Font          13 px
Glass Opacity        82%
Blur                  24 px
Status Dot           On
Trend Arrow          Off
Theme                 Follow macOS
```

- 修复菜单栏“设置…”点击后窗口没有置前的问题
- 关闭设置窗口后隐藏而不是销毁，下一次打开更稳定
- 原生 UnderWindowBackground 负责底层模糊，CSS 负责可控玻璃表面
- HTML 预览与真实窗口共用 `frontend/floating.css`

详细变化见 `CHANGELOG_V0.6.md`。

## 继承的 V0.3 网络监测能力

- macOS 菜单栏实时显示主目标延迟，例如 `23 ms`
- 桌面透明悬浮延迟数字，始终置顶、可拖动、可鼠标穿透
- 多目标独立并行 TCP Connect 探测
- 单目标 Timeout 不阻塞其他目标
- 每目标独立 Interval / Timeout
- **Auto / IPv4 / IPv6** 地址族选择
- Auto 模式下 DNS 返回多个地址时自动 fallback
- DNS 时间与 TCP Connect RTT 分离
- 设置页“**立即测试**”，无需保存即可验证 Host / Port
- 立即测试显示：DNS 耗时、TCP 耗时、实际连接地址、失败原因
- Current / Avg / Min / Max / **P95** / Jitter / Failure Rate / DNS
- `Timeout / Refused / Offline / DNS Timeout / DNS Error / Stale / Disabled / Paused` 状态区分
- 最近 60 秒趋势图
- **Stale 检测**：长时间没有新采样时不继续显示旧延迟值
- **目标端点变更自动清空旧统计**，避免修改 Host/Port 后历史数据混合
- **配置 generation 防串数据**：旧异步探测完成后不会写回新配置
- 高延迟连续 N 次告警
- TCP 连续失败 N 次告警
- **恢复通知**：可达恢复或高延迟恢复后发送一次通知
- 通知冷却
- 悬浮窗目标名称显示开关
- 悬浮窗透明度、数字字号可调
- 登录自动启动
- V0.1 / V0.2 配置兼容
- Apple Silicon ARM64 构建脚本

## 为什么 V0.3 需要这些修正

### 1. 修改 Host/Port 后不能沿用旧统计

V0.2 中如果保留相同 Target ID，只修改：

```text
10.0.0.10:443
      ↓
10.0.0.20:443
```

旧 60 秒样本可能继续存在。V0.3 使用 `host + port + addressFamily` 生成 endpoint key；端点变化后 Runtime 直接重建。

### 2. 防止旧异步结果写回新配置

例如：

```text
A: 10.0.0.10:443 正在 Timeout
             │
             ├── 用户修改目标为 10.0.0.20:443
             │
             └── 旧 Probe 2 秒后才返回
```

V0.3 给配置增加运行时 generation。旧 generation 的结果会被丢弃，不会覆盖新目标状态。

### 3. 多地址 DNS fallback

域名可能同时存在：

```text
A     → IPv4
AAAA  → IPv6
```

V0.2 只使用第一个解析地址，可能因为第一个地址不可达而误判服务异常。V0.3 支持：

```text
Auto    IPv4 优先，失败后继续尝试其他解析地址
IPv4    只使用 A / IPv4
IPv6    只使用 AAAA / IPv6
```

整个 DNS + fallback 过程受 Target Timeout 总预算限制，但最终显示的 TCP RTT 仍只统计实际成功连接的 TCP Connect 时间，不把 DNS 时间混进去。

### 4. Stale

长期睡眠、网络切换或异常调度后，旧的 `23 ms` 不应该一直看起来像“当前值”。V0.3 的 Stale 阈值为：

```text
max(5 秒, interval × 3 + timeout)
```

超过后当前延迟清空并显示 `Stale`。

## 立即测试

设置页修改目标后直接点击：

```text
立即测试
```

成功示例：

```text
OK · TCP 23.4ms · DNS 1.2ms · 10.10.10.100:6443
```

失败示例：

```text
Refused · DNS 0.8ms · 10.10.10.20:443 · TCP connection refused
```

这适合区分：

- DNS 解析问题
- IPv4 / IPv6 路径问题
- 网络 Timeout
- 主机可达但端口 Refused
- 服务正常但 TCP RTT 偏高

## 统计指标说明

| 指标 | 含义 |
|---|---|
| Current | 最近一次成功 TCP Connect RTT |
| Avg | 最近 60 秒成功样本平均值 |
| Min / Max | 最近 60 秒成功样本极值 |
| P95 | 最近 60 秒成功样本 95 分位延迟 |
| Jitter | 连续成功 RTT 的绝对变化均值 |
| Failure Rate | 最近 60 秒 TCP Probe 失败比例 |
| DNS | 最近一次 DNS 解析耗时，不计入 TCP RTT |

这里使用 **Failure Rate**，不称为 ICMP “丢包率”，因为 TCP Connect 失败包含 Timeout、Refused、DNS 错误等多种原因。

## 源码结构

```text
TcpLatency_Mac_V0.6/
├── frontend/
│   ├── index.html
│   ├── floating.css
│   ├── floating.js
│   ├── settings.html
│   ├── settings.css
│   └── settings.js
├── scripts/
│   ├── dev-macos.sh
│   ├── build-macos-arm64.sh
│   └── static-check.sh
└── src-tauri/
    ├── capabilities/default.json
    ├── src/main.rs
    ├── build.rs
    ├── Cargo.toml
    └── tauri.conf.json
```

## macOS 开发运行

要求：

- macOS 12+
- Xcode Command Line Tools
- Rust stable（Rust >= 1.77.2）

```bash
cd TcpLatency_Mac_V0.6
./scripts/static-check.sh
./scripts/dev-macos.sh
```

也可以：

```bash
cd src-tauri
cargo check
cargo test
cargo tauri dev
```

## Apple Silicon ARM64 打包

```bash
./scripts/build-macos-arm64.sh
```

产物通常位于：

```text
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/
src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/
```

## 配置兼容

V0.4 继续兼容 V0.1 / V0.2 / V0.3 配置。首次载入旧 UI 配置时会执行一次界面迁移，监测目标和告警配置保持不变：

```text
addressFamily = auto
notifyRecovery = true
floatingShowTarget = true
floatingOpacity = 0.82
floatingFontSize = 42
floatingSize = standard
floatingShowStatusDot = true
floatingShowTrend = false
uiVersion = 4
```

## 当前边界

- 历史趋势目前仅保存在内存最近 60 秒，不做 SQLite 长期归档。
- TCP Connect RTT 不等价于 ICMP RTT。
- `Connection Refused` 代表网络路径至少收到了目标端 RST，但对于“监测指定服务端口是否可用”的场景仍记为 Probe 失败。
- DNS fallback 为轻量实现，不解析权威 DNS TTL，也不是完整 Happy Eyeballs 算法。
- 透明悬浮窗口继续使用 Tauri `macOSPrivateApi`，更适合内部 `.app/.dmg` 使用。

## 下一阶段建议

后续不建议马上扩展成“大而全监控平台”，优先考虑两项：

1. **TCP / TLS / HTTP 分阶段耗时**：TCP Connect、TLS Handshake、TTFB 分开显示，用于定位“网络慢还是应用慢”。
2. **异常事件时间线 + SQLite 小时级聚合**：只持久化异常和聚合值，而不是把每秒样本全部写盘。


## V0.5 原生磨砂玻璃动效

悬浮 HUD 在 macOS 使用 Tauri `Effect::HudWindow` + `EffectState::Active` 应用系统级 Window Effect。前端叠加非常轻的半透明 tint、高光漂移、指针反射、数值淡变和状态点脉冲。系统开启“减少动态效果”时自动关闭非必要动画。

---

## V0.7 Native Window Enhancement

V0.7 增加 macOS 原生 AppKit 窗口桥接，主要用于解决菜单栏工具的前台激活、Spaces、原生浮动层级和背景拖动问题。

### 推荐构建与运行

```bash
./script/build_and_run.sh
```

验证 App 是否成功启动：

```bash
./script/build_and_run.sh --verify
```

查看运行日志：

```bash
./script/build_and_run.sh --logs
```

LLDB：

```bash
./script/build_and_run.sh --debug
```

该入口会构建真实的 Tauri `.app` bundle 后通过 macOS Launch Services 启动。对于“设置窗口是否真正置前”“菜单栏 App 激活策略”“原生窗口材质”等问题，应优先用该方式验证，而不是直接运行裸 `target/debug/tcp-latency`。

### 设置窗口行为

当用户从菜单栏或双击悬浮 HUD 打开“设置”时：

```text
Accessory 菜单栏模式
        ↓
Regular 前台模式
        ↓
显示设置窗口
        ↓
AppKit activate
        ↓
makeKeyAndOrderFront
        ↓
用户关闭设置
        ↓
Hide 设置窗口
        ↓
恢复 Accessory
```

### V0.7 磨砂层职责

```text
AppKit / Tauri UnderWindowBackground
        └── 真正桌面 blur

frontend/floating.css
        ├── glass tint
        ├── border / rim
        ├── highlight
        ├── status color
        ├── typography
        └── micro motion
```

真实 App 不再额外执行第二层 WebKit blur，避免 V0.6 在部分 Mac 上出现比 HTML 预览更白、更糊的问题。
