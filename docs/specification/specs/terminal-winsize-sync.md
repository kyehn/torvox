# Spec: Terminal WinSize Sync

> Status: Implemented | Since: v6 (reference-adoption-v6)

## Purpose

Maintain kernel-side pixel dimensions (`ws_xpixel`/`ws_ypixel`) in sync with
the GPU renderer's actual cell size, so fullscreen TUI applications (e.g. htop,
midnight commander) receive correct pixel dimensions via `TIOCGWINSZ`.

## Design

### Data flow

```
Kotlin Compose layout
  → JNI set_pixel_size(width_px, height_px)
    → PtyPair::set_pixel_size()
      → cache pixel_width / pixel_height (AtomicU16)
      → TIOCSWINSZ with rows/cols preserved + new pixel dims
```

### Key invariants

1. **Rows/cols never drift**: `set_pixel_size` reads the current `Winsize` via
   `TIOCGWINSZ` first, replaces only `ws_xpixel`/`ws_ypixel`, then writes back
   the complete struct via `TIOCSWINSZ`.
2. **Pre-computed by TerminalRuntime**: Pixel dimensions are calculated once at
   layout time (font size × cell count) and passed down — the PTY layer never
   computes them itself.
3. **Spawn seed**: PTY starts with 24×80 rows/cols; pixel dims start at 0 and
   are updated on first layout.

### Files

- `native/src/terminal/pty.rs` — `PtyPair::set_pixel_size()`, `read_winsize()`
- `native/src/android/ffi.rs` — JNI bridge `set_pixel_size_inner()`
- `native/src/terminal/ghostty_terminal/` — `TerminalRuntime::resize()` triggers winsize sync

### Test contract

- `set_pixel_size_reflected_in_winsize` — verifies pixel dims survive resize round-trip
- `spawn_seed_is_24x80` — verifies initial rows/cols
- `resize_preserves_pixel_dims` — verifies row/col resize doesn't clobber pixel dims
