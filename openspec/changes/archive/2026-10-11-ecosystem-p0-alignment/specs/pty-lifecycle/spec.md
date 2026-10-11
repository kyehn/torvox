# Spec Delta

## Purpose

PTY 生成与收尾的生命周期行为：spawn 携带像素尺寸、无变化时跳过 resize、前台进程组优先收尾、IUTF8 语义与上游输入路径一致。本能力覆盖 Rust 侧 PTY 拥有者与 Kotlin 侧尺寸来源之间的可观测契约。

## ADDED Requirements

### Requirement: 生成携带像素尺寸且无变化跳过

spawn 与 resize 携带的 winsize MUST 含像素字段（列数乘单元格宽、行数乘单元格高），尺寸无变化的 resize MUST 跳过，不向 PTY 发送信号，避免无辜 `SIGWINCH` 导致行被擦除。

#### Scenario: 生成携带像素字段

- **WHEN** 以 80 列与实测单元格度量生成会话
- **THEN** 透传的 `ws_xpixel/ypixel` 等于列数乘单元格宽与行数乘单元格高

#### Scenario: 无变化 resize 不发信号

- **WHEN** 请求的列行与上次生效值相同
- **THEN** 本次 resize 被跳过，终端内容不被擦除

### Requirement: 收尾先终止前台进程组

会话关闭 MUST 先向 subtree_leader 发送挂断信号，再回退关闭 shell 进程；挂断后 MUST 以有界轮询等待退出，超时再升级为强制终止，避免前台子进程残留。

#### Scenario: 前台进程组先被挂断

- **WHEN** 关闭存在前台子进程的会话
- **THEN** 前台进程组先收到挂断信号，shell 进程随后被关闭

#### Scenario: 有界轮询后升级

- **WHEN** 挂断信号发出后进程未在 200ms 内退出
- **THEN** 会话关闭流程升级为强制终止并继续回收资源

### Requirement: IUTF8 语义与宽字符退格一致

PTY 从端 MUST 置 `IUTF8` 标志，使内核按 UTF-8 处理擦除；CJK 字符退格 MUST 一次擦除一个完整汉字，与码点 1:1 语义一致。

#### Scenario: CJK 一次退一字

- **WHEN** 终端处于 UTF-8 模式且删除一个汉字
- **THEN** 一次退格擦除该汉字的全部字节，不残留半个字符
