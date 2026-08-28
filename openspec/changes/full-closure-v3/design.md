# Design: full-closure-v3

## Architecture

- **CJK**: `cjk.rs`优先级→`pipeline.rs`保序→`glyph_cache.rs` outline缓存→`shaping.rs`分段。
- **IME**: `TerminalScreen.kt` LaunchedEffect采样16ms×3 → `imeFollow` placement-phase offset → settle后padding+单次 `applyGridResize` → `TerminalSurface` reconfigure.
- **BlackScreen**: `surfaceCreated` guard → `ON_RESUME` surface有效时清pause → `ffi.rs` reconfigure vs recreate分支.
- **Drawer**: `surface.requestFocus()` → `surface.windowInsetsController.show()` 对称.

## Tests

- 后端: `cjk_priority_tests` 5用例, `cargo test --workspace 997`
- 前端: `CursorPixelAcceptanceTest`, `SelectionTapDismissTest`, `screenrecord` 60fps, `dumpsys gfxinfo`
- 日志: `FALLBACK_CANDIDATE`, `RECONFIGURE`, `attachWindow` 审计
