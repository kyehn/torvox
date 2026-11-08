# 任务

- [x] 核对规范对限频诊断的要求：窗口内事件计数 MUST 随那一行交出
- [x] 读请求洪泛削减告警改用带计数的闸门，行内加 `shed_reads`
- [x] 选择器名超长告警改用带计数的闸门，行内加 `oversized_selections`
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
