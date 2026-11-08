# 任务

- [x] 核对规范对逐帧诊断的要求：窗口内事件计数 MUST 随那一行交出
- [x] 滚动指令投递失败告警改按窗口汇总，行内加 `failed_scrolls`
- [x] 渲染遇到未知会话的告警改按窗口汇总，行内加 `unknown_session_frames`
- [x] VT 应答写回失败告警改按窗口汇总，行内加 `failed_writes`
- [x] 子进程已退出时丢弃待写 VT 应答并记 debug，不再逐帧重试
- [x] 补回归用例：退出态丢弃待写应答、写回失败按窗口汇总
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] 归档本 change 并同步主规约
