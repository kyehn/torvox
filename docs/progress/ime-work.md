# IME/Lag Fix — Investigation & Plan

## Current State

### Root causes identified

**A. Cached instances ignore projection_height (performance + visual)**

In `gpu-renderer/src/gpu.rs`, `build_cell_instances_into()`:
- The cached-row path (line 2526) is checked **before** the `projection_height` break (line 2539)
- When surface height shrinks (IME opens), `render_height` decreases → `projection_height` decreases
- Clean rows beyond the new `projection_height` **still get copied from cache** and sent to GPU
- This wastes GPU bandwidth and contributes to lag
- Also: cached instances were built with old render dimensions, and if `cell_h` changed they'd be wrongly positioned

**B. IME resize triggers PTY resize (visual shift)**

In `TerminalSurface.kt` → `applySurfaceResize()`:
- `updateNativeWindow(width, height)` is called
- Which triggers `bridge.update_native_window()` → `surface.update_native_window()`
- Which calls `store_cell_metrics()` → `recompute_grid()` is NOT called here
- But `updateNativeWindow` in Kotlin then calls `syncGridDimensions()` which does NOT resize the PTY
- However `recomputeGrid()` IS called separately from `onSurfaceTextureAvailable`
- When IME appears, `onSizeChanged` fires → `applySurfaceResize` → only `setSurfaceSize` + `updateNativeWindow`, NOT `recomputeGrid`
- So the PTY rows/cols DON'T change when IME appears — this is somewhat correct already

Wait — let me re-check. `recomputeGrid` is NOT called on IME appearance. But when the layout measure changes, `LocalDensity` change →... Let me check if `recomputeGrid` is ever called from the layout side.

Actually the composable has `onSizeChanged` and `onSurfaceTextureSizeChanged`. `onSurfaceTextureSizeChanged` also calls `applySurfaceResize(width, height)` which does NOT call `recomputeGrid`.

**BUT** — when IME appears, the layout changes → `onSizeChanged(width, height)` fires → the `rows` and `cols` stored in Kotlin state are NOT updated because `syncGridDimensions` gets `rows/cols` from the bridge which also haven't changed (since PTY wasn't resized).

Then `ModifierBar` composable uses these `rows` and `cols` — the row/col display is stale but not harmful.

**C. adjustResize + imePadding double-padding (visual)**

`windowSoftInputMode="adjustResize"` in AndroidManifest + `imePadding()` on the root Column.

With `adjustResize`:
- Window content height = screen_height - IME_height
- TextureView is measured at this smaller height
- `imePadding()` reads `WindowInsets.ime.bottom` = IME_height
- Adds bottom padding = IME_height
- So the Column's total content area (above padding) = screen_height - 2*IME_height

Impact: TextureView gets a very small actual height. This is partially correct in that the terminal should render less content. But the double-reduction is excessive.

**D. onSizeChanged multiple redundant calls (lag)**

When IME opens/closes, `onSizeChanged` fires multiple times during animation. Each call:
- Acquires bridge pointer
- Calls `setSurfaceSize` + `updateNativeWindow`
- These trigger native function calls
- The render thread re-renders after each one

This is inherently expensive, even more so with the cache projection_height bug iterating too many rows.

## Fix Plan

### 1. GPU: fix projection_height check to run BEFORE cached-row path

**File**: `gpu-renderer/src/gpu.rs`  
**Change**: In `build_cell_instances_into()`, move the projection_height check before the cached-row branch:

```rust
for row in 0..rows {
    if projection_height > 0.0 && (row as f32 * cell_h) >= projection_height {
        break;
    }
    
    if use_cache && !config.dirty_rows[row as usize] {
        // cached path (existing)
    }
    // new cell processing (existing)
}
```

### 2. GPU: dirty all rows when render_height changes vs cached height

**File**: `android-gui/src/surface.rs`  
**Requirement**: When `self.render_height` changes (IME resize), mark all rows dirty so the GPU rebuilds instances at the correct `projection_height`.

**Key**: Compare new `render_height` against `self.frame_count > 0` and a stored `prev_render_height`. If changed, skip cache.

**Simpler approach**: In `render()`, when `prev_cells.len() != total_cells`, all rows are dirty (already implemented at line 648). Since `rows` and `cols` don't change with IME (PTY isn't resized), `total_cells = snapshot.rows * snapshot.cols` stays the same, so this condition won't trigger.

**Better approach**: Add a `prev_render_height` field to `AndroidSurface`. If `self.render_height != self.prev_render_height`, force all rows dirty.

### 3. Maintain scroll offset constant across IME open/close

**Key**: When IME appears, keep `scroll_offset` where it was. The user should see the same content at the top, just fewer rows.

Already the behavior since `recomputeGrid` isn't called on IME resize. But if the PTY IS resized later (e.g., on keyboard close), the scroll might shift.

**Change**: Ensure that when PTY resize happens (outside IME), the scroll offset is preserved.

### 4. Layout: remove imePadding() duplication

**Option A**: Remove `adjustResize` (keep only `imePadding`)
- On API 30+, `imePadding()` with `windowSoftInputMode="adjustNothing"` works correctly
- System positions the window behind the IME, Compose handles insets
- `imePadding()` handles the padding correctly
- **Risk**: Changing to `adjustNothing` changes resize behavior, might affect other layout elements

**Option B**: Keep `adjustResize` but conditionally apply `imePadding()` only when IME won't overlap
- Complex, error-prone

**Option C**: Remove `imePadding()`, rely on `adjustResize` solely
- Simple, and the window is already resized above IME
- BUT `ModifierBar` and other elements below the terminal may need IME-aware padding

**Recommendation**: **Option A** — switch to `adjustNothing` + use Compose's `imePadding()` correctly. This is the modern approach for Compose on API 30+.

Wait — but changing window soft input mode changes how the whole activity responds. Let me reconsider.

**Option D**: Keep `adjustResize`, remove `imePadding()` from the root Column. Leave the `modifier` to only handle `safeDrawingWindowPadding()`.
- The window is already resized above IME by `adjustResize`
- The terminal surface gets the correct reduced size
- The `ModifierBar` or other UI elements don't need IME padding because they're below the fold

Actually, `imePadding()` was added by me to fix the ModifierBar position. But if `adjustResize` is active, the window is already resized, so `imePadding()` would push the bar too far. This confirms the double-padding hypothesis.

**Recommendation**: Keep both `adjustResize` and `imePadding()`, but make `imePadding()` respect already-resized insets:
- On API 30+, `WindowInsets.Type.ime()` may report 0 insets when `adjustResize` resized the window
- If so, `imePadding()` adds 0 — no double-padding problem
- This is device/Android-version dependent

**Actual fix**: I need to test on the actual API 30+ device/emulator. Let me just try removing `imePadding()` first and see if the ModifierBar is still positioned correctly.

Actually wait, let me re-read the whole TerminalScreen composable layout to understand the full picture.

### 5. Debounce rapid resize calls

**File**: `TerminalSurface.kt`  
**Problem**: `onSizeChanged` fires multiple times during IME animation
**Fix**: Throttle/debounce resize calls. Only apply the final size, not intermediate animation frames.

## Summary of Changes Needed

| # | Component | Fix | Priority |
|---|-----------|-----|----------|
| 1 | gpu.rs | Move projection_height check before cache path | P0 |
| 2 | surface.rs | Add prev_render_height tracking, force dirty on change | P0 |
| 3 | TerminalSurface.kt | Debounce resize calls | P1 |
| 4 | TerminalScreen.kt | Fix imePadding duplication | P1 |
| 5 | TerminalScreen.kt | Remove column around surface | P1 |
| 6 | TerminalSurface.kt | Ensure scroll offset preserved across IME resize | P0 |

## Files to Edit

1. `gpu-renderer/src/gpu.rs` — projection_height before cache
2. `android-gui/src/surface.rs` — prev_render_height tracking
3. `android/app/src/main/java/io/torvox/terminal/TerminalSurface.kt` — debounce
4. `android/app/src/main/java/io/torvox/terminal/TerminalScreen.kt` — layout fixes

## Verification

1. Build native lib: `cargo build --target x86_64-linux-android --package android-gui`
2. Build APK: `./gradlew assembleDebug`
3. Install on emulator, test:
   - Open app
   - Type something visible on screen
   - Open IME — verify top content is pixel-identical
   - Close IME — verify content is pixel-identical to before
   - Measure frame rate / lag
4. Repeat for multiple shell prompts at different scroll offsets
