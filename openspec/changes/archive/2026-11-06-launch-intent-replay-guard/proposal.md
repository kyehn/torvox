# Proposal

## Why

配置变更（密度、语言）会重建 `MainActivity`，重建后 `onCreate` 再次收到**同一个**启动
Intent——`getIntent()` 仍是当初那一条。Failsafe Intent 早已按「重建不重放」处置，
但同一条 Intent 携带的另外两项状态改写没有同样处置：

- 桌面通知点击带来的会话切换：`handleLaunchIntent` 无条件调
  `requestSessionSwitch(notificationSessionId)`。用户在通知点击后手动切到别的会话，
  一次配置变更就把界面拽回通知所属的会话；
- 打开设置页的 extra：`openSettingsRequests++` 无条件自增。用户关掉设置页后的一次
  配置变更会把设置页重新打开。

反方向也有问题：设置覆盖层的开合状态由 `remember` 持有（组合一销毁即丢失），
重建后是否打开取决于**启动 Intent**——手动打开的设置页在重建后凭空消失，而带
extra 启动的设置页在关掉后又被重建弹回来。同一件事的两种表现，取决于它是怎么
被打开的。

## What Changes

- `MainActivity.handleLaunchIntent` 的两项状态改写（通知会话切换、打开设置）改为
  只在**新投递**的 Intent 上生效：重建时的重复投递不再重放。
- `TerminalViewModel.requestSessionSwitch` 接收「是否新投递」，在陈旧请求上直接
  忽略：不切换，也不留下待补切的请求。
- 设置覆盖层的开合状态改由 `rememberSaveable` 持有，跨 Activity 重建保持。
- 规约 `app-lifecycle` 增补两条要求（重建不重放启动 Intent 的状态改写、覆盖层
  状态跨重建保持），`desktop-notification` 的「点击切换会话」要求补上重建不重放
  的条款。

## Capabilities

### Modified Capabilities

- `app-lifecycle`：启动 Intent 的状态改写与设置覆盖层状态在 Activity 重建时的处置。
- `desktop-notification`：点击切换会话在 Activity 重建时不重放。

## Impact

- Kotlin：`MainActivity.kt`（Intent 处置的门控、设置覆盖层状态）、`TerminalViewModel.kt`
  （`requestSessionSwitch` 接收「是否新投递」）。
- 行为变化仅限 Activity 重建：重建不再重放通知切换与打开设置；设置覆盖层的开合
  状态跨重建保持。新投递的 Intent 行为不变。
- 不改依赖、不改协议、不改设置项、不改原生侧。
