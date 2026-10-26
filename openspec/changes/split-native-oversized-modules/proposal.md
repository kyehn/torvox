# Proposal

## Why

`nix develop -c npx --yes aislop@latest scan` 当前报 12 条 Code Quality 告警：
11 个文件超过 1100 行、1 个函数超过 132 行。这些文件承载了会话生命周期、
渲染管线、输入编码、查询、字体加载、PTY 与终端网格等彼此独立的职责，
单文件可达 6000 余行，评审与定位成本极高。

| 位置 | 规模 |
| --- | --- |
| `native/src/android/ffi.rs` | 6073 行 |
| `native/src/terminal/ghostty_terminal/tests.rs` | 4579 行 |
| `native/src/terminal/ghostty_terminal/internal.rs` | 3696 行 |
| `native/src/render/tests.rs` | 3073 行 |
| `native/src/render/font/mod.rs` | 2239 行 |
| `native/src/terminal/session.rs` | 2168 行 |
| `native/src/render/cell_builder.rs` | 1883 行 |
| `native/src/terminal/pty.rs` | 1840 行 |
| `native/src/render/pass.rs` | 1723 行 |
| `native/src/render/context.rs` | 1658 行 |
| `native/src/render/font/font_db.rs` | 1646 行 |
| `native/src/terminal/ghostty_terminal/internal.rs:1137` | `run_inner` 429 行 |

## What Changes

- 把 `run_inner` 拆成「状态初始化 / 等待事件 / 空闲推进 / 命令批处理」四段，
  命令分派再按命令族拆成独立函数。
- 把 `ffi.rs` 按职责拆成子模块：会话生命周期、渲染、输入编码、查询、字体、
  剪贴板与事件泵；测试模块移出实现文件。
- 把 `render/` 下的 `font/mod.rs`、`font/font_db.rs`、`cell_builder.rs`、
  `context.rs`、`pass.rs` 与 `render/tests.rs` 按职责拆分。
- 把 `terminal/` 下的 `session.rs`、`pty.rs`、`ghostty_terminal/internal.rs`
  与 `ghostty_terminal/tests.rs` 按职责拆分。
- 只移动与重组代码：函数签名、控制流、返回值、日志与错误语义全部不变。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无：本次只做等价重构，不改变任何对外行为。

## Impact

- `native/src/**`：文件重组与函数拆分。
- 不改 `docs/specification/`、`docs/STATUS.md`、`Cargo.toml` 等受保护文件。
- 行为不变：由 Rust 编译、`clippy -D warnings`、`cargo test --lib` 与
  `aislop scan` 保证。
