# Dependencies and SBOM — Torvox

## 1. Dependency Management

### 1.1 Rust Dependencies
- Managed via Cargo workspace (`Cargo.toml` — `[workspace.dependencies]`)
- All shared dependencies declared in `[workspace.dependencies]` with consistent versions
- Pinned via `Cargo.lock` (committed to git)
- Dependency order: strict one-way crate graph (see `docs/architecture.md#2.1`):

  ```
  libghostty-vt / libghostty-vt-sys
      ↑
  torvox-core (no_std)
      ↑
  torvox-terminal
      ↑
  torvox-renderer
      ↑
  torvox-gui-android
      ↑
  android/app (Kotlin + Compose)
  ```

- Violations of the one-way constraint break the build and are enforced by `cargo metadata --no-deps --format-version 1`
- Upstream `libghostty-rs` pinned via git commit URL in `[workspace.dependencies]` (no crates.io release)

### 1.2 Nix Dependencies
- Build environment and all tools declared via `flake.nix` devShell
- `flake.lock` pinned (committed to git) for reproducible development environments
- Inputs: `nixpkgs`, `flake-parts`, `fenix` (Rust toolchain)
- All lint and audit tools (cargo-audit, cargo-machete, clippy, etc.) declared as devShell packages

### 1.3 Android Dependencies
- Gradle-managed via `android/build.gradle.kts` (root) and `android/app/build.gradle.kts` (app module)
- Kotlin + Jetpack Compose UI with standard AndroidX libraries:
  - `androidx.compose:compose-bom:2026.06.01` (Compose Bill of Materials)
  - `androidx.core:core-ktx`, `lifecycle-runtime-ktx`, `activity-compose`
  - `androidx.compose.ui`, `ui-graphics`, `material3`, `material-icons-extended`
  - `androidx.navigation:navigation-compose`, `androidx.datastore:datastore-preferences`
- Dependency injection: `com.google.dagger:hilt-android:2.60` with KSP compiler
- JNA bridge: `net.java.dev.jna:jna:5.19.1@aar`
- Test frameworks: JUnit 4, MockK, Turbine, Robolectric, Roborazzi, Cucumber, Espresso, UI Automator, ArchUnit

## 2. Vulnerability Scanning

- [`cargo-audit`] scans Rust crate dependencies for known security vulnerabilities
- Runs in CI via `torvox-integration-tests/tests/tool_lint.rs` (`cargo_audit_finds_no_vulnerabilities` test)
- Also invoked in `scripts/check-rust.nu` as part of the full CI pipeline
- `cargo-deny` is intentionally **not** configured for this project:
  - The `deny_toml_must_not_exist` test in `tool_lint.rs` asserts that no `deny.toml` file exists in the repository
  - Per project policy documented in `docs/architecture.md#5.7-cargo-audit-over-cargo-deny`: existing CI infrastructure uses `cargo-audit`, and build determinism via Nix flake pinning ensures audit consistency across environments
  - `cargo-deny` is present in `flake.nix` devShell packages (for ad-hoc use) but has no configuration file

## 3. License Compliance

- All dependencies must use OSI-approved open-source licenses
- License checking is done via **manual review** (no automated license scanning tool is configured)
- The project policy explicitly excludes `cargo-deny` configuration, which means no automated `allow-list` or `deny-list` license enforcement
- Rust crate licenses are verified during dependency upgrades by maintainers
- Kotlin/Android library licenses are reviewed via Gradle dependency metadata

## 4. Unused Dependency Detection

- [`cargo-machete`] scans Rust workspaces for declared but unused dependencies
- Runs in CI via `torvox-integration-tests/tests/tool_lint.rs` (`cargo_machete_finds_no_unused_deps` test)
- Uses `--skip-target-dir` flag per project convention (avoids false positives from cached build artifacts)
- Do NOT use `--with-metadata` flag — it causes false positives with proc-macro dependencies like `quickcheck` (see AGENTS.md pitfalls)

## 5. Supply Chain

- **Upstream libghostty-rs**: Pinned via git commit URL in `Cargo.toml` `[workspace.dependencies]` (lines 62–63):
  ```toml
  libghostty-vt = { git = "https://github.com/Uzaaft/libghostty-rs.git", package = "libghostty-vt" }
  libghostty-vt-sys = { git = "https://github.com/Uzaaft/libghostty-rs.git", package = "libghostty-vt-sys" }
  ```
  The exact commit is locked in `Cargo.lock` for reproducible builds.

- **Zig correctness patches**: Applied via `scripts/bootstrap-libghostty.nu`:
  1. Clones Ghostty source to `vendor/ghostty` (if not already present)
  2. Applies `patches/libghostty-vt-correctness.patch` (cursor_style save/restore, DECAWM wraparound, scroll N=0→1 fixes)
  3. Uses `--forward` flag so already-applied hunks are skipped (idempotent)

- **No vendored crates in tree**: The `vendor/` directory is reserved exclusively for build-time source clones (e.g., Ghostty source for patching) — it is not a crate vendoring directory.

- **Nix flake pinning**: `flake.lock` pins all Nix inputs (`nixpkgs`, `flake-parts`, `fenix`) to specific revisions, providing reproducible development environments across machines.
