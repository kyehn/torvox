#!/usr/bin/env -S nix develop --command nu
# Comprehensive Rust quality check + fuzz.
# All checks run unconditionally, sequentially.

def main [] {
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
