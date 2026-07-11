# Quality Gate

## Pre-commit

```bash
nix develop --command "cargo nextest run --workspace --profile ci"            # all tests pass
nix develop --command "cargo clippy --all -- --deny warnings"                # zero lint warnings
nix develop --command "cargo fmt --check"                                   # formatting clean
nix develop --command "cargo geiger --package torvox-core"                   # no new unsafe in core
nix develop --command "nu scripts/check-rust.nu"
```

### Property Tests

```bash
nix develop --command "cargo nextest run --package torvox-core --test property_tests"
nix develop --command "cargo mutants --timeout 30"                          # mutation score (config in .cargo/mutants.toml)
```

## Android Verification

```bash
nix develop --command "cd android && ./gradlew spotlessCheck detekt"         # Kotlin style and static analysis
nix develop --command "cd android && ./gradlew testDebugUnitTest"                           # unit tests
nix develop --command "cd android && ./gradlew lint"                                       # Android lint
```

## Bridge Changes

When modifying `torvox-core` types:

1. Ensure JNA bindings in `TorvoxBridge.kt` cover all changed types
2. Verify `bridge.rs` types are synced with `torvox-core` (`bridge.rs` is the
   single FFI export location — do not add a second one)
3. Run `nix develop --command "cargo test --package torvox-gui-android"`

> The six Android test types (unit, Roborazzi, Compose UI, Maestro,
> Android UI testing framework, Espresso) are described in TESTING.md. A change touching
> keyboard encoding, IME, OSC 7 current working directory, or PTY flags must be covered by at least one
> of those test types.

## End-to-End

```bash
nix develop --command "nu scripts/test-emulator.nu"                          # automated emulator tests
```
