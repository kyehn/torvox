# pty-lifecycle Specification

## Purpose

PTY 生成与收尾的生命周期行为：生成播种行列尺寸、resize 携带像素尺寸、无变化时跳过 resize、前台进程组优先收尾、IUTF8 语义与上游输入路径一致。本能力覆盖 Rust 侧 PTY 拥有者与 Kotlin 侧尺寸来源之间的可观测契约。

## Requirements

### Requirement: 生成播种行列尺寸且 resize 携带像素尺寸

生成时的内核 winsize MUST 按调用方给的 rows/cols 播种，使 shell 在首次 resize 前
不看到 0x0；像素字段在首次 `setPixelSize` 之前为 0，MUST NOT 在生成处按单元格度量
推算——生成调用的入口只给行列数。

resize 携带的 winsize MUST 含像素字段（列数乘单元格宽、行数乘单元格高），尺寸无变化的
resize MUST 跳过，不向 PTY 发送信号，避免无辜 `SIGWINCH` 导致行被擦除。

#### Scenario: 生成播种行列尺寸

- **WHEN** 以 24 行 80 列生成会话
- **THEN** 内核 winsize 的行列为 24 与 80，像素字段为 0

#### Scenario: resize 携带像素字段

- **WHEN** 在已给定像素尺寸的会话上请求 resize
- **THEN** 透传的 `ws_xpixel/ypixel` 等于列数乘单元格宽与行数乘单元格高

#### Scenario: 无变化 resize 不发信号

- **WHEN** 请求的列行与上次生效值相同
- **THEN** 本次 resize 被跳过，终端内容不被擦除

### Requirement: 收尾先终止前台进程组

会话关闭 MUST 先向 shell 所在进程组（`getpgid(child)` 取到的组）发送挂断信号，
再回退关闭 shell 进程；挂断后 MUST 以有界宽限等待退出，超时再升级为强制终止，
避免前台子进程残留。子进程经 `setsid` 自成会话首进程，不存在独立的
`subtree_leader` 概念，规范 MUST NOT 再使用该词。

#### Scenario: 前台进程组先被挂断

- **WHEN** 关闭存在前台子进程的会话
- **THEN** shell 所在进程组先收到挂断信号（`kill(-pgid, SIGHUP)` 并补 `SIGCONT`），
  shell 进程随后被关闭；子进程与本进程同组时退化为直杀子进程

#### Scenario: 有界轮询后升级

- **WHEN** 挂断信号发出后进程未在 50ms（`TRAILING_EXIT_GRACE`）内退出
- **THEN** 会话关闭流程升级为强制终止（`kill(-pgid, SIGKILL)`，失败再直杀子进程）
  并继续回收资源

### Requirement: IUTF8 语义与宽字符退格一致

PTY 从端 MUST 置 `IUTF8` 标志，使内核按 UTF-8 处理擦除；CJK 字符退格 MUST 一次擦除一个完整汉字，与码点 1:1 语义一致。

#### Scenario: CJK 一次退一字

- **WHEN** 终端处于 UTF-8 模式且删除一个汉字
- **THEN** 一次退格擦除该汉字的全部字节，不残留半个字符
