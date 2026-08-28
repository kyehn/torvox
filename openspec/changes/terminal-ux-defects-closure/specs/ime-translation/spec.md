# IME Hybrid Follow Spec (2026-08-26, v2)

## Requirement

When the IME shows/hides, the terminal follows the keyboard with NO text
squashing, NO per-frame grid reflow, and the prompt stays visible after the
transition for both short sessions (content at the grid top) and long sessions
(prompt at the grid bottom).

## Design — hybrid pan-then-reflow

| Phase | Trigger | UI behavior | Grid |
|---|---|---|---|
| Animating | IME inset differs from the settled value | Terminal `Column` + `ModifierBar` PAN via a placement-phase `Modifier.offset { }` lambda (inset read during placement, zero recomposition) | unchanged (rows/cols/cell metrics constant — no PTY resize, no reflow, no SurfaceView scale artifact) |
| Settled | Inset unchanged for `IME_SETTLE_FRAMES` (3) consecutive samples, polled every 16 ms by a `delay` loop | Both composables switch (same frame) to a real layout `padding(bottom = settled)` | `surfaceChanged` fires once → `applySurfaceResize` → `attachWindow` reconfigures the LIVE wgpu swapchain in place (`reconfigure_swapchain`, never recreating the surface — recreation races the render thread and fails with `ERROR_NATIVE_WINDOW_IN_USE_KHR`) → `applyGridResize(width, height)` subtracts ONLY the modifier bar (the view height already excludes the settled IME padding — double subtraction undersized the grid by exactly the keyboard height) |

Why the settle phase must be a real resize: a pure pan reveals the grid
BOTTOM. Short sessions keep their content at the grid TOP, so a pan hides the
prompt behind the keyboard. The settled reflow shrinks the viewport so the
prompt is always inside it (fresh session: rows 0..N; long session:
auto-scrolled bottom).

Why the animation phase must be a pan: a per-frame layout resize makes the
compositor scale the stale surface buffer (visible text squash) and a
per-frame grid reflow costs a full rebuild per frame (SwiftShader ≈ 40 ms →
slideshow). The pan moves only the window; nothing re-renders.

## Scenarios

- Given any session, when the IME shows, then during the animation
  `rows/cols` are unchanged, no `applyGridResize` log line appears, and the
  pan tracks the inset per frame; after 3 stable frames the padding applies,
  `applyGridResize` runs exactly once, and the prompt is visible above the
  ModifierBar.
- Given a fresh session (prompt at grid row 2), when the IME settles, then the
  reflowed viewport includes row 2 — the prompt is visible at the top of the
  area above the keyboard.
- Given `seq 1 60` (prompt at grid bottom), when the IME settles, then the
  reflowed viewport auto-scrolls to the bottom — the prompt is visible just
  above the ModifierBar.
- Given a text selection, when any IME inset delta arrives, then the selection
  handles + context menu are dismissed (`onApplyWindowInsets` clears the
  selection) — popups positioned at show time can never be stale relative to
  the pan.
- Given the IME hides, then the reverse sequence runs: pan back to 0 during
  the animation, one reflow back to the full grid at settle.

## Verification (all executed on emulator-5554, Pixel 9 API 35)

- `Missed App frame` during IME show: 0 (was 74 before the placement-phase
  offset); during hide: ≤4 (SwiftShader/Gboard environment floor).
- `adb shell input text` fresh-session flow: `$ echo fresh` + output + prompt
  visible above the keyboard after settle (screenshot H1).
- `seq 1 60` flow: rows 37–60 + prompt visible after settle (screenshot H2).
- `applyGridResize` log line appears exactly once per IME transition.
- No text squash at any phase (cell metrics constant during pan; reflow keeps
  font-derived cell size).
- Gates: `cargo test --workspace` 997 pass, `cargo clippy --all --deny
  warnings` clean, `spotlessCheck detekt` clean.
