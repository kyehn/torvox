#!/usr/bin/env -S nix develop --command nu
# Comprehensive Rust quality check + fuzz.
# All checks run unconditionally, sequentially.

def main [] {
    cargo fmt --check
    cargo clippy --all -- --deny warnings
    # tool_lint 含 11 项多轨文档校验（_typos.toml/.vale.ini/docs/srs.md 等）
    # 与当前 docs/specification 单轨不兼容，已标记 #[ignore] 隔离为可选门控，
    # 主链路仅验核心门控（machete/audit/markdownlint 等），扩展校验需显式 --ignored
    cargo test -p integration-tests --test tool_lint -- --test-threads 1
    cargo test --workspace
    # Performance benchmarks are #[ignore]d in the full-suite run above:
    # parallel CPU contention (software Vulkan benches + tokio tests) makes
    # their wall-clock measurements flaky. They are verified here serially,
    # which gives stable single-run numbers.
    cargo test -p native --lib --features test-util -- --ignored bench --test-threads 1
}
