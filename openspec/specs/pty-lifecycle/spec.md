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

### Requirement: VT 应答回写逐帧有界

VT 引擎的应答缓冲（DECRPM/DSR/DA 等 `on_pty_write` 回调产物）无界，而每次回写是同步
写且可等到帧级排空期限，全程持会话锁。回写 MUST 逐帧有界：写完上限条即止，其余留队
下帧续写；首次写失败 MUST 停止本帧的剩余回写（MUST NOT 按同一失败条件逐条重试），
失败那条与未写部分 MUST 留待下帧。待回写队列 MUST 显式封顶，超出即丢最旧并记错误
（应答是请求-响应语义，不被消费的应答不会被回头读）。

#### Scenario: 病态输出下单帧回写条数有上界

- **WHEN** 一个输出块内含远超单帧上限条数的 DSR 查询
- **THEN** 本帧写出的条数恰为上限，其余留在队列中下帧续写，既不丢也不积压在本帧

#### Scenario: 首次写失败即停

- **WHEN** 回写过程中某一条因主端满而失败
- **THEN** 本帧不再尝试其余应答，失败那条回到队首待下帧重试

### Requirement: 拆卸可重入、读循环可中断且组杀不误伤本进程

拆卸路径 MUST 可重入：重复请求退出 MUST NOT 发出第二组信号、MUST NOT panic，
第二次调用与第一次的结果一致。

等待线程已回收子进程并置位退出标志后，拆卸 MUST NOT 再按该 PID 发信号；
`getpgid(child)` 返回 `ESRCH` 时同样 MUST 直接返回，避免 PID 被系统复用后误伤无关进程。

读取循环 MUST 在 PTY 消失或错误致命时自行停止，MUST NOT 对从端关闭导致的 `EIO` 无限重试；
瞬时错误重试的判定 MUST 由纯函数表达并有其单测。

拆卸 MUST 不因读循环阻塞：请求退出时先断开输出通道使阻塞的发送端立刻返回，
读线程与等待线程的 join MUST 有宽限期限，超时即分离线程而 MUST NOT 无限等待。

组杀 MUST 先确认目标组不是本进程自身的组，且组号下界 MUST 大于 1；
确认不了（本进程组号取不到）时 MUST 退化为直杀子进程。

#### Scenario: 重复拆卸

- **WHEN** 对同一会话连续两次请求退出
- **THEN** 只发一组挂断与强杀信号，第二次调用不 panic 且结果与第一次一致
      （`native/src/terminal/session.rs` 的 `request_exit` 以 `exited` 标志短路，
      并把输出通道换新使后续取用仍安全）

#### Scenario: 子进程已回收后不再发信号

- **WHEN** 等待线程已回收子进程，随后才请求退出
- **THEN** 拆卸直接返回，不调用 `kill(2)`，避免命中复用后的 PID

#### Scenario: 从端关闭后读循环停止

- **WHEN** PTY 从端关闭使 `read(2)` 返回 `EIO`
- **THEN** 读取线程停止并退出循环，而不是持续重试
      （判定纯函数为 `session.rs` 的 `read_error_action`，用例
      `read_error_action_classifies_errno`）

#### Scenario: 拆卸不被阻塞的写路径拖住

- **WHEN** 子进程不读 stdin 而持续产生输出、输出通道已满
- **THEN** 请求退出仍先断开通道，读线程与等待线程在宽限期内 join

#### Scenario: 子进程尚未 setsid 时不组杀

- **WHEN** 子进程在 `setsid` 前仍与本进程同组
- **THEN** 本次收尾退化为直杀子进程，MUST NOT 以该组为信号目标
      （目标选择纯函数为 `session.rs` 的 `exit_signal_target`，用例
      `exit_signal_target_declines_group_kill_when_unsafe`：同组、组号 ≤ 1、
      本进程组号取不到三种退化都直杀子进程）

### Requirement: 网格 resize 被丢弃时不得把旧尺寸当权威

resize 的结果 MUST 区分两种情形：PTY 与网格都已缩放，以及 PTY winsize 已变、
而发往 VT 线程的网格命令被丢弃（通道满或 VT 线程卡住）。被丢弃时本会话缓存的
网格尺寸仍是旧值，调用方 MUST NOT 把它当作已生效的权威值发布
（`native/src/terminal/session.rs` 的 `ResizeOutcome`）。

被丢弃 MUST 置脏，使下一次 resize 即使行列与上次相同也重试修复——这是
「尺寸无变化的 resize 跳过」的唯一例外，PTY 与网格的不一致 MUST NOT 静默留存。

#### Scenario: 网格命令被丢弃后可重试

- **WHEN** 通道满导致某次 resize 的网格命令被丢弃
- **THEN** PTY winsize 已变而网格尺寸仍为旧值，且会话已被置脏

#### Scenario: 同尺寸重试不被跳过

- **WHEN** 上一次 resize 被丢弃后，再次以相同行列请求 resize
- **THEN** 命令被重发而 MUST NOT 按「尺寸无变化」跳过

### Requirement: 子进程侧的 fork/exec 不变量

子进程路径 MUST 只使用 `fork()` 之前已分配的数据，MUST NOT 在 fork 之后分配堆内存或取
可能加锁的资源。

子进程 MUST 依次：关闭多余 fd、`setsid()` 并经 `TIOCSCTTY` 把从端设为控制终端、
把从端 `dup2` 到 0／1／2、以显式 `argv` 与 `envp` 调用 `execve`。主端 fd MUST 带
`FD_CLOEXEC`，MUST NOT 泄漏给子进程。

执行失败 MUST 可诊断：errno MUST 编码进退出码（`100 + errno`，errno 范围 1..=133），且 MUST 把
可读原因写进从端，MUST NOT 静默退出使调用方只看到「空会话」。

等待线程 MUST 只把 `100 + errno`（errno 1..=133）的退出码解释为 `execve` 失败；
其他退出码（例如 shell 的 `exit 254`）MUST NOT 记为执行失败。

termios MUST 置 `IUTF8`，且 MUST 关闭软件流控（`IXON`／`IXOFF` 清零）。

#### Scenario: 从端成为控制终端

- **WHEN** 会话生成成功
- **THEN** 子进程的 0／1／2 均指向从端，且该从端是其控制终端（`setsid` 后 `TIOCSCTTY`）

#### Scenario: 主端不泄漏给子进程

- **WHEN** 子进程完成 exec
- **THEN** 主端 fd 已按 `FD_CLOEXEC` 关闭，子进程内不可见

#### Scenario: exec 失败可诊断

- **WHEN** 目标二进制不存在或不可执行
- **THEN** 退出码含 errno 编码（由等待线程解码并在 logcat 可观测），
      且从端出现可读的错误行 `execve failed: errno=N`

#### Scenario: 普通非零退出不误报 exec 失败

- **WHEN** shell 正常执行并返回 `exit 254`
- **THEN** 等待线程按普通退出码处理，MUST NOT 输出 `possible execve errno` 错误日志

#### Scenario: 软件流控关闭

- **WHEN** 读取会话的 termios
- **THEN** `IUTF8` 置位，`IXON` 与 `IXOFF` 均为零

### Requirement: 主端满时写入按期限等待且不丢字节

PTY 主端带 `O_NONBLOCK`：子进程不读 stdin 时写入返回 `EAGAIN`。写入方 MUST NOT
因此丢弃剩余字节——丢弃会让粘贴被静默截断，也会把 VT 应答截断在转义序列中间，使
子应用永久等待一个永远不完整的回答。

写入方 MUST 等到主端重新可写，且等待 MUST 受一个**绝对墙钟期限**约束。期限 MUST
按调用场景给定：用户输入可等满排空上限（等的是子进程消费），逐帧回写的 VT 应答走
帧级短期限（全程持会话锁，等满会把渲染与看门狗一起按住）。

期限 MUST 只在**已有进展**时续期，且续期 MUST 由调用方显式选择：逐帧回写给的是
绝对预算，续期等于取消封顶。

主端挂断（子进程退出）时 MUST 立即报错，MUST NOT 继续空转到期限——否则整段期限
变成满速空转，既烧 CPU 又拖住持锁的调用方。

被信号打断 MUST NOT 记为失败，且 MUST NOT 消耗期限。

写入失败时 MUST 一并给出**已写出的字节数**：调用方据此只把未写出的部分退回队列，
否则「部分写出后超时」会让已发出的前缀被重发，子应用收到两条应答。

#### Scenario: 主端满时等满期限后如实报未写出的长度

- **WHEN** 主端缓冲已满，写入只写出前若干字节后持续 `EAGAIN`
- **THEN** 写入在期限到达时返回错误，并带上已写出的字节数；调用方只把剩余部分退回队列

#### Scenario: 主端挂断立即失败

- **WHEN** 子进程退出使主端挂断
- **THEN** 写入立即返回错误，MUST NOT 等到期限

#### Scenario: 信号打断不算失败

- **WHEN** 等待主端可写期间收到信号
- **THEN** 写入继续等待同一期限，MUST NOT 把本次打断报成失败

#### Scenario: 逐帧回写不续期

- **WHEN** 逐帧回写 VT 应答时主端持续满
- **THEN** 期限不被续期，本帧至多等待一个帧级期限即让出
