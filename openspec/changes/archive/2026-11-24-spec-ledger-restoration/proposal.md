# Proposal

## Why

`openspec/specs/upstream-alignment/` 被删除时，连同其承载的 28 条现行约束与 7 张台账一起退场，
但 `research-ledger` 的索引表与补齐行台账仍逐行指向这些条目名。现状有三类后果：

- 索引表里 54 处「能力裁定补齐行」、39 处「语料判定补齐行」、19 处「参照面与反面做法收敛
  补齐行」以及其余 10 余处条目名引用全部落空：报告结论仍在索引行的「判定」列里，但其
  可复核依据所在的台账已不在规范内，`research-ledger` 自己要求的「MUST NOT 因文档已删而
  让结论失去落点」被违反。
- 删除同时带走了一批与研究台账无关的现行约束：跨 JNI 的入参收窄与异常分级、扁平数据
  不用 JSON、OSC 52 读取请求逐帧有界、上游 VT 语义不由本仓测试覆盖、同一上限只由一个
  具名常量表达、规范内路径指针必须指向现存文件。这些是代码行为的硬约束，不是研究结论。
- `docs/persistence/terminal-protocols.md` 仍要求「rev 升级后须重核「上游 API 取用判定
  登记」」，而该登记条目已不存在。

## What Changes

- 把 28 条现行约束按职责归入既有能力，不重建 `upstream-alignment` 这类模糊分类：
  - `code-health`：上游 rev 跟踪、上游 API 取用判定登记、不采用回调不得留通道描述、
    选择手势类上游 API 的不采用依据、同一上限只由一个具名常量表达、规范内路径指针规则；
    「禁改区待维护者裁定项台账」去掉标题里的括号后缀，使引用它的条目名能精确对上。
  - `terminal-state-regression`：上游 VT 语义不由本仓测试覆盖、上游测试资产采纳门槛、
    语料实测与分流裁定、未采纳用例逐例理由、同类安卓终端做法的成对比较与按覆盖面缺口
    采纳、VT 能力缺口按绑定层与终端分别记录。
  - `research-ledger`：能力裁定、语料判定、参照面与反面做法、Android 侧复查、已复查结论、
    六大上游对标、研究阶段残留与覆盖缺口七张台账，以及「复查的否定结论不得用未经验证的
    概括」这条台账纪律。
  - `jni-boundary`：跨 JNI 扁平数据用直接量、保留 JSON 的边界登记、入参不得静默收窄、
    抛错后不得再调 JNI。
  - `pty-lifecycle`：OSC 52 读取请求的收割与应答须对每个会话有界。
- 新增能力 `osc-coverage`：OSC 段的处理归属与核对表（本仓不自建 OSC 解析器，核对表使各段
  归属可脱离代码检索直接读到）。
- 台账内指向已删条目的引用改为现行条目名：`upstream-alignment`「OSC 52 读取请求的收割与
  应答须对每个会话有界」改为 `pty-lifecycle` 同名条目；OSC 133 的「未声明」判定改指
  `research-ledger`「能力裁定补齐行」中「输出分块与命令状态着色（OSC 133 语义）」行。

## Capabilities

### New Capabilities

- `osc-coverage`：OSC 段的处理归属与核对表。

### Modified Capabilities

- `code-health`：补上游 rev 跟踪、上游 API 取用登记等六条约束；禁改区台账条目改名并同步其
  内部引用。
- `terminal-state-regression`：补上游 VT 语义与测试资产采纳的八条约束。
- `research-ledger`：补七张台账与台账纪律条目。
- `jni-boundary`：补入参收窄、异常分级、扁平数据与 JSON 边界四条约束。
- `pty-lifecycle`：补 OSC 52 读取请求逐帧有界一条约束。

## Impact

- `openspec/changes/2026-11-24-spec-ledger-restoration/`：提案、任务与六份规约增量。
- `openspec/specs/`：归档时同步上述条目，并新增 `osc-coverage` 能力目录。
- 源码、`docs/specification/`、受保护文件：均不修改。
