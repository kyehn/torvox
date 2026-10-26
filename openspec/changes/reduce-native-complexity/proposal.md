# Proposal

## Why

原生侧的代码质量扫描（`aislop`）在 45 个 Rust 文件上报出 14 条 Code Quality
告警：11 个文件超过 1100 行、3 个函数超过 132 行。其中三个超长函数都落在
每帧或每轮都要跑的关键路径上，评审时难以逐段核对：

| 位置 | 长度 | 频率 |
| --- | --- | --- |
| `native/src/android/ffi.rs:2346` `render_inner` | 552 行 | 每帧 |
| `native/src/terminal/ghostty_terminal/internal.rs:990` `run_inner` | 555 行 | VT 线程每轮 |
| `native/src/android/ffi.rs:1890` `pump_sessions` | 156 行 | 每帧 |

同时，按「符号在全仓只出现一次」的判据清点公开 API，`MockPtyHandle` 的
`set_exited` / `is_exited` 没有任何调用方：它们是让 `MockPtyInner::child_exited`
变成恒假字段的唯一入口，于是 `MockPty::write` 里的 BrokenPipe 分支与
`MockPty::read` 里的 EOF 分支都是**永不执行的死代码**。项目规范要求「不得保留
死代码」。

`render_inner` 里「新帧」与「空闲帧」两条路径各自重建一遍脏行掩码（清空、
按行数扩容、与上一帧比对、标脏光标行、标脏高亮行、整屏标脏），两段逻辑同源
却各写一遍，是本仓重复率的主要来源之一。

## What Changes

- 删除 `MockPty` 替身上永不生效的退出态：`set_exited`、`is_exited`、
  `child_exited` 字段及两处随之不可达的分支。
- 把 `render_inner` 按既有的四个阶段拆成独立函数：准备、收集帧数据、
  新帧渲染、空闲帧渲染；阶段间的锁序与「渲染状态锁内绝不嵌套注册表/会话锁」
  的约束保持不变。
- 把 `render_inner` 两条路径共用的脏行掩码构建抽成一个 helper，消除重复。
- 把 `pump_sessions` 的剪贴板读请求处理（洪泛削减 + 一次性槽位 + 应答线程）
  抽成独立函数，使主泵只保留「采集 → 处理 → 推事件」三段。
- 把 `run_inner` 的终端初始化与回调注册（回滚上限、Kitty 介质、PTY 回写、
  OSC 52 写回、XTWINOPS 尺寸）抽成独立函数，使 VT 线程主循环只保留事件循环。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无：本次只做等价重构与死代码清理，不改变任何对外行为。

## Impact

- `native/src/terminal/mock_pty.rs`：删除退出态 API 与随之不可达的分支。
- `native/src/android/ffi.rs`：`render_inner`、`pump_sessions` 拆分。
- `native/src/terminal/ghostty_terminal/internal.rs`：`run_inner` 拆分。
- 不改 `Cargo.toml`、`build.gradle.kts`、`docs/specification/` 等保护文件。
- 行为不变：由既有 Rust 单测、`cargo clippy -D warnings` 与行为等价性逐段核对保证。
