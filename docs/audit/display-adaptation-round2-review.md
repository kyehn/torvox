# 显示适配第二轮审查

第二轮全量复核：规范七篇、全部被跟踪源码与文档逐文件读取，并独立
复核五个参考项目的 Manifest 参数与帧驱动实现。本轮不改源码、测试、
脚本、受保护文件与状态文档。

## 覆盖证据

- 基线：主分支最新提交快照；被跟踪文件逐个读取并计数，覆盖率清单
  可复算，不抽样。
- 参考项目：termux-app、ghostty-android-terminal、termlib、Drosh、
  awkox/AndroidTerminal、libghostty-rs（规范点名）全部下载至本地
  参考目录并逐处核对 Manifest、帧回调与配置回调。

## 与上一轮的对照

| 发现 | 上一轮判定 | 本轮复核结论 |
| --- | --- | --- |
| 密度变化不重推 | 真问题 | 未修复：`onConfigurationChanged` 仍只比 uiMode 与 fontScale，`configChanges` 无 `density` |
| 启动下界按显示尺寸 | 真问题 | 未修复：`start()` 仍用 `displayMetrics` 算 300–600 / 250–500 下界 |
| 帧回调链无停止路径 | 真问题 | 未修复：`doFrame` 仍无条件自投；常驻注释引用的 `pending_flash_phase` 字段全仓不存在 |
| 配置声明少四项 | 真问题 | 未修复 |
| 刷新率常量按 60Hz 表述 | 非缺陷，改表述 | 未改表述，仍按 60Hz 换算 |

结论：上一轮的全部发现成立，实现未改；本轮不重复记录，只作复核
留档，修复跟踪在 `display-adaptation-round2` 变更任务中。

## 参考项目复核要点

- termux-app 的 `configChanges` 为八项，ghostty-android-terminal 为九项，
  两者都包含 `density`、`smallestScreenSize`、`keyboard`、`navigation`；
  本项目六项少四项，无证据表明缩减是有意设计。
- Drosh 的帧循环 `FrameLoop` 用 `start()`/`stop()` 成对启停，`doFrame`
  先判 `running`；本项目只有启动没有停止，照此补停止路径即可。
- termlib、AndroidTerminal 不提供以 Activity 为基准的对照，不构成反例。

## 未收口项（属其他变更，不在本轮处置）

- 「终端会话概率卡死」：`session-liveness` 主规约已存在，`docs/STATUS.md`
  标未收口；实现需授权。
- 代码规模与大文件治理：`code-health` 主规约与 `reference-code-health-review.md`
  已记录待办；拆分需授权。
