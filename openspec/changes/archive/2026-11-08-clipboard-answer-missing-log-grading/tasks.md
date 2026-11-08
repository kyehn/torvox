# 任务

- [x] 定位逐次刷屏的告警：应答线程的超时与通道断开两处
- [x] 新增渲染暂停标志的原子镜像，由 `setRenderPaused` 导出同步
- [x] 应答缺失诊断改按窗口汇总，两种情形各一把闸门
- [x] 分级按宿主是否仍在渲染：暂停记 debug，仍在渲染记 warn
- [x] 补单测：分级口径（暂停 / 仍在渲染）
- [x] 补单测：窗口计数不丢
- [x] 补单测：渲染暂停镜像跟随宿主
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
