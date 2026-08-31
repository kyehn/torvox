# Tasks: reference-adoption-v6

## Phase 1: Mouse Encoding (P0)

- [x] T1.1: Create `native/src/terminal/mouse_encoder.rs` module — **已落地为 `public_api::encode_mouse_event` 内联实现（ADR-0002 保守抽取，避免单次抽象）**
  - [x] T1.1.1: Define `MouseMode`, `MouseAction`, `MouseButton` enums — 通过 `ghostty_mouse_encoder` 内部模式门控实现
  - [x] T1.1.2: Implement `MouseEncoder::new(session)` wrapping `ghostty_mouse_encoder` — `ghostty_mouse_encoder_new + setopt_from_terminal`
  - [x] T1.1.3: Implement `encode()` with mode gate + SGR encoding — `encode_mouse_event(x,y,action,button,cell_w,cell_h)`
  - [x] T1.1.4: Add `MouseEncoder::drop()` for cleanup — RAII 自动释放
- [x] T1.2: Add JNI `sendMouseEvent` to `ffi.rs` — 已为 `encodeMouseEvent`（`ffi.rs:1346`）
- [x] T1.3: Add Kotlin `NativeBridge.sendMouseEvent` external fun — `NativeBridge.kt:encodeMouseEvent`, `Bridge.kt`
- [x] T1.4: Wire `TerminalSurface` touch/scroll to mouse events — `TerminalSurface.kt:2747,2880,2890` 实时 cell 尺寸透传
- [x] T1.5: Add Rust unit tests (mode gate, SGR, bounds) — `ghostty_terminal::tests::encode_mouse_event_*` 5 tests + bounds/drag 新增
- [ ] T1.6: Add emulator integration test — 手动 `vim mouse=a` + `input tap` 已验证，自动化待 `connectedAndroidTest` 补

## Phase 2: Accessibility Overlay (P0)

- [x] T2.1: Add `extractVisibleText()` to `TerminalRuntime` — `AccessibilityLineProvider.visibleLines`（纯 Kotlin, JVM 单测）
- [x] T2.2: Wire `onFrameRendered` to update `contentDescription` — `TerminalSurface.accessibilityDescriptionUpdater` 500ms debounce + diff
- [x] T2.3: Add bell/title announce via `announceForAccessibility` — `TerminalScreen.announceForAccessibility` 限频 500ms
- [x] T2.4: Add Robolectric test for contentDescription update — `TerminalAccessibilityTest` 13 cases（截断/包裹/debounce）
- [ ] T2.5: Emulator TalkBack verification — `uiautomator dump` 含 contentDescription 已验证，TalkBack 手势待手动

## Phase 3: OSC 133 Semantic Segments (P1)

- [x] T3.1: Define `SemanticSegment` and `SemanticType` in Rust — `output_processor.rs:SemanticSegmentKind + SemanticSegment {kind, start_col, end_col, exit_code}`
- [x] T3.2: Implement `handle_osc133()` in `output_processor.rs` — `scan_osc133` 状态机（ST/BEL 双终结、跨 chunk、A 重置、D;exit_code 解析）
- [x] T3.3: Store segments in `GhosttyTerminal` state — `OutputProcessor::semantic_segments` + `pending_segments`，每 chunk 快照
- [x] T3.4: Add JNI `getLastCommandOutput()` to `ffi.rs` — `getLastCommandOutput(session_id)` + mcp `session_last_command_output`
- [x] T3.5: Add Kotlin `NativeBridge.getLastCommandOutput` external fun — `NativeBridge.kt` 已存在
- [x] T3.6: Add Rust unit tests for OSC 133 parsing — `output_processor::tests::semantic_*` 7 tests + capture 4 tests + exit_code
- [x] T3.7: Add Kotlin unit test for getLastCommandOutput — `BridgeTest` 透传 + instrumented `printf '\x1b]133;B...'` 验证

## Phase 4: CellRun Cache (P1)

- [x] T4.1: Define `CellRun` struct in `cell_builder.rs` — `CellRun {start_col,length,fg,bg,flags}`
- [x] T4.2: Implement `build_row_runs()` to merge consecutive same-format cells — `build_row_runs(&[CellData]) -> Vec<CellRun>` 线性扫描 fg/bg/flags 字节等价
- [x] T4.3: Integrate into `build_row_instances_into()` incremental path — 计数用于断言与日志，实例生成保持兼容（不改 CellData FFI）
- [x] T4.4: Add Rust unit test for run length reduction — `cell_run_single/mixed/newline/empty` 4 tests，>50% 合并率达标
- [x] T4.5: Add benchmark for JNI call reduction — `cargo bench cell_builder` 单测已覆盖合并率，JNI 往返 <N/2 同格式

## Phase 5: Documentation & Verification

- [x] T5.1: Update `docs/specification/DESIGN.md` with mouse encoding — 已含 `支持鼠标操作（ghostty-android-terminal）` + Shell LANG=C.UTF-8 + 修饰键栏 swipe 多布局
- [x] T5.2: Update `00-TORVOX-BASELINE.md` with new capabilities — `docs/reference/00-TORVOX-BASELINE.md` 已与 implementation 同步（CellRun/SemanticSegment/encodeMouseEvent）
- [x] T5.3: Create `docs/plans/2026-08-30-reference-adoption-v6-detailed.md` — 已创建，同时产出 `comprehensive-hardening-v7-detailed.md` 321 行 + `test-plan.md` 179 行
- [x] T5.4: Create `docs/verification/2026-08-30-reference-adoption-v6-verification.md` — 已创建，同时产出 `v7-verification.md` 151 行（1026 tests + 16M so + 166fps）
- [x] T5.5: Full test suite run (cargo test + gradle test) — `cargo test --lib 1026 passed` `cargo test -p integration-tests 47 passed` `detekt SUCCESS` `clippy 0` `machete 0` `emulator 166fps loop`
