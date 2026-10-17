# Proposal

## Why

用户要求把 `docs/research` 两份文档集（退役前快照：31 份编号文档集 + 74 份现行报告，
合计 105 份；逐份清单与顺序见 `tasks.md`）**逐份**阅读、把每句话指向的待办**逐个**解决、
解决后**逐份**删除，全过程串行、每小步独立提交，不得并发、不得互相牵连。

归档变更 `2026-10-15-research-docs-retirement` 与 `2026-10-15-research-ledger-closure`
曾一次性删除 74 份并以「报告文件名为键」的索引表承接结论，但那是**整批**处置：单份报告的
句子是否逐句有着落，无法从提交历史上逐份核对，也无法定位是哪一步把哪句话判掉的。
本变更把同一件事改成**逐份台账**：一份文档一个小节、一次提交，删除前必须先证明该份的
每个可执行句都有去向（已实现／已裁定采纳／已裁定不采纳并写明理由／`PROHIBITED`／
未声明不立项），否则不许删。

## What Changes

- **`upstream-alignment` 新增 Requirement「研究报告逐份处置台账」**：105 份按 `tasks.md` 的固定顺序
  逐份处置（30 份编号报告 → 编号集内那份非编号的生态汇总报告 → 74 份现行报告），每份 MUST 四步
  留证（读全、逐句判入五类去向、补齐缺失去向、删除该份），并写入三条硬约束：单份删除 MUST 与其
  结论落点同批提交；同一提交 MUST NOT 删除两份及以上；指出本仓代码 MUST 用符号名、MUST NOT 用行号。
- **`upstream-alignment` 新增 Requirement「逐份处置补齐行台账」**：逐份核对中发现的、此前无
  规范去向的可执行句逐句补一行（键为报告文件名 + 该句 + 去向），共 105 个键、187 行；
  每行 MUST 给得出可检索的去向（规范条目名、代码路径或不立项判定），且与该份的删除同批提交。
- **`pty-lifecycle` 增量三则**：拆卸可重入／读循环可中断／组杀不误伤本进程、
  网格 resize 被丢弃时不得把旧尺寸当权威、子进程侧 fork／exec 不变量。
- **`docs/research/` 逐份删除**：按 `tasks.md` 的固定顺序（编号报告 → 非编号的生态汇总报告 →
  74 份现行报告，末段为 `LC_ALL=C` 目录序），每份读全、逐句判定、把缺失结论补进规范或代码后
  删除该份，单独提交。
- **真实缺口修复**：逐份核对中发现的、规范或代码尚未承接的可执行项，按 `DESIGN` 与
  `PROHIBITED` 裁定后落地（代码 + 测试 + 规范），与触发它的那份报告同批提交。
- **收尾**：全部 105 份处置完毕后，连续三轮审阅（可复现机械复核／逐句对码／门禁复跑），直至无问题。

## Capabilities

- **New Capabilities**: 无。
- **Modified Capabilities**: `upstream-alignment`（新增逐份处置台账 Requirement；逐份核对
  中若发现某能力缺口，其 delta 追加到本变更对应能力目录并在 `tasks.md` 登记）。

## Impact

- Affected specs: `openspec/specs/upstream-alignment/spec.md`（新增两条台账 Requirement）、
  `openspec/specs/pty-lifecycle/spec.md`（新增拆卸／resize／fork-exec 三则增量）。
- Affected docs: `docs/research/`（105 份逐份删除）。
- Affected code: 仅当逐份核对发现真实缺口时改动触发该缺口的实现与用例。
- 行为变化：无（除非某份报告的结论被判定为真实缺口并落地，届时在该份的小节内写明）。
