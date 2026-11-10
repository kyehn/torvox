# openspec 目录与规约漂移核对

本文记录一次针对 openspec 目录的核对：清点未归档变更与主规约，逐个判定未归档变更
该不该并入主规约，并抽查近期改动涉及的能力规约与实现是否一致。审查不修改
`docs/STATUS.md`，不改动受保护文件。

## 审查范围与证据

- 目录：`openspec/specs/`（52 个能力规约、399 条 Requirement）与
  `openspec/changes/`（191 份归档留档与未归档变更）。
- 校验：`openspec validate --all --strict` 全部通过。
- 未归档变更逐个读提案、设计、任务与规约增量，把增量里的每条要求与实现现状、
  审查留档逐项对照。

## 发现

### 已完成的代码健康审查变更可以归档并同步主规约

`2026-11-08-code-health-reference-review` 的研究与留档任务全部完成，其规约增量
的四条要求分别由 `docs/audit/reference-code-health-review.md` 满足：统一源码行数
口径与生产、测试、合计三档数字，跨源码语言一致的超大文件阈值与超限文件行数清单，
参考项目研究留档的缺口登记，规范声明的静态检查工具与验证门禁的差异登记。增量
自身也写明受保护落点未获授权时 MUST 登记为待办而不改动受保护目录的实际内容——
三份未完成任务正是这种待办，另有一项明写「另立变更」。

处置：执行 `openspec archive 2026-11-08-code-health-reference-review -y`，主规约
`code-health` 增加四条 Requirement（规模口径、超大文件阈值、参考项目留档、静态
检查工具与门禁一致），既有四条保留不变，主规约 `## Purpose` 未被改写。

### 两条变更描述的是尚未实现的行为，不能并入主规约

`2026-11-08-display-adaptation` 与 `2026-11-08-power-aware-adaptive-rendering` 的
规约增量要求：配置变化（显示密度、字体缩放、输入设备）就地生效而不重建 Activity、
首建会话按当前窗口尺寸而非整块显示尺寸、帧回调链按活跃度启停、真实 vsync 周期
数据源接入并据此结算帧时长窗口、前台服务执行锁只在有存活会话时持有且失效后重取。
这些行为在实现里尚不存在，对应任务全部标注「需授权」；其中一条增量自己写明
「当前数据源尚未接入，属于已知未完成项」。

处置：两条变更保持未归档。提前归档会把未实现的行为写进主规约，主规约就此失真，
后续实现时反而无从判断哪一条是真约束。

### 主规约与实现的抽查一致

- `session-liveness`：忙标记、跨 JNI 边界换算已忙时长、渲染与 VT 与输出投递与
  Kotlin 生命周期四层判定，以及各层的恢复动作，与 `monitor/RenderWatchDog.kt`、
  `monitor/SessionLiveness.kt` 的实现一致；看门狗身份复核的三条 Requirement 已在
  归档变更中同步。
- `desktop-notification`：OSC 9 与 OSC 777 的采集、前台应用内提示与后台系统通知、
  点击切回会话、排空有界、通知 id 与常驻通知分离，与 `runtime/DesktopNotificationDrain.kt`、
  `runtime/DesktopNotificationPlayer.kt` 的实现一致。
- `text-search`：命中总数、导航、视口高亮、关闭后原生回到 idle 的要求与上游
  `search::Search` 单一来源一致；计数文案「共 N 个」与「第 k 个，共 N 个」的形状
  与 `ui/TextSearchBar.kt` 的渲染一致。
- `logcat-hygiene`：限频与按窗口汇总的要求逐条对应到实现侧的行输出——逐帧慢帧
  诊断、逐帧慢循环诊断、事件队列溢出、OSC 52 写回失败与应答缺失、命令入队失败，
  每类都有独立窗口与窗口内累计次数。

## 明确不作处置的项

- **超大文件按同一阈值拆分**：`code-health` 新增的要求把它列为跨语言不一致的
  未完成项，处置方式是「另立变更并需改源码授权」，不属本次核对范围。
- **参考项目研究留档补齐**与**静态检查工具接入门禁**：落点分别是受保护目录与
  受保护脚本，需明确同意后才可改动；本次只核对它们是否已按规约登记为待办。

## 复现方法

- 目录清点：`ls openspec/specs`、`ls openspec/changes`、`ls openspec/changes/archive`。
- 严格校验：`openspec validate --all --strict`。
- 归档：`openspec archive <变更名> -y`；归档后核对主规约新增的 Requirement 条数与
  `## Purpose` 是否仍为原内容。
