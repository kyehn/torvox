# Design

## Context

见 proposal.md 的 Why。此处只记实现本变更时必须成立的事实前提：

1. `openspec/specs/upstream-alignment/spec.md` 是**次级规范**，行为与架构的真值在
   `docs/specification/`（禁改区）。本变更只订正次级规范里与真值不符的陈述。
2. 该规范里带索引表的两条 Requirement 分别是
   「研究报告逐份结论登记在本规范内」（正文 29 行）与
   「研究报告逐份索引与文档可移除性」（正文含 111 行索引表，198 行）。
   漂移全部发生在这两处的正文里，索引表的**行**本身除一处判定理由外都成立。
3. 索引表的行数为 111（脚本按 `| \`…\`` 逐行计数，键名无重复）：
   前 6 行是综合研究报告，其后 74 行是逐仓报告 73 份 + `ecosystem.md`，
   末 31 行是退役编号文档集（`00`–`30` 去 `26` 的 30 份 + 生态汇总报告）。
   这与正文「已退役 105 份 + 现役 6 份」的旧口径一一对应，故新口径是
   「全部 111 份已退役」，不是新造的数字。
4. `docs/specification/PROHIBITED.md` 现行「配置文件」节写的是
   「解析 `termux.properties` 中未在 `config.md` 列出的键，以及 `colors.properties`、
   `termux.float.properties`。允许读取的键仅按 `config.md` 列出；其他键一律忽略。」
   ——禁的是**未列出的键**，不是这个文件本身。
5. 实现的键面：`TermuxPropertiesParser.parse` 读 16 个标量键
   （`TermuxProperties.kt` 的 `boolean`／`integer`／`float`／枚举分支与
   `extra-keys`／`default-working-directory`），外加
   `SessionShortcut.entries` 的 4 个 `shortcut.*` 键，合计 20 个，
   与 `docs/specification/config.md` 的「键」节逐条对应。

## Goals / Non-Goals

**Goals:**

- 让 `upstream-alignment` 的每处事实陈述都能被 `git` 或文件实测复核。
- 索引表的 111 行仍能独立回答「该报告的结论与依据」，删除文档后不丢结论。
- `docs/spec-code-gaps.md` 的键面计数与 `config.md`／实现一致。

**Non-Goals:**

- 不重判任何报告的「采纳／不采纳」结论，只订正支撑该结论的理由陈述。
- 不把 111 行索引表拆成多条 Requirement——它是一个键空间，拆开会让「按文件名查」这件事
  需要跨多条拼接。
- 不动 `docs/specification/`、`docs/STATUS.md` 与任何归档变更。

## Decisions

### 决策一：用 MODIFIED 整体替换两条 Requirement，不新增「口径更正」条目

漂移的是既有 Requirement 正文里的一句话，不是新增能力。若新增一条更正 Requirement，
同一事实会在规范里同时存在两个版本，读者仍要判断哪个为准。

- 备选（否决）：新增 Requirement「研究目录退役口径更正」并声明优先——会制造两处登记，
  且 openspec 的 MODIFIED 机制本就是为「既有 Requirement 正文变了」准备的。

### 决策二：份数口径取「全部 111 份已退役」，不保留「现役」表述

`docs/research/` 已随 `f8b1d762` 整体删除，`git log --diff-filter=D -- 'docs/research/*'`
可复核 6 份综合研究报告与 99 份逐仓／编号报告的删除提交。正文继续写「现役」会让人
去一个不存在的目录取原文。

- 备选（否决）：把「现役」改写为「曾现役」但仍单列一组——「现役／曾现役」的区别只在
  文档是否在库，而结论落点（索引行）在两种情况下都是同一张表，分组反而制造
  「在库的行要另行维护」的错觉。

### 决策三：`termux-app.md` 索引行只改判定理由，判定结论仍为「不适用」

该报告的可执行项里，「上游 `termux.properties` 及其设置项体系」对本仓不适用——
本仓没有上游那套设置页，键面由 `config.md` 固定。变的只是「为何不适用」的理由：
旧理由「`PROHIBITED.md` 明禁解析该文件」已不成立；新理由是
「上游设置项体系本仓无该设置面；文件本身按 `termux-configuration` 只解析
`config.md` 列出的允许键」。

- 备选（否决）：把该行判定改成「部分采纳」并登记 20 个键的落地——键的逐条语义已由
  `termux-configuration` 承接，索引行再登记一遍会与该 Requirement 形成两套键面清单，
  而本规范的要求是索引行「指向」既有落点，不是复制落点。

### 决策四：`docs/spec-code-gaps.md` 的键面计数取 20，并补表内 4 行

`PROHIBITED.md` 把 `config.md` 定为「允许读取的键」的权威清单，实现按 `config.md`
逐条解析，故权威计数是 20。旧文写 16 是 `shortcut.*` 实现之前留下的，
且同一文档的结论节已经描述了 `shortcut.*` 的实现——同一文档内部已经不自洽。

- 备选（否决）：维持 16，把 `shortcut.*` 写成「另 4 个快捷键」附在表后——键面一分为二后，
  「未在 `config.md` 列出的键一律忽略」这条禁令就没有单一可数的边界了。

### 决策五：审计轮次记录按轮次升序重排

第 1 轮条目被排在第 2–5 轮之后且孤立成段。轮次记录是按轮次推进的时间线，
倒序会让「第几轮查了什么」不可读。重排只动顺序，不动内容。

## Risks / Trade-offs

- [Risk] MODIFIED 增量要完整复制两条 Requirement（其中一条含 111 行索引表），
  手抄必然出错 → 用脚本从 `openspec/specs/upstream-alignment/spec.md` 原文切出整块
  Requirement，再对切出的文本做定点替换，逐条断言被替换串在全文只出现一次；
  归档后逐字 diff 主 spec，确认只有预期改动。

- [Risk] 归档命令把 delta 写回主 spec 时若按「追加」而非「替换」处理，会产生重复
  Requirement → 归档后立刻用 `grep -c '^### Requirement:'` 核对条目数不变，
  并 `git diff` 逐行复核。

- [Trade-off] 索引表 111 行留在单条 Requirement 内，规范文件偏大；这是既有形态，
  拆分收益不足以抵消「按文件名查一条落点」的检索退化，本次不动。

## Migration Plan

纯文档变更，无数据迁移。回滚方式是 `git revert` 归档提交。

## Open Questions

无。四处漂移的判定依据全部可在仓内实测（`git log`、文件计数、解析器键面），
不需要推迟到日后回答的问题。
