# Tasks

## 一、原生侧

- [x] 1.1 `poll_notification_event_inner` 改为先取已入队的通知，取不到才推进会话收割
- [x] 1.2 取用与序列化的重复代码收进一个辅助函数（两条返回路径共用）

## 二、Android 侧

- [x] 2.1 抽出可单测的通知排空函数（取事件、解码、按会话序号投递），每轮排空有上限
- [x] 2.2 `TerminalRuntime` 的通知轮询改用该函数，语义与上限保持一致

## 三、验证

- [x] 3.1 Kotlin 用例：排空达到上限即停止、队列为空即返回、解码失败只丢该条
- [x] 3.2 `cargo test --offline --lib`、`:app:testDebugUnitTest`、detekt、spotlessCheck 通过
- [x] 3.3 `openspec validate --all --strict` 通过
- [x] 3.4 `markdownlint-cli2` 对本变更目录 0 issues
