# Proposal

## Why

后台桌面通知走 `NotificationManager.notify()`。Android 13+ 起，用户关闭本应用的通知后
`notify()` **静默丢弃**——不抛任何异常。平台文档明确：用户选择「不允许」时，所有通知
渠道被封锁，应用「不能发送通知」。

本仓的投递器把 `notify()` 包在 try/catch 里，catch 的注释写着「POST_NOTIFICATIONS 未授权
时 notify 抛 SecurityException」。这个前提是错的：权限未授予时不会有异常，通知直接消失。
于是用户看到的是「程序明明请求了通知，通知栏里什么都没有」，日志里也没有任何线索——
既不知道是被权限挡掉的，也无从判断是程序没发还是宿主丢了。

同一前提也写进了 `MainActivity` 的权限申请注释。

## What Changes

- 系统通知投递前先问 `areNotificationsEnabled()`：通知被用户关闭时记一条诊断并放弃
  投递，不让失败静默消失。
- 诊断按窗口汇总（复用 `CountedLogThrottle`）：程序持续发通知时逐条记会刷屏。
- 修正两处与平台行为不符的注释（投递器与 `MainActivity`）。
- `desktop-notification` 规约补「通知被用户关闭」场景。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `desktop-notification`：给「通知的分派形态由应用前后台状态决定」补场景。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPlayer.kt`：投递前
  检查通知开关，被关闭时记诊断。
- `android/app/src/main/java/terminal/emulator/MainActivity.kt`：修正权限注释。
- `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationSystemPostTest.kt`：
  新增 Robolectric 用例，钉住「允许时投递、关闭时不投递且有诊断」。
- `openspec/specs/desktop-notification/spec.md`：归档时同步场景。
