# 任务

## 1. 取证（已完成）

- [x] 1.1 六个参考仓库浅克隆到 `/tmp/refs`，所钉 rev 与 `Cargo.lock` 一致（d2036ba）。
      验证：`git -C /tmp/refs/uzaaft_libghostty-rs rev-parse HEAD` 与锁文件同值。
- [x] 1.2 通读 `terminal.rs` 全量公开 API 与其余 16 个模块的 `pub` 面，逐项对出本仓调用点。
      验证：五份审计报告（A 终端面、B 渲染选择搜索、C 架构对标、D 选择细节、E 质量）齐备。
- [x] 1.3 核实 `on_clipboard_write` 必须 `reply`（上游文档「返回前必须 reply，否则拒绝写入」），确认本仓注释过时。
      验证：`terminal.rs` 2700 行起文档与示例零 `reply` 缺失用例。

## 2. 已修项（已合入，各自独立提交并推送）

- [x] 2.1 剪贴板回调补 `reply`（满通道回 `Busy`，否则 `Ok`）。
      验证：`cargo test -p native --lib -- osc52/clipboard` 17 项全绿。
- [x] 2.2 下划线判定三处收敛为 `has_visible_underline`/`is_double_underline`。
      验证：`-- style` 16 项全绿。
- [x] 2.3 `pty_write` 删 `>0xF7` 改空格，仅剔除 NUL。
      验证：`-- pty_write` 7 项全绿。
- [x] 2.4 渲染初始背景与 `set_background_color` 以调色板为单源，删 `CATPPUCCIN_MOCHA_BACKGROUND`。
      验证：`-- render::` 242 项全绿。
- [x] 2.5 `blend_highlight` 与测试断言的字节转浮点收敛至 `byte_to_float`。
      验证：`-- highlight` 24 项与 `-- cell_builder` 17 项全绿。
- [x] 2.6 选择柄翻转回传归属并收紧等值判据，调用方更新 `handleDragState`。
      验证：`:app:testDebugUnitTest --tests SelectionStateTest` 7 项全绿；实现口径已登记进 `text-selection` 规范。

## 3. 规范落地

- [x] 3.1 新增「上游 API 取用判定」Requirement，承载结论表。
- [x] 3.2 `openspec validate --changes` 通过。
- [x] 3.3 `openspec archive` 后 `openspec validate --specs` 全绿。
      （补记于 `research-residual-audit-closure`，2026-10-16 实测：31 passed / 0 failed；
      余下告警均为既有「>500 字符长 Requirement」风格项）

## 4. 待办（本次不做，阻塞点见规范表）

两项待办的**实际裁定与当初设想的做法不同**，故不按原样勾选为「已实现」，而是按
实际落点更正（补记于 `research-residual-audit-closure`，2026-10-16）：

- [x] 4.1 按键编码迁至上游 `key::Encoder`：需先裁定 Android 键码到上游 `Key` 的映射表与 JNI 桥形态，另立变更。
      ——已由 `2026-10-11-adopt-upstream-key-encoder` 实现并归档：常驻规约
      `key-input-encoding` 载「按键编码 MUST 取用上游 `key::Encoder`，本仓 MUST NOT
      自维护按键转义序列表」，编码前 MUST `set_options_from_terminal` 取活体模式状态。
- [x] 4.2 粘贴走 `Terminal::paste`（bracketed）：需先裁定 IME 提交路径与模式查询归属，另立变更。
      ——已由 `2026-10-11-adopt-bracketed-paste` 实现并归档，但**实际做法与本条设想相反**：
      常驻规约 `paste-encoding` 明写「MUST NOT 取用上游 `Terminal::paste`：其含换行
      文本返回 `Error::Rejected` 并要求嵌入方自行分帧」，本仓改走上游 `paste::encode`
      并自行处理 bracketed 标志。故本条按「已达成目标（bracketed 语义经上游承担）
      但路径与初稿相反」结案，不得读成照原计划实现了 `Terminal::paste`。
