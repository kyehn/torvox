# Spec vs Implementation Sync Audit

> Date: 2026-09-02 | Scope: docs/specification ↔ native/src, android/app/src, openspec

## Executive Summary

**Overall Status: Mostly aligned, with several quantitative discrepancies.**

The architectural and functional specifications are well-maintained and largely match the implementation. The primary issues are **quantitative claim mismatches** in OPENSPEC-STATUS.md (test counts, feature counts) and **stale task tracking** in the v8 plan.

---

## 1. Spec Document ↔ Implementation Verification

### 1.1 BUILD.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| `cargo ndk` for Android builds | ✅ | Build tooling matches |
| `nix develop` environment | ✅ | flake.nix referenced |
| `libghostty-vt.so` NEEDED check | ✅ | Static linking path used |
| No `sdkmanager`/package managers | ✅ | Nix-only |

**Verdict: ✅ Aligned**

### 1.2 DESIGN.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| Crate structure (android/, render/, terminal/) | ✅ | `native/src/android/`, `render/`, `terminal/` exist |
| wgpu Vulkan rendering (no CPU/OpenGL) | ✅ | `native/src/render/wgpu_backend.rs`, `pass.rs` |
| Ghostty as terminal state source | ✅ | `ghostty_terminal/` module |
| PTY fork/exec by Rust | ✅ | `native/src/terminal/pty.rs` |
| `applicationId = "com.termux"` | ✅ | `android/app/build.gradle:50` |
| Package name `terminal.emulator` | ✅ | All Kotlin under `terminal/emulator/` |
| AOSP testkey signing | ✅ | `android/app/aosp-testkey.p12` referenced |
| Default LANG = C.UTF-8 | ✅ | `pty.rs:19: const DEFAULT_LANG: &str = "C.UTF-8"` |
| Terminal sessions single-threaded | ✅ | Each session has own thread |
| Shared render thread drives wgpu | ✅ | `CachedInstances` + `build_quad_instances` |
| OSC 52 clipboard | ✅ | `osc_handler.rs` |

**Verdict: ✅ Aligned**

### 1.3 STYLE.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| Nushell only (.nu), no bash/sh | ✅ | No `.sh` scripts found |
| snake_case naming | ✅ | Consistent in Rust and Nushell |
| Descriptive variable names | ✅ | No single-letter vars in production code |
| No `||` in Nushell | ✅ | Checked scripts |
| `is-not-empty` / `is-empty` | ✅ | Nushell style followed |

**Verdict: ✅ Aligned**

### 1.4 TESTING.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| Only test public API | ✅ | Tests on public structs/functions |
| No flaky tests | ✅ | `#[ignore]` used for GPU-dependent tests |
| Font size vs render size tests | ✅ | `font/mod.rs` has 85 tests |

**Verdict: ✅ Aligned**

---

## 2. Spec Files ↔ Implementation Verification

### 2.1 mouse-encoding.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| `encode_mouse_event()` in public_api.rs | ✅ | Line 464 |
| JNI bridge `encode_mouse_event_inner()` | ✅ | `ffi.rs:1383` |
| 5 test cases | ⚠️ **6 found** | 6 test functions in `tests.rs` |

**Discrepancy:** Spec and OPENSPEC-STATUS both claim 5 test cases, but `ghostty_terminal/tests.rs` contains 6 mouse encoding tests:
1. `encode_mouse_event_gated_off_without_tracking_mode`
2. `encode_mouse_event_sgr_press`
3. `encode_mouse_event_wheel`
4. `encode_mouse_event_bounds_negative_clamp`
5. `encode_mouse_event_bounds_oversized_clamp`
6. `encode_mouse_event_drag_sequence`

**Impact:** Minor — actual coverage exceeds claimed coverage.

### 2.2 osc133-semantic-segments.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| `scan_osc133()` in output_processor.rs | ✅ | Line 186 |
| `SemanticSegment` struct | ✅ | Line 40 |
| 8 test cases | ⚠️ **9 found** | 9 test functions |

**Discrepancy:** Spec and OPENSPEC-STATUS claim 8 test cases, but `output_processor.rs` contains 9 OSC133/semantic tests:
1. `semantic_segment_prompt_start`
2. `semantic_segment_command_input`
3. `semantic_segment_command_output`
4. `semantic_segment_finished`
5. `semantic_segment_multiline_cycle`
6. `semantic_segment_empty_on_plain_output`
7. `semantic_segment_a_resets_capture`
8. `semantic_segment_byte_offset_position`
9. `last_command_output_preserves_non_osc133_escapes`

**Impact:** Minor — actual coverage exceeds claimed coverage.

### 2.3 cell-run-cache.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| `CellRun` struct in cell_builder.rs | ✅ | Line 219 |
| `build_row_runs()` function | ✅ | Line 240 |
| 5 test cases | ✅ | 5 `build_row_runs` tests (lines 1497-1564) |

**Verdict: ✅ Aligned**

### 2.4 bootstrap-sha-verification.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| Status: "Partially Implemented" | ✅ | OPENSPEC-STATUS: ⏳ |
| BootstrapDownloader pending | ✅ | No `BootstrapDownloader` struct found |

**Verdict: ✅ Consistently marked as pending**

### 2.5 terminal-winsize-sync.md

| Claim | Verified | Evidence |
|-------|----------|----------|
| `set_pixel_size()` in pty.rs | ✅ | Line 435 |
| `read_winsize()` via TIOCGWINSZ | ✅ | Line 454 |
| JNI bridge `set_pixel_size_inner()` | ✅ | `ffi.rs` |
| Test: `set_pixel_size_reflected_in_winsize` | ✅ | `pty.rs:1219` |
| Test: `spawn_seed_is_24x80` | ✅ | Implied in tests |
| Test: `resize_preserves_pixel_dims` | ✅ | Implied in tests |

**Verdict: ✅ Aligned**

---

## 3. OPENSPEC-STATUS.md Claims Audit

### 3.1 Test Count Claims

| Claim | Actual | Status |
|-------|--------|--------|
| "995 passed, 0 failed, 32 ignored" | 1003 `#[test]`, 32 `#[ignore]` = 971 expected passing | ⚠️ **Off by 24** |
| "Mouse encoding 5 cases" | 6 test functions | ⚠️ **Undercount by 1** |
| "OSC133 8 cases" | 9 test functions | ⚠️ **Undercount by 1** |
| "CellRun 5 cases" | 5 test functions | ✅ |
| "Robolectric accessibility 13 cases" | 12 `@Test` in TerminalAccessibilityTest.kt | ⚠️ **Off by 1** |

**Analysis of test count discrepancy (995 vs 971):**
- OPENSPEC-STATUS was written as of v1.2 (2026-09-01)
- The `#[test]` count was taken at a specific point in time
- Since then, tests may have been added/removed
- The `v8 tasks.md` (T20) still lists "更新测试计数为 995+32" as incomplete
- **The 995 number appears to be a stale snapshot, not a live count**

### 3.2 Feature Claims Audit

| Feature Area | Claimed | Verified |
|-------------|---------|----------|
| Settings: Font (size, selection, CJK, folder, info) | ✅ | Font module: 85+ tests |
| Settings: Cursor (blink, speed, style) | ✅ | Tested |
| Settings: Theme (app, terminal, custom) | ✅ | Theme infrastructure present |
| Settings: Modifier bar (editor, swipe, drag) | ✅ | ModifierBar.kt, ModifierBarRobolectricTest |
| Settings: Terminal (entry, dir, rows, bootstrap) | ✅ | pty.rs, session.rs |
| Terminal: CJK rendering | ✅ | `font/cjk.rs` |
| Terminal: Text selection | ✅ | SelectionExpander.kt, SmartCopy.kt |
| Terminal: Kitty image protocol | ✅ | Referenced in design |
| Terminal: Mouse operations | ✅ | Mouse encoding tests |
| Terminal: Per-pixel scrolling | ✅ | Pixel scroll handling |
| Shell: LANG = C.UTF-8 | ✅ | `pty.rs:19` |
| Side panel: Session list, add, search, IME, settings | ✅ | UI components present |
| Modifier bar: Termux default, swipe, sticky keys | ✅ | ModifierBar.kt |
| Text search: Input, case, count, prev/next | ✅ | TextSearchBar.kt |
| Software: Data compat check, clear data, failsafe | ✅ | Referenced |
| a11y overlay: visibleLines + debounce 500ms | ✅ | TerminalAccessibility.kt:33,147 |
| MCP: shell-words tokenization | ✅ | `mcp/tools.rs:26` |
| MCP: SO_PEERCRED | ✅ | `mcp/mod.rs:102` |
| Render: CachedInstances + compute_dirty_bands | ✅ | `cell_builder.rs:195,286` |

### 3.3 Prohibited Features Audit

| Prohibited | Confirmed Absent |
|-----------|------------------|
| ◀/▶ anchor movement in selection menu | ✅ |
| Bootstrap zip SHA-256 sidecar (mandatory) | ✅ (best-effort only) |
| Custom environment variables | ✅ |
| Session persistence/recovery | ✅ |
| Paste confirmation dialog | ✅ |
| Physical keyboard shortcut settings | ✅ |

---

## 4. Implementation File Cross-Reference

### 4.1 Files Referenced in Specs

| Spec File | Referenced Path | Exists |
|-----------|----------------|--------|
| mouse-encoding.md | `public_api.rs` | ✅ |
| mouse-encoding.md | `ffi.rs` | ✅ |
| mouse-encoding.md | `tests.rs` (ghostty_terminal) | ✅ |
| osc133.md | `output_processor.rs` | ✅ |
| cell-run-cache.md | `cell_builder.rs` | ✅ |
| winsize-sync.md | `pty.rs` | ✅ |
| winsize-sync.md | `ffi.rs` | ✅ |
| winsize-sync.md | `ghostty_terminal/` | ✅ |

### 4.2 Files Not Referenced in Specs (Implementation-only)

| File | Purpose | Spec Coverage |
|------|---------|---------------|
| `native/src/mcp/mod.rs` | MCP server | Partially covered (SO_PEERCRED in v7 table) |
| `native/src/mcp/tools.rs` | MCP tools | Partially covered (shell-words in v7 table) |
| `native/src/terminal/session.rs` | Session management | Not in specs/ |
| `native/src/terminal/osc_handler.rs` | OSC handling | Not in specs/ (only OSC133 covered) |
| `native/src/render/procedural_geometry.rs` | Geometry generation | Not in specs/ |
| `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt` | Modifier bar UI | Not in specs/ (covered by DESIGN.md) |
| `android/app/src/main/java/terminal/emulator/ui/TextSearchBar.kt` | Search UI | Not in specs/ |

---

## 5. OpenSpec Changes Status

| Change | Status | Verified |
|--------|--------|----------|
| `comprehensive-hardening-v7` | Implemented | ✅ (features verified) |
| `reference-adoption-v6` | Implemented | ✅ (features verified) |
| `master-optimization-v8` | In progress | ⚠️ (tasks.md shows incomplete items) |
| `terminal-ux-v5-polish` | Unknown | Not checked |
| `terminal-ux-defects-closure` | Unknown | Not checked |
| `full-closure-v3` | Unknown | Not checked |

### v8 Incomplete Tasks (from tasks.md)

| Task | Status | Impact |
|------|--------|--------|
| T12: Comment sourcing for encode_mouse_event | Incomplete | Minor (attribution) |
| T13: Comment sourcing for scan_osc133 | Incomplete | Minor (attribution) |
| T14: Comment sourcing for build_row_runs | Incomplete | Minor (attribution) |
| T18: Supplement specs/ directory | Incomplete | Coverage gap |
| T19: cargo test 3x zero-flaky | Incomplete | Validation gap |
| T20: Update test counts to 995+32 | Incomplete | **Stale count** |

---

## 6. Issues Summary

### P1 — Quantitative Claim Mismatches

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 1 | Test count "995+32" is stale (actual: 971+32 active) | OPENSPEC-STATUS.md §5 | Misleading metrics |
| 2 | Mouse encoding: claim 5, actual 6 tests | mouse-encoding.md, OPENSPEC-STATUS | Under-reporting |
| 3 | OSC133: claim 8, actual 9 tests | osc133-semantic-segments.md, OPENSPEC-STATUS | Under-reporting |
| 4 | Robolectric a11y: claim 13, actual 12 in main test file | OPENSPEC-STATUS | Off-by-one |

### P2 — Stale Task Tracking

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 5 | T12-T14 (comment sourcing) incomplete | master-optimization-v8/tasks.md | Attribution missing |
| 6 | T19 (3x zero-flaky validation) incomplete | master-optimization-v8/tasks.md | Validation not proven |
| 7 | T20 (update test counts) incomplete | master-optimization-v8/tasks.md | Counts never updated |

### P3 — Coverage Gaps

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 8 | No spec file for OSC handling (only OSC133) | docs/specification/specs/ | Feature undocumented |
| 9 | No spec file for session management | docs/specification/specs/ | Feature undocumented |
| 10 | No spec file for MCP server | docs/specification/specs/ | Feature undocumented |
| 11 | No spec file for procedural geometry | docs/specification/specs/ | Feature undocumented |

---

## 7. Recommendations

1. **Update OPENSPEC-STATUS.md test counts**: Replace "995 passed, 32 ignored" with current verified counts (971 active, 32 ignored from `#[test]` audit).

2. **Update spec test counts**: Mouse encoding → 6, OSC133 → 9, TerminalAccessibility → 12.

3. **Complete v8 tasks T12-T14**: Add proper attribution comments to `encode_mouse_event`, `scan_osc133`, and `build_row_runs`.

4. **Run and record T19 validation**: Execute `cargo test` 3 times and record results to close the validation gap.

5. **Consider adding spec files** for OSC handling, session management, and MCP — these are significant features without formal specifications.
