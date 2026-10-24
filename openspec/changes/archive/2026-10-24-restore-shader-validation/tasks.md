## 1. 恢复着色器单一来源与测试

- [x] 1.1 `pipeline.rs` 恢复 `CELL_SHADER`/`KGP_SHADER` 常量并供两条管线创建使用
- [x] 1.2 恢复 naga 解析、校验与入口点断言测试
- [x] 1.3 运行 `cargo fmt`、`cargo clippy --all-targets -- --deny warnings`、`cargo test pipeline`

## 2. 规范与归档

- [x] 2.1 本变更 delta 写入 `render-stability` 要求
- [x] 2.2 `openspec validate`、markdownlint 通过后归档
