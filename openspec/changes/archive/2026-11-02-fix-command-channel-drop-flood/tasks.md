# 任务

## 一、缺陷核定

- [x] 解析 `logcat_2026-11-02_17-33-06.txt`：应用 pid 5660，tag
      `native::terminal::ghostty_terminal::public_api` 共 22778 条，占应用侧条目的
      96.7%，集中在 `17:31:25`–`17:31:27`（9635 + 12656 + 487 条）。
- [x] 确认该 tag 的全部条目为同一文案
      `ghostty_terminal: cmd_tx full/dropped failed: sending on a full channel`，
      来自 `public_api.rs` 中 `try_send` 失败分支的 `log::warn!`。
- [x] 确认命令入队失败共有 12 处调用点（`vt_write`、`pty_write`、`flush`、
      `set_theme`、`set_cursor_style`、`scroll_viewport`、`resize`、
      `set_cell_pixel_size`、`reset`、`request_repaint`、`set_selection`、
      `clear_selection`），各写各的 `log::warn!`，无一经过限频闸门。
- [x] 确认仓库已有 `native/src/slow_log.rs` 的 `CountedLogThrottle`，渲染侧慢帧诊断、
      剪贴板队列溢出告警、事件队列溢出告警均已接入，唯独命令入队失败未接入。

## 二、实现

- [x] `native/src/slow_log.rs`：`CountedLogThrottle` 的计数改为 `AtomicU32`，方法改收
      `&self`，使同一把闸门可被渲染线程与输入线程共享。
- [x] `native/src/terminal/ghostty_terminal/mod.rs`：`GhosttyTerminal` 增加
      `cmd_drop_log` 闸门。
- [x] `native/src/terminal/ghostty_terminal/public_api.rs`：12 处命令入队失败告警
      统一走 `warn_command_dropped`，按 500ms 窗口放行并带上窗口内累计丢弃次数。

## 三、回归钉

- [x] `native/src/slow_log.rs`：`counted_throttle_keeps_the_total_across_threads`——
      多线程共享同一把闸门时，已交出与待交出的计数之和 MUST 等于事件总数。
- [x] `native/src/terminal/ghostty_terminal/input_output_tests.rs`：
      `full_command_channel_counts_every_dropped_command`——命令通道满时每次丢弃
      MUST 计入闸门，窗口内累计次数不得丢失。

## 四、门禁

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- --deny warnings`
- [x] `cargo test --package native`（790 passed; 0 failed）
- [x] `openspec validate --all --strict`
