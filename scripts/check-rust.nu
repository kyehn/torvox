#!/usr/bin/env -S nix develop --command nu
# Comprehensive Rust quality check + fuzz.
# All checks run unconditionally, sequentially.

def main [] {
    cargo fmt --check
    cargo clippy --all -- --deny warnings
    # tool_lint spawns external tools (typos, markdownlint, semgrep, etc.). Run with
    # cargo test (no nextest overhead) single-threaded to avoid memory pressure.
    # Non-tool tests use nextest for parallelism.
    cargo test -p torvox-integration-tests --test tool_lint -- --test-threads 1
    cargo nextest run --workspace --profile ci --retries 2 -E 'not binary(tool_lint)'

    for target in [
        "fuzz_vt_parser"
        "fuzz_osc_parse"
        "fuzz_grid_resize"
        "fuzz_grid_ops"
        "fuzz_selection"
        "fuzz_attrs"
    ] {
        ^cargo fuzz run --fuzz-dir fuzz $target -- -max_total_time=5
    }
}
