# Torvox Cross-Validation Audit Report

> Date: 2026-09-02
> Scope: Over-engineering, dead code, magic numbers, dependencies, spec sync
> Method: 5 independent audits cross-validated against source code

---

## Executive Summary

| Severity | Count | Description |
|----------|-------|-------------|
| 🔴 Critical | 1 | Cursor blink limits differ across FFI boundary (50–2000 Rust vs 100–1000 Kotlin) |
| 🔴 High | 6 | 5 dead public functions + 1 unused dependency (`mlkit`) |
| 🟡 Medium | 26 | 8 magic numbers, 17 naming violations, 1 missing `#[cfg(test)]` |
| 🟡 Medium | 4 | Stale spec counts (test total, mouse encoding, OSC133, a11y) |
| 🟢 Low | 10 | Over-engineering candidates (borderline; some provide test seams) |
| 🟢 Low | 5 | Test infrastructure in production crate (~3,749 lines) |

**False positives eliminated:** 5 (TerminalQueryPort, recomputeGrid, SearchDebouncer, MAX_GRAPHEME_CLUSTERS/KGP_STORAGE_LIMIT, searchJson).

---

## P0 — Critical: Cross-FFI Inconsistency

### Cursor Blink Speed Limits

| Side | File | Range |
|------|------|-------|
| Rust | `native/src/android/ffi.rs:3678` | `.clamp(50, 2000)` |
| Kotlin | `android/app/.../Bridge.kt:393` | `.coerceIn(100, 1000)` |

**Impact:** A user setting 50ms in Kotlin gets silently clamped to 100ms. A user setting 1500ms in Rust gets clamped to 1000ms. The two sides disagree on valid input ranges with no shared constant or documentation.

**Fix:** Define a shared constant (`CURSOR_BLINK_MIN_MS = 50`, `CURSOR_BLINK_MAX_MS = 2000`) in a single location (e.g., a Kotlin `object` or a Rust `const` re-exported via JNI). Both sides reference the same bounds.

---

## P1 — High: Dead Public Functions

All five functions have zero callers outside their defining module.

| Function | File:Line | Reason Dead |
|----------|-----------|-------------|
| `trigger_capture()` | `renderdoc_capture.rs:55` | RenderDoc integration stub; no call site |
| `is_available()` | `renderdoc_capture.rs:46` | Same module; unused guard |
| `set_capture_path()` | `renderdoc_capture.rs:72` | Same module; unused configuration |
| `snapshot_rebuild_count()` | `public_api.rs:309` | Public accessor; internal field is used but this method is never called |
| `try_take_snapshot_with_scroll()` | `public_api.rs:347` | Zero callers in entire codebase |
| `take_kitty_graphics_image()` | `public_api.rs:391` | Zero callers in entire codebase |
| `list_all_font_families()` | `pipeline.rs:781` | Zero callers in Rust or Kotlin/JNI |

**Recommended action:** Delete all five. If `renderdoc_capture` is kept for future use, gate the entire module behind `#[cfg(feature = "renderdoc")]` and remove the public API surface until needed.

---

## P1 — High: Unused Dependency

| Dependency | File | Line |
|------------|------|------|
| `com.google.mlkit:text-recognition:16.0.1` | `android/app/build.gradle.kts` | 279 |

Declared as `androidTestImplementation` but zero imports in any `.kt` file. Likely a remnant from an abandoned OCR experiment.

**Action:** Remove the dependency line.

---

## P2 — Medium: Magic Numbers

| Value | Location | Context |
|-------|----------|---------|
| `50`, `2000` | `ffi.rs:3678` | Cursor blink clamp (also see P0) |
| `100`, `1000` | `Bridge.kt:393` | Cursor blink coerce (also see P0) |
| `0..10`, `10ms` | `ffi.rs:1451` | Exit-code poll retry constants |
| `54` | `ffi.rs:3480`, `Bridge.kt:692` | Theme payload size (duplicated across FFI) |
| `16` | `ffi.rs:3343,3348,3378` | Search highlight record byte size |
| `200` | `BellHandler.kt:77,114` | Bell tone duration (ms) |
| `100` | `TerminalScreen.kt:301` | Accessibility preview truncation limit |

**Recommended action:** Extract each into a named constant with a descriptive comment. For values shared across FFI (like `54`), define once and reference from both sides.

---

## P2 — Medium: Naming Violations (Single-Letter Variables)

### Rust (8 locations)

| File:Line | Variable | Suggested Name |
|-----------|----------|----------------|
| `ffi.rs:2670–2671` | `w`, `h` | `width`, `height` |
| `ffi.rs:3999` | `s` | `speed_ms` or `duration` |
| `osc_handler.rs:448–449` | `t`, `b` | `top`, `bottom` |
| `url_regex.rs:62` | `m` | `match_result` or `mat` |
| `cell_builder.rs:412–413` | `o`, `n` | `old_run`, `new_run` |
| `context.rs:835–836` | `w`, `h` | `width`, `height` |
| `procedural_geometry.rs:117–120` | `a`, `b`, `c`, `d` | `top_left`, `top_right`, `bottom_right`, `bottom_left` |
| `procedural_geometry.rs:235` | `f` | `face` or `font_face` |

### Kotlin (9 locations)

| File:Line | Variable | Suggested Name |
|-----------|----------|----------------|
| `SelectionExpander.kt:39` | `c` | `char` or `character` |
| `BellHandler.kt:92` | `v` | `volume` or `amplitude` |
| `TerminalTheme.kt:56` | `v` | `value` |
| `ColorPickerDialog.kt:300` | `x` | `position` or `offset` |
| `TerminalSurface.kt:300` | `h` | `height` |
| `TerminalRuntime.kt:1249` | `d` | `distance` or `delta` |
| `TerminalRuntime.kt:1546` | `n` | `count` or `index` |
| `TerminalRuntime.kt:3522` | `e` | `event` or `exception` |
| `TerminalViewModel.kt:1104` | `w` | `width` |

---

## P2 — Medium: Missing `#[cfg(test)]` Gate

**`from_fixture()` in `native/src/render/font/pipeline.rs:172`**

This public function is only called from `#[cfg(test)]` contexts (`mod.rs:434,447`, `shaping.rs:122`). It should be gated with `#[cfg(test)]` to prevent accidental use in production code and to document its intent.

---

## P2 — Medium: Stale Spec Counts

| Spec Claim | Actual | Delta | Source |
|------------|--------|-------|--------|
| Rust tests: 995 | 1003 `#[test]` annotations | +8 | `grep -r '#\[test\]' native/src` |
| Mouse encoding tests: 5 | 6 | +1 | `tests.rs` encode_mouse_* functions |
| OSC133 tests: 8 | 9 | +1 | `output_processor.rs` semantic tests |
| Robolectric a11y tests: 13 | 1 Robolectric + 12 plain JUnit = 13 total, but only 1 is Robolectric | Spec mislabels | `BellHandlerTest.kt` is Robolectric; `TerminalAccessibilityTest.kt` is plain JUnit |

**Note:** The a11y count (13 total) is correct, but the spec incorrectly labels all 13 as "Robolectric." Only 1 (`accessibility_callback_observes_accepted_bells_for_every_mode`) runs under RobolectricTestRunner. The other 12 in `TerminalAccessibilityTest.kt` are standard JUnit tests.

---

## P3 — Low: Over-Engineering Candidates

These findings are confirmed but **borderline** — the patterns provide some value even if heavyweight.

| Finding | Verdict | Notes |
|---------|---------|-------|
| `Shell` sealed interface | **Not dead** | Used in `Bridge.kt:130–132` and `TerminalRuntime.kt:1032,1098,2430–2433`. Provides type safety. |
| `argbToRgbFloats` | **Not dead** | Called once in production (`Bridge.kt:721`) + 5 times in tests (`CursorColorTest.kt`). Test usage is real. |
| `RenderResult` data class | **Confirmed** | Sole return type of `renderWithNewOutput()`. `Pair<Int, Boolean>` would work. Low priority. |
| `PollResult.merge` | **Confirmed** | 16+ fields, valid pattern but verbose. Could be simplified. |
| `EnvOp` enum | **Confirmed (borderline)** | `HashMap<String, Option<String>>` is simpler, but enum is expressive and idiomatic. |
| `McpState` boilerplate | **Confirmed** | 14 `Mutex<Option<...>>` fields + 14 setters. Pattern is valid but verbose. |
| `GridSnapshot`/`CellSnapshot` | **Confirmed** | Large structs (11/17 fields). Test/debug paths only. |

---

## P3 — Low: Test Infrastructure in Production Crate

| Module | Lines | Gate |
|--------|-------|------|
| `render/procedural_geometry.rs` | 474 | `#[cfg(test)]` |
| `render/snapshot_reference.rs` | 647 | `pub(crate)` (no cfg gate) |
| `terminal/test_helpers.rs` | 769 | `#[cfg(any(test, feature = "test-util"))]` |
| `render/screenshot_tests.rs` | 1,400 | `include!` into `tests.rs` (`#[cfg(test)]`) |
| `terminal/snapshot_test.rs` | 459 | `#[cfg(any(test, feature = "test-util"))]` |
| **Total** | **3,749** | |

`snapshot_reference.rs` is declared as `pub(crate)` without a `#[cfg(test)]` gate, meaning it compiles in production builds. The others are properly gated.

**Recommendation:** Move to a dedicated `test-support` crate or gate `snapshot_reference.rs` behind `#[cfg(any(test, feature = "test-util"))]`.

---

## False Positives Eliminated

| Original Finding | Why False |
|------------------|-----------|
| `TerminalQueryPort` interface yagni | `Bridge.kt:98` implements it; provides test seam for future mocking |
| `recomputeGrid` dead | Called from `TerminalRuntime.kt:3934`, `TerminalSurface.kt:460,2965` |
| `SearchDebouncer` interface yagni | `FakeScheduler : DebounceScheduler` in `TerminalAccessibilityTest.kt:147` — interface enables JVM unit testing without Android Looper |
| `MAX_GRAPHEME_CLUSTERS` / `KGP_STORAGE_LIMIT` dead | Used at `internal.rs:1432,1437` and `internal.rs:464` respectively |
| `searchJson` unnecessary | Kotlinx.serialization default Json throws on unknown keys; `ignoreUnknownKeys = true` is required |

---

## Recommended Priority Order

1. **Fix P0** (cursor blink limits) — behavioral bug, silent truncation of user settings
2. **Delete P1 dead code** (5 functions + 1 dependency) — low risk, immediate cleanup
3. **Extract P2 magic numbers** — improves maintainability, prevents future drift
4. **Fix P2 naming violations** — improves readability, low risk
5. **Gate `from_fixture()`** — one-line fix
6. **Update stale spec counts** — documentation accuracy
7. **Consider P3 over-engineering** — only if touched anyway; low ROI for standalone PR
8. **Consider P3 test infra relocation** — larger refactor, do during a dedicated cleanup sprint
