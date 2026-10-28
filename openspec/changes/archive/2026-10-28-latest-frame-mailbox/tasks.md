# 任务

- [x] 建 change（proposal/design/tasks/specs）
- [x] 回归钉：`newest_frame_reaches_consumer_after_a_burst_of_pushes`（改动前失败：取到首帧 `line00`）
- [x] 单元帧交付改为单槽最新帧邮箱（`ghostty_terminal/frame_mailbox.rs`）
- [x] 移除帧丢弃告警、限频闸门及其用例
- [x] 同步注释口径（渲染侧暂停期取帧、命令字段文档、帧推送路径）
- [x] 原生侧门禁：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、`cargo test --all-targets --no-fail-fast`
- [x] `openspec validate --all --strict` 通过
