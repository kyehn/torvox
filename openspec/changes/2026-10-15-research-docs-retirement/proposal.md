# Proposal

## Why

用户要求：`docs/research` 两份文档集（用户指定 commit 的编号文档集 31 份 + 当前 74 份）
的每一句可执行结论都被完全解决，已解决问题的文档部分被移除，最终全部研究文档可被移除。

现状盘点（本轮逐份复读 74 份 + 归档台账比对）：

1. **索引层缺失**：规范内已有「已复查结论台账」（语料）、「Android 侧终端项目的复查台账」
   （25 项）、「能力裁定补齐行」、「禁改区待维护者裁定项台账」、「OSC 覆盖逐项核对」等表，
   但它们以**资产/能力**为键，不以**报告文件**为键。实测 74 份报告中有 20 余份
   （`zebra`、`fbpad`、`HiSH`、`batata`、`lumina`、`havoc`、`Bobcat`、`TerminalFX`、
   `iced_term`、`zashterminal`、`omnipty`、`zerotty`、`DomTerm`、`termora`、`libvtty`、
   `qwertty`、`ketraterm`、`kmscon/libtsm`、`BossTerm`、`mintty`、`fyne` 等）
   在全部表内既无文件名、也无仓库名出现。删掉任何一份前，无法判定「删了会不会丢结论」——
   这正是「可移除」的前置条件。
2. **一份文档仍承载有效判断**：`ecosystem.md` 的参照面评级表、禁止项清单、测试边界三节
   是唯一仍被规范正文引用的内容；它必须并进规范，否则删除即丢判断。
3. **旧的编号文档集已被删除**，但其处置方式（31 份 → 现行报告 + 规范）在规范内无登记。

## What Changes

- **`upstream-alignment` 新增 Requirement「研究报告逐份索引与文档可移除性」**：
  以**报告文件名为键**的 74 行索引表，每行写明实质、判定（采纳／不采纳及门槛／
  未声明不立项／PROHIBITED／仅参照）、依据位置；并写入可移除性条款
  （索引完整后 `docs/research` MUST 可整体删除；日后若重新引入报告，索引行 MUST 同批提交）。
- **删除 `docs/research/`**：全部 74 份（73 份逐仓报告 + `ecosystem.md`）。
  `ecosystem.md` 仍有效的判断（参照面评级、禁止项、测试边界）由索引表与既有台账承接。
- **清引用**：`FontFileOverrideInstrumentedTest` 的注释指向 `docs/research`，改为指向
  `openspec/specs/upstream-alignment` 的禁改区待维护者裁定项台账（该边界的常驻位置）。
- **收尾在途变更**：`2026-10-15-research-ledger-closure` 的三轮审阅与禁改区登记任务
  在本变更内一并完成并归档。

## 不做什么

- 不改 `scripts/`、`.github/`、`android/app/build.gradle.kts`（AGENTS.md 禁改区）。
- 不为索引新增任何功能面：索引只登记既有判定，不重开已裁定的能力面。
- 不重写归档变更（历史台账保持原样）。

## Impact

- Affected specs: `upstream-alignment`（新增索引 Requirement）。
- Affected docs: `docs/research/`（整体删除）。
- Affected code: `android/app/src/androidTest/.../FontFileOverrideInstrumentedTest.kt`（注释指向）。
- 行为变化：无（纯文档与规范整理）。
