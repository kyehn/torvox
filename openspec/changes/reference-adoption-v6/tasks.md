# Tasks: reference-adoption-v6

## Phase 1: Mouse Encoding (P0)

- [ ] T1.1: Create `native/src/terminal/mouse_encoder.rs` module
  - [ ] T1.1.1: Define `MouseMode`, `MouseAction`, `MouseButton` enums
  - [ ] T1.1.2: Implement `MouseEncoder::new(session)` wrapping `ghostty_mouse_encoder`
  - [ ] T1.1.3: Implement `encode()` with mode gate + SGR encoding
  - [ ] T1.1.4: Add `MouseEncoder::drop()` for cleanup
- [ ] T1.2: Add JNI `sendMouseEvent` to `ffi.rs`
- [ ] T1.3: Add Kotlin `NativeBridge.sendMouseEvent` external fun
- [ ] T1.4: Wire `TerminalSurface` touch/scroll to mouse events
- [ ] T1.5: Add Rust unit tests (mode gate, SGR, bounds)
- [ ] T1.6: Add emulator integration test

## Phase 2: Accessibility Overlay (P0)

- [ ] T2.1: Add `extractVisibleText()` to `TerminalRuntime`
- [ ] T2.2: Wire `onFrameRendered` to update `contentDescription`
- [ ] T2.3: Add bell/title announce via `announceForAccessibility`
- [ ] T2.4: Add Robolectric test for contentDescription update
- [ ] T2.5: Emulator TalkBack verification

## Phase 3: OSC 133 Semantic Segments (P1)

- [ ] T3.1: Define `SemanticSegment` and `SemanticType` in Rust
- [ ] T3.2: Implement `handle_osc133()` in `output_processor.rs`
- [ ] T3.3: Store segments in `GhosttyTerminal` state
- [ ] T3.4: Add JNI `getLastCommandOutput()` to `ffi.rs`
- [ ] T3.5: Add Kotlin `NativeBridge.getLastCommandOutput` external fun
- [ ] T3.6: Add Rust unit tests for OSC 133 parsing
- [ ] T3.7: Add Kotlin unit test for getLastCommandOutput

## Phase 4: CellRun Cache (P1)

- [ ] T4.1: Define `CellRun` struct in `cell_builder.rs`
- [ ] T4.2: Implement `build_row_runs()` to merge consecutive same-format cells
- [ ] T4.3: Integrate into `build_row_instances_into()` incremental path
- [ ] T4.4: Add Rust unit test for run length reduction
- [ ] T4.5: Add benchmark for JNI call reduction

## Phase 5: Documentation & Verification

- [ ] T5.1: Update `docs/specification/DESIGN.md` with mouse encoding
- [ ] T5.2: Update `00-TORVOX-BASELINE.md` with new capabilities
- [ ] T5.3: Create `docs/plans/2026-08-30-reference-adoption-v6-detailed.md`
- [ ] T5.4: Create `docs/verification/2026-08-30-reference-adoption-v6-verification.md`
- [ ] T5.5: Full test suite run (cargo test + gradle test)
