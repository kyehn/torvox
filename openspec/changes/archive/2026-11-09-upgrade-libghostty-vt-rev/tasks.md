# 任务

- [x] 确认上游 master 新 rev 与本仓所钉 rev 的提交数差
- [x] 更新 `native/Cargo.toml` 的 `libghostty-vt` rev
- [x] 格式化器按新 API 重建（整屏建 → 设选区 → 设 unwrap/trim），补 SAFETY 说明
- [x] `kitty_graphics()` 四处调用点解包 `Option`
- [x] 回滚字节预算改传 `Some(usize::MAX)`
- [x] 鼠标编码器尺寸写失败时记错误并丢弃事件
- [x] 运行 `cargo clippy --all-targets -- --deny warnings` 与全量 `cargo test`
- [x] 构建双 ABI 原生库并运行 `:app:testDebugUnitTest`
- [x] 更正 `upstream-alignment` 的上游 API 取用判定行与回滚上限台账行
- [x] 运行 OpenSpec 严格校验
- [x] 归档本 change 并同步主规约
