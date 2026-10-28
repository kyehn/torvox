# 任务

- [x] 确认分叉：`pack_atlas` 校验预算，`layout_entries` 不校验
- [x] 抽出唯一判据 `atlas_byte_size`：受检跨步 × 高，且不超过 `KGP_STORAGE_LIMIT`
- [x] `pack_atlas` 与 `layout_entries` 共用该判据，删除打包侧的三段重复校验
- [x] 回归测试改为断言两条路径同时拒绝超预算布局
- [x] 判别力验证：`layout_entries` 去掉判据后测试必须失败
- [x] `cargo fmt`、`cargo clippy --all-targets -- --deny warnings` 通过
- [x] `cargo test` 全绿
- [x] 规范补上「复用路径共用预算判据」的要求与场景
