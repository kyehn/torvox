# 1. Crate Consolidation

## Status

Accepted

Date: 2026-07-27

## Requirement IDs

NFR-02, NFR-05

## Context

The torvox codebase originally had **5 Rust crates** — `terminal-core/` (no_std
data model), `terminal-engine/` (Ghostty wrapper + session), `gpu-renderer/`
(wgpu pipeline), `android-gui/` (boltffi bridge + surface), and
`mcp-server/` (standalone AI server) — plus a `fuzz/` workspace,
`integration-tests/`, and `benchmarks/`.

In practice these crates are never built, released, or tested independently.
The crate boundaries existed for theoretical isolation but created real costs:

- 5 `Cargo.toml` files with duplicated dependency boilerplate
- Bidirectional dependency validation via `cargo metadata` trick
- 20-level directory nesting
- Every type crossing a crate boundary becomes a public API, bloating the
  interface surface
- Cross-crate refactoring requires touching 5+ files for a single logical
  change
- Workspace compilation is slower than a single crate due to less incremental
  reuse

## Decision

Merge all Rust source code into **one `native/` crate** at the workspace root.
The `mcp/` module lives inside `native/` as an optional feature, not a
separate crate.

The resulting crate structure:

```text
torvox/
├── native/            ← single Rust crate (lib + cdylib)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs     ← JNI entry points
│       ├── mcp.rs     ← feature-gated
│       ├── lock_util.rs
│       ├── android/
│       ├── render/
│       └── terminal/
├── android/           ← Kotlin+Compose (unchanged)
├── exec-bin/
└── integration-tests/
```

## Alternatives Considered

### Keep all 5 crates (status quo)

- **Rejected**: Creates the costs listed above with no measurable benefit.
  The project has one developer and one deliverable (an Android APK).

### Keep 2 crates: `core/` (no_std types) + `native/` (everything else)

- **Rejected**: The no_std types (`GridSnapshot`, `Cell`, `DirtyMask`) are
  Ghostty data model replicas that will be removed per ADR-0002. No reason to
  preserve the boundary.

## Consequences

### Positive

- Single `Cargo.toml`, single dependency tree, single `lib.rs`
- No crate-boundary ceremony (`pub` vs `pub(crate)` discipline relaxed)
- All internal types are private to the crate — stable API surface shrinks to
  the JNI exports
- Faster compilation (no workspace multithreading overhead for small project)
- Directory depth drops from ~20 to ~7 levels

### Negative

- Loss of compartmentalized compilation — a change anywhere rebuilds the
  entire crate (mitigated by incremental compilation)
- Risk of accidental dependency cycles within the crate (mitigated by
  `mod` ordering discipline documented in `native/src/lib.rs`)

## Compliance

- The `cargo metadata` dependency-direction check is removed (no longer needed
  with one crate)
- Directory tree under `native/src/` must not exceed 5 levels

## Status Note (Jul 2026)

This decision was fully implemented in Phase 4 of the re-architecture:

- `terminal-engine`, `gpu-renderer`, and `android-gui` merged into `native/`
- `fuzz/` and `benchmarks/` removed (no longer needed)
- `mcp/` lives inside `native/` as a module (implemented with mcp feature gate; test-util implies mcp)
- Remaining workspace members: `native`, `exec-bin`, `integration-tests`
