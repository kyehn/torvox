# Acceptance Criteria — Torvox

## Overview

Each functional requirement (FR-xxx) and non-functional requirement (NFR-xxx) has
a set of acceptance criteria. A requirement is considered **accepted** when all
of its criteria pass in the appropriate verification environment.

See `docs/traceability.yml` for the full traceability matrix linking
requirements to design, API, tests, and verification methods.

---

## 1. Terminal Emulation

### FR-001: VT/xterm Escape Sequence Processing

**Requirement**: The system SHALL process VT/xterm escape sequences using the
Ghostty parser (`libghostty-vt`).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | ECMA-48 CSI, DCS, and OSC sequences produce correct terminal state transitions | `cargo test --package torvox-terminal -- ecma48_correctness` ([`ecma48_correctness.rs`](torvox-terminal/tests/ecma48_correctness.rs)) |
| 2 | Vttest screen and cursor test sequences produce expected output | `cargo test --package torvox-terminal -- vttest_sequences` ([`vttest_sequences.rs`](torvox-terminal/tests/vttest_sequences.rs)) |
| 3 | Structured fuzz-generated VT input does not cause crashes or state corruption | `cargo test --package torvox-terminal -- fuzz_vt_structured` ([`fuzz_vt_structured.rs`](torvox-terminal/tests/fuzz_vt_structured.rs)) |

### FR-002: Terminal Grid Data Model

**Requirement**: The system SHALL maintain a terminal grid data model (`Grid`)
consisting of rows of cells, each with a character code, foreground/background
color, and text attributes (`Attrs`).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Grid operations (insert, delete, clear rows/cols) produce correct cell layout | `cargo test --package torvox-core -- grid_ops` ([`torvox-core/tests/grid_ops.rs`](torvox-core/tests/grid_ops.rs)) |
| 2 | Grid state machine matches reference model under random operations | `cargo test --package torvox-core -- property_tests` ([`torvox-core/tests/property_tests.rs`](torvox-core/tests/property_tests.rs)) |
| 3 | Grid serialization round-trips through rkyv without data loss | `cargo test --package torvox-core -- grid_snapshot_integration` ([`torvox-core/tests/grid_snapshot_integration.rs`](torvox-core/tests/grid_snapshot_integration.rs)) |

### FR-003: SGR (Select Graphic Rendition) Attributes

**Requirement**: The system SHALL support SGR parameters: bold, dim, italic,
underline, double underline, blink, reverse, hidden, strikethrough, overline,
and protected.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | All 11 SGR attribute codes are parsed and produce the correct `Attrs` bitfield | `cargo test --package torvox-terminal -- sgr_parser_tests` ([`sgr_parser_tests.rs`](torvox-terminal/tests/sgr_parser_tests.rs)) |
| 2 | Property tests over random SGR sequences produce consistent attribute combinations | `cargo test --package torvox-terminal -- sgr_proptest` ([`sgr_proptest.rs`](torvox-terminal/tests/sgr_proptest.rs)) |
| 3 | Full SGR compatibility suite passes against a reference terminal implementation | `cargo test --package torvox-terminal -- sgr_full_compat` ([`sgr_full_compat.rs`](torvox-terminal/tests/sgr_full_compat.rs)) |

### FR-004: Color Support (ANSI, 256-Color, Truecolor)

**Requirement**: The system SHALL support 16 ANSI color palette indices plus
256-color and truecolor (24-bit RGB) foreground/background specifications.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | All 16 ANSI color palette indices (0–15) are mapped to correct RGB values | `cargo test --package torvox-core -- terminal_colors` ([`torvox-core/tests/terminal_colors.rs`](torvox-core/tests/terminal_colors.rs)) |
| 2 | 256-color (38;5;n / 48;5;n) and truecolor (38;2;r;g;b / 48;2;r;g;b) sequences produce correct `Color` values | `cargo test --package torvox-terminal -- colors` ([`torvox-terminal/tests/ported_kitty_full/colors.rs`](torvox-terminal/tests/ported_kitty_full/colors.rs)) |
| 3 | Color palette indices resolve to theme-defined values when a custom theme is active | Code review: `torvox-core/src/color.rs` maps index → `RgbColor` through active theme |

### FR-005: Alternate Screen Buffer

**Requirement**: The system SHALL support alternate screen buffer mode (SM/RM
1049) for full-screen applications (e.g., vim, less).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Entering alternate screen (CSI ? 1049 h) switches to a fresh buffer and hides scrollback | `cargo test --package torvox-terminal -- alt_screen` ([`alt_screen.rs`](torvox-terminal/tests/alt_screen.rs)) |
| 2 | Exiting alternate screen (CSI ? 1049 l) restores the primary buffer and scrollback content | `cargo test --package torvox-terminal -- alt_screen` ([`alt_screen.rs`](torvox-terminal/tests/alt_screen.rs)) |
| 3 | Alternate screen buffer has independent dimensions and cursor state | Integration test coverage in `torvox-terminal/tests/alt_screen.rs` |

### FR-006: Cursor Positioning and Style

**Requirement**: The system SHALL support cursor positioning and movement (CUU,
CUD, CUF, CUB, CUP, HVP, etc.) and cursor style (block, bar, underline, beam)
with visible/hidden state.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | All cursor movement CSI sequences move the cursor to the expected position | `cargo test --package torvox-terminal -- cursor_cmds_tests` ([`cursor_cmds_tests.rs`](torvox-terminal/tests/cursor_cmds_tests.rs)) |
| 2 | Cursor style (DECSUSR, CSI SP q) selection switches between block, bar, underline, and beam | `cargo test --package torvox-terminal -- dec_modes_dedicated` ([`dec_modes_dedicated.rs`](torvox-terminal/tests/dec_modes_dedicated.rs)) |
| 3 | Cursor visibility (DECTCEM) toggles on/off correctly | `cargo test --package torvox-terminal -- dec_modes_dedicated` ([`dec_modes_dedicated.rs`](torvox-terminal/tests/dec_modes_dedicated.rs)) |

### FR-007: Scrolling Regions

**Requirement**: The system SHALL support scrolling regions (`scroll_up`,
`scroll_down`, `insert_lines`, `delete_lines`) with configurable top/bottom
boundaries.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Scroll up/down within a DECSTBM region scrolls only the specified lines | `cargo test --package torvox-core -- grid_ops` ([`torvox-core/tests/grid_ops.rs`](torvox-core/tests/grid_ops.rs)) |
| 2 | Insert/delete lines shift content as expected with configurable boundaries | `cargo test --package torvox-terminal -- grid_state_machine` ([`grid_state_machine.rs`](torvox-terminal/tests/grid_state_machine.rs)) |
| 3 | Fuzz-generated grid operations do not panic or corrupt cell data | `cargo run --package fuzz -- fuzz_grid_ops` ([`fuzz/fuzz_targets/fuzz_grid_ops.rs`](fuzz/fuzz_targets/fuzz_grid_ops.rs)) |

### FR-008: Tab Stops

**Requirement**: The system SHALL support tab stops (set, clear, move).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Tab stop set (HTS), clear (TBC), and forward/backward tab (HT, CBT) move the cursor to the correct column | `cargo test --package torvox-terminal -- tabs` ([`torvox-terminal/tests/ported_kitty_full/tabs.rs`](torvox-terminal/tests/ported_kitty_full/tabs.rs)) |
| 2 | Default tab stops are set every 8 columns on terminal reset | Code review: `torvox-core/src/control.rs` initial tab width |

### FR-009: SIGWINCH on Terminal Resize

**Requirement**: The system SHALL report terminal size changes via SIGWINCH to
the child process.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Resizing the session triggers `SIGWINCH` to the child process group | `cargo test --package torvox-terminal -- lifecycle_test` ([`lifecycle_test.rs`](torvox-terminal/tests/lifecycle_test.rs)) |
| 2 | The child process receives updated terminal dimensions after resize | `cargo test --package torvox-terminal -- session_state_machine` ([`session_state_machine.rs`](torvox-terminal/tests/session_state_machine.rs)) |

---

## 2. Rendering Pipeline

### FR-010: GPU-Accelerated Terminal Rendering

**Requirement**: The system SHALL render the terminal grid using wgpu (Vulkan)
as the sole graphics backend. OpenGL and CPU software paths are not supported.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | wgpu instance and surface create successfully on a Vulkan-capable device | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |
| 2 | Headless wgpu rendering produces correct pixel output (OCR-confirmed) | `cargo test --package torvox-renderer -- text_ocr_test` ([`text_ocr_test.rs`](torvox-renderer/tests/text_ocr_test.rs)) |
| 3 | No OpenGL or CPU rendering dependencies present in `Cargo.toml` | Code review: no `opengl`, `glutin`, or `Canvas.drawText` references in production code |

### FR-011: Text Shaping and Glyph Rasterization

**Requirement**: The system SHALL shape text runs using `cosmic-text` and
rasterize glyphs using `swash`, caching results in a GPU atlas.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A text run shaped with `cosmic-text` produces correct glyph positions and clusters | `cargo test --package torvox-renderer -- font_metrics` ([`font_metrics.rs`](torvox-renderer/tests/font_metrics.rs)) |
| 2 | Rasterized glyphs render at the correct pixel dimensions | `cargo test --package torvox-renderer -- text_ocr_test` ([`text_ocr_test.rs`](torvox-renderer/tests/text_ocr_test.rs)) |
| 3 | Shaped results are cached and cache hits return identical glyph data | Code review: `torvox-renderer/src/font.rs` — shaped text cache with 4,096 entry cap |

### FR-012: GPU Texture Atlas Management

**Requirement**: The system SHALL pack glyph bitmaps into a GPU texture atlas
using `guillotiere` for dynamic rectangle allocation and eviction.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `guillotiere` allocates rectangles in the atlas texture for each glyph | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | Atlas evicts least-recently-used glyphs when capacity is reached | Code review: `torvox-renderer/src/gpu.rs` — atlas eviction path |
| 3 | Atlas allocation tracks at least 10,000 glyph entries | Code review: `torvox-renderer/src/font.rs` — atlas capacity constant |

### FR-013: Dirty Mask for Incremental Rendering

**Requirement**: The system SHALL maintain a dirty mask (`DirtyMask`) that tracks
which rows of the grid have changed and limit rendering to those rows.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Writing a character marks only the affected row as dirty | `cargo test --package torvox-core -- grid_ops` ([`torvox-core/tests/grid_ops.rs`](torvox-core/tests/grid_ops.rs)) |
| 2 | Property tests verify that dirty bits are set and cleared consistently | `cargo test --package torvox-core -- property_tests` ([`torvox-core/tests/property_tests.rs`](torvox-core/tests/property_tests.rs)) |
| 3 | A full-grid operation (e.g., clear screen) marks all visible rows dirty | Code review: `torvox-core/src/cell.rs` — `DirtyMask` API and `torvox-core/src/grid.rs` — row invalidation logic |

### FR-014: Cursor Rendering

**Requirement**: The system SHALL render a cell cursor (block, bar, underline,
beam) with configurable color and blink behavior.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Each cursor style (block, bar, underline, beam) is rendered as a distinct visual shape in the wgpu pipeline | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | Cursor color respects the configured theme cursor color | Code review: `torvox-core/src/config.rs` (`Theme.cursor`) and shader uniform path |
| 3 | Blink cursor toggles visibility at the configured rate | Code review: `torvox-renderer/src/gpu.rs` — blink timer integration |

### FR-015: Selection Rendering

**Requirement**: The system SHALL render text selection highlights (character,
word, line, block modes) as colored overlays on the affected cells.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Selection in character, word, line, and block mode produces highlights on the correct cells | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | Selection background color is configurable and defaults to the theme selection color | Code review: `torvox-core/src/config.rs` (`Theme.selection_foreground`, `Theme.selection_background`) |
| 3 | Empty selection renders no highlight overlay | Code review: `torvox-renderer/src/gpu.rs` — instance count is zero when selection is empty |

### FR-016: Font Configuration

**Requirement**: The system SHALL support font configuration: family, size, line
spacing, and fallback to preferred monospace fonts (Roboto Mono, JetBrains Mono,
etc.).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Changing `FontConfig.family` switches the rendered glyph set | `cargo test --package torvox-renderer -- font_metrics` ([`font_metrics.rs`](torvox-renderer/tests/font_metrics.rs)) |
| 2 | Font size changes produce proportionally scaled glyphs in the atlas | `cargo test --package torvox-terminal -- font_test` ([`font_test.rs`](torvox-terminal/tests/font_test.rs)) |
| 3 | Fallback chain loads a monospace font when the primary family is unavailable | Code review: `torvox-renderer/src/font.rs` — font fallback loading logic |

### FR-017: Theme-Based Color Rendering

**Requirement**: The system SHALL render the terminal background, foreground,
and 16-color ANSI palette from the active theme configuration.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Applying a theme sets the background and foreground rendered by wgpu | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | All 16 ANSI palette colors from the theme are used when rendering foreground/background | `cargo test --package torvox-core -- terminal_colors` ([`torvox-core/tests/terminal_colors.rs`](torvox-core/tests/terminal_colors.rs)) |
| 3 | Theme colors are applied uniformly across the entire terminal grid | Code review: `torvox-renderer/src/gpu.rs` — uniform buffer upload of theme colors |

### FR-018: GPU Surface Recovery

**Requirement**: The system SHALL recover from GPU surface destruction (e.g.,
Android activity restart) by recreating the render pipeline and continuing
without data loss.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | After surface destruction, the wgpu pipeline and swap chain are recreated without crashing | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |
| 2 | Terminal grid content is preserved across surface recreation (no data loss) | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |
| 3 | After 100 consecutive render errors, the render thread exits permanently | Code review: `torvox-renderer/src/gpu.rs` — error counter threshold check |

### FR-019: Kitty Graphics Protocol (KGP)

**Requirement**: The system SHALL support the Kitty Graphics Protocol (KGP) for
rendering inline images as textured quads.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | KGP sequences (`_Gi`) produce textured quads in the instance buffer | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | KGP images are rendered at the correct cell-aligned position and size | Code review: `torvox-renderer/src/gpu.rs` (`KgpInstance`) — instance positioning logic |
| 3 | KGP placeholder cells (SPACE with `KGP_IMAGE` attribute) are rendered as image areas | Code review: `torvox-core/src/cell.rs` — `CellFlags::KGP_IMAGE` bit |

---

## 3. Input Handling

### FR-020: Kitty Keyboard Protocol

**Requirement**: The system SHALL encode physical keyboard input using the Kitty
Keyboard Protocol (KBP) for extended modifier and key reporting.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Key presses with modifiers (Ctrl, Shift, Alt, Super) produce correct KBP escape sequences | `cargo test --package torvox-terminal -- session_roundtrip` ([`session_roundtrip.rs`](torvox-terminal/tests/session_roundtrip.rs)) |
| 2 | KBP disambiguation codes distinguish between modified and unmodified keys | Code review: `torvox-terminal/src/keyboard.rs` — event encoding logic |

### FR-021: IME Text Input (CJK)

**Requirement**: The system SHALL support IME (Input Method Editor) text input
for composing CJK and other complex characters, with `Composing` state
management.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Composing state transitions are tracked correctly in the terminal state | `cargo test --package torvox-core -- grapheme` ([`torvox-core/tests/grapheme.rs`](torvox-core/tests/grapheme.rs)) |
| 2 | CJK composed characters render at double-width cell positions | `cargo test --package torvox-core -- unicode_icu_conformance` ([`torvox-core/tests/unicode_icu_conformance.rs`](torvox-core/tests/unicode_icu_conformance.rs)) |
| 3 | Unicode grapheme clusters are segmented correctly for cursor movement | `cargo test --package torvox-core -- unicode_icu_conformance` ([`torvox-core/tests/unicode_icu_conformance.rs`](torvox-core/tests/unicode_icu_conformance.rs)) |

### FR-022: Selection Modes (Character, Word, Line, Block)

**Requirement**: The system SHALL support terminal selection in four modes:
character (`Char`), word (`Word`), line (`Line`), and block (`Block`).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Each selection mode returns the correct text span from the grid | `cargo test --package torvox-core -- selection_text` ([`torvox-core/tests/selection_text.rs`](torvox-core/tests/selection_text.rs)) |
| 2 | Selection round-trips through serialize/deserialize without data loss | `cargo test --package torvox-terminal -- selection_roundtrip` ([`selection_roundtrip.rs`](torvox-terminal/tests/selection_roundtrip.rs)) |
| 3 | Fuzz-generated selection operations do not panic | `cargo run --package fuzz -- fuzz_selection` ([`fuzz/fuzz_targets/fuzz_selection.rs`](fuzz/fuzz_targets/fuzz_selection.rs)) |

### FR-023: Word Boundary and URL Detection

**Requirement**: The system SHALL automatically expand word-mode selections to
word boundaries and detect URLs (`http://`, `https://`, `ftp://`, `www.`) for
URL-aware selection expansion.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Word-mode selection expands to word boundaries (alphanumeric contiguous spans) | `cargo test --package torvox-core -- selection_text` ([`torvox-core/tests/selection_text.rs`](torvox-core/tests/selection_text.rs)) |
| 2 | URL-like patterns (`http://`, `https://`, `ftp://`, `www.`) are detected and selection expands to the full URL | `cargo test --package torvox-core -- selection_integration` ([`torvox-core/tests/selection_integration.rs`](torvox-core/tests/selection_integration.rs)) |

### FR-024: Touch Input Gestures

**Requirement**: The system SHALL support touch input gestures: tap to place
cursor, long-press for selection handles, and swipe for scrollback navigation.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Tap gesture places the cursor at the touched cell position | `cd android && ./gradlew testDebugUnitTest` — `GestureInteractionTest` ([`android/app/src/test/java/io/torvox/ui/GestureInteractionTest.kt`](android/app/src/test/java/io/torvox/ui/GestureInteractionTest.kt)) |
| 2 | Long-press gesture initiates selection mode with visible handles | `cd android && ./gradlew testDebugUnitTest` — `TouchGestureTest` ([`android/app/src/test/java/io/torvox/ui/TouchGestureTest.kt`](android/app/src/test/java/io/torvox/ui/TouchGestureTest.kt)) |
| 3 | Swipe gesture scrolls the scrollback buffer | `cd android && ./gradlew connectedDebugAndroidTest` — `TouchGestureInstrumentedTest` ([`android/app/src/androidTest/java/io/torvox/ui/TouchGestureInstrumentedTest.kt`](android/app/src/androidTest/java/io/torvox/ui/TouchGestureInstrumentedTest.kt)) |

### FR-025: Backspace and Right-Alt Mode Configuration

**Requirement**: The system SHALL support configurable backspace mode (DEL
`0x7f` or BS `0x08`) and right-Alt mode (character modifier or meta).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `BackspaceMode::Del` sends `0x7f` on backspace key; `BackspaceMode::Bs` sends `0x08` | `cargo test --package torvox-core -- config_drift` ([`torvox-core/tests/config_drift.rs`](torvox-core/tests/config_drift.rs)) |
| 2 | `RightAltMode::Esc` sends `ESC + char`; `RightAltMode::Modifier` sends an 8-bit-modified character | `cargo test --package torvox-core -- config_integration` ([`torvox-core/tests/config_integration.rs`](torvox-core/tests/config_integration.rs)) |
| 3 | Configuration values default to `BackspaceMode::Del` and `RightAltMode::Esc` if unspecified | Code review: `torvox-core/src/config.rs` — `Default` impl for `TerminalConfig` |

---

## 4. Session Management

### FR-026: PTY Child Process Spawn

**Requirement**: The system SHALL spawn a child process (shell or custom
executable) connected to a pseudo-terminal (PTY) via `fork/exec`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A PTY pair is created and a child process is spawned with the PTY slave as its controlling terminal | `cargo test --package torvox-terminal -- lifecycle_test` ([`lifecycle_test.rs`](torvox-terminal/tests/lifecycle_test.rs)) |
| 2 | The child process receives input written to the PTY master and its output is readable from the master | `cargo test --package torvox-terminal -- bash_integration` ([`bash_integration.rs`](torvox-terminal/tests/bash_integration.rs)) |

### FR-027: Dedicated PTY Reader Thread

**Requirement**: The system SHALL read PTY output on a dedicated reader thread
and forward parsed output to the grid update pipeline via a `flume` channel.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A reader thread is spawned on session start and reads PTY master fd until EOF | `cargo test --package torvox-terminal -- concurrent_session` ([`concurrent_session.rs`](torvox-terminal/tests/concurrent_session.rs)) |
| 2 | Parsed terminal events are delivered via `flume` channel to the grid update handler | `cargo test --package torvox-terminal -- session_state_machine` ([`session_state_machine.rs`](torvox-terminal/tests/session_state_machine.rs)) |
| 3 | Concurrent session tests verify no data races or lost events on the channel | `cargo test --package torvox-terminal -- shuttle_concurrent` ([`shuttle_concurrent.rs`](torvox-terminal/tests/shuttle_concurrent.rs)) |

### FR-028: Process Waiter Thread

**Requirement**: The system SHALL wait for child process exit on a dedicated
waiter thread and emit a `ProcessExited` event on termination.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A waiter thread blocks on `waitpid` and detects child exit status | `cargo test --package torvox-terminal -- lifecycle_test` ([`lifecycle_test.rs`](torvox-terminal/tests/lifecycle_test.rs)) |
| 2 | A `ProcessExited` event with the correct exit code is emitted when the child terminates | `cargo test --package torvox-terminal -- dst_simulation` ([`dst_simulation.rs`](torvox-terminal/tests/dst_simulation.rs)) |

### FR-029: Session Resize

**Requirement**: The system SHALL support resizing a terminal session (changing
rows and columns) and forwarding the new size to the child process via
SIGWINCH.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Resizing the grid updates row and column counts and reflows content | `cargo test --package torvox-core -- grid_ops` ([`torvox-core/tests/grid_ops.rs`](torvox-core/tests/grid_ops.rs)) |
| 2 | Fuzz-generated resize operations do not panic or produce invalid grid states | `cargo run --package fuzz -- fuzz_grid_resize` ([`fuzz/fuzz_targets/fuzz_grid_resize.rs`](fuzz/fuzz_targets/fuzz_grid_resize.rs)) |
| 3 | After resize, SIGWINCH is delivered and the child sees updated `TIOCGWINSZ` | `cargo test --package torvox-terminal -- lifecycle_test` ([`lifecycle_test.rs`](torvox-terminal/tests/lifecycle_test.rs)) |

### FR-030: Bounded Scrollback Buffer

**Requirement**: The system SHALL maintain a bounded scrollback buffer with a
configurable maximum (default 50,000 lines), evicting oldest entries when the
limit is exceeded.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Scrollback buffer evicts the oldest line when `max_scrollback` is exceeded | `cargo test --package torvox-terminal -- memory_bounds` ([`memory_bounds.rs`](torvox-terminal/tests/memory_bounds.rs)) |
| 2 | Scrollback buffer count never exceeds the configured `max_scrollback` value | `cargo test --package torvox-terminal -- memory_bounds` ([`memory_bounds.rs`](torvox-terminal/tests/memory_bounds.rs)) |
| 3 | Scrollback survives grid resize operations without data loss | `cargo test --package torvox-core -- grid_snapshot_integration` ([`torvox-core/tests/grid_snapshot_integration.rs`](torvox-core/tests/grid_snapshot_integration.rs)) |

### FR-031: Scrollback Search

**Requirement**: The system SHALL support a scrollback search feature that finds
text matching a pattern (regex or literal) within the scrollback history.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Scrollback search returns matching lines with correct line numbers and column ranges | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | Regex patterns match correctly across line boundaries (no false negatives or panics) | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 3 | Search with no matches returns an empty result set | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |

### FR-032: Alternate Screen Scrollback Management

**Requirement**: The system SHALL clear the scrollback buffer when entering the
alternate screen and restore it on exit.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Scrollback is empty immediately after entering alternate screen | `cargo test --package torvox-terminal -- alt_screen` ([`alt_screen.rs`](torvox-terminal/tests/alt_screen.rs)) |
| 2 | Scrollback content is restored to its pre-alternate-screen state on exit | `cargo test --package torvox-terminal -- alt_screen` ([`alt_screen.rs`](torvox-terminal/tests/alt_screen.rs)) |

---

## 5. OSC Sequence Handling

### FR-033: OSC 7 — Current Working Directory

**Requirement**: The system SHALL intercept OSC 7 sequences
(`ESC ] 7 ; <uri> ST`) and extract the current working directory path as a
`CwdEvent`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Parsing `ESC ] 7 ; file://host/path ST` produces a `CwdEvent` with the correct path | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 2 | Fuzz-generated OSC 7 sequences produce valid `CwdEvent` values without panicking | `cargo run --package fuzz -- fuzz_osc_handler` ([`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |

### FR-034: OSC 8 — Hyperlinks

**Requirement**: The system SHALL intercept OSC 8 sequences
(`ESC ] 8 ; <params> ; <url> ST`) and extract hyperlink open/close events as
`HyperlinkEvent`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Parsing OSC 8 with a URL produces a `HyperlinkEvent::Open` with the correct URI | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 2 | Parsing OSC 8 without a URL produces a `HyperlinkEvent::Close` | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 3 | Fuzz-generated OSC 8 sequences parse without panicking | `cargo run --package fuzz -- fuzz_osc_handler` ([`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |

### FR-035: OSC 52 — Clipboard Access

**Requirement**: The system SHALL intercept OSC 52 sequences
(`ESC ] 52 ; <selection> ; <base64> ST`) and decode clipboard content as a
`ClipboardEvent`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | OSC 52 with base64-encoded text decodes to a `ClipboardEvent` with the correct plaintext | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 2 | OSC 52 with an empty payload produces a request event (clipboard read) | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 3 | Malformed base64 in OSC 52 is rejected without panicking | Code review: `torvox-terminal/src/osc_handler.rs` — base64 decode error handling |

### FR-036: OSC 9 / OSC 777 — Notifications

**Requirement**: The system SHALL intercept OSC 9 (iTerm2) and OSC 777 (rxvt)
sequences and extract notification title/body as `NotificationEvent`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | OSC 9 with title and body produces a `NotificationEvent` | `cargo run --package fuzz -- fuzz_osc_handler` ([`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |
| 2 | OSC 777 with semicolon-separated title;body produces a `NotificationEvent` | `cargo run --package fuzz -- fuzz_osc_handler` ([`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |

### FR-037: Unrecognised OSC Passthrough

**Requirement**: The system SHALL pass through unrecognised OSC sequences (e.g.,
OSC 0 for title, OSC 4 for palette change) to the VT parser unchanged.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Unrecognised OSC sequences (OSC 0, OSC 4) are forwarded to the VT parser without interception | `cargo test --package torvox-terminal` — `fuzz_osc_parse` test ([`fuzz/fuzz_targets/fuzz_osc_parse.rs`](fuzz/fuzz_targets/fuzz_osc_parse.rs)) |
| 2 | No data is dropped or corrupted when passthrough sequences contain printable ASCII | Code review: `torvox-terminal/src/osc_handler.rs` — passthrough branch |

### FR-038: Partial OSC Sequence Handling

**Requirement**: The system SHALL handle partial OSC sequences that arrive split
across multiple input chunks, accumulating state across `process()` calls.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | An OSC sequence split across two input chunks is correctly assembled and dispatched | `cargo run --package fuzz -- fuzz_osc_handler` ([`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |
| 2 | Fuzz-generated chunk boundaries do not cause incorrect OSC parsing | `cargo run --package fuzz -- fuzz_osc_parse` ([`fuzz/fuzz_targets/fuzz_osc_parse.rs`](fuzz/fuzz_targets/fuzz_osc_parse.rs)) |

---

## 6. Clipboard and Notifications

### FR-039: Copy to System Clipboard

**Requirement**: The system SHALL copy selected text to the system clipboard on
user request (e.g., copy action from selection).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A user-initiated copy action forwards the selected text to the clipboard bridge | `cargo test --package torvox-gui-android -- bridge_integration` ([`bridge_integration.rs`](torvox-gui-android/tests/bridge_integration.rs)) |
| 2 | Bridge round-trip tests verify that clipboard data survives Kotlin↔Rust serialization | `cargo test --package torvox-terminal -- bridge_roundtrip` ([`bridge_roundtrip.rs`](torvox-terminal/tests/bridge_roundtrip.rs)) |

### FR-040: OSC 52 Paste (Clipboard Read)

**Requirement**: The system SHALL read clipboard content when requested by
terminal applications via OSC 52 (paste).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | An OSC 52 query (empty payload) triggers a clipboard read and the result is forwarded to the PTY | `cargo test --package torvox-terminal -- osc52` ([`osc52.rs`](torvox-terminal/tests/osc52.rs)) |
| 2 | The clipboard content is base64-encoded before writing to the PTY as OSC 52 response | Code review: `torvox-terminal/src/osc_handler.rs` — clipboard response encoding |

### FR-041: Android Notifications via OSC

**Requirement**: The system SHALL display Android notifications for
terminal-emitted OSC 9/777 notification sequences.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | OSC 9 / OSC 777 sequences produce a `NotificationEvent` with extracted title and body | `cargo test --package torvox-terminal` (covered by [`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |
| 2 | Notification events are forwarded to the Android notification manager via the bridge | Code review: `torvox-gui-android/src/bridge.rs` — notification event handling path |

---

## 7. SSH/Mosh Connectivity

### FR-042: SSH/Mosh Executable

**Requirement**: The system SHALL provide an executable (`torvox-exec`) capable
of establishing SSH and Mosh connections.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `torvox-exec` parses SSH and Mosh connection arguments and invokes the correct binary | `cargo test --package torvox-exec -- basic` ([`torvox-exec/tests/basic.rs`](torvox-exec/tests/basic.rs)) |
| 2 | `torvox-exec --help` exits with code 0 and prints usage information | `cargo test --package torvox-exec -- basic` ([`torvox-exec/tests/basic.rs`](torvox-exec/tests/basic.rs)) |

### FR-043: SSH/Mosh Session Integration

**Requirement**: The system SHALL integrate SSH/Mosh sessions with the terminal
session lifecycle (PTY management, resize forwarding).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `torvox-exec` connects its child process to the PTY master fd | `cargo test --package torvox-exec -- basic` ([`torvox-exec/tests/basic.rs`](torvox-exec/tests/basic.rs)) |
| 2 | Terminal resize signals propagate through the PTY to the SSH/Mosh child process | Code review: `torvox-terminal/src/session.rs` — resize forwarding to session process |

---

## 8. MCP Server Integration

### FR-044: MCP Server over Unix Socket

**Requirement**: The system SHALL run an MCP (Model Context Protocol) server over
a Unix domain socket, communicating via JSON-RPC 2.0 with newline-delimited
JSON.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | MCP server binds to a Unix domain socket and accepts JSON-RPC 2.0 connections | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | Invalid JSON or non-JSON-RPC requests return standard JSON-RPC error responses | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |

### FR-045: Read-Only MCP Tools

**Requirement**: The MCP server SHALL expose tools for listing sessions, reading
grid state, reading scrollback, reading cursor position, and reading selected
text.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `list_sessions` tool returns a list of active terminal session IDs | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | `read_grid` tool returns the current terminal grid content for a session | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 3 | `read_scrollback`, `read_cursor`, and `read_selection` tools return correct state | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |

### FR-046: Write MCP Tools (Gated)

**Requirement**: The MCP server SHALL expose tools for writing to the PTY,
sending signals, resizing the terminal, and setting clipboard content (gated
behind `--mcp-allow-write`).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Without `--mcp-allow-write`, write tools return a permission-denied error | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | With `--mcp-allow-write`, `send_input` writes data to the PTY and `set_terminal_size` resizes the session | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |

### FR-047: Scrollback Search MCP Tool

**Requirement**: The MCP server SHALL expose a scrollback search tool that
matches a regex pattern and returns matching line numbers, text, and column
ranges.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `scrollback_search` with a valid regex returns matching lines with line numbers and column ranges | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | `scrollback_search` with an invalid regex returns an error, not a panic | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 3 | `SearchMatch` structure contains `line_number`, `text`, and `columns` fields | Code review: `torvox-mcp/src/lib.rs` — `SearchMatch` struct definition |

### FR-048: Input Queue Automation

**Requirement**: The MCP server SHALL expose an input queue mechanism that
watches for a prompt pattern in scrollback and automatically injects queued
text (AI agent automation).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `watch_prompt` registers a prompt pattern and triggers when matched in scrollback | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 2 | Queued input is written to the PTY after the prompt pattern is detected | `cargo test --package torvox-mcp` (inline tests in [`torvox-mcp/src/lib.rs`](torvox-mcp/src/lib.rs)) |
| 3 | Input queue is cleared after injection and stops watching | Code review: `torvox-mcp/src/lib.rs` — `InputQueue` lifecycle |

---

## 9. Android Bridge

### FR-049: boltffi Bridge Types

**Requirement**: The system SHALL bridge Rust terminal state to Kotlin using
boltffi data types (`BridgeCell`, `BridgeAttrs`, `BridgeGrid`) mapped over JNA.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `BridgeCell`, `BridgeAttrs`, and `BridgeGrid` are exported from `torvox-gui-android/src/bridge.rs` | Code review: `torvox-gui-android/src/bridge.rs` — type definitions |
| 2 | JNA mappings in `TorvoxBridge.kt` match the Rust boltffi structures bit-for-bit | `cargo test --package torvox-gui-android -- ffi_contract_tests` ([`ffi_contract_tests.rs`](torvox-gui-android/tests/ffi_contract_tests.rs)) |
| 3 | Bridge round-trip tests verify data integrity across the Rust/Kotlin boundary | `cargo test --package torvox-gui-android -- bridge_integration` ([`bridge_integration.rs`](torvox-gui-android/tests/bridge_integration.rs)) |

### FR-050: rkyv Snapshot Synchronization

**Requirement**: The system SHALL synchronize the terminal grid, cursor,
selection, and scrollback to the Kotlin UI layer via serialized snapshots
(rkyv format).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Grid, cursor, selection, and scrollback are serialized to rkyv format without data loss | `cargo test --package torvox-core -- grid_snapshot_integration` ([`torvox-core/tests/grid_snapshot_integration.rs`](torvox-core/tests/grid_snapshot_integration.rs)) |
| 2 | A comprehensive snapshot (all grid states) round-trips through rkyv serialization | `cargo test --package torvox-terminal -- snapshot_comprehensive` ([`snapshot_comprehensive.rs`](torvox-terminal/tests/snapshot_comprehensive.rs)) |
| 3 | Fuzz-generated wire data deserializes without panicking | `cargo test --package torvox-gui-android -- fuzz_wire` ([`fuzz_wire.rs`](torvox-gui-android/tests/fuzz_wire.rs)) |

### FR-051: JNI for NDK Functions

**Requirement**: The system SHALL use JNI for NDK-level functions (ANativeWindow
lifecycle, surface creation/destruction) via `jni_bridge.rs`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | JNI functions registered in `jni_bridge.rs` match the expected native method signatures in Kotlin | `cargo test --package torvox-gui-android -- ffi_contract_tests` ([`ffi_contract_tests.rs`](torvox-gui-android/tests/ffi_contract_tests.rs)) |
| 2 | `ANativeWindow_fromSurface` and `ANativeWindow_release` are called in the correct lifecycle order | Code review: `torvox-gui-android/src/jni_bridge.rs` — surface lifecycle pairing |

### FR-052: Surface Lifecycle Management

**Requirement**: The system SHALL handle Android surface creation and
destruction events, recreating the wgpu surface and render pipeline as needed.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Surface creation triggers wgpu surface and render pipeline initialization | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |
| 2 | Surface destruction triggers wgpu surface teardown without leaving dangling GPU resources | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |

### FR-053: ProGuard/R8 Compatibility

**Requirement**: The system SHALL support ProGuard/R8 obfuscation with
`-dontoptimize` to preserve JNA reflection-based binding.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Release APK builds succeeds with `-dontoptimize` in ProGuard rules | `cd android && ./gradlew assembleDebug` — verifies build succeeds |
| 2 | JNA binding classes are not obfuscated by R8 (retained by ProGuard rules) | Code review: `android/app/proguard-rules.pro` — `-keep` rules for JNA types |

---

## 10. Configuration and Themes

### FR-054: 16 Built-In Color Themes

**Requirement**: The system SHALL provide 16 built-in color themes: Catppuccin
Mocha, Catppuccin Latte, Dracula+, Nord, Tokyo Night, Rose Pine, Gruvbox Dark,
Gruvbox Light, Everforest Dark, One Dark, One Light, Monokai, Ayu Dark, Ayu
Light, Kanagawa Wave, and Night Owl.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `Theme::all_built_in()` returns exactly 16 themes with the expected names | `cargo test --package torvox-core -- config_integration` ([`torvox-core/tests/config_integration.rs`](torvox-core/tests/config_integration.rs)) |
| 2 | Each built-in theme has non-default values for all 16 ANSI color slots | `cargo test --package torvox-core -- config_drift` ([`torvox-core/tests/config_drift.rs`](torvox-core/tests/config_drift.rs)) |
| 3 | Built-in themes are accessible from the Kotlin UI via the bridge | `cd android && ./gradlew testDebugUnitTest` — `BuiltInThemeTest` ([`android/app/src/test/java/io/torvox/ui/theme/BuiltInThemeTest.kt`](android/app/src/test/java/io/torvox/ui/theme/BuiltInThemeTest.kt)) |

### FR-055: Custom Theme via TOML

**Requirement**: The system SHALL support custom theme definition via TOML with
fields for name, background, foreground, cursor, selection background, and 16
ANSI color slots.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A TOML file with valid theme fields parses into a `Theme` struct with correct values | `cargo test --package torvox-core -- config_integration` ([`torvox-core/tests/config_integration.rs`](torvox-core/tests/config_integration.rs)) |
| 2 | A TOML file with missing required fields produces a parse error | `cargo test --package torvox-core -- config_drift` ([`torvox-core/tests/config_drift.rs`](torvox-core/tests/config_drift.rs)) |
| 3 | Custom themes are applied to rendering and produce correct color output | Code review: `torvox-core/src/config.rs` — `Theme::parse_custom()` and theme application path |

### FR-056: Terminal Configuration

**Requirement**: The system SHALL support configuration of terminal dimensions
(rows, cols), scrollback size, shell path, font size, backspace mode, and
right-Alt mode via `TerminalConfig`.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | All `TerminalConfig` fields have sensible defaults and can be overridden | `cargo test --package torvox-core -- config_integration` ([`torvox-core/tests/config_integration.rs`](torvox-core/tests/config_integration.rs)) |
| 2 | Changing `rows`/`cols` changes the initial terminal grid dimensions | `cargo test --package torvox-core -- config_drift` ([`torvox-core/tests/config_drift.rs`](torvox-core/tests/config_drift.rs)) |
| 3 | `shell_path` overrides the default shell binary spawned in the PTY | Code review: `torvox-terminal/src/session.rs` — shell path resolution from config |

---

## 11. Non-Functional: Safety

### NFR-001: Zero Unsafe in torvox-core

**Requirement**: `torvox-core` SHALL contain zero `unsafe` blocks. The build MUST
fail if `cargo geiger --package torvox-core` reports any `unsafe` usage.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `cargo geiger --package torvox-core` reports zero unsafe blocks | `cargo geiger --package torvox-core` (enforced in CI by `torvox-integration-tests/tests/tool_lint.rs`) |
| 2 | CI pipeline rejects any PR that introduces `unsafe` in `torvox-core` | Code review: `scripts/check-rust.nu` — gate on geiger output |

### NFR-002: SAFETY Comments on Unsafe

**Requirement**: All `unsafe` blocks in the codebase (confined to
`torvox-terminal/src/pty.rs` for `fork/exec` and FFI boundary code) SHALL be
preceded by a `// SAFETY:` comment explaining the invariants.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Every `unsafe` block in the workspace has a preceding `// SAFETY:` comment | `cargo geiger --all` + code review (enforced in CI by `torvox-integration-tests/tests/tool_lint.rs`) |
| 2 | Unsafe blocks exist only in `torvox-terminal/src/pty.rs` and FFI boundary code | `cargo geiger --all` — non-zero unsafe count only in permitted crates |

### NFR-003: No Panic in Error Paths

**Requirement**: The system SHALL not panic in error paths. Library functions
SHALL return `Result` or `Option` rather than panicking.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | All `fn` signatures in library crates use `Result` or `Option` for fallible operations | `cargo test --workspace` — no panic-related test failures |
| 2 | CI detects any `unwrap()`, `expect()`, or `panic!()` in library code that is not in test-only blocks | Code review and Clippy lint (`clippy::unwrap_used`) |

### NFR-004: thiserror, Not anyhow

**Requirement**: The system SHALL use `thiserror 2` (not `anyhow`) for error
types in library crates.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | No library crate depends on `anyhow` | `cargo tree --no-default-features` or `cargo machete` with verify; CI enforces via `torvox-integration-tests/tests/tool_lint.rs` |
| 2 | Error types in library crates derive `thiserror::Error` | Code review: per-crate `error.rs` or inline error enum definitions |

### NFR-005: Thread Panic Containment

**Requirement**: The system SHALL handle thread panics gracefully: the PTY
reader thread, process waiter thread, and render thread SHALL NOT bring down
the entire process on panic.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A panic in the reader thread is caught and logged without crashing the process | `cargo test --package torvox-terminal -- dst_simulation` ([`dst_simulation.rs`](torvox-terminal/tests/dst_simulation.rs)) |
| 2 | Concurrent session tests verify that thread isolation works under stress | `cargo test --package torvox-terminal -- shuttle_concurrent` ([`shuttle_concurrent.rs`](torvox-terminal/tests/shuttle_concurrent.rs)) |

---

## 12. Non-Functional: Performance

### NFR-006: GPU-Only Rendering

**Requirement**: The render thread SHALL use wgpu (Vulkan) for GPU-accelerated
rendering. Software rendering via CPU text drawing (`Canvas.drawText`) is
forbidden.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Headless wgpu render tests pass without a display server | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | No `android.graphics.Canvas.drawText` call exists in production rendering code | Code review: no `Canvas` reference in `torvox-renderer/src/` |
| 3 | Atomic OCR test confirms GPU-rendered output matches expected text | `cargo test --package torvox-renderer -- text_ocr_test` ([`text_ocr_test.rs`](torvox-renderer/tests/text_ocr_test.rs)) |

### NFR-007: Glyph Atlas Capacity

**Requirement**: The glyph atlas SHALL be managed by `guillotiere` with a cache
capacity of at least 10,000 glyph entries and eviction when full.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Atlas capacity constant is ≥ 10,000 | Code review: `torvox-renderer/src/font.rs` — atlas capacity constant |
| 2 | Glyph eviction does not cause visible rendering artifacts | `cargo test --package torvox-renderer -- font_metrics` ([`font_metrics.rs`](torvox-renderer/tests/font_metrics.rs)) |

### NFR-008: Bounded Scrollback Memory

**Requirement**: The scrollback buffer SHALL be bounded to a configurable
maximum (default 50,000 lines) with automatic eviction of oldest entries.
SHALL NOT exhibit unbounded memory growth.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Scrollback memory usage stabilises after reaching `max_scrollback` lines | `cargo test --package torvox-terminal -- memory_bounds` ([`memory_bounds.rs`](torvox-terminal/tests/memory_bounds.rs)) |
| 2 | Eviction fires when the buffer exceeds `max_scrollback` | `cargo test --package torvox-terminal -- memory_bounds` ([`memory_bounds.rs`](torvox-terminal/tests/memory_bounds.rs)) |

### NFR-009: Bounded Thread Count

**Requirement**: Each terminal session SHALL use a bounded number of threads
(6–7): PTY reader, process waiter, render thread, plus shared threads.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | A single session spawns exactly 3 session-specific threads (reader, waiter, render) | `cargo test --package torvox-terminal -- session_state_machine` ([`session_state_machine.rs`](torvox-terminal/tests/session_state_machine.rs)) |
| 2 | Concurrent session tests verify thread counts do not leak across sessions | `cargo test --package torvox-terminal -- concurrent_session` ([`concurrent_session.rs`](torvox-terminal/tests/concurrent_session.rs)) |

### NFR-010: Dirty Row-Only Repaint

**Requirement**: The frame pipeline SHALL only repaint dirty rows as tracked by
the `DirtyMask` bitfield, avoiding full-grid redraws on every frame.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Property tests verify that clean rows produce zero dirty bits | `cargo test --package torvox-core -- property_tests` ([`torvox-core/tests/property_tests.rs`](torvox-core/tests/property_tests.rs)) |
| 2 | After rendering a frame, the dirty mask is cleared for all processed rows | Code review: `torvox-renderer/src/gpu.rs` — dirty mask handshake |

### NFR-011: Shaped Text Cache Cap

**Requirement**: The shaped text cache SHALL be capped at 4,096 entries to avoid
unbounded memory growth from repeated shaping of different text runs.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Shaped text cache capacity is set to 4,096 | Code review: `torvox-renderer/src/font.rs` — cache capacity constant |
| 2 | Cache eviction does not cause incorrect glyph rendering | `cargo test --package torvox-renderer -- font_metrics` ([`font_metrics.rs`](torvox-renderer/tests/font_metrics.rs)) |

---

## 13. Non-Functional: Maintainability

### NFR-012: One-Way Crate Dependencies

**Requirement**: The crate dependency graph SHALL be strictly one-way with no
circular dependencies. The build SHALL fail on cycle detection.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `cargo metadata --no-deps --format-version 1` confirms the directional crate graph | `cargo test --workspace` (enforced in CI by `torvox-integration-tests/tests/tool_lint.rs`) |
| 2 | Kotlin layer dependency test verifies Android module depends only on the bridge | `cd android && ./gradlew testDebugUnitTest` — `LayerDependencyTest` ([`android/app/src/test/java/io/torvox/architecture/LayerDependencyTest.kt`](android/app/src/test/java/io/torvox/architecture/LayerDependencyTest.kt)) |

### NFR-013: Clippy Clean

**Requirement**: The codebase SHALL pass `cargo clippy --all -- --deny warnings`
with zero warnings. No `#[allow]` attributes in production source code.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `cargo clippy --all -- --deny warnings` exits with code 0 | `cargo clippy --all -- --deny warnings` (enforced in CI by `torvox-integration-tests/tests/tool_lint.rs`) |
| 2 | No `#[allow]` attribute exists in any `src/` file (test helpers excepted) | Code review: `grep -r '#\[allow' --include='*.rs'` on `src/` directories |

### NFR-014: Formatting Consistency

**Requirement**: The codebase SHALL pass `cargo fmt --check` with consistent
formatting.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `cargo fmt --check` exits with code 0 (no unformatted files) | `cargo fmt --check` (enforced in CI by `torvox-integration-tests/tests/tool_lint.rs`) |

### NFR-015: Kotlin Lint and Format

**Requirement**: The Kotlin codebase SHALL pass `./gradlew spotlessCheck detekt`
with zero violations.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `./gradlew spotlessCheck` exits with code 0 (no formatting violations) | `cd android && ./gradlew spotlessCheck detekt` (CI: `scripts/test-android-gradle.nu`) |
| 2 | `./gradlew detekt` exits with code 0 (no lint violations) | `cd android && ./gradlew detekt` |

### NFR-016: Bridge Type Synchronization

**Requirement**: When `torvox-core` types change, the bridge types in
`torvox-gui-android/src/bridge.rs` and `TorvoxBridge.kt` SHALL be updated
correspondingly.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | FFI contract tests verify Rust bridge types match Kotlin JNA mappings | `cargo test --package torvox-gui-android -- ffi_contract_tests` ([`ffi_contract_tests.rs`](torvox-gui-android/tests/ffi_contract_tests.rs)) |
| 2 | Bridge round-trip tests verify no data corruption across the FFI boundary | `cargo test --package torvox-gui-android -- bridge_integration` ([`bridge_integration.rs`](torvox-gui-android/tests/bridge_integration.rs)) |

---

## 14. Non-Functional: Compatibility

### NFR-017: Android Platform Target

**Requirement**: The system SHALL target Android as the primary platform, using
Kotlin + Compose for the UI layer.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `./gradlew assembleDebug` produces a valid debug APK | `cd android && ./gradlew assembleDebug` |
| 2 | Instrumented UI tests pass on an Android device or emulator | `cd android && ./gradlew connectedDebugAndroidTest` |

### NFR-018: Vulkan via wgpu (Software Fallback)

**Requirement**: The system SHALL use Vulkan via wgpu for rendering. On systems
without a physical GPU, Mesa's Lavapipe SHALL be used. On Android emulators,
SwiftShader SHALL be used.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Headless wgpu tests pass with `VK_ICD_FILENAMES` pointing to Lavapipe | `cargo test --package torvox-renderer -- gpu_headless_test` ([`gpu_headless_test.rs`](torvox-renderer/tests/gpu_headless_test.rs)) |
| 2 | Emulator test boot verifies SwiftShader Vulkan ICD is loaded | Code review: `scripts/setup-emulator.nu` — SwiftShader configuration |

### NFR-019: Deterministic Nix Build

**Requirement**: The build SHALL be deterministic via Nix flake, pinning all
dependencies including the Zig compiler (for Ghostty), Rust toolchain, and
Android SDK.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `nix flake check` validates the flake without evaluation errors | `nix flake check` |
| 2 | Lock file (`flake.lock`) is committed and contains pinned versions for all inputs | Code review: `flake.lock` is tracked in git and updated only via `nix flake update` |

### NFR-020: Ghostty Dynamic Library Linking

**Requirement**: The Ghostty library (libghostty-vt) SHALL be linked as a dynamic
library (dylib) with the SONAME versioned suffix stripped for Android
compatibility.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `libghostty-vt.so` in the APK lib directory has no versioned SONAME | `nu scripts/check-rust.nu` — SONAME check (CI: `scripts/check-rust.nu`) |
| 2 | `readelf -d` on the built `.so` shows `SONAME` as `libghostty-vt.so` (no `.0` suffix) | Manual verification on APK extraction |

### NFR-021: Application ID and Signing

**Requirement**: The APK SHALL use the application ID `com.termux` and SHALL be
signed with the AOSP testkey (not self-signed certificates).

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `android/app/build.gradle` sets `applicationId = "com.termux"` | Code review: `android/app/build.gradle` — `applicationId` field |
| 2 | APK is signed with AOSP testkey (`testkey.x509.pem` / `testkey.pk8`) | Code review: `scripts/fetch-aosp-testkey.nu` — key download and signing config |
| 3 | Release APK build succeeds with the testkey | `cd android && ./gradlew assembleDebug` |

---

## 15. Non-Functional: Reliability

### NFR-022: Render Thread Surface Recovery

**Requirement**: The render thread SHALL detect GPU surface loss and recreate the
wgpu pipeline automatically. After 100 consecutive errors (~10 seconds), the
thread SHALL exit permanently and require a new surface to restart.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Surface recreation is triggered on `wgpu::SurfaceError::Lost` and completes without crash | `cargo test --package torvox-gui-android -- gpu_noop_tests` ([`gpu_noop_tests.rs`](torvox-gui-android/tests/gpu_noop_tests.rs)) |
| 2 | After 100 consecutive render errors, the render thread exits with a terminal error | Code review: `torvox-renderer/src/gpu.rs` — error counter and thread exit logic |

### NFR-023: OSC Payload Size Limit

**Requirement**: The OSC handler SHALL cap payload size at 1 MB
(`MAX_PAYLOAD_BYTES`) to prevent denial-of-service via oversized OSC sequences.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | OSC sequences with payload > 1 MB are truncated or rejected | `cargo test --package torvox-terminal` (via fuzz in [`fuzz/fuzz_targets/fuzz_osc_handler.rs`](fuzz/fuzz_targets/fuzz_osc_handler.rs)) |
| 2 | `MAX_PAYLOAD_BYTES` constant is defined and equals 1,048,576 (1 MB) | Code review: `torvox-terminal/src/osc_handler.rs` — `MAX_PAYLOAD_BYTES` constant |

### NFR-024: PTY Read Error Recovery

**Requirement**: The system SHALL recover from PTY read errors without crashing
the session. The reader thread SHALL log errors and continue reading.

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | Simulated PTY read errors do not terminate the session or crash the process | `cargo test --package torvox-terminal -- dst_simulation` ([`dst_simulation.rs`](torvox-terminal/tests/dst_simulation.rs)) |
| 2 | Concurrent session stress tests verify robustness against I/O errors | `cargo test --package torvox-terminal -- concurrent_session` ([`concurrent_session.rs`](torvox-terminal/tests/concurrent_session.rs)) |

---

## Verification Environment Summary

| Environment | Command | Used For |
|-------------|---------|----------|
| Rust workspace | `cargo test --workspace` | Full Rust test suite |
| Core crate | `cargo test --package torvox-core` | Data model, grid, selection, config, IME |
| Terminal crate | `cargo test --package torvox-terminal` | VT parsing, PTY, sessions, OSC, clipboard, alt screen |
| Renderer crate | `cargo test --package torvox-renderer` | wgpu pipeline, font atlas, GPU rendering |
| GUI crate | `cargo test --package torvox-gui-android` | Bridge, JNI, surface lifecycle, fuzz wire |
| MCP crate | `cargo test --package torvox-mcp` | MCP server tools, scrollback search, input queue |
| Exec crate | `cargo test --package torvox-exec` | SSH/Mosh executable |
| Integration | `cargo test --package torvox-integration-tests` | Tool lint, render smoke tests |
| Kotlin unit | `cd android && ./gradlew testDebugUnitTest` | Gesture tests, theme tests, layer tests |
| Kotlin lint | `cd android && ./gradlew spotlessCheck detekt` | Formatting and static analysis |
| Android build | `cd android && ./gradlew assembleDebug` | APK compilation |
| Instrumented | `cd android && ./gradlew connectedDebugAndroidTest` | Device/emulator UI tests |
| Lint | `cargo clippy --all -- --deny warnings` | Static analysis |
| Format | `cargo fmt --check` | Code formatting |
| Safety | `cargo geiger --package torvox-core` | Unsafe block audit |
| Nix | `nix flake check` | Flake validation |
| Rust CI | `nu scripts/check-rust.nu` | Combined Rust checks (clippy, fmt, test, audit, machete) |
| Android CI | `cd android && ./gradlew spotlessCheck detekt` | Kotlin quality gate |
