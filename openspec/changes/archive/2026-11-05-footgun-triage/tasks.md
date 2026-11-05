# 任务

## 一、扫描命中判定

- [x] 用 `footgun-scan` 扫 `android/app/src/main`（81 个文件）与
      `android/app/src/androidTest`（80 个文件），记录范围与命中数。
- [x] 逐条判定主源码集的 10 处命中，每处写明位置、陷阱文档判据与现场代码依据。
- [x] 仪器测试源码集 0 处命中，记录在案。
- [x] 判定结论写进 `triage.md`，不得以「扫描器没有报 bug」收尾。

## 二、本会话 Rust 改动复核

- [x] 用 `cargo fmt` / `cargo clippy -D warnings` / `cargo clippy -- -W clippy::pedantic`
      / `cargo test --offline` 复核本批次 Rust 改动。
- [x] 按 Must fix / Should fix / Consider / 已做得好四档给出结论。
- [x] 修复「共享夹具丢掉了持锁线程的 panic 断言」这处真实回退：
      `PtyWriteLockHold` 增加显式 `release()`（判红），`Drop` 只作兜底（不判红、不拖死
      进程），两个调用点改为 `hold.release()`。

## 三、规约

- [x] `code-health` ADDED「扫描命中必须逐条判定并留档」。

## 四、验证

- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo clippy --offline --all-targets -- --deny warnings` 通过。
- [x] `cargo test --offline session::` 通过。
- [x] `openspec validate --all --strict` 通过。
- [x] `markdownlint-cli2` 无 issue。
