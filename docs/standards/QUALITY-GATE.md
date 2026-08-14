# Quality Gate

## Pre-commit

Ensure git hooks are installed and configured.

```bash
cargo nextest run --workspace --profile ci            # all tests pass
cargo clippy --all -- --deny warnings                # zero lint warnings
cargo fmt --check                                   # formatting clean
nu scripts/check-rust.nu
```

### Property Tests

```bash
cargo nextest run --package native --test property_tests
cargo mutants --timeout 30                          # mutation score
```

## Android Verification

```bash
cd android && ./gradlew spotlessCheck detekt         # Kotlin style and static analysis
cd android && ./gradlew testDebugUnitTest                           # unit tests
cd android && ./gradlew lint                                       # Android lint
```

## Bridge Changes

When modifying JNI exports or `NativeBridge.kt`:

1. Ensure `System.loadLibrary("native")` in `NativeBridge.kt` matches the cdylib name
2. Verify `#[no_mangle] extern "system"` function signatures match Kotlin `external fun` declarations
3. Run `cargo test --package native --lib` to validate native compilation

> The six Android test types (unit, Roborazzi, Compose UI, Maestro,
> Android UI testing framework, Espresso) are described in TESTING.md. A change touching
> keyboard encoding, IME, OSC 7 current working directory, or PTY flags must be covered by at least one
> of those test types.

## End-to-End

```bash
nu scripts/test-emulator.nu                          # automated emulator tests
```

---

## Documentation Maintenance

### Requirement ID Discipline

When modifying the codebase, check if the change affects a requirement documented
in `docs/requirements/` directory (StrictDoc .sdoc files):

- **New feature**: Add a new FR-xxx entry to `docs/requirements/functional_requirements.sdoc` and corresponding
  acceptance criteria to `docs/acceptance.md`
- **Changed behavior**: Update affected requirement descriptions in `docs/requirements/functional_requirements.sdoc`
- **Deprecated behavior**: Mark the requirement as inactive in `docs/requirements/functional_requirements.sdoc`
- **New design decision**: Create an ADR in `docs/adr/` referencing the relevant
  requirement ID

### Traceability Matrix Updates

After any change to requirements, design, API, or tests:

1. Update `docs/traceability.yml` to reflect new or changed mappings
2. Verify all referenced IDs (FR-xxx, NFR-xxx, file paths) still resolve
3. Run `cargo test -p integration-tests --test tool_lint` to validate
   structural consistency

### ADR Lifecycle

- **Creating**: Copy `docs/adr/template.md`, fill in the decision, set status to
  `Proposed`
- **Approving**: Change status to `Accepted` after team review
- **Replacing**: Mark old ADR as `Superseded`, create new ADR referencing it
- **Retiring**: Mark as `Deprecated` with a note on why
- **Format gate**: `adrs doctor` must pass before commit. ADRs use the Nygard
  format (`# N. Title`, `Date:`, `## Status`, `## Context`, `## Decision`,
  `## Consequences`, `## Compliance`). Run `adrs doctor --cwd .` locally; CI
  runs it via `tool_lint.rs` (`adrs_doctor_finds_no_issues`).

### Requirements Gate

- Requirements live in `docs/requirements/` as StrictDoc `.sdoc` files
  (`functional_requirements.sdoc` = FR-xxx, `non_functional_requirements.sdoc` = NFR-xxx).
- **Format gate**: `strictdoc export docs/requirements` must pass before commit (exit 0).
  CI runs it via `tool_lint.rs` (`strictdoc_validates_requirements`).

### Documentation Validation

The following checks run in CI via `tool_lint.rs`:

- `typos_finds_no_typos` — Spelling check on all files
- `markdownlint_finds_no_violations` — Markdown formatting
- `vale_finds_no_violations` — Prose style and consistency
- `adrs_doctor_finds_no_issues` — ADR Nygard format/structure gate
- `strictdoc_validates_requirements` — Requirement item structure gate
- `doc_srs_matches_sdoc_ids` — `docs/srs.md` ↔ `docs/requirements/*.sdoc` ID 集合一致
- New doc-specific checks (see `tool_lint.rs` for `docs_*` test functions):
  - SRS requirement ID format validation
  - Traceability cross-reference integrity
  - Acceptance→SRS ID linkage

### Before Commit

Add the following to the pre-commit checklist:

- [ ] `docs/requirements/` updated if requirements changed
- [ ] `docs/traceability.yml` updated if requirement/design/API/test mapping changed
- [ ] New ADR created if a design decision was made
- [ ] All documentation lint checks pass (`cargo test -p integration-tests --test tool_lint`)

## Toolchain Constraints

This project uses git hooks for local quality enforcement and CI for verification:

1. **Pre-push hook**: Runs `cargo fmt --check`, `cargo clippy --all -- --deny warnings`, and `./gradlew spotlessCheck` before allowing push. Timeout: 30s per command. Bypass with `--no-verify` in emergencies.

2. **Commit-msg hook**: Advisory conventional commit check. Blocks "changes"/"wip" messages. Warns on non-conventional format but allows the commit.

3. **CI enforcement**: CI runs the same checks independently of hooks. Even if hooks are bypassed, CI will catch violations.

---

## Golden Image Ban Policy (FR-057)

Golden images (reference PNG screenshots used for pixel-by-pixel comparison) are
**strictly banned** from this repository. Reason: they are not human-verified
and cannot be audited for correctness during code review.

### What is banned

- ✅ **Allowed**: OCR verification (tests that render output and use `rapidocr`
  CLI to detect expected text)
- ✅ **Allowed**: Pixel-level logical assertions (tests that sample specific
  pixel coordinates and assert color values, e.g. "center pixel is red")
- ✅ **Allowed**: Temporary PNG files written to `std::env::temp_dir()` for
  intermediate processing (e.g. OCR input)
- ❌ **Banned**: Golden images stored in the repo (`*.png` in any
  `screenshots/`, `test-screenshots/`, `test_data/*_golden.png`, or
  `resources/roborazzi/` directory)
- ❌ **Banned**: GitHub-hosted reference screenshots that must be identical
  across environments
- ❌ **Banned**: test logic whose sole purpose is to compare against a golden
  image (use OCR text verification or pixel assertion instead)

### Enforcement

- No golden-image-comparison pattern exists in `.gitignore` (old
  `native/src/render/` test directories were removed with crate consolidation).
- CI has no golden-image-comparison step.
- All rendering tests must use either OCR verification (`rapidocr`) or
  pixel-coordinate assertions.
- Any committed golden image will be rejected by code review (SRS FR-057).

## Font File Ban Policy (FR-057)

Font files (`.ttf`, `.otf`, `.woff`, `.woff2`, `.eot`) are **strictly banned**
from this repository. Reason: fonts SHALL be installed via Nix (`flake.nix`),
not bundled as binary blobs in git.

- ✅ **Allowed**: Font references in `flake.nix` and Nix shell
- ❌ **Banned**: Any `.ttf`, `.otf`, `.woff`, `.woff2`, or `.eot` file in any
  directory of the repository

### Enforcement (Docs)

- `*.ttf`, `*.otf`, `*.woff`, `*.woff2`, `*.eot` are in `.gitignore`.
- CI has no local font file dependency — all fonts come from the Nix store.
- Any committed font file will be rejected by code review (SRS FR-057).
