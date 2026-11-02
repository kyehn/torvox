## ADDED Requirements

### Requirement: 持会话锁的应答写入以墙钟为界

任何**持会话锁**的 PTY 写入路径 MUST 使「等主端写入锁 + 排空」整段受一个**绝对墙钟
期限**约束：取锁 MUST 有界（期限内等不到即放弃本次写入），排空 MUST NOT 因部分写入
而续期。

本纪律适用于**全部**持会话锁的应答写入路径，不限于逐帧收割：OSC 52 剪贴板读取应答
同样持会话锁写 PTY，其取锁与排空 MUST 与逐帧收割同口径。

未写完的字节 MUST 退回续写队列，由后续帧补齐；期限内取不到锁时整条退回队列，且
MUST NOT 记为失败——写入只是被推迟，不是丢失。

实现：`Session::answer_clipboard_read` 经 `PtyMaster::write_with_timeout_counted` 写入
（`try_lock_for` 有界取锁 + 绝对预算排空），未写完的部分与取锁失败的整条都退回
`Session::pty_write_back_queue`，由 `Session::drain_pty_write_back` 逐帧续写。

#### Scenario: 用户输入占着主端写入锁时应答不拖住会话锁

- **WHEN** 用户输入写入正持有主端写入锁，OSC 52 读取应答在另一个线程上写 PTY
- **THEN** 应答写入 MUST 在一个帧级期限内返回，MUST NOT 等待写入锁释放到排空上限

#### Scenario: 取锁失败时应答退回队列而非丢弃

- **WHEN** 应答写入在期限内取不到主端写入锁
- **THEN** 整条应答 MUST 退回续写队列，MUST NOT 被丢弃，且 MUST NOT 记为失败

#### Scenario: 部分写出只退回未写出的部分

- **WHEN** 应答只写出前若干字节后主端持续满
- **THEN** 只有未写出的部分 MUST 退回续写队列，已写出的前缀 MUST NOT 被重发

#### Scenario: 放行后续写把应答补齐

- **WHEN** 应答已退回续写队列，主端随后恢复可写
- **THEN** 逐帧续写 MUST 把整条应答完整送达子进程，MUST NOT 截断在 base64 中间
