# Proposal

## Why

一次全仓审查与五个参考项目的对照，确认渲染与电源路径已经相当成熟，但
“为什么能省电”这一层缺少可复核的契约：

- 当前 `Choreographer` 只做 vsync 唤醒门控，不自读显示刷新率、不声明固定帧率，
  不分支省电模式；实验峰值帧率比当前显示模式更高时，显示节拍可以低于预算。
  `render-loop-scroll-cadence` 已钉住了滚动合并与闭锁安全网，但没有写清何时
  不能把定时闭锁当成真实帧率、也不能把读取到的刷新率当成硬件帧预算。
- 相同范围的空闲策略已经落地：5 秒无信号且无滚动运动时停止泵帧，空闲闭锁
  500ms，后台暂停渲染线程；但前台服务唤醒锁的获取、续期与通知类型仍分散在
  `TerminalForegroundService` 中，未形成可独立验证的契约，也未区分“维持 shell
  CPU”的 vorer 次等执行锁与不再提升屏幕亮度的普通唤醒锁。
- 平台刷新率不是固定值。Android 官方口径强调应用不能假设固定刷新率，刷新率
  变化要重新读取；显示可能被省电或热节流限制到 60Hz，固定高帧率声明会持续
  消耗功耗；设备对帧率声明只能在当前显示能力下执行。当前实现已经避开了
  固定声明，但没有把这些约束写成必须遵守的行为。
- `composition` 的保持时长不能以固定的毫秒换算代替显示周期：健康活跃闭锁
  34ms 只表示“足够安全”，停摆兜底 17ms 只表示“在主线程阻塞时仍能跟进”。
  120Hz 上 17ms 约两个显示周期，足够；若把它们理解成帧时间预算，会误导
  后续的节流为“超过 17ms 就强制跳过一帧”。

## What Changes

- 新增电源感知自适应渲染的 `display-adaptation` 增量：显示节拍仍唯一来自
  显示帧回调；定时闭锁只作安全网；闭锁可用门可由现有 vsync 新鲜度
  标志推导；高刷时每个显示周期至多一帧；不得声明固定帧率获得更高
  显示刷新率；不得把省电或热节流的显示降频解释为功能故障。
- 新增终端前台服务的电源保持契约：以“维持使用者 shell 的 CPU”作为
  前台服务的执行锁条件；唤醒锁与 Wi-Fi 锁只在有存活会话时生效；唤醒锁
  超时只是失锁安全网，活动会话必须重取；续期发生在持有锁的过程中，
  若锁确已失效则必须回到获取路径；灭屏后的网络策略不能由临时
  省电状态自动改变。
- 文档定位为“为什么不做”的候选方案与“为什么做”的行为契约，不修改源码、
  测试、脚本或受保护文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `display-adaptation`：补刷新率不是事实的契约、闭锁必须按显示周期倍数
  表述，并在有活跃输出或输入时强调同一显示周期至多一帧。
- `terminal-foreground-service`：补前台服务执行锁的获取、续期、
  失效后重取与灭屏后的 Wi‑Fi 锁策略。
- `render-loop-scroll-cadence`：追加电源感知的输出可观察性要求——实际
  vsync 时长要报告到帧时长窗口，而非常数 16.7ms；无新帧时不得呈现
  或提交空帧。
- `code-health`：追加不新增第二套节拍、不做定时器轮询显示的
  明确反例要求。

## Impact

- `openspec/changes/2026-11-08-power-aware-adaptive-rendering/`
  `proposal.md`、`design.md`、`tasks.md` 与三个能力的规约增量。
- 归档时同步 `openspec/specs/display-adaptation/spec.md`、
  `openspec/specs/terminal-foreground-service/spec.md`、
  `openspec/specs/render-loop-scroll-cadence/spec.md` 与
  `openspec/specs/code-health/spec.md`。
- 仪器化数据源尚未接入，先作为未完成项；本文不声称
  该数据已经存在。
- 源码、测试、AndroidManifest 与受保护脚本均不在本次改动范围。

## Success Criteria

- 规约增量能明确区分“节拍”和“刷新率”、以及“闭锁”和“预算”。
- 文档主人可以仅凭本次文档与当前代码定位问题，不需要再查任务历史。
- `openspec validate --all --strict` 通过，且旧有 52 个能力不失败。
