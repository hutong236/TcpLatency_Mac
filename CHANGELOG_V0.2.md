# V0.2 Changelog

## 新增

- 多目标同时监测
- 每目标独立采样与统计
- 最近 60 秒延迟趋势图
- 鼠标穿透/锁定悬浮窗
- 系统异常通知
- 通知连续次数阈值
- 通知冷却
- 菜单栏目标快速切换
- 设置页全部目标状态表
- Target Enabled 开关
- 悬浮延迟短时趋势箭头
- V0.1 配置兼容迁移

## 架构变化

V0.1：

```text
active target -> single probe loop -> single sample queue
```

V0.2：

```text
scheduler
   ├── target A -> async TCP probe -> runtime A
   ├── target B -> async TCP probe -> runtime B
   └── target C -> async TCP probe -> runtime C

activeTargetId
   ├── Tray title
   ├── Floating window
   └── 60s chart
```
