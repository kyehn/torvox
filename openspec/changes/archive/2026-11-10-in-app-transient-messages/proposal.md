# Proposal

## Why

应用内瞬时提示走平台提示窗口（`Toast`），平台侧为此打出三类 W 级告警，且提示本身还会被丢弃：

- 平台对同一 UID 同时只允许一个提示窗口，连发两条时第二条抛 `BadTokenException`，平台打出
  `Adding more than one toast window for UID at a time` 与 `ToastPresenter: BadTokenException`。
- 应用的目标 SDK 低于 30，平台提示由应用自己渲染成窗口。窗口到期摘除时平台已把该提示移出
  队列，应用随后回报令牌得到 `Toast already killed`；提示已到期后再取消它则得到
  `Toast already cancelled`。
- 平台按包限制提示频率（20 秒内 3 条、42 秒内 5 条、68 秒内 6 条）。应用不在前台时该限额
  生效，超额的那条被直接丢弃并打出 `above allowed toast quota`——用户看不到任何反馈。

连续切换字体的实测里，这三类告警与一次配额丢弃都出现过。上一轮只做到「同一时刻只留一条」，
把窗口冲突消掉了，窗口生命周期与配额两条仍在。

## What Changes

- 规约 `logcat-hygiene` 移除「同一时刻只允许一条提示窗口」，新增「应用内瞬时提示不得借用
  平台提示窗口」。
- 应用侧实现随之调整：提示改由应用内通道投递，界面层以 Snackbar 呈现；平台提示窗口不再
  承担应用内反馈。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`：应用内瞬时提示的呈现通道约束。

## Impact

- `openspec/specs/logcat-hygiene/spec.md`
- `android/app/src/main/java/terminal/emulator/TransientMessageChannel.kt`
- `android/app/src/main/java/terminal/emulator/FontManager.kt`
- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`
- `android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt`
- `android/app/src/main/java/terminal/emulator/runtime/ToastNotifier.kt`（删除）
