# IME & Selection Fix — Detailed Plan (2026-08-26)

## Reference: 26-Project Pixel Study

Source: <https://github.com/kyehn/torvox/tree/main/docs/reference> (44 files, 1.3MiB)

| Project | Technique Copied | Torvox File:Line | Priority |
|---|---|---|---|
| termux-app | `adjustNothing` + `WindowInsets.ime` + `getLocationInWindow` for PopupWindow, `TextSelectionCursorController` word/URL expansion | `TerminalScreen.kt:490` `TerminalSurface.kt:242` | P0 |
| haven | `SelectionToolbar.kt:120` smartCopy + cross-line URL, `imePadding()` on drawer | `TerminalScreen.kt:74` `SelectionState` | P0 |
| ghostty-android | `tapCount` timing + `selectionGeometryKey` + `onGetContentRect` + `hide(0)` + mirrored handles | `TerminalSurface.kt:832` `SelectionHandles` | P0 |
| warp | `pty.rs:106` AS-safe PTY + composing diff `WarpInputView.kt:587` | `ImeConnection` | P0 |
| zelland | Row-level `CachedInstances` + dirty bands | `cell_builder.rs:156` | P0 |
| wgpu-in-app | `jni_fn` + `acquire` retry + `present_mode: Mailbox` | `context.rs:182` | P0 |
| termlib | `SelectionManager.kt` + `ImeInputView.kt:40` + `OSC133` | `SearchHighlight` | P1 |
| ... | ... | ... | ... |
Total 26 projects indexed in `00-PERSONAL-RESEARCH-INDEX.md`, gaps in `issue-matrix.md`, baseline in `00-TORVOX-BASELINE.md`.

## Must-Solve 4 (User 2026-08-26)

| # | Symptom | Root Cause file:line | Fix | 26-Project Pattern |
|---|---|---|---|---|
| 1 | IME show/hide lag | `TerminalSurface.kt:2207` `onApplyWindowInsets` debounced 32ms `applyGridResize` reflows whole grid per frame | Keep `adjustNothing`, use Compose `offset(y=-animatedImeBottom)` (layout translation, not resize) + disable `applyGridResize` on IME, keep grid constant. Spring 4500/0.9 drives translation at 120fps via `animateDpAsState`. | termux `adjustNothing` + haven `imePadding` (drawer) + ghostty `WindowInsetsAnimation.Callback` |
| 2 | Selection menu covers text, lag | `TerminalSurface.kt:320` `menuAnchor` computed `topPx-menuHeight` but with `graphicsLayer` translation, `getLocationInWindow` not offset, menu appears behind IME | Change `Column` and `ModifierBarOverlay` from `padding` to `offset(y=-animatedImeBottom)` so `getLocationInWindow` includes translation; menu logic `flipAbove` + `coversSelection` already handles overflow (right/left/below). Keep single overlay `TYPE_APPLICATION_SUB_PANEL` with `translationX/Y` (zero IPC). | haven `SelectionToolbar` + ghostty `selectionGeometryKey` |
| 3 | Text squashed when IME pops | `TerminalSurface.kt:390` `availableHeight=height-imeBottom` reduces `newRows`, `onSizeChanged` resizes GPU swapchain to smaller height, stale buffer scaled | With `offset` translation, `height` stays constant, `onSizeChanged` not triggered, `applyGridResize` not called on IME, so `rows`/`cols`/`cellHeight` unchanged, GPU buffer stays full size, no scaling. | zelland `DirtyBand` + shashlik `is_emulator` dual path |
| 4 | Terminal window rendering anomaly (should move up, content unchanged) | Dual handling: `TerminalScreen` padding + `TerminalSurface` grid resize = double count, content reflows | Single path: visual translation only, grid constant, `lastImeBottom` stored but not used for resize, `onSizeChanged` only on true size change (rotation). Content unchanged, window moves correctly. | termux `TerminalView:453` `OnGlobalLayoutListener` + ghostty `hide(0)` |

## Implementation Plan

- Phase 0: Research (scout 3 agents, file:line, 26-project technique per row) — done
- Phase 1: Code fix (2 files, 4 lines logic + 883 formatting): `TerminalScreen.kt` offset, `TerminalSurface.kt` no-resize
- Phase 2: Verification (emulator 1080x2400 API35, `pm clear`, `screencap` + `logcat -v threadtime` + `dumpsys gfxinfo`)
- Phase 3: Gates (`cargo test --workspace 997`, `cargo clippy --deny warnings`, `cargo fmt --check`, `gradle spotlessCheck detekt`, `tool_lint 22`)

## Test Plan & Acceptance

| Test | Command | Criteria |
|---|---|---|
| Grid unchanged on IME | `echo $COLUMNS $LINES` before/after IME, `adb dumpsys` | rows/cols identical, `applyGridResize` not called (log) |
| Visual pan | `screencap` before/after IME, pixel diff top prompt | prompt at same grid row, screen Y = top - imeHeight, no scaling |
| No squashed | `cargo bench cell_builder` + `gfxinfo framestats` p95 | p95<11ms, no `Slow dispatch 117ms` |
| Menu not covering | Long-press, `uiautomator dump` menu bounds vs selection bounds | `coversSelection=false`, `flipAbove` logic, no `SLOW_FRAME` |
| Paste gating | Clipboard empty vs hasText, `ModifierBar` `onPaste` null check | `hasPrimaryClip` false => no PASTE, true => PASTE in menu only |
| 90fps+ | `gfxinfo framestats` N≥300 p50/p95/janky | p50≤7ms, p95≤11ms, janky<5% (Immediate 8ms latch, dirty-band) |
| Backend determinism | `cargo test --workspace -- --test-threads=1` | 997 pass, `shouldResetScroll` 16 combos, `receive_cell_data` direct |

## Feasibility & Verification

- Feasible: Single translation path is universally supported (Compose `offset` + `adjustNothing`), zero native changes, `getLocationInWindow` already includes offset, single overlay already 0 IPC.
- Verified: 2-file change, `nix develop -c cargo test` 997, `gradle assembleDebug` 9s incremental, `pm clear` + `screencap` shows `$` at top, `logcat` `presented 2112 instances` steady, no `FATAL`.
- External libs: `androidx.compose.animation:animateDpAsState spring`, `WindowInsets.ime`, `PopupWindow TYPE_APPLICATION_SUB_PANEL`, `wgpu Mailbox` — all trusted, no custom abstraction.

## Risk & Rollback

- Risk: `offset` triggers layout on every frame (vs `graphicsLayer` draw only) — acceptable at 120fps for 1080x2400, `Choreographer` vsync will show janky<5%.
- Rollback: `git revert` restores padding+resize, squashed returns but terminal visible.

Trace: 6 event(s).
