# Spec: Mouse Event Encoding (SGR/Normal)

> Status: Implemented | Since: v7 (reference-adoption-v6)

## Purpose

Encode mouse position and action into terminal escape sequences using the SGR
(`?1006`) extension of X10/Normal tracking, enabling terminal applications that
request mouse reporting to receive pixel-precise coordinates.

## Design

### Mouse tracking modes

| Mode | DECSET  | Behavior |
|------|---------|----------|
| Normal | 1000 | Button press/release only |
| Button | 1002 | Press, release, motion while pressed |
| Any | 1003 | All motion events |

### SGR encoding format

```
CSI < Cb ; Cx ; Cy M    (press)
CSI < Cb ; Cx ; Cy m    (release)
```

Where `Cb` encodes button + modifier + action, `Cx` and `Cy` are 1-based
cell coordinates.

### Key invariants

1. **Cell coordinates, not pixels**: The JNI bridge receives pixel positions
   and cell dimensions; the encoder divides to get cell coords.
2. **Bounds clamping**: Out-of-bounds positions are clamped to
   `[1, rows]` / `[1, cols]` — never sent as 0 or negative.
3. **Drag events**: Motion-while-pressed sends button 32 (drag flag).

### Files

- `native/src/terminal/ghostty_terminal/public_api.rs` — `encode_mouse_event()` (zelland pattern)
- `native/src/android/ffi.rs` — JNI bridge `encode_mouse_event_inner()`
- `native/src/terminal/ghostty_terminal/tests.rs` — 5 test cases

### Test contract

- `encode_mouse_event_gated_off_without_tracking_mode` — no tracking → None
- `encode_mouse_event_sgr_press` — SGR press format verified byte-by-byte
- `encode_mouse_event_wheel` — scroll wheel encoding
- Bounds/drag tests for edge cases
