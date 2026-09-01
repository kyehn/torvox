# Master Comprehensive Optimization Plan — v8

> 日期：2026-09-01 | 状态：活跃
> 从 `3fc8b6d` baseline 出发，参考 26 项目优点，以 openspec 为长期规范载体

## 总览

本文档是 Master Optimization v8 的完整实施计划，涵盖代码修复、代码精简、文档产出、自动化验证、三轮审阅循环。目标：全代码库逐步评审、保守精简、依赖滚动、自动化验证闭环。

## 已完成（阶段 0 — 基线冻结）

- [x] 恢复 `docs/reference` 44 文件（`3fc8b6d` baseline）
- [x] 修复 cell_builder 索引 bug
- [x] cargo test 995+32 暖缓存基线（57s, 0 failed）
- [x] OpenSpec v8 change 文档（proposal/design/tasks）

## 进行中（阶段 1 — 代码修复）

- [x] DEFAULT_LANG `en_US.UTF-8` → `C.UTF-8`（DESIGN spec §Shell, pty.rs）
- [ ] 常量化 magic numbers（pty.rs, ffi.rs）
- [ ] 注释溯源补全

## 待做（阶段 2 — 文档产出）

- [x] OPENSPEC-STATUS.md v1.1（对齐任务标记与实现）
- [x] 本计划（master-comprehensive-optimization-plan.md）
- [x] 测试计划（master-test-plan.md）
- [x] 验证协议（master-verification-protocol.md）
- [x] 依赖变更日志（dependency-changelog-2026-09-01.md）

## 待做（阶段 3 — 验证脚本）

- [x] verify-build-artifacts.nu
- [x] verify-emulator.nu
- [x] verify-cjk.nu
- [x] verify-all.nu（入口）

## 待做（阶段 4 — 三轮审阅）

- [ ] 第 1 轮 review：代码 + 文档 + 脚本
- [ ] 第 2 轮 review：修复第 1 轮问题后复审
- [ ] 第 3 轮 review：最终确认零阻塞

## 待做（阶段 5 — 收口）

- [ ] cargo test 995+32 确认全绿（3 次重复零 flaky）
- [ ] 更新 comprehensive-hardening-v7/tasks.md 勾选
- [ ] git commit + push

## 26 参考项目优先级矩阵

| 优先级 | 吸收项 | 状态 | 来源项目 |
|--------|--------|------|----------|
| P0-01 | 多击选择 tapCount | 待实现 | ghostty-android |
| P0-02 | Callback2 菜单锚定 | 待实现 | termux-app |
| P0-03 | 手柄拖动边缘滚动 | 待实现 | termux-app |
| P0-04 | wrap 感知拼接 | 待实现 | termux-app |
| P0-05 | IME composing diff | ✅ 已实现 | warp |
| P0-06 | bootstrap sha256 | ⏳ best-effort | warp |
| P0-07 | PTY 初始 winsize | ✅ 已实现 | warp |
| P0-08 | ArgumentTokenizer | ✅ 已实现 | termux-kotlin |
| P0-09 | SO_PEERCRED | ✅ 已实现 | termux-kotlin |
| P0-10 | EnvOp overlay | ✅ 已实现 | zed-port |
| P0-11 | URL 超链接正则 | ✅ 已实现 | zed-port |
| P0-12 | 前台进程组跟踪 | ✅ 已实现 | zed-port |
| P0-13 | OSC 133 语义段 | ✅ 已实现 | termlib |
| P0-14 | CellRun 游程 | ✅ 已实现 | termlib |
| P0-15 | 行级脏缓存 | ✅ 已实现 | zelland |
| P0-16 | 搜索收窄匹配 | ✅ 已实现 | GNOME Console |

P0 实现率：12/16（75%），剩余 4 项属于 UI 交互层或可选增强。

## 验证闭环

```
cargo test 995+32 → clippy 0 → machete 0 → 构建验证 → 模拟器 90fps+ → 三轮 review
```

每次提交前通过 L1-L3（本地），L7-L8（预发布）在模拟器上执行。
