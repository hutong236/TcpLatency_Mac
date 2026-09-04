# TCP Latency V0.5 — 原生磨砂玻璃与微动效

## 新增

- macOS 原生 `HudWindow` Window Effect，透明窗口使用系统 Vibrancy/Blur，而不是只靠 CSS 假磨砂。
- 启动浮现：约 240ms 的淡入 + 微缩放 + 极轻模糊收敛。
- 玻璃高光缓慢漂移，8 秒周期、低强度。
- 鼠标悬停时高光跟随指针，窗口仅放大约 0.8%。
- 数值更新使用 150ms 微淡变/微缩放，不做翻牌动画。
- 延迟状态变化增加一次性材质亮度响应。
- Critical/Timeout/Offline/Refused/DNS Error 状态点使用低频呼吸脉冲。
- 完整支持 `prefers-reduced-motion`，系统开启“减少动态效果”后禁用非必要动画。
- V0.4 的 Compact / Standard / Large、鼠标穿透、Light/Dark、状态色等全部保留。

## 设计原则

- 背景不随告警整体变红/变橙。
- 动画只帮助感知状态变化，不持续抢注意力。
- 原生 macOS 材质负责真实背景模糊，CSS 只负责轻微 tint、反光和边缘质感。
