# pty-lifecycle 增量

## ADDED Requirements

### Requirement: 拆卸可重入、读循环可中断且组杀不误伤本进程

拆卸路径 MUST 可重入：重复请求退出 MUST NOT 发出第二组信号、MUST NOT panic，
第二次调用与第一次的结果一致。

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

执行失败 MUST 可诊断：errno MUST 编码进退出码（`100 + errno`，截到 155），且 MUST 把
可读原因写进从端，MUST NOT 静默退出使调用方只看到「空会话」。

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

#### Scenario: 软件流控关闭

- **WHEN** 读取会话的 termios
- **THEN** `IUTF8` 置位，`IXON` 与 `IXOFF` 均为零
