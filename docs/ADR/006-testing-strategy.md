# ADR 006: 测试策略 — 多层测试 + 模糊测试 + 属性测试 + MIRI

**状态**: 已接受
**日期**: 2026-05-26
**决策者**: 项目负责人

---

## 上下文

终端模拟器是安全敏感、状态密集型应用。VT 解析器必须正确处理任意字节输入 (包括恶意构造的转义序列)。PTY 管理涉及 `unsafe` 系统调用。渲染管线在 GPU 上运行, 传统测试难以覆盖。

Torvox 是 AI 辅助开发的项目——60% 的 AI 生成代码有 bug (社区报告)。测试是唯一可靠的 AI 代码质量保证。

## 决策

**五层测试策略 + 模糊测试 + 属性测试 + MIRI + cargo geiger。**

### 测试层次

```
┌──────────────────────────────────────┐
│ 第 5 层: 模糊测试 (cargo-fuzz)       │ ← 夜间 CI, 1B+ 迭代
│   vt_parser, osc_parser, utf8_parser │
├──────────────────────────────────────┤
│ 第 4 层: 集成测试                    │ ← 每 PR
│   parse_and_render, session_lifecycle│
│   vttest_compliance                  │
├──────────────────────────────────────┤
│ 第 3 层: 属性测试 (proptest 1.11)     │ ← 每 PR, 10K+ 案例
│   VT 序列生成, Grid 不变量      │
├──────────────────────────────────────┤
│ 第 2 层: 单元测试 (#[test])          │ ← 每 PR, 每公共函数
│   每个 crate 的 tests/ 模块          │
├──────────────────────────────────────┤
│ 第 0 层: 编译时检查                  │ ← 每次构建
│   cargo clippy, cargo geiger, MIRI   │
└──────────────────────────────────────┘
```

### 第 0 层: 编译时检查

| 工具 | 命令 | 频率 | 目标 |
|------|------|------|------|
| `cargo clippy` | `cargo clippy --deny warnings` | 每次构建 | 零警告 |
| `cargo geiger` | `cargo geiger --all-features` | 每 PR | torvox-core/torvox-terminal 零 unsafe |
| `MIRI` | `MIRIFLAGS="-Zmiri-tag-raw-pointers" cargo miri test` | 夜间 | 关键路径无 UB |
| `cargo fmt` | `cargo fmt --check` | 每 PR | 格式化一致 |

### 第 1 层: 单元测试

**Rust**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_default_is_empty() {
        let cell = Cell::default();
        assert_eq!(cell.char, ' ');
        assert_eq!(cell.fg, Color::default());
    }

    #[test]
    fn grid_resize_preserves_content() {
        let mut grid = Grid::new(24, 80);
        grid[0][0].char = 'A';
        grid.resize(24, 120);
        assert_eq!(grid[0][0].char, 'A');
    }
}
```

**要求**: 每个公共函数至少 1 个单元测试。`cargo nextest --workspace` 替代 `cargo test`。

**Kotlin**:

```kotlin
@Test
fun `session event maps correctly`() {
    val event = TerminalEvent.Bell(sessionId = 1L)
    assertEquals(1L, event.sessionId)
}
```

**要求**: ViewModel 逻辑测试。`./gradlew test` 每 PR。

### 第 2 层: 属性测试 (proptest 1.11)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parser_never_panics(input in "[\\x00-\\xff]{0..1000}") {
        let mut parser = libghostty_vt::Parser::new();
        let mut performer = TestPerformer::new();
        for byte in input.bytes() {
            parser.advance(&mut performer, byte);
        }
    }

    #[test]
    fn grid_width_invariant(rows in 1u16..=200u16, cols in 1u16..=500u16) {
        let grid = Grid::new(rows, cols);
        assert_eq!(grid.rows(), rows);
        assert_eq!(grid.cols(), cols);
    }

    #[test]
    fn dirty_line_bitmask_consistency(changes: Vec<(usize, bool)>) {
        let mut dirty = DirtyMask::new(24);
        for (line, set) in changes {
            if line < 24 {
                if set { dirty.mark(line); } else { dirty.clear(line); }
            }
        }
        for line in 0..24 {
            assert_eq!(dirty.is_dirty(line), dirty.is_dirty(line));
        }
    }
}
```

**要求**: VT 解析器 + Grid + PTY 编码 必须有属性测试。10K+ 案例/运行。

### 第 3 层: 集成测试

```rust
// torvox-integration-tests/tests/session_lifecycle.rs
#[test]
fn session_spawn_and_exit() {
    let session = Session::spawn("/bin/sh", 24, 80).unwrap();
    session.write(b"exit\n").unwrap();
    let event = session.wait_exit(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, TerminalEvent::ProcessExited { .. }));
}

#[test]
fn session_echo_hello() {
    let session = Session::spawn("/bin/sh", 24, 80).unwrap();
    session.write(b"echo hello\n").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let state = session.terminal();
    assert!(state.contains_text("hello"));
}
```

**要求**: 跨 crate 交互测试。VT 解析→Grid→渲染 路径。

### 第 4 层: 模糊测试

```rust
// torvox-fuzz/fuzz_targets/vt_parser.rs
#![no_main]
use libfuzzer_sys::fuzzer_input;
use libghostty_vt::Parser;

fn fuzz(data: &[u8]) {
    let mut parser = Parser::new();
    let mut performer = TestPerformer::new();
    for byte in data {
        parser.advance(&mut performer, *byte);
    }
}
```

**目标**: 3 个模糊目标 (vt_parser, osc_parser, utf8_parser)。夜间 CI 1B+ 迭代。零崩溃。

### 确定性回放测试

```rust
#[test]
fn replay_deterministic() {
    let recording = include_bytes!("../recordings/vim_exit.bin");
    let mut session1 = replay_session(recording);
    let mut session2 = replay_session(recording);
    assert_eq!(session1.terminal(), session2.terminal());
}
```

**录制格式**: PTY 原始输出 → postcard 序列化 (dev-dep) → 文件。相同输入 → 相同 Grid 状态。

### CI 测试矩阵

| 层 | CI 触发 | 工具 | 超时 |
|----|---------|------|------|
| 编译时检查 | 每次 PR | clippy, geiger, fmt | 5 分钟 |
| 单元测试 | 每次 PR | cargo nextest --workspace | 10 分钟 |
| 属性测试 | 每次 PR | proptest (10K cases) | 5 分钟 |
| Kotlin 测试 | 每次 PR | ./gradlew test | 10 分钟 |
| Android lint | 每次 PR | ./gradlew lint | 5 分钟 |
| 集成测试 | 每次 PR | torvox-integration-tests | 15 分钟 |
| MIRI | 每夜 | cargo miri test | 60 分钟 |
| 模糊测试 | 每夜 | cargo fuzz (1B iterations) | 120 分钟 |
| 基准测试 | 每夜 | cargo bench | 30 分钟 |

### 质量门脚本

见 `scripts/quality-gate.sh`。8 步质量门: fmt → clippy → nextest → proptest → geiger → Android lint → Android test → 集成测试。

## 后果

**正面**:
- AI 生成代码的 bug 发现率 >95% (单元+属性+模糊组合)
- VT 解析器对任意输入健壮 (模糊测试保证)
- 无 unsafe 泄漏 (cargo geiger CI 门)
- 无未定义行为 (MIRI 夜间检查)
- 确定性回放支持调试和时间旅行

**负面**:
- 测试代码量可能超过实现代码 (2:1 比例)
- CI 时间增加 (夜间模糊 ~2 小时)
- MIRI 不支持所有平台 (仅 Linux)

**缓解措施**:
- `cargo nextest` 并行测试运行 (比 cargo test 快 3x)
- 模糊测试仅在夜间运行 (不阻塞 PR)
- MIRI 仅检查关键路径 (PTY, VT 解析器)
- 录制测试用例减少手动构造成本
