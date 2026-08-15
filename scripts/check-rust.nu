#!/usr/bin/env -S nix develop --command nu
# Comprehensive Rust quality check + fuzz.
# All checks run unconditionally, sequentially.

# libghostty-vt-sys 0.2.1 pins this ghostty commit in its build.rs.
const GHOSTTY_PIN = "a887df42c56f6de86c0fe6da9c4eeca37931e083"

def ensure-ghostty-source [] {
    # libghostty-vt-sys build.rs clones ghostty at GHOSTTY_PIN unless
    # GHOSTTY_SOURCE_DIR is set. The nix devshell LD_LIBRARY_PATH breaks the
    # system git https helper (glibc mismatch), so clone with a clean
    # LD_LIBRARY_PATH and point the crate at the local copy instead.
    # Returns the source path; the caller sets GHOSTTY_SOURCE_DIR — $env
    # writes inside a def do not propagate to the caller.
    let src = ($env.PWD | path join ".cache" $"ghostty-src-($GHOSTTY_PIN | str substring 0..7)")
    if not ($src | path join "build.zig" | path exists) {
        mkdir ($src | path dirname)
        ^env -u LD_LIBRARY_PATH git clone --filter=blob:none --no-checkout --quiet https://github.com/ghostty-org/ghostty.git $src
        ^env -u LD_LIBRARY_PATH git -C $src checkout --quiet $GHOSTTY_PIN
    }
    $src
}

def main [] {
    $env.GHOSTTY_SOURCE_DIR = (ensure-ghostty-source)
    cargo fmt --check
    cargo clippy --all -- --deny warnings
    cargo test -p integration-tests --test tool_lint -- --test-threads 1
    cargo test --workspace
    # Performance benchmarks are #[ignore]d in the full-suite run above:
    # parallel CPU contention (software Vulkan benches + tokio tests) makes
    # their wall-clock measurements flaky. They are verified here serially,
    # which gives stable single-run numbers.
    cargo test -p native --lib --features test-util -- --ignored bench --test-threads 1
}