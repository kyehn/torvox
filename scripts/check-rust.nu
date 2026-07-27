#!/usr/bin/env -S nix develop --command nu
# Comprehensive Rust quality check + fuzz.
# All checks run unconditionally, sequentially.

def main [] {
    cargo fmt --check
    cargo clippy --all -- --deny warnings
    cargo test -p integration-tests --test tool_lint -- --test-threads 1
    cargo test --workspace
    # Shader validation tests removed — crate was merged into native/.
    print "Check completed successfully."
}
