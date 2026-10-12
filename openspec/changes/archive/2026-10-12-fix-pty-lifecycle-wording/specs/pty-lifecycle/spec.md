## MODIFIED Requirements

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
