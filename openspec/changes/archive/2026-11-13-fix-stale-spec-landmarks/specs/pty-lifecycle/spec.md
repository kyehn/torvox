# pty-lifecycle Specification

## MODIFIED Requirements

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
      （`native/src/terminal/session/mod.rs` 的 `request_exit` 以 `exited` 标志短路，
      并把输出通道换新使后续取用仍安全）

#### Scenario: 子进程已回收后不再发信号

- **WHEN** 等待线程已回收子进程，随后才请求退出
- **THEN** 拆卸直接返回，不调用 `kill(2)`，避免命中复用后的 PID

#### Scenario: 从端关闭后读循环停止

- **WHEN** PTY 从端关闭使 `read(2)` 返回 `EIO`
- **THEN** 读取线程停止并退出循环，而不是持续重试
      （判定纯函数为 `native/src/terminal/session/spawn.rs` 的 `read_error_action`，用例
      `read_error_action_classifies_errno`）

#### Scenario: 拆卸不被阻塞的写路径拖住

- **WHEN** 子进程不读 stdin 而持续产生输出、输出通道已满
- **THEN** 请求退出仍先断开通道，读线程与等待线程在宽限期内 join

#### Scenario: 子进程尚未 setsid 时不组杀

- **WHEN** 子进程在 `setsid` 前仍与本进程同组
- **THEN** 本次收尾退化为直杀子进程，MUST NOT 以该组为信号目标
      （目标选择纯函数为 `native/src/terminal/session/lifecycle.rs` 的
      `exit_signal_target`，用例 `exit_signal_target_declines_group_kill_when_unsafe`：
      同组、组号 ≤ 1、本进程组号取不到三种退化都直杀子进程）
