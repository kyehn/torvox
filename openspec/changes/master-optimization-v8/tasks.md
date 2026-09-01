# Tasks: Master Optimization v8

> 日期：2026-09-01 | 状态：进行中

## P0 — 必须完成

- [x] T1 修复 DEFAULT_LANG 为 C.UTF-8（DESIGN spec §Shell，pty.rs）+ 测试断言更新
- [x] T2 创建 OPENSPEC-STATUS.md v1.1（对齐任务标记与实现）
- [x] T3 创建本计划文档（master-comprehensive-optimization-plan.md）
- [x] T4 创建测试计划（master-test-plan.md）
- [x] T5 创建验证协议（master-verification-protocol.md）
- [x] T6 创建 verify-build-artifacts.nu 脚本
- [x] T7 创建 verify-emulator.nu 脚本
- [x] T8 创建 verify-cjk.nu 脚本
- [x] T9 创建 verify-all.nu 入口脚本

## P1 — 重要

- [x] T10 常量化 ffi.rs 中 magic numbers（DEFAULT_CURSOR_BLINK_SPEED_MS, DEFAULT_FONT_CELL_SIZE, CLIPBOARD_POLL_INTERVAL_MS）
- [x] T11 常量化 mcp/mod.rs 中 magic numbers（DEFAULT_TERMINAL_ROWS, DEFAULT_TERMINAL_COLS）
- [ ] T12 注释溯源：encode_mouse_event 补 zelland 引用
- [ ] T13 注释溯源：scan_osc133 补 termlib 引用
- [ ] T14 注释溯源：build_row_runs 补 termlib CellRun 引用
- [x] T15 创建 dependency-changelog-2026-09-01.md
- [x] T16 验证 comprehensive-hardening-v7/tasks.md 勾选与实现对齐
- [x] T10b GPU 测试 `#[ignore]` 门控（22 个 GPU-dependent 测试）

## P2 — 改善

- [x] T17 更新 comprehensive-hardening-v7/tasks.md 已实现项勾选
- [ ] T18 补充 v8 specs/ 目录（terminal-winsize-sync、bootstrap-sha-verification 等 spec）
- [ ] T19 cargo test 确认全绿（3 次重复零 flaky）
- [ ] T20 更新 v8 proposal/design/tasks 测试计数为 995+32

## 依赖关系

T1 → T2 → T3-T5（文档互引）→ T6-T9（脚本）→ T10-T14（代码改善）→ T15-T20（验证收口）
