# Proposal

## Why

第 6 轮全量审计发现 `openspec/specs/upstream-alignment/spec.md` 有四处事实漂移，
都指向「规范说的不是仓库现在的样子」：

1. 该规范三处仍称有 6 份综合研究报告「现役在 `docs/research/`」，但 `docs/research/`
   已被提交 `f8b1d762`（`chore: 移除 docs/research 并收敛 STATUS 与 semgrep 覆盖配置`）
   整体删除，目录今日并不存在。规范因此把一份已不存在的目录登记成「现役」，
   读者按此索引行去 `docs/research/` 找原文必然落空。
2. `termux-app.md` 的索引行把上游 `termux.properties` 归入「不适用」，理由写的是
   「`PROHIBITED.md` 明禁解析该文件」。但 `PROHIBITED.md` 现行口径是
   「允许读取的键仅按 `config.md` 列出；其他键一律忽略」，本仓已实现并解析
   `config.md` 列出的 20 个键（`termux-configuration` 有对应 Requirement）。
   索引行给出的理由与现行禁令、与现行实现都不符。

两处都落在「索引行 MUST 能独立回答该报告的结论与依据」这条 Requirement 身上，
而这条 Requirement 正是本仓删除研究文档的前置条件——前提写错，前置条件就失去意义。

## What Changes

- 订正 `upstream-alignment` 的「研究报告逐份结论登记在本规范内」：
  研究报告口径由「已退役 105 份 + 现役 6 份」改为「全部 111 份已整体退役」。
- 订正 `upstream-alignment` 的「研究报告逐份索引与文档可移除性」：
  - 正文与索引表说明改为「6 份综合研究报告曾现役，后随 `docs/research/` 整体删除，
    其结论由同键索引行承接」；
  - `termux-app.md` 索引行的 `termux.properties` 判定理由由「`PROHIBITED.md` 明禁解析该文件」
    更正为「上游设置项体系本仓无该设置面；文件本身按 `termux-configuration` 只解析
    `config.md` 列出的允许键」。
- 同步订正 `docs/spec-code-gaps.md`（审计记录，非规范）：
  - 「配置键面：16 个键」漏计 4 个 `shortcut.*` 键，更正为 20 个键并补表内对应行；
  - 审计轮次记录顺序错乱（第 1 轮条目孤立在末行），按轮次顺序重排。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `upstream-alignment`：两条 Requirement 的事实陈述订正——
  「研究报告逐份结论登记在本规范内」（研究文档全部退役后的份数口径）与
  「研究报告逐份索引与文档可移除性」（索引表是唯一常驻载体；`termux-app.md`
  索引行的 `termux.properties` 判定理由）。

## 不做什么

- 不改 `docs/specification/`（禁改区）：本变更的判定依据全部来自其现行文本，
  规范侧只订正 openspec 的次级规范。
- 不重开任何已裁定的能力面：索引行只订正事实陈述，不改判定结论。
- 不删除、不新增任何研究报告：全部 111 份早已退役，本变更不触碰 `git log` 中的历史。
- 不改归档变更：`openspec/changes/archive/` 保持原样，它是历史台账而非现行规范。

## Impact

- Affected specs: `upstream-alignment`（两条 Requirement 的 MODIFIED 增量）。
- Affected docs: `docs/spec-code-gaps.md`（键面计数与审计轮次顺序）。
- Affected code: 无。纯规范与文档订正，不改变任何运行时行为。
