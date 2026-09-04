# TCP Latency V0.6

## 修复：设置窗口点击无反应

- 新增统一 `show_settings_window()`。
- macOS 菜单栏应用平时仍保持 `Accessory` 模式。
- 打开设置时临时切换为 `Regular` 激活策略并保持 Dock 隐藏，解决隐藏设置窗 `show + set_focus` 后仍没有置前的问题。
- 如果设置窗已最小化，会先 `unminimize()`。
- 设置窗关闭时不销毁窗口，改为隐藏并恢复 `Accessory` 模式，下一次打开更稳定。
- 菜单栏“设置…”和悬浮窗双击都走同一条打开逻辑。

## 磨砂玻璃 UI 一致性

V0.5 的 HTML 预览和真实窗口分别维护视觉参数，且真实窗口使用 `HudWindow` 材质，因此在不同 macOS 背景下会出现明显偏差。

V0.6 调整为：

- 原生 `UnderWindowBackground` 只负责真实桌面背景模糊。
- 最终可见的 tint / border / sheen / pointer highlight 由 `floating.css` 负责。
- 默认 82% 透明度时，CSS 参数与 HTML 预览完全采用同一组数值。
- 实际窗口和预览都使用 228×100 / 24px 圆角的 Standard 几何尺寸。
- 去掉真实窗口额外的 4px body padding，避免实际卡片比预览小一圈。
- Hover 不再放大卡片，避免透明窗口边缘裁切；只增强高光。
- 鼠标高光改为 `requestAnimationFrame` 节流，降低常驻时的主线程开销。
- 增加 `prefers-reduced-transparency` 降级。

> 原生 macOS 磨砂最终仍会受系统版本、Light/Dark、壁纸和“降低透明度”系统设置影响，因此桌面背景模糊本身不能做到逐像素等同静态 HTML；V0.6 将可控的前景层统一后，视觉差异主要只剩系统原生背景材质。
