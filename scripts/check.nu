#!/usr/bin/env -S nix develop --command nu

def main [] {
    cd native
    cargo fmt --all -v -- --check
    cargo clippy --all-targets -- --deny warnings
    # 约 1100 行代码只在 `#[cfg(target_os = "android")]` 下编译（FFI、字体库、
    # Vulkan 后端），主机目标的 clippy 完全看不到它们。Android 目标必须单独
    # lint 一次，否则那部分代码的告警永远没人看。
    cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings
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
    # openspec 规范是契约快照：门禁必须自己盯住它，否则规范与实现的漂移
    # 只能靠人工发现（曾出现过：规范条目超长、拆分后未同步）。
    openspec validate --all --strict
    cd android
    ./gradlew --continue detekt spotlessCheck app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:compileBenchmarkReleaseKotlin baselineprofile:compileNonMinifiedReleaseKotlin -Dorg.gradle.internal.test.results.binary.enabled=false
}
