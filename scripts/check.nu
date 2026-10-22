#!/usr/bin/env -S nix develop --command nu

def main [] {
    cd native
    cargo fmt --all -v -- --check
    cargo clippy --all-targets -- --deny warnings
    cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings
    cargo machete --with-metadata
    cargo test --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
    cargo bench --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick
    cargo build --package native --profile release
    cd ..
    semgrep scan --error --dataflow-traces --time --config .semgrep/rules.yml
    markdownlint-cli2 "**/*.md"
    openspec validate --all --strict
    cd android
    ./gradlew --continue detekt spotlessCheck app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:compileBenchmarkReleaseKotlin baselineprofile:compileNonMinifiedReleaseKotlin -Dorg.gradle.internal.test.results.binary.enabled=false
}
