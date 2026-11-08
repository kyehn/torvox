# 任务

- [x] 核对规范对限频诊断的要求：窗口内事件计数 MUST 随那一行交出
- [x] OSC 52 负载截断告警改用带计数的闸门
- [x] OSC 52 写入丢弃告警改用带计数的闸门，两类各持一把
- [x] 截断行改给上限与窗口内次数，不再给单次事件的原始长度
- [x] 补回归用例：两类诊断的窗口计数攒着下次交出
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
