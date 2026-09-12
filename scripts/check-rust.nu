#!/usr/bin/env -S nix develop --command nu

def main [] {
    cargo fmt --check
    cargo clippy --all -- --deny warnings
    cargo test --workspace
    cargo test -p native --lib -- --test-threads 1
}
