# Proposal

## Why

PTY 主端是 O_NONBLOCK 的：子进程不读 stdin 时写入返回 `EAGAIN`。这条路径的行为
此前只有代码注释说明，规范里没有对应要求：

- 排空期限是绝对墙钟预算还是每次进展都续期，两种语义对「逐帧回写 VT 应答」的结果
  完全不同——续期等于取消封顶，会把渲染线程按住整段排空上限。
- 期限到达后必须交回**已写出的字节数**，否则调用方无法只退回未写部分，「部分写出后
  超时」会让已发出的前缀被重发，子应用收到两条应答。
- 主端挂断（子进程退出）时必须立即失败。继续空转到期限只会烧 CPU 并拖住持锁的调用方。

同时，轮询主端可写此前是手写 `libc::pollfd` + `unsafe`，与仓库其余系统调用统一用
`nix` 的口径不一致。

## What Changes

- 规范补上「主端满时写入按期限等待且不丢字节」的要求与场景。
- 排空等待改用 `nix::poll`（`PollFd` + `PollFlags` + `PollTimeout`），去掉手写
  `pollfd` 与 `unsafe`。
- 读取线程的 `poll` 与 errno 归类一并改用 `nix`，错误判定由裸 errno 数字改为
  `nix::errno::Errno`。
- `Pty` trait 不再暴露裸 fd，改为提供主端的借用 fd 视图供 `poll` 使用。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `pty-lifecycle`：补上写入排空的期限语义、挂断判定与部分写出的记账约定。

## Impact

- `native/src/terminal/pty.rs`
- `native/src/terminal/session/spawn.rs`
- `native/src/android/ffi/input.rs`
- `native/Cargo.toml`（nix 增开 `poll` feature）
