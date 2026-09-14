#!/usr/bin/env -S nix develop --command nu

def main [] {
    cargo fmt --all -v -- --check
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo machete --with-metadata
    semgrep scan --error --dataflow-traces --time --config .semgrep/rust-deny-patterns.yml --config .semgrep/rust-arch.yaml
    cargo test --workspace --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
    markdownlint-cli2 "**/*.md" "#target/**" "#android/**/build/**"
    cargo bench --workspace
    cargo-llvm-cov --html
}
