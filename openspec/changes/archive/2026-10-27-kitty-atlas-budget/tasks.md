# 任务

- [x] 定位图集分配顺序错误：`layout_strip` → `pack_atlas` 先分配，`set_kgp_atlas` 才校验
- [x] 预算取既有量级 `KGP_STORAGE_LIMIT`（64MiB），不另造常数
- [x] 条带布局宽度累加改受检加法：溢出即放弃布局并记日志
- [x] 图集字节数用受检算术算出，超预算时在分配像素缓冲**之前**拒绝并记日志
- [x] 回归测试：超预算布局在分配前被拒绝，且布局计算本身不受预算影响
- [x] 判别力验证：临时关闭预算判断，测试必须失败
- [x] `cargo fmt`、`cargo clippy --all-targets -- --deny warnings` 通过
- [x] `cargo test` 全绿
- [x] 规范补上「图集打包必须在分配前拒绝超预算布局」的要求与场景
