# Tasks

## 一、实现

- [x] 1.1 新增 `desktopNotificationId`：桌面通知的系统通知 id 由会话 id 派生，落在
      与前台服务常驻通知分区开的号段里，同一会话恒得同一 id
- [x] 1.2 桌面通知的通知 id 与 `PendingIntent` 请求码改用该 id

## 二、验证

- [x] 2.1 单元测试：桌面通知与常驻通知并存、常驻通知刷新不吃掉桌面通知、同一会话
      id 稳定、桌面通知 id 不与常驻通知 id 相撞（4 个用例）
- [x] 2.2 回归确认：去掉修复后上述用例失败（3 个失败）
- [x] 2.3 `:app:testDebugUnitTest` 全量通过（911 个用例，0 失败）
- [x] 2.4 `detekt`、`spotlessCheck` 通过
- [x] 2.5 `openspec validate --all --strict` 通过
- [x] 2.6 markdownlint 对本变更目录 0 issues
- [x] 2.7 semgrep 对改动文件无命中
