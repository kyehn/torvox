# Design

## 决策一：失败原因只由子进程回报，退出码不再承担判定

子进程是唯一知道 execve 是否失败的一方。旧实现让它把 errno 编码进退出码
（`100 + errno`），等待线程再反推——可退出码这一通道与 shell 的退出码惯例正面
相撞：shell 被信号杀掉时按惯例退出 `128 + 信号号`，落在同一区间内。

区间无法再收窄到无歧义：errno 的取值范围（1..=133）与 `128 + 信号号`（129..=192）
重叠区在 128..=133，而 execve 的常见失败原因里恰好有落在这一段的（ELOOP=40、
ENAMETOOLONG=36 反而安全，落在重叠区的是 28..=33 这批）。收窄区间等于按概率猜，
猜错就把真失败说成正常退出——比误报更糟。

故改为专用通道：fork 前建管道，写端带 `FD_CLOEXEC`。execve 成功时写端随 exec
关闭，父进程读到 EOF；失败时子进程写入 4 字节 errno 后退出，父进程读到确切值。
判定的依据从此唯一且无歧义。

## 决策二：管道写端必须活过 `close_stray_fds`

子进程在 exec 前会关闭除标准流外的全部 fd（避免 shell 继承父进程的无关 fd）。
管道写端必须被排除在这轮清理之外，否则 execve 失败时父进程读到的仍是 EOF，
「失败」又变成「没有原因」。故 `close_stray_fds` 增加一个保留 fd 参数。

## 决策三：退出码保留 `100 + errno` 编码，只是不再用于判定

UI 侧仍按退出码显示「[Process completed (code N)]」，该编码对 execve 失败是有信息量
的（用户能看到非 0 的异常码）。保留它可避免连带改动 Kotlin 侧的上报与展示；
判定职责移交管道后，编码与真实 errno 不一致的风险也随判定一起消失。

## 验证

- Rust 单测：失败路径管道给出 `ENOENT`、成功路径管道为空、会话退出码为
  `100 + errno`。
- 模拟器：会话收尾的 logcat 里不再出现 `E/native::terminal::session`，只剩
  debug 级的「session exit: signalling process group」与 info 级的「child exited」。
- `cargo fmt --check`、`cargo test`、`cargo clippy --all-targets -- -D warnings`、
  `cargo ndk -t arm64-v8a clippy --all-targets -- -D warnings` 全绿。
