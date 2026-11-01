# Proposal

## Why

原生测试套件里有 4~5 个用例**时红时绿**，红的原因不是产品代码有缺陷，而是用例
把「机器足够快」当成了前提。这类用例的判据一旦依赖机器速度，就不再是回归钉：
它会在 CI 的共享机器上随机失败，在本地全绿，把真实的缺陷淹没在噪声里。

噪声有两个来源：

1. **逐行 `vt_write` 把一条字节流拆成成百上千条命令。**
   命令批尾是**唯一**的帧发布点，每满 64 条命令就要重建一次整幅网格。压力用例
   逐行写 2 500 行时，同一条字节流被拆成 2 500 条命令，于是要付 39 次与被测
   语义（回滚深度、顺序、查询期限）毫无关系的整幅网格重建。
2. **固定期限按「快机器」标定。**
   `flush_with_timeout(5s)`、`flush()` 的固定 10s 排空期限，都是在本机
   （ReleaseFast 上游 Zig 库、全量套件并行）标定的。上游 `libghostty-vt` 的 Zig
   部分在 cargo 的 dev profile 下走 **Debug 优化模式**（`libghostty-vt-sys` 的
   `zig_optimize_mode()` 读 `DEBUG` 环境变量），Debug 模式下
   `Screen.cursorDownScroll` / `PageList.grow` 每次滚动都要跑一遍
   `assertIntegrity()`（全量遍历页面与 tracked pins）——同一份字节流的处理成本
   因此随构建配置相差数倍至数十倍。

实测（同一台机器、同一份字节流）：

| 上游 Zig 构建模式 | `rows=24 lines=1600` 的处理耗时 | 全量套件 |
| --- | --- | --- |
| `ReleaseFast` | 3.76ms | 787 passed / 19.76s |
| Debug（cargo dev profile 默认） | 数倍至数十倍 | 787 passed / 69.36s（改动前 4~5 项随机红） |

上游库的构建模式在 `.cargo/config.toml` / `native/build.rs` 里控制，而这两个文件
是**受保护文件**（改动须用户明确同意），故本次不在构建配置上动手，改为让**用例
本身不再把机器速度当判据**。

## What Changes

- `ghostty_terminal::tests`：新增 `write_lines` 辅助，把整段行文本**一次** `vt_write`
  入队。对上游是同一条字节流（解析器跨调用保持状态，分片与整段等价），被测语义不变。
- `selection_search.rs` / `grid_text.rs` 的三个压力用例改用 `write_lines`，
  并把「排空」从无期限的 `flush()` 换成 `flush_with_timeout(DRAIN_TIMEOUT)`，
  超时时给出「排空未完成」这一明确断言，而不是一个含混的深度偏差。
- `DRAIN_TIMEOUT` 定为 30s、`batch_latency.rs` 的排空期限放宽到 60s、
  `flush_with_timeout` 放宽到 30s：期限只用来把「VT 线程卡死」与「机器慢」分开，
  不参与被测语义判定——命令批无界时通道恒非空，再长的期限也等不到排空，
  故放宽期限不削弱判别力。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `terminal-state-regression`：新增一条 Requirement，把「压力用例不得把机器速度
  当判据」这条既有实践写成规范——它此前只体现为零散的期限数值，没有成文口径。

## 不做什么

- 不改 `.cargo/config.toml` 与 `native/build.rs`（受保护文件）：不把上游 Zig 库的
  优化模式固定为 `ReleaseFast`。改动须用户明确同意，本次不越界。
- 不改上游 `libghostty-vt` 的任何代码（外部 git 依赖，非本仓）。
- 不缩短任何现有期限来「换回」速度：期限放宽不削弱判别力（见上）。
- 不动 `docs/specification/`（禁改区）与 `docs/STATUS.md`（文件首行明禁修改）。

## Impact

- Affected specs: `terminal-state-regression`（一条 ADDED Requirement）。
- Affected code:
  - `native/src/terminal/ghostty_terminal/tests/mod.rs`（新增 `write_lines`、`DRAIN_TIMEOUT`）
  - `native/src/terminal/ghostty_terminal/tests/selection_search.rs`（3 个压力用例）
  - `native/src/terminal/ghostty_terminal/tests/grid_text.rs`（1 个查询期限用例）
  - `native/src/terminal/ghostty_terminal/tests/batch_latency.rs`（排空期限放宽）
