# 任务

- [x] 取证：CI 两条工作流的链接失败在本地 `cargo clean -p libghostty-vt-sys` 后复现
- [x] 取证：解出 `compiler_rt.o` 的 null 符号引用与 lld/GNU ld 行为差异
- [x] `.cargo/config.toml`：宿主目标改用 GNU ld 并关闭 `--eh-frame-hdr`
- [ ] 验证：`cargo build -p native`（debug + release）、`cargo test --workspace --no-run`、
      `cargo bench --quick` 全绿
- [ ] 跑 `scripts/check-rust.nu`
- [ ] 更新 openspec/specs 并归档 change