# Proposal

## Why

维护者口径：**不测上游，不写重复代码**。仓内两批测试资产违反了这一条：

1. `native/tests/external_benchmarks/`（9 例，2026-10-13 落地）的期望值全部由上游决定：
   Unicode UCD 宽度表、kitty 规范正文与上游编码器字节、ghostty `corpus/stream-initial`
   模糊种子。实测分歧 100% 落在上游与终端惯例（kitty 16 处、宽度 268 个码位），
   本仓产品修复为零；判红的唯一含义是上游 rev 升级改了行为，与本仓功能正确性无关。
   代价是 `cargo test` 变成联网测试（取数失败即 panic）加一个 `ureq` dev-dependency。
2. `native/tests/bdd/` + `native/tests/features/`（7 特性 12 场景）逐条复述 `native/src`
   里已有的单测：12/12 场景都能找到对应且更强的 src 用例（如 `osc_clipboard_split_buffer`、
   `hyperlink_at_returns_uri_inside_link`、`scrollback_keeps_order_and_newest_line_stays_visible`），
   Gherkin 只多一层间接；另有 `cucumber`＋`tokio` 两个 dev-dependency，且
   `TerminalWorld::run("tests/features")` 按进程 cwd 解析路径，从仓库根直接跑二进制即
   0 场景 + 解析错误（只有 cargo 把 cwd 设成包根才通过）。

## What Changes

- 删除 `native/tests/external_benchmarks/`、`native/tests/bdd/`、`native/tests/features/`。
- `native/Cargo.toml`：删 `[[test]] name = "bdd"` 块与 dev-dependencies
  `cucumber`、`tokio`、`ureq`；`criterion`／`proptest`／`shuttle` 保留
  （分别服务 benches、`output_processor` 单测、`EventQueue`）。
- 保留 `native/tests/concurrency.rs`（测本仓 `EventQueue`）。
- `openspec/specs/upstream-alignment/spec.md` 直改（`skip_specs: true`）：
  「上游 VT 语义不由本仓测试覆盖」补外部真值类断言的删除事实与新增
  「外部基准不入仓」Scenario；把「已落地」的台账行更新为「采纳后撤回／已退场」，
  新增台账行「外部基准与 BDD 套件的退场」并写明日后复查 MUST NOT 重建。

## 不做什么

不引入替代套件、不加等价覆盖：被删的断言没有独占覆盖（重复项留在 src 单测，
上游真值类断言按口径不引入）。
