# Quality Gate

## Pre-commit

```bash
cargo nextest run --workspace --profile ci            # all tests pass
cargo clippy --all -- --deny warnings                # zero lint warnings
cargo fmt --check                                   # formatting clean
cargo geiger --package torvox-core                   # no new unsafe in core
nu scripts/check-rust.nu
```

### Property Tests

```bash
cargo nextest run --package torvox-core --test property_tests
cargo mutants --timeout 30                          # mutation score (config in .cargo/mutants.toml)
```

## Android Verification

```bash
cd android && ./gradlew spotlessCheck detekt         # Kotlin style and static analysis
cd android && ./gradlew testDebugUnitTest                           # unit tests
cd android && ./gradlew lint                                       # Android lint
```

## Bridge Changes

When modifying `torvox-core` types:

1. Ensure JNA bindings in `TorvoxBridge.kt` cover all changed types
2. Verify `bridge.rs` types are synced with `torvox-core` (`bridge.rs` is the
   single FFI export location — do not add a second one)
3. Run `cargo test --package torvox-gui-android`

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
in `docs/srs.md`:

- **New feature**: Add a new FR-xxx entry to `docs/srs.md` and corresponding
  acceptance criteria to `docs/acceptance.md`
- **Changed behavior**: Update affected requirement descriptions in `docs/srs.md`
- **Deprecated behavior**: Mark the requirement as deprecated in `docs/srs.md`
- **New design decision**: Create an ADR in `docs/adr/` referencing the relevant
  requirement ID

### Traceability Matrix Updates

After any change to requirements, design, API, or tests:

1. Update `docs/traceability.yml` to reflect new or changed mappings
2. Verify all referenced IDs (FR-xxx, NFR-xxx, file paths) still resolve
3. Run `cargo test -p torvox-integration-tests --test tool_lint` to validate
   structural consistency

### ADR Lifecycle

- **Creating**: Copy `docs/adr/template.md`, fill in the decision, set status to
  `Proposed`
- **Approving**: Change status to `Accepted` after team review
- **Replacing**: Mark old ADR as `Superseded`, create new ADR referencing it
- **Retiring**: Mark as `Deprecated` with a note on why

### Documentation Validation

The following checks run in CI via `tool_lint.rs`:

- `typos_finds_no_typos` — Spelling check on all files
- `markdownlint_finds_no_violations` — Markdown formatting
- `vale_finds_no_violations` — Prose style and consistency
- New doc-specific checks (see `tool_lint.rs` for `docs_*` test functions):
  - SRS requirement ID format validation
  - Traceability cross-reference integrity
  - Acceptance→SRS ID linkage

### Before Commit

Add the following to the pre-commit checklist:

- [ ] `docs/srs.md` updated if requirements changed
- [ ] `docs/traceability.yml` updated if requirement/design/API/test mapping changed
- [ ] New ADR created if a design decision was made
- [ ] All documentation lint checks pass (`cargo test -p torvox-integration-tests --test tool_lint`)
