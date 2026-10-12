#!/usr/bin/env -S nix develop --command nu

const MANIFEST = "native/Cargo.toml"

def main [] {
    cargo fmt --manifest-path $MANIFEST --all -v -- --check
    cargo clippy --manifest-path $MANIFEST --workspace --all-targets -- --deny warnings
    cargo machete --with-metadata native
    semgrep scan --error --dataflow-traces --time --config .semgrep/rules.yml
    cargo test --manifest-path $MANIFEST --workspace --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path $MANIFEST --no-deps --workspace
    markdownlint-cli2 "**/*.md" "#**/target/**" "#android/**/build/**"
    cargo bench --manifest-path $MANIFEST -p native --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick
}
