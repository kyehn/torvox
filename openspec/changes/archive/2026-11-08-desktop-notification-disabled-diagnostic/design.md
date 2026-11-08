# 设计

## 平台事实

- Android 13（API 33）起，通知需要 `POST_NOTIFICATIONS` 运行时权限。
- 用户选择「不允许」后，本应用的通知渠道被封锁，`notify()` **静默丢弃**，不抛异常。
- 平台推荐的检查方式是 `NotificationManager.areNotificationsEnabled()`：它同时覆盖
  「权限未授予」与「用户在系统设置里关掉了本应用的通知」两种情形。

依据：Android 官方文档《Notification runtime permission》——「your app can't send
notifications unless it qualifies for an exemption」「All notification channels are
blocked」「Before your app sends a notification, confirm whether the user has enabled
notifications for your app. To do so, call areNotificationsEnabled()」。

## 取舍

- **检查放在投递前，而不是靠捕获异常**：异常不是这条路径的失败信号——被封锁时根本没有
  异常，捕获法永远看不到失败。
- **诊断放在投递器内部，而不是宿主轮询处**：投递器是唯一知道「这条通知没能送出去」的
  地方；放在轮询处只能看到「取到了一条通知」，分不清投递成功与否。
- **诊断按窗口汇总**：远端程序可以持续发通知，逐条记日志会刷屏。复用已有的
  `CountedLogThrottle`（与 `RenderErrorLedger` 同款），窗口内只出一条并带上次数。
- **不额外申请权限**：权限已在 `MainActivity` 启动时申请；此处只做投递前的判定。

## 测试口径

Robolectric 提供 `ShadowNotificationManager`：可关掉通知开关并查看已投递的通知集合。
两条用例分别钉住「开关打开时确实投递（正文含通知内容、标题或副标题含会话序号）」与
「开关关闭时不投递且留下 warn 级诊断」。判别力已验证：去掉开关检查后第二条用例判红。
