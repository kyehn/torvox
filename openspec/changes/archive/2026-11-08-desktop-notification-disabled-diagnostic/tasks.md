# 任务

- [x] 核对平台事实：Android 13+ 通知被关闭时 `notify()` 静默丢弃，不抛异常
- [x] 系统通知投递前检查 `areNotificationsEnabled()`，被关闭时放弃投递
- [x] 用 `CountedLogThrottle` 按窗口汇总被丢弃的通知条数并记 warn 诊断
- [x] 修正投递器与 `MainActivity` 中与平台行为不符的权限注释
- [x] 新增 Robolectric 用例：允许时确实投递、关闭时不投递且有诊断
- [x] `:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessKotlinCheck`、`:app:lintDebug` 通过
- [x] 归档本 change 并同步主规约
