# Proposal

## Why

`docs/extensibility-research.md` 是本仓自产的可扩展性研究报告（库化交付、会话传输接口扩展点、
远程后端选型）。它有两处与本仓规则冲突：

1. **研究台账协议**：该能力要求「每份研究报告 MUST 有一条以报告文件名为键的索引行」，
   且「索引完整是删除研究文档的前置条件；满足后报告本体 MUST 可被删除」。该报告未入索引，
   属于「缺少索引行的报告 MUST NOT 入库」。
2. **静态门禁红**：`semgrep` 的「禁 API 引用」规则对全仓 Markdown 生效，
   报告正文照录 `PROHIBITED.md` 明禁项时命中 3 处阻断（`docs/persistence/` 与 `README.md`
   按「参考资料照录事实」的既有口径豁免，本报告不在该口径内）。

报告的可执行项全部指向「不采纳／未立项／需用户明确同意」，无任何仍需遵守的结论待落地，
故按协议先登记索引行、再删除本体。

## What Changes

- `research-ledger` 索引表新增一行 `extensibility-research.md`：写明其实质、判定与依据，
  并把「全部 111 份报告已整体删除」的计数口径补上本仓自产的第 112 份。
- 删除 `docs/extensibility-research.md`（72 行）。

## 不做什么

- 不实现报告提出的任何扩展面：应用内插件运行时／脚本加载、Model Context Protocol、
  Termux:API 兼容层均撞 `PROHIBITED.md` 明禁。
- 不做库化拆分与远程后端：`Cargo.toml`、`settings.gradle.kts`、`build.gradle.kts` 属受保护文件，
  改动须用户明确同意；远程后端属产品决策且 `DESIGN.md` 未声明该功能面。
- 不放宽 `semgrep` 规则、不新增豁免路径：门禁保持原样，靠删除违规报告转绿。
- 不改 `docs/specification/`、`docs/STATUS.md`、`docs/persistence/`。

## Impact

- Affected specs: `research-ledger`（索引表新增一行 + 计数口径校正）。
- Affected docs: `docs/extensibility-research.md`（删除）。
- Affected code: 无。
- 行为变化：无（纯文档与规范整理）；静态门禁由 3 处阻断转为 0。
