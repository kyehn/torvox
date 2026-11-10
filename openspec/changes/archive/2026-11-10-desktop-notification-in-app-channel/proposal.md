# Proposal

## Why

前台桌面通知（OSC 9 / OSC 777）仍借用平台提示窗口（`Toast`），与应用内的其余提示形成两套机制，
且平台侧为此打出 W 级告警：

- 应用的目标 SDK 是 28，低于平台把文本提示改为「系统渲染」的门槛（R），故 `Toast` 在应用进程里
  被渲染成真正的窗口。窗口到期摘除时平台已把该提示移出队列，应用随后回报令牌得到
  `Toast already killed`；提示已到期后再取消它则得到 `Toast already cancelled`。两条都是窗口生命
  周期的必然结果：每一条到期的提示都会留下平台侧条目。
- 同一 UID 同时只允许一个提示窗口：连续两条桌面通知时第二条抛 `BadTokenException`，平台打出
  `Adding more than one toast window for UID at a time`。
- 平台按包限额提示频率：应用退到后台再切回前台时，超额的那条提示被直接丢弃并打出
  `above allowed toast quota`——用户看不到任何反馈。

上一轮已把应用内的其余瞬时提示改走应用内通道，树内只剩桌面通知这一处平台提示窗口。

## What Changes

- 规约 `desktop-notification` 的「通知的分派形态由应用前后台状态决定」：前台的呈现形态由
  「以 Toast 呈现」改为「以应用内提示通道呈现」，后台仍以系统通知呈现。
- 规约 `logcat-hygiene` 的「应用内瞬时提示不得借用平台提示窗口」：把前台桌面通知纳入同一条
  通道与同一条约束。
- 实现随之调整：`DesktopNotificationPlayer` 不再构造平台提示窗口，前台路径经
  `TransientMessageChannel` 投递；通道改为依赖注入提供的进程内单例，使桌面通知与界面提示共用
  同一条通道。

## Capabilities

### Modified Capabilities

- `desktop-notification`：前台桌面通知的呈现形态。
- `logcat-hygiene`：应用内瞬时提示的呈现通道约束。

## Impact

- `openspec/specs/desktop-notification/spec.md`
- `openspec/specs/logcat-hygiene/spec.md`
- `android/app/src/main/java/terminal/emulator/TransientMessageChannel.kt`
- `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPlayer.kt`
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`
- `android/app/src/main/java/terminal/emulator/MainActivity.kt`
- `docs/specification/DESIGN.md`：其中「应用位于前台时使用 Toast Notification」的措辞随之过时，
  该文件受保护，须经用户同意后方可修改。
