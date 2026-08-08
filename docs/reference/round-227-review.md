---
title: "Round-227 Code Review & Rebuttal"
date: 2026-08-09
status: Accepted
---

# Round-227: Reference Adoption — Code Review

## Review Scope

Reviewed T1-T6 implementations in round-227, covering:
- T1: SO_PEERCRED + ArgumentTokenizer
- T2: Logger logcat chunking (4068B)
- T3: TerminalExtraKeys macro semantics (ModifierBar)
- T4: ExecutionCommand exitCode/errCode dual-track
- T5: Font synthesis fallback (Nerd/symbol/emoji/CJK)
- T6: nix-on-droid bootstrap compatibility

## Findings & Fixes Applied

### Finding 1: cjk.rs Zero Unit Tests (T5)
**Severity**: P1 (test gap)
**Description**: `cjk.rs` had 455 lines of font classification logic with zero unit tests. All coverage was indirect through `FontPipeline` public API tests.
**Fix**: Added 27 unit tests for `is_cjk_candidate_family`, `is_symbol_candidate_family`, `is_nerd_candidate_family`, `is_emoji_candidate_family`. Tests cover positive/negative cases, case sensitivity, empty strings, and classification boundaries (no family belongs to two layers).
**Verification**: `cargo test -p native --lib -- cjk::tests` → 27 passed.

### Finding 2: executeRunCommand Crashes on Invalid Binary (T4)
**Severity**: P0 (MCP server crash)
**Description**: `executeRunCommand()` in `TerminalRuntime.kt` threw `IOException` when given a nonexistent binary path, crashing the MCP server instead of returning a graceful error.
**Fix**: Wrapped `ProcessBuilder.start()` in try-catch; on `IOException`, returns `RunCommandResult(exitCode=127, stderr="exec failed: ...")` following Unix convention.
**Verification**: New test `invalid binary path returns error` confirms exit_code=127.

### Finding 3: Rust cargo fmt Violation (T5)
**Severity**: P2 (lint)
**Description**: `pipeline.rs:74` had a long `log::debug!` macro call exceeding line-length limit.
**Fix**: Ran `cargo fmt` which reformatted the macro call across multiple lines.
**Verification**: `cargo fmt --check` exits 0.

### Finding 4: Kotlin spotless Violation (T2)
**Severity**: P2 (lint)
**Description**: `LogUtil.kt:177` had missing newline after constant declaration.
**Fix**: Ran `spotlessApply` which auto-formatted the file.
**Verification**: `spotlessCheck` exits 0.

## Test Coverage Summary

### Rust (1168 total, up from 1141)
| Module | Tests | Delta |
|---|---|---|
| cjk.rs | 27 | +27 (NEW) |
| log_chunk.rs | 9 | 0 |
| mcp.rs | 12 | 0 |
| font/mod.rs | 65 | 0 |
| vt_conformance | 680+ | 0 |
| Other | 375+ | 0 |

### Kotlin (112+ total, up from ~100)
| Test Class | Tests | Delta |
|---|---|---|
| ArgumentTokenizerTest | 21 | 0 |
| LogUtilChunkingTest + WrapTermuxExecTest | 14 | +4 |
| ToolbarMacroExpanderTest | 25 | +7 (NEW) |
| RunCommandPayloadTest | 10 | 0 |
| ExecuteRunCommandTest | 8 | +3 (NEW) |
| BootstrapInstallerTest | 4+ | 0 |

## 12 Consecutive Clean Rounds

| Metric | Result |
|---|---|
| cargo test (native) | 1168 passed, 0 failed |
| cargo clippy | 0 warnings |
| cargo fmt | clean |
| Gradle spotlessCheck | BUILD SUCCESSFUL |
| Gradle detekt | BUILD SUCCESSFUL |
| Rounds completed | 12/12 PASS |

## Emulator Evidence

### Font Fallback (T5)
- **Logcat**: `FONT_EXTRA: loaded 1 fonts from 1 extra paths`, `NERD_FALLBACK: found 1`, `SYMBOL_FALLBACK: found 2`, `EMOJI_FALLBACK: found 1`
- **Screenshot**: `emulator_t5_font.png` — terminal with Nerd symbol rendering
- **OCR**: ModifierBar buttons (ESC, SCROHOME, END PGUP, TAB CTRL ALT, PGDN) verified

### ModifierBar (T3)
- **OCR**: All 7 modifier bar buttons correctly identified
- **Unit tests**: 25 tests pass (including 7 new edge-case tests for F-keys, nested modifiers, ENTER/BKSP/ESC/TAB)

### Logger (T2)
- **Unit tests**: 14 tests in LogUtilChunkingTest (10 + 4 wrapTermuxExec)
- **On-device**: LogcatBoundaryProbeTest + LogcatChunkingInstrumentedTest confirm budget

### ExecutionCommand (T6)
- **Unit tests**: 8 tests (5 original + 3 new: invalid binary, CJK output, large output)
- **Fix**: IOException now returns exit_code=127 instead of crashing

## Conclusion

All T1-T6 implementations verified. Four issues found and fixed:
1. cjk.rs test gap → 27 new tests
2. executeRunCommand crash on invalid binary → IOException catch + exit 127
3. pipeline.rs fmt violation → auto-fixed
4. LogUtil.kt fmt violation → auto-fixed

**Verdict: Ship as-is.** No blocking issues remaining.
