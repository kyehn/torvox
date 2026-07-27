# Testing Guide

## Principles

- Tests are specs — no test means no spec
- Only test public API
- One test equals one behavior
- No flaky tests — use deterministic synchronization

## Rust Tests

```bash
cargo nextest run --workspace --profile ci            # all Rust tests
cargo nextest --package native                         # native crate only
cargo nextest run --package native --test property_tests
```

## Test File Locations

All Rust code lives in the single `native` crate, with tests inside each module:

| Module | Tests (in-source) | Integration Tests |
|--------|-------------------|-------------------|
| `native/src/terminal/ghostty_terminal/` | `tests.rs` — grid ops, cell iterator, CellData, snapshot, config | — |
| `native/src/render/` | `tests.rs` — GPU headless, font, shader validation, OCR, screenshot | — |
| `native/src/android/` | (tests guarded by `#[cfg(target_os = "android")]`) | — |
| `native/src/mcp.rs` | Inline tests — tool listing, tool calls | — |
| native crate | — | `native/tests/` (if any) |
| `exec-bin` | — | `tests/basic.rs` |

### Property and Fuzz Testing

Property-based testing (proptest/quickcheck) and structured fuzz targets were removed with the fuzz/ workspace deletion in Phase 3.

All testing now uses Rust unit tests (`cargo test`) and integration tests (`cargo test -p integration-tests`).

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
| 1 | **Unit** (Rust) | `native/src/terminal/`, `native/src/render/`, `native/src/mcp.rs` | Pure logic: VT parse, grid/scrollback, OSC, keyboard encode, MCP. Runs on host via `cargo nextest`. |
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

Used by `native/src/render/tests.rs` to verify font rendering end-to-end: renders text with swash, saves PNG, OCR-verifies the output.

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
| **property** | Property-based test (proptest/quickcheck) | (removed — see fuzz/ deletion) |
| **fuzz** | Fuzz target | (removed — see fuzz/ deletion) |
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
