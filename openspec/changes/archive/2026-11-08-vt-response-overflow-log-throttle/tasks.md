# 任务

- [x] 定位逐帧刷屏的告警：`drain_pty_write_back` 的队列溢出分支
- [x] 队列裁剪与计数记入收敛到一处，返回窗口内累计条数
- [x] 会话新增溢出告警的限频闸门字段并初始化
- [x] 告警改按窗口输出，分级维持 error
- [x] 补单测：首条必可见且带条数
- [x] 补单测：窗口内再次溢出不再输出
- [x] 补单测：窗口内条数攒着下次交出
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
