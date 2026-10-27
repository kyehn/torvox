# 任务

- [x] 规范补上「主端满时写入按期限等待且不丢字节」的要求与场景
- [x] `Pty` trait 改暴露主端的借用 fd 视图，替身不再伪造裸 fd
- [x] 排空等待改用 `nix::poll`，去掉手写 `pollfd` 与 `unsafe`
- [x] 读取线程的 `poll` 与 errno 归类改用 `nix`
- [x] `nix` 增开 `poll` feature
- [x] `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo ndk -t arm64-v8a clippy` 通过
- [x] `cargo test` 全绿（737 + 2）
- [x] `openspec validate --all --strict` 通过
