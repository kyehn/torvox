# 任务

- [x] 定位重复日志的两个记录点（字体管线的解析日志、JNI 层的下发日志）
- [x] 字体管线的解析日志改为只在字面真的换了时记录
- [x] JNI 层的下发日志改为只在字面切换时记录
- [x] 确认字族找不到的告警不受去重影响
- [x] 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings` 与 `cargo test --lib font`
- [x] 定向运行 OpenSpec 严格校验，确认全部能力通过
- [x] 归档本 change 并同步主规约
