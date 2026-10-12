#!/usr/bin/env -S nix develop --command nu

def main [] {
    cd native
    cargo fmt --all -v -- --check
    cargo clippy --all-targets -- --deny warnings
    cargo machete --with-metadata
    semgrep scan --error --dataflow-traces --time --config .semgrep/rules.yml
    cargo test --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
    markdownlint-cli2 "**/*.md"
    cargo bench --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick
    cargo build --package native --profile release
    cd ../android
    ./gradlew --continue detekt spotlessCheck app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:compileBenchmarkReleaseKotlin baselineprofile:compileNonMinifiedReleaseKotlin -Dorg.gradle.internal.test.results.binary.enabled=false
}
