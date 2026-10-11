# Proposal

## Why

本轮拉取六个参考仓库并通读 `libghostty-rs` 全量源码（含 `terminal.rs` 4102 行与
`render/selection/search/fmt/mouse/key/paste/kitty/screen/style` 各模块），对出
17 项取用偏差：已修 6 项（剪贴板 `reply` 缺失、下划线三处重复、`pty_write` 二进制
破坏、渲染背景第三源、字节转浮点手写、选择柄翻转无归属回传），其余若只留口头结论，
后来者必重复调研或把「合规分叉」当缺陷移植回来。

## What Changes

- 在主规范新增「上游 API 取用判定」要求，承载逐项结论表：已修项给提交依据，
  不做项给 DESIGN 条款依据，待办项给阻塞点。
- 不改运行时行为（代码修复已在 6 个独立提交先行落地并推送）。

### New Capabilities

无。

### Modified Capabilities

- `upstream-alignment`：新增上游 API 取用判定表（采用 / 不采用 / 待办三态，每项
  附可复核依据：上游源码位置、本仓文件行号或 DESIGN 条款）。

## Impact

- `openspec/specs/upstream-alignment/spec.md`：新增一条 Requirement（含结论表）。
- 无新增依赖、无二进制资产、不改运行时行为。
