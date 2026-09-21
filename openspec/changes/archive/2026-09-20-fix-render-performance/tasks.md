# Tasks

## T1: Scroll jank — render loop cadence during scroll

- [x] Root-cause analysis: scroll jank is the render loop parking at the 500ms
      idle latch between gesture motion events, not GPU render cost
      (`frame avg=0-1ms` vs `loop avg≈500ms`).
- [x] Fix: `SCROLL_MOTION_WINDOW_NANOS` motion-recency gate keeps the loop on
      the active 17ms latch + vsync pumping while the gesture moves.
- [x] Verify: `cargo test -p native --lib` (548 pass; 1 GPU-unavailable
      benchmark failure, environmental, no Rust touched).
- [x] Verify: emulator logcat loop-timing before/after scroll (see REPORT.md).
- [ ] Archive change after merge to main.

## T2: IME animation smoothness (done)

- [x] Forensics: IME show/hide fire ZERO `setRenderPaused`/`attachWindow`/
      `applySurfaceResize` — the pause chain is not engaged on
      `windowSoftInputMode="adjustNothing"`; render loop runs active
      (~62fps) for the animation + 5s (idle-clock staleness), with
      count=0 cheap frames, then returns to the 500ms idle latch.
- [x] Root cause: `TerminalScreen` reads `WindowInsets.ime.getBottom()` in the
      main composition body — every insets frame of the keyboard animation
      recomposes the whole screen (terminal Column + ModifierBar + search
      layer), i.e. main-thread animation-frame cost grows with the UI tree.
- [x] Fix: leaf observer `WindowImeBottomPx` reads the insets and writes a
      state; settle / cursor-follow / cursor-min-pan effects run as
      `snapshotFlow` collectors (`distinctUntilChanged` + `collectLatest`,
      cancel-restart semantics identical to the old keyed `LaunchedEffect`s);
      offsets stay in placement-phase `Modifier.offset {}` lambdas reading the
      states (layout invalidation only, no recomposition) — the main
      composition no longer recomposes per animation frame.
- [x] Verify: `cargo test -p native --lib` (551 passed, 0 failed);
      `:app:testDebugUnitTest` (598 passed, incl. `TerminalImePanTest`
      pan-formula cases); emulator: keyboard show → modifier bar fully above
      keyboard (OCR), `$` prompt untouched; hide → bar returns to bottom;
      loop cadence idle → active(5s) → idle, zero `setRenderPaused`.
- [ ] Archive change after merge to main.

## T3: Backspace latency — input-path render wake (in progress)

- [x] Root-cause analysis: after >5s idle the render loop parks at the 500ms
      idle latch with the vsync pump stopped; the ONLY immediate wake source
      is `SessionEntry.notifyRender()`. Input writes split into three Kotlin
      paths: `runtime.writeToPty` notifies; `Bridge.processKeyEvent`
      (hardware keys / IME `sendKeyEvent` KEYCODE_DEL backspace) and
      `Bridge.encodeMouseEvent` write the PTY via the `onPtyWrite` hook but
      NEVER notify — the shell echo waits out the 500ms idle-latch tick
      (~500ms input→echo). Ghostty backspace handling and the IME/encoding
      layers are NOT the bottleneck (frame avg 0-1ms; commit-batch path
      bounded by the 50ms fallback flush).
- [x] Fix: every PTY write wakes the loop — `onPtyWrite` wiring now also
      `notifyRender()` (hardware keys + mouse); latch gate extracted to pure
      `shouldUseIdleLatch()`; unit tests pin the gate truth table and the
      notifyRender refreshes idle-clock contract.
- [x] Verify: `cargo test -p native --lib` (551 passed, 0 failed);
      `:app:testDebugUnitTest` (602 passed, 0 failed, incl.
      `RenderLatchCadenceTest`); spotless/detekt clean on changed files.
- [ ] Emulator logcat input→echo around the idle latch (main agent's
      emulator session, see REPORT.md).
- [ ] Archive change after merge to main.