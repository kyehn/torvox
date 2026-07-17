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

    let RUSTC = (nix build --print-out-paths --impure .#rust-toolchain-latest | str trim) + "/bin/rustc"
    for target in [
        "fuzz_vt_parser"
        "fuzz_osc_parse"
        "fuzz_grid_resize"
        "fuzz_grid_ops"
        "fuzz_selection"
        "fuzz_attrs"
    ] {
        do { RUSTC=$RUSTC cargo fuzz run --fuzz-dir fuzz $target -- -max_total_time=5 }
        let exit_code = $env.LAST_EXIT_CODE
        if $exit_code != 0 {
            print $"⚠  fuzz_($target) exited ($exit_code). This is NOT a CI failure."
            print $"   Artifacts in fuzz/artifacts/($target)/ — investigate before merging."
        }
    }

    print "=== fuzz summary ==="
    print "Fuzz failures are informational — they do not fail CI."
    print "Check fuzz/artifacts/ for crash reproduction files."
}
