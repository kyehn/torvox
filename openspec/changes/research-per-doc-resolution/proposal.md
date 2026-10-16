# Proposal

## Why

用户要求把 `docs/research` 两份文档集（用户指定 commit `9aea751d` 的 31 份编号文档集 +
`8f5bc831` 的 74 份现行报告，合计 105 份）**逐份**阅读、把每句话指向的待办**逐个**解决、
解决后**逐份**删除，全过程串行、每小步独立提交，不得并发、不得互相牵连。

归档变更 `2026-10-15-research-docs-retirement` 与 `2026-10-15-research-ledger-closure`
曾一次性删除 74 份并以「报告文件名为键」的索引表承接结论，但那是**整批**处置：单份报告的
句子是否逐句有着落，无法从提交历史上逐份核对，也无法定位是哪一步把哪句话判掉的。
本变更把同一件事改成**逐份台账**：一份文档一个小节、一次提交，删除前必须先证明该份的
每个可执行句都有去向（已实现／已裁定采纳／已裁定不采纳并写明理由／`PROHIBITED`／
未声明不立项），否则不许删。

## What Changes

- **`upstream-alignment` 新增 Requirement「研究报告逐份处置台账」**：以 105 份报告文件名为键，
  每行写明处置状态（逐份已读／逐句去向／删除提交），并写入两条硬约束：单份文档的删除
  MUST 与其结论落点同批提交；同一提交 MUST NOT 混入其它报告的删除。
- **`docs/research/` 逐份删除**：按 `tasks.md` 的固定顺序（编号集 `00`–`30` 在前，其后的
  74 份按删除前提交内的目录序），每份读全、逐句判定、把缺失结论补进规范或代码后删除该份，
  单独提交。
- **真实缺口修复**：逐份核对中发现的、规范或代码尚未承接的可执行项，按 `DESIGN` 与
  `PROHIBITED` 裁定后落地（代码 + 测试 + 规范），与触发它的那份报告同批提交。
- **收尾**：全部 105 份处置完毕后，连续三轮独立审阅，直至无问题。

## Capabilities

- **New Capabilities**: 无。
- **Modified Capabilities**: `upstream-alignment`（新增逐份处置台账 Requirement；逐份核对
  中若发现某能力缺口，其 delta 追加到本变更对应能力目录并在 `tasks.md` 登记）。

## Impact

- Affected specs: `openspec/specs/upstream-alignment/spec.md`（新增台账 Requirement）。
- Affected docs: `docs/research/`（105 份逐份删除）。
- Affected code: 仅当逐份核对发现真实缺口时改动触发该缺口的实现与用例。
- 行为变化：无（除非某份报告的结论被判定为真实缺口并落地，届时在该份的小节内写明）。
