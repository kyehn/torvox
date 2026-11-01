# 任务

## 一、事实核定（第 6 轮审计的实测记录）

- [x] 确认 `docs/research/` 不存在：`ls docs/` 实测目录只有 `STATUS.md`、`spec-code-gaps.md`、
      `persistence/`、`specification/`；`git log --diff-filter=D -- 'docs/research/*'`
      可复核 6 份综合研究报告（`f8b1d762`）与 99 份逐仓／编号报告的删除提交。
- [x] 确认索引表行数为 111 且键名无重复：脚本按 `| \`…\`` 逐行计数，
      前 6 行是综合研究报告、其后 74 行是逐仓报告 73 份 + `ecosystem.md`、
      末 31 行是退役编号文档集（`00`–`30` 去 `26` 的 30 份 + 生态汇总报告）。
- [x] 确认 `PROHIBITED.md` 现行口径：禁的是「未在 `config.md` 列出的键」，
      不是 `termux.properties` 文件本身；`config.md` 的「键」节列出 20 个键。
- [x] 确认实现的键面：`TermuxPropertiesParser.parse` 读 16 个标量键，
      外加 `SessionShortcut.entries` 的 4 个 `shortcut.*` 键，合计 20 个。
- [x] 确认 `termux-app.md` 索引行的旧理由失效：其写作「`PROHIBITED.md` 明禁解析该文件」，
      与现行 `PROHIBITED.md`／`termux-configuration` 均不符。

## 二、规范增量

- [x] `upstream-alignment` MODIFIED「研究报告逐份结论登记在本规范内」：
      份数口径由「已退役 105 份 + 现役 6 份」改为「全部 111 份已整体退役」。
- [x] `upstream-alignment` MODIFIED「研究报告逐份索引与文档可移除性」：
      - 正文段改为「全部 111 份已整体删除，索引表是唯一常驻载体；6 份综合研究报告的索引行
        MUST 承接其全部仍需遵守的结论」；
      - Scenario「索引覆盖到文件」的 WHEN 补「（现已整体退役）」；
      - 索引表说明段改为「曾现役、后随 `docs/research/` 整体删除」；
      - `termux-app.md` 索引行的 `termux.properties` 判定理由由「`PROHIBITED.md` 明禁解析该文件」
        更正为「上游设置项体系本仓无该设置面；文件本身按 `termux-configuration` 只解析
        `config.md` 列出的允许键」。
- [x] 增量由脚本从主 spec 原文切出整块 Requirement 再做定点替换生成，
      每个被替换串断言在全文只出现一次；归档前逐字 diff 确认只有上述 5 处改动，
      111 行索引表全部保持单行（不得在表格行内换行，否则表格断裂）。
- [x] `openspec validate --all --strict` 通过。

## 三、审计文档同步（`docs/spec-code-gaps.md`，非禁改区）

- [x] 「配置键面」小节由「16 个键」更正为「20 个键」，表内补 `shortcut.create-session`／
      `shortcut.next-session`／`shortcut.previous-session`／`shortcut.rename-session` 四行；
      结论节同步更正「实解析 16 个键」的口径。
- [x] 「审计轮次记录」按轮次升序重排：第 1 轮条目原先孤立排在末行，现移到列表首位；
      补第 6 轮记录（含本变更的三处漂移与实测依据）。

## 四、验证

- [x] 归档后主 spec 的 `### Requirement:` 条目数与 `#### Scenario:` 条目数不变
      （MODIFIED 为替换而非追加）。
- [x] 全仓 grep 确认无「现役在 `docs/research/`」类悬空陈述残留。
- [x] markdownlint（`openspec/**/*.md` 与 `docs/spec-code-gaps.md`）0 issues。
