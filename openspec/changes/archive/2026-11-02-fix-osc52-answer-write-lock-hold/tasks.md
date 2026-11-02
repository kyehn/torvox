# 任务

## 一、缺陷核定

- [x] 确认 `Session::answer_clipboard_read` 走 `PtyMaster::write_with_timeout`：
      `native/src/terminal/session/output.rs` 的写入调用取的是 `write_with_timeout`，
      而该入口内 `pty_write_order()` 是无期限的 `lock()`。
- [x] 确认 `write_with_timeout` 的排空期限按进度续期：
      `native/src/terminal/pty.rs` 的 `write_all` 第三参为 `refresh_budget_on_progress`，
      `write_with_timeout` 传 `true`——每有部分写入即重置期限，持锁时长无上界。
- [x] 确认该路径持会话锁：`native/src/android/ffi/render/clipboard.rs` 的两个调用点
      都在 `session.lock()` 之内（洪泛削减分支在渲染线程上，正常应答分支在应答线程上）。
- [x] 确认 `pty::WRITE_BACK_TIMEOUT` 的文档已把 OSC 52 应答算作持会话锁的帧级路径，
      但该常量此前只由逐帧收割（`drain_pty_write_back`）真正遵守。
- [x] 确认同类缺陷在逐帧收割那条路上已修：
      `native/src/terminal/session/tests.rs` 的
      `write_back_yields_the_frame_when_a_user_write_holds_the_master_lock` 即其回归钉。

## 二、实现

- [x] `terminal/session/output.rs`：`Session::answer_clipboard_read` 的写入入口改为
      `PtyMaster::write_with_timeout_counted`（有界取锁 + 绝对预算排空）。
- [x] `terminal/session/output.rs`：部分写出的场合只把未写出的部分退回
      `pty_write_back_queue`；取锁失败的场合把整条退回队列并记调试日志。
- [x] `terminal/session/output.rs`：补齐方法文档，写明该路径与逐帧收割同口径，
      以及「应答不丢、写入只是被推迟」的语义。

## 三、回归钉

- [x] `terminal/session/tests.rs`：
      `clipboard_answer_yields_when_a_user_write_holds_the_master_lock`——
      用 gate 确定性地把用户输入停在主端写入临界区内（不靠 sleep，无竞态），
      断言应答写入在帧级期限内返回、应答整条退回队列、放行后逐帧续写把完整应答送达。
- [x] 判别力核验：临时还原旧写法后该用例判红（`会话锁不得在用户输入持写入锁时被
      住满排空上限: Timeout`），恢复修复后转绿。

## 四、门禁

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- --deny warnings`
- [x] `cargo test --package native`（788 passed; 0 failed）
- [x] `openspec validate --all --strict`
- [x] `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings`
- [x] `cargo machete --with-metadata`
- [x] `semgrep scan --error --dataflow-traces --time --config .semgrep/rules.yml`
- [x] `markdownlint-cli2 "**/*.md"`
