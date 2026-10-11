# Proposal

## Why

`upstream-alignment` 规范有一条自己没兑现的要求：

> 不满足者 MUST NOT 引入，**MUST 在本规范中记录不引入的理由**，使结论可被复查。

以及一条 Scenario 要求「每项都给出违反上述哪一条的理由；不得写成『其断言是自身期望值』这类**未经验证的概括**」。

实际情况：主规范 `openspec/specs/upstream-alignment/spec.md` 里**一条结论都没有**。九套上游
资产（alacritty / wezterm / kitty / termux / rio / contour / foot / xterm.js / esctest2）的判定
只存在于归档变更 `2026-10-12-record-upstream-test-asset-evaluation` 的 proposal 里。归档是历史
台账、不是活规范；结论因此无法被复查，只能靠翻归档。

同时，规范的「阻塞条件消失后必须重新评估」条款要求复查 esctest2 的阻塞点，而阻塞点是否仍在
**从未在本规范内被复核过**。

## What Changes

- 把九套资产的逐项结论与实测数字登记进主规范，每条给出可复核依据（源码位置或语料事实），
  不再依赖归档 proposal。
- 复核 esctest2 的阻塞条件：`DECRQCRA` 在绑定层所钉的终端提交里是否仍缺失。
- 实测唯一未被结构性否决的语料（Unicode UAX #29 字素簇），判定它对本仓自有字素搬运通道的
  判别力，判定后删除探针，只把数字与差异聚类写进记录。
- 顺带修一处本仓真实隐患：渲染路径把字素簇上限硬编码成 `7`，与快照路径共用的具名常量
  `MAX_GRAPHEME_CLUSTERS = 8` 脱钩。

### New Capabilities

无。

### Modified Capabilities

- `upstream-alignment`：新增「已复查结论登记在规范内」要求，承载逐项结论表、实测记录表，
  以及具名常量 MUST 取代派生字面量的通用约束。

## Impact

- `openspec/specs/upstream-alignment/spec.md`：新增一条 Requirement（含两张表）。
- `native/src/terminal/ghostty_terminal/internal.rs`：渲染路径的 `[0u32; 7]` 与 `.take(7)`
  改用 `MAX_GRAPHEME_CLUSTERS - 1`。行为不变（今日两条路径本就一致，实测已证）。
- 无新增依赖、无二进制资产、不 vendor 语料、不改运行时行为。
- 探针 `native/tests/upstream_probe.rs` 为一次性产物，判定后删除，不进版本库。
