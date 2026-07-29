# Patches: vendored libghostty-vt Rust bindings

This directory contains vendored sources for the `libghostty-vt` and
`libghostty-vt-sys` crates. These are lightweight Rust wrappers around
Ghostty's C terminal parser (vendored from `vendor/ghostty/`).

## Upstream

- **Repository**: <https://github.com/ghostty-org/ghostty>
- **Vendored commit**: `ae8727401d8c549671c36cdc326a94f47c94b635`
  (2026-07-03, `docs: clarify macOS dependencies (#13498)`)
- **C source path**: `src/terminal/libghostty-vt.*` in the ghostty repo

## Difference from upstream

These vendored crates are **not** a direct copy of any published crate.
They were hand-crafted from the Ghostty C API to provide safe Rust bindings
with the following modifications vs the original Ghostty source:

1. **`ghostty_terminal_new` signature**: C API takes `(alloc, terminal, cols, rows)`
   as separate `u16` parameters, *not* a `TerminalOptions` struct. The binding
   was patched to match this (see `libghostty-vt-sys/src/bindings.rs`).
2. **`TerminalOptions` removed**: The `Options` struct now holds `cols`, `rows`,
   `max_scrollback` directly instead of converting from a C struct.
3. **`max_scrollback`**: Passed through to the C API (not in the original Rust
   bindings).

## Updating

To update to a newer Ghostty commit:

```bash
# 1. Update vendor
cd vendor/ghostty
git fetch origin
git checkout <new-commit>

# 2. Regenerate bindings
#    (requires the Zig toolchain used by the project)
#    Then diff patches/ against the new bindings

# 3. Update this file's commit reference
```

## File structure

```
patches/
├── libghostty-vt-sys/     # Low-level C bindings (via `#[link]` + `extern "C"`)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── bindings.rs    # The generated/adapted C function declarations
│       └── terminal.rs    # Safe-ish wrappers over raw C calls
└── libghostty-vt/         # High-level Rust wrappers
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── terminal.rs    # Terminal struct, Options, new/scrollback/config
        ├── key.rs         # Key mapping utilities
        └── alloc.rs       # Custom allocator for Ghostty
```
