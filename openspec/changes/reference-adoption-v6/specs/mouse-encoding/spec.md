# Spec: mouse-encoding

## Purpose

Standard mouse event encoding for terminal applications that use mouse mode (vim, htm, midnight commander, etc.).

## Requirements

- REQ-M1: Support SGR (mode 1006), Button (mode 1002), Any (mode 1003), and X10 (mode 1000) mouse modes
- REQ-M2: Encode Press, Release, Drag, ScrollUp, ScrollDown actions
- REQ-M3: Map pixel coordinates to cell coordinates using renderer's cell dimensions
- REQ-M4: Gate encoding on active mouse mode (no events when mode is None)
- REQ-M5: Use libghostty-vt's `ghostty_mouse_encoder` C API for encoding

## Test Cases

| ID | Input | Expected |
|----|-------|----------|
| TC-M1 | mode=None, any event | No output |
| TC-M2 | mode=SGR, press at (100,200), cell=(13,27) | SGR sequence `\x1b[<0;7;14M` |
| TC-M3 | mode=SGR, scroll at (50,50) | SGR scroll sequence |
| TC-M4 | mode=SGR, press at (-1,-1) | Clamped to (0,0) |
| TC-M5 | mode=SGR, release | SGR release sequence `m` suffix |

## Traceability

- Source: zelland `terminal.rs:41-175` (ghostty_mouse_encoder)
- Source: termlib `MouseModeTracker`
- Gap: torvox has `MouseModeTracker.kt` (mode tracking) but no encoding
