#!/usr/bin/env -S nix develop --command nu

def main [] {
    cd native
    cargo fmt --all -v -- --check
    cargo clippy --all-targets -- --deny warnings
    cargo machete --with-metadata
    cargo test --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
    cargo bench --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick
    cargo build --package native --profile release
    # 以下两项以仓库根为基准：semgrep 的 `--config .semgrep/rules.yml` 与
    # markdownlint 的忽略/通配都按 cwd 解析，在 `native/` 下两者都找不到任何文件
    # （配置在仓库根），门禁会静默扫零个文件而显得全绿。
    cd ..
    semgrep scan --error --dataflow-traces --time --config .semgrep/rules.yml
    markdownlint-cli2 "**/*.md"
    cd android
    ./gradlew --continue detekt spotlessCheck app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:compileBenchmarkReleaseKotlin baselineprofile:compileNonMinifiedReleaseKotlin -Dorg.gradle.internal.test.results.binary.enabled=false
}
