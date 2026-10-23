# Tasks

- [x] 1.1 在模拟器上复现正常会话收尾的 E 级日志（退出码 129 被判成 errno=29）
- [x] 1.2 fork 前建 CLOEXEC 管道：execve 成功随 exec 关闭，失败时子进程写入 errno
- [x] 1.3 等待线程按管道判定 execve 失败，删掉退出码反推分支
- [x] 1.4 `close_stray_fds` 保留管道写端，否则失败时父进程读不到 errno
- [x] 1.5 补 Rust 单测：失败路径给出确切 errno、成功路径管道为空、会话退出码编码
- [x] 1.6 模拟器复测：会话收尾不再有 E 级日志
- [x] 1.7 `logcat-hygiene` 增量：execve 失败的判定依据
- [x] 1.8 `cargo fmt`/`cargo test`/`cargo clippy`/ndk clippy 全绿
