# 任务

## 1. 解析纯函数与错误分类

- [x] 1.1 在 `native/src/android/ffi.rs` 加 `SearchHighlightError` 枚举（`TooShort` /
      `NegativeCount` / `CountExceedsRecords` / `TruncatedTail`），每个变体携带定位所需的
      实测值（如 `TooShort { length: usize }`、`NegativeCount { count: i32 }`、
      `CountExceedsRecords { declared: usize, available: usize }`、
      `TruncatedTail { remaining: usize }`）。
      验证：`cargo clippy -- -D warnings` 无告警（枚举须 `Debug`）。
      实施补记：另加 `Display` impl——`Debug` 不算字段的真实消费者，clippy 会报
      4 处 dead_code；`Display` 同时让 logcat 诊断按类别可读。
- [x] 1.2 抽纯函数 `parse_search_highlights(bytes: &[u8]) -> Result<Vec<SearchHighlight>, SearchHighlightError>`，
      内含全部判定：前缀不足、负计数、计数超出记录数、尾部残缺。
      验证：`cargo build -p native` 通过。
- [x] 1.3 保留 count 封顶的防御性上限？不保留——改为返回 `CountExceedsRecords`：
      现状 `count.min(可用记录数)` 静默截断正是要消除的行为。
      验证：1.4 的「计数超出记录数」用例断言该具体变体。
- [x] 1.4 `setSearchHighlights` 的 JNI 闭包改为调用纯函数；对 `Err` 逐变体
      `log::error!` 出声并**不更新** `search_highlights`（保持上一帧，不清空）。
      验证：`cargo clippy -- -D warnings` 通过。
      实施补记：JVM 侧 `convert_byte_array` 失败原先静默 `return Ok(())`，一并补出声。

## 2. 单测

- [x] 2.1 四类畸形各一条用例，逐类断言**具体错误变体**与其中的实测值
      （如 `TooShort { length: 3 }`），不得只断言「是 Err」或「不 panic」。
      验证：`cargo test --lib search_highlight` 全绿。
- [x] 2.2 合法载荷逐字段断言：2 条记录的 `row`/`start_col`/`end_col_exclusive`/`color`
      与字节流相等（含负数 row 与 alpha 边界 127/128 两端）。
      验证：`cargo test --lib search_highlight` 全绿。
- [x] 2.3 零计数 4 字节载荷解析成功为空列表（清除高亮是合法操作，不是畸形）。
      验证：`cargo test --lib search_highlight` 全绿。
- [x] 2.4 **判别力验证**：把 `apply_search_highlight` 的阈值临时改坏（或把某条判定改成
      恒真/恒假），确认至少一条用例变红；随后改回。
      验证：红→绿两次运行结果：
      绿 9/9（正确实现）；红 4 passed / 5 failed（把解析改回加固前的静默容错——
      负计数经 `max(0)` 抹平、声明数用 `.take()` 静默截断），失败的恰是五类畸形用例
      （前缀不足、负计数、计数超出、尾部残缺、判定顺序）；恢复后 9/9 复绿。

## 3. 注释订正与规范

- [x] 3.1 订正 `TerminalScreen.kt:673-676` 注释：当前项靠不透明度（255）区分，
      其余命中项 alpha=160 同样 ≥128 故同样反色，仅降低不透明度；删去「不反色」。
      验证：`grep -n "不反色" TerminalScreen.kt` 无命中（全仓主源码亦无该误述）。
- [x] 3.2 `SearchHighlightColors.kt` 的 `OTHER_MATCH_ALPHA` 注释同步说明 160 与 128 阈值
      的关系（现注释已说「同样反色」，补上「因 160 ≥ 阈值 128」这一依据）。
      验证：人工复核与 Rust `SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD` 一致（160 ≥ 128）。
- [x] 3.3 `openspec validate harden-search-highlight-wire-format --strict` 通过，
      确认 spec delta 的 ADDED Requirement 标题与主 spec 不冲突。
      验证：`Change 'harden-search-highlight-wire-format' is valid`，markdownlint 0 issues。
      实施补记：原 spec 把「尾部残缺」写成「声明数够但记录凑不齐」，该写法下场景不可达
      （声明数超限会先命中「计数超出」）。已改为「负载长度不是记录长的整数倍」并明确
      判定顺序，另加 `count_exceeds_takes_precedence_over_truncated_tail` 用例锁死顺序。

## 4. 全量验证

- [x] 4.1 `cargo test --lib` 全量通过（nix develop 内，预期 533+ 例 0 失败）。
      验证：542 passed / 0 failed（较 533 基线 +9 即本次新增用例）。
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
      验证：命令无告警输出。
- [x] 4.3 `./gradlew :app:testDebugUnitTest` 全量通过（本变更不改 Kotlin 行为，
      仅注释；仍需确认无编译回归）。
      验证：`BUILD SUCCESSFUL`，531 例 / 0 failures / 0 errors。
- [x] 4.4 `openspec archive harden-search-highlight-wire-format` 合入主 spec 后，
      `openspec validate --specs` 全绿。
      验证：归档为 `2026-10-11-harden-search-highlight-wire-format`（+1 added）；
      主 spec `text-search` 出现第 3 个 Requirement「搜索高亮线格式的畸形输入必须被拒绝并出声」；
      `openspec validate --specs` 23 passed / 0 failed。
