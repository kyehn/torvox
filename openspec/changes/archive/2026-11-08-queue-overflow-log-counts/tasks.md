# 任务

- [x] 核对规范对限频诊断的要求：窗口内事件计数 MUST 随那一行交出
- [x] `CountedLogThrottle` 补 `with_interval`（两队列静默期为 1s）
- [x] 事件队列溢出告警改用带计数的闸门，行内加 `dropped_events`
- [x] 桌面通知队列溢出告警改用带计数的闸门，行内加 `dropped_notifications`
- [x] 补回归用例：两类队列的窗口计数攒着下次交出
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
