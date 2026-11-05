# Tasks

## 一、原生侧

- [ ] 1.1 注册上游 `on_desktop_notification` 回调：OSC 9 与 OSC 777 的标题、正文按替换字符
      口径解码，经通知通道投递；注册失败记日志
- [ ] 1.2 通知通道与队列有界：通道满时丢最旧并限频告警，内存不随程序发送速率增长
- [ ] 1.3 `Event` 增通知事件变体，原生侧增通知队列的轮询导出（与事件队列同一套 JSON 形状）

## 二、Android 侧

- [ ] 2.1 `PollEvent` 增通知变体，`Bridge` 增通知轮询与解码（解码失败按既有口径记日志并丢弃）
- [ ] 2.2 通知分派：应用在前台用 Toast、位于后台用系统通知，两者内容均含会话序号
- [ ] 2.3 系统通知带会话归属，点击后切换到目标会话；表面未就绪时等待就绪后切换
- [ ] 2.4 Activity 生命周期上报应用前后台状态（不以渲染是否暂停代替）

## 三、验证

- [ ] 3.1 Rust 用例：OSC 9 通知经上游回调产出带会话归属的通知事件
- [ ] 3.2 Kotlin 用例：前台走 Toast、后台走系统通知
- [ ] 3.3 `cargo test --offline --lib`、`:app:testDebugUnitTest`、detekt、spotlessCheck 通过
- [ ] 3.4 `openspec validate --all --strict` 通过
- [ ] 3.5 `markdownlint-cli2` 对本变更目录 0 issues
