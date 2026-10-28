## 未被解决的问题

截至 2026-10-27 复核：当前无未解决问题。

## 验证基线

截至 2026-10-28：原生侧 `cargo check --all-targets`、
`cargo clippy --all-targets -- --deny warnings` 通过；`cargo test --all-targets`
742 例全绿（含 Mesa lavapipe 下的 GPU 用例与 shuttle 并发用例）。
Kotlin/Gradle 侧未在沙盒验证（缺 Android SDK），须在 `nix develop` 内执行。
