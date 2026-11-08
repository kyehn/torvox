# 任务

- [x] 定位逐次刷屏的告警：剪贴板应答回写失败的两条调用点
- [x] 新增回写失败告警的限频闸门并接线两条调用点
- [x] 告警改按窗口输出，分级维持 warn，会话已结束仍记 debug 且不计数
- [x] 补单测：首条立即交出窗口计数
- [x] 补单测：窗口内的失败攒着下次交出
- [x] 补单测：会话已结束不计入失败窗口
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
