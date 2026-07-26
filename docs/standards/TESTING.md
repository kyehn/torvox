# Testing Guide

## Principles

- Tests are specs — no test means no spec
- Only test public API
- One test equals one behavior
- No flaky tests — use deterministic synchronization

## Rust Tests

```bash
cargo nextest run --workspace --profile ci            # Rust tests
cargo nextest --package terminal-core                 # core only
cargo nextest --package terminal-engine             # terminal only
cargo nextest run --package terminal-core --test property_tests
```

## Test File Locations

| Crate | Integration Tests |
|-------|------------------|
| terminal-core | `tests/property_tests.rs` (quickcheck), `tests/grid_ops.rs`, `tests/terminal_colors.rs`, `tests/config_drift.rs`, `tests/grapheme.rs`, `tests/unicode_icu_conformance.rs` |
| terminal-engine | `tests/fuzz_vt_structured.rs`, `tests/grid_state_machine.rs`, `tests/session_state_machine.rs`, `tests/concurrent_session.rs`, `tests/dst_simulation.rs`, `tests/memory_bounds.rs`, `tests/shuttle_concurrent.rs`, `tests/ecma48_correctness.rs`, `tests/vttest_sequences.rs`, `tests/osc52.rs`, `tests/layout.rs`, `tests/ref_snapshot.rs`, `tests/fuzz_replay.rs`, `tests/cross_backend.rs`, `tests/ported_alacritty_ref.rs`, `tests/vttest_ref_files.rs`, `tests/proptest_csi.rs` (CSI cursor/scroll/erase), `tests/sgr_proptest.rs` (SGR attribute params) |
| android-gui | `tests/fuzz_wire.rs`, `tests/bridge_integration.rs`, `tests/bridge_safety.rs`, `tests/gpu_noop_tests.rs` |
| benchmarks | `benches/terminal_bench.rs` (criterion) |
| exec-bin | `tests/basic.rs` |

## Property and Fuzz Testing

- `tc()` helper for color test construction
- Color tolerance: `COLOR_TOLERANCE = 5.0 / 255.0`
- VtSegment: Text, Csi, Esc, Osc, Control, PrivateCsi, DecPrivate, Sgr, Dcs
- Grid state machine: WriteChar, Newline, Backspace, CursorUp/Down/Left/Right, CarriageReturn, Tab, ClearLine, ClearScreen, InsertLines, DeleteLines, ScrollUp, Resize, AlternateBuffer, SetOriginMode, ScrollRegion, OriginMode, InsertMode, ReverseIndex — ModelGrid vs real Grid
- DST simulation: PtyOutput, UserInput, Resize, Render, SurfaceCreated, SurfaceDestroyed, Flush, WriteText — 100K ops, 10 seeds
- Shuttle concurrency: nightly-only, enable via `RUSTFLAGS="--cfg shuttle_tests" cargo +nightly test -p terminal-engine`
- Structured VT fuzz: `cargo fuzz run fuzz_vt_structured` (6 target types, 20s each)
- Wire format fuzz: `cargo fuzz run fuzz_wire`

## Android Tests

```bash
cd android && ./gradlew testDebugUnitTest            # unit tests
cd android && ./gradlew roborazziDebug                # screenshot tests
cd android && ./gradlew connectedDebugAndroidTest     # instrumented
```

### Six test types and where each lives

torvox verifies Android behavior with six distinct test types. Use the
right type for the behavior under test — do not collapse them into one.

| # | Type | Location | What it covers |
|---|------|----------|----------------|
| 1 | **Unit** (Rust) | `terminal-core/tests/`, `terminal-engine/tests/`, `android-gui/tests/`, `benchmarks/benches/` | Pure logic: VT parse, grid/scrollback, OSC, keyboard encode, bridge round-trip. Runs on host via `cargo nextest`. |
| 2 | **Roborazzi** (screenshot) | `android/app/src/test/java/io/torvox/screenshot/*ScreenshotTest.kt`; goldens in `android/app/src/test/resources/roborazzi/` | Pixel-exact Compose/UI rendering under Robolectric. |
| 3 | **Compose UI** | `android/app/src/test/java/io/torvox/ui/*ComposeTest.kt` (Robolectric) and `android/app/src/androidTest/java/io/torvox/ui/*ComposeTest.kt` (instrumented) | Compose widget state/interaction (theme switch, selection handles). |
| 4 | **Maestro** | `android/app/src/androidTest/java/io/torvox/ui/*.yaml` flow files (e.g. `SelectionMaestroTest.yaml`) | End-to-end on-device flows driven by Maestro YAML. |
| 5 | **Android UI testing framework** | `android/app/src/androidTest/java/io/torvox/ui/*UiAutomatorTest.kt` (e.g. `TerminalUiAutomatorTest`, `SelectionUiAutomatorTest`, `TextSearchUiAutomatorTest`) | Cross-app / system-level interaction via UiAutomator. |
| 6 | **Espresso** | `android/app/src/androidTest/java/io/torvox/ui/*EspressoTest.kt` (e.g. `TerminalActivityEspressoTest`, `SelectionEspressoTest`, `TextSearchEspressoTest`) | In-app View-level interaction via Espresso. |

### Roborazzi Golden Management

Golden images live in `android/app/src/test/resources/roborazzi/` and are committed to git.

- **Script runner**: `nu scripts/test-android-gradle.nu`

CI fails on golden mismatch. Download `gradle-reports` artifact from the failed run

### RapidOCR Text Verification

RapidOCR (via `rapidocr-onnxruntime`) is available in the dev shell for OCR-verifying screenshots on Linux.

Used by `gpu-renderer/tests/text_ocr_test.rs` to verify font rendering end-to-end: renders text with swash, saves PNG, OCR-verifies the output.

## Emulator Tests

```bash
nu scripts/test-emulator.nu                         # automated emulator tests
```

---

## Traceability

### Requirement-to-Test Mapping

Every functional requirement (FR-xxx) and non-functional requirement (NFR-xxx) in
`docs/srs.md` must be traceable to at least one test. The traceability matrix is
maintained in `docs/traceability.yml`.

### Verification Methods

| Method | Description | CI Command |
|--------|-------------|------------|
| **unit** | Rust unit/integration test | `cargo nextest run --workspace --profile ci` |
| **doctest** | Rust doc-test (executable examples in `///` comments) | `cargo test --doc` |
| **property** | Property-based test (proptest/quickcheck) | `cargo nextest run --package terminal-core --test property_tests` |
| **fuzz** | Fuzz target | `cargo fuzz run <target> -- -max_total_time=5` |
| **lint** | Lint/static analysis check | `cargo clippy --all -- --deny warnings` |
| **android-unit** | Android unit test (Robolectric) | `./gradlew testDebugUnitTest` |
| **screenshot** | Roborazzi screenshot test | `./gradlew roborazziDebug` |
| **instrumented** | Android instrumented test | `./gradlew connectedDebugAndroidTest` |
| **maestro** | Maestro E2E flow | `maestro test <flow.yaml>` |
| **ui-automator** | UiAutomator cross-app test | Via instrumented test suite |
| **espresso** | Espresso in-app interaction test | Via instrumented test suite |
| **emulator** | Full emulator E2E test | `nu scripts/test-emulator.nu` |
| **tool-lint** | External tool quality check | `cargo test -p integration-tests --test tool_lint` |
| **docs-validate** | Documentation structural validation | `cargo test -p integration-tests --test tool_lint -- docs_*` |

### Adding Tests for New Requirements

When adding a new requirement to `docs/srs.md`:

1. Determine which verification method(s) apply
2. Add or update test(s) in the appropriate test directory
3. Update `docs/traceability.yml` with the new requirement-to-test mapping
4. Run the relevant test command and confirm it passes

### SRS ID Checks

The following structural checks ensure traceability integrity:

- Every `FR-\d{3}` / `NFR-\d{3}` in `docs/srs.md` follows the format
- Every referenced requirement in `docs/traceability.yml` exists in `docs/srs.md`
- Every acceptance criterion in `docs/acceptance.md` references a valid requirement ID
- Every ADR in `docs/adr/` references at least one requirement ID

These checks run as part of `tool_lint.rs` (see `cargo test -p integration-tests --test tool_lint`).
