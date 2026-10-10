## 上下文

本变更只记录调研结论与依赖口径，不改运行时行为。唯一的行为改动是把
`libghostty-vt` 从 `8953a740` 跟进到上游 master `d2036ba`，适配点只有
`on_clipboard_write` 回调签名（返回 `Result` → 返回 `()`）。

## 任务

- [x] 复查上游 `libghostty-rs` master 与本仓所钉 rev 的差距，并把依赖跟进到 master。
- [x] 逐项复查八套上游测试资产，给出「引入 / 不引入」与理由。
- [x] 复核 esctest2 的阻塞条件，给出绑定层与终端上游两处提交号。
- [x] 新增 `upstream-alignment` 规范：依赖跟踪口径、采纳门槛、缺口分层记录。
- [x] 全量 Rust 测试（533 项）与经 JNI 驱动真实原生库的 JVM 测试保持全绿。
