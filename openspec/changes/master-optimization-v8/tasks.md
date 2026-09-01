# Tasks: Master Optimization v8

> 日期：2026-09-01 | 状态：活跃

## P0 — 必须完成

- [x] T1 修复 DEFAULT_LANG 为 C.UTF-8（DESIGN spec §Shell，pty.rs）
- [ ] T2 创建 OPENSPEC-STATUS.md v1.1（对齐任务标记与实现）
- [ ] T3 创建本计划文档（master-comprehensive-optimization-plan.md）
- [ ] T4 创建测试计划（master-test-plan.md）
- [ ] T5 创建验证协议（master-verification-protocol.md）
- [ ] T6 创建 verify-build-artifacts.nu 脚本
- [ ] T7 创建 verify-emulator.nu 脚本
- [ ] T8 创建 verify-cjk.nu 脚本
- [ ] T9 创建 verify-all.nu 入口脚本

## P1 — 重要

- [ ] T10 常量化 pty.rs 中 magic numbers
- [ ] T11 常量化 ffi.rs 中 magic numbers
- [ ] T12 注释溯源：encode_mouse_event 补 zelland 引用
- [ ] T13 注释溯源：scan_osc133 补 termlib 引用
- [ ] T14 注释溯源：build_row_runs 补 termlib CellRun 引用
- [ ] T15 创建 dependency-changelog-2026-09-01.md
- [ ] T16 验证 comprehensive-hardening-v7/tasks.md 勾选与实现对齐

## P2 — 改善

- [ ] T17 更新 comprehensive-hardening-v7/tasks.md 已实现项勾选
- [ ] T18 补充 v8 specs/ 目录（terminal-winsize-sync、bootstrap-sha-verification 等 spec）
- [ ] T19 cargo test 1017 确认全绿（3 次重复零 flaky）

## 依赖关系

T1 → T2 → T3-T5（文档互引）→ T6-T9（脚本）→ T10-T14（代码改善）→ T15-T19（验证收口）
