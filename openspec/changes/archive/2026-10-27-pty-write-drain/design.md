# Design

## Context

PTY 主端带 `O_NONBLOCK`。写入在子进程不读 stdin 时返回 `EAGAIN`，`Pty::write_all`
负责把它变成「等主端可写」：

- 期限来源有两种：用户输入走 `WRITE_DRAIN_TIMEOUT`（5s，等的是子进程消费），逐帧
  回写的 VT 应答走 `WRITE_BACK_TIMEOUT`（50ms，全程持会话锁）。
- `refresh_budget_on_progress` 决定每次有进展是否续期。用户输入续期（慢速消费者不被
  累计等待误伤），逐帧回写不续期（它给的是绝对墙钟预算，续期等于取消封顶）。
- 失败时必须带上已写出的字节数：`PtyMaster::write_with_timeout` 丢掉该计数（用户输入
  不容忍重发），而 VT 应答的续写队列要用它把**未写出**的部分退回队列。

等待主端可写此前是手写 `libc::pollfd` + `libc::poll`，带一处 `unsafe`；读取线程的
`poll` 同理，且错误判定用裸 errno 数字（`libc::EINTR` / `libc::EAGAIN` / `libc::EIO`）。

## Goals / Non-Goals

- Goal：写入排空语义写进规范，期限、挂断与记账三件事都有可断言的约定。
- Goal：轮询与 errno 归类统一走 `nix`，去掉手写 `pollfd` 与 `unsafe`。
- Non-Goal：不改期限取值、不改用户输入与 VT 应答的调用点。
- Non-Goal：不改 termios 配置路径——`nix` 的 `Termios` 由 `from_bits_truncate` 构造，
  会静默丢掉设备特有位（如 `IUCLC`），故 `tcgetattr`/`tcsetattr` 仍走 `libc`。

## Decisions

### 决策一：trait 暴露借用 fd 视图，而不是裸 fd

`Pty` trait 原有 `master_fd() -> RawFd`。`nix::poll::PollFd::new` 要求
`BorrowedFd<'_>`，而测试替身（`MockPty` 等）没有真实主端——它们原先返回 `-1`。

改为 `master_borrowed_fd(&self) -> Option<BorrowedFd<'_>>`，默认实现返回 `None`：
没有可轮询的真实主端时，`write_all` 退化为「等满期限即报未排空」，与真实主端被填满
时的观感一致。`PtyPair` 覆写为 `Some(self.master.as_fd())`。

裸 fd 的 `PtyPair::master_fd()` 保留为固有方法（测试用 `ioctl`/`tcgetattr` 仍需要它），
但不再是 trait 成员——替身不必再伪造一个 `-1`。

### 决策二：期限是绝对预算，续期由调用方显式选择

期限在进入 `write_all` 时一次性算定。`refresh_budget_on_progress` 为真时，每次有
进展才把期限重置为「此刻 + 排空上限」；为假时期限不动。信号打断（`EINTR`）不消耗
期限——信号不算等待。

### 决策三：挂断立即失败

`poll` 返回后必须查 `revents`：子进程退出时 `poll` 立刻返回，但置的是
`POLLHUP`/`POLLERR` 而非 `POLLOUT`。此时立即返回 `BrokenPipe`，而不是让调用方重入
`write` 拿到 `EAGAIN` 再等一轮——那会把整段排空期限变成满速空转。

### 决策四：errno 用 `nix::errno::Errno` 表达

读取循环的错误归类此前比对裸 errno 数字。改用 `Errno` 后判定由「某个整数」变为
「某个错误」，且 `Errno::from_raw` 对未知值有兜底（`UnknownErrno`），不会把未知
错误误判成重试。

## Risks / Trade-offs

- [替身不再有 `-1` 主端，`write_all` 的等待分支在替身上改为睡眠到期限] → 与原行为
  同形（原实现的 `poll(-1)` 也是等满期限后报未排空），既有「主端满时只退回未写部分」
  的用例仍覆盖该分支。
- [nix 的 `PollFd::revents()` 在内核返回未知标志位时给出 `None`] → 按「无致命标志」
  处理，与此前直接读裸 `revents` 的口径一致（未知位既非 `POLLERR` 也非 `POLLHUP`）。

## Migration Plan

无数据迁移。

## Open Questions

无。
