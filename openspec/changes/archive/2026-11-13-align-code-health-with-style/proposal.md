# Proposal

## Why

`code-health` 能力里有两条要求与当前仓库事实正面冲突，每轮复查都会重新撞上：

- **「超大文件阈值必须跨源码语言一致应用」要求登记未完成项并按阈值拆分源码文件。**
  `docs/specification/STYLE.md`（行为与架构的规范来源）已把「行数、嵌套层数、文件大小」
  明列为仅风格相关的问题，可忽略且不得为其耗时。于是同一件事在两处规范里得到相反指令：
  一处要求为行数拆分并登记未完成，另一处禁止为此耗时。
- **「审计留档」没有落点。** 承载重复率、静态形状扫描与依赖活跃度核对记录的那份留档
  已随过期的审计文档一并删除，规约里的六处引用因此全部指向不存在的文件。扫描类判定
  已有新家（`upstream-alignment` 的「实测基线」小节），规约却还在说「审计留档」。

两者都不是缺陷修复，而是让规约重新可解析：留档要指得到，风格问题要归口。

## What Changes

- `code-health` 的「超大文件阈值必须跨源码语言一致应用」改为服从 `STYLE.md`：不因文件
  大小或行数登记未完成项，也不为凑行数拆分；拆分仍按职责边界，不按行数切。
- 新增一条「审计留档的落点必须可解析」，把审计留档指向 `upstream-alignment` 的
  「实测基线」小节，并要求留档被删除或改名时同步更新落点。
- 关闭 `code-health-reference-review` 中已随上述裁定失效的「按同一阈值拆分界面与运行时侧
  超大文件」待办；另两项需维护者明确同意的待办保持未完成。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `code-health`：文件规模治理归口到 `STYLE.md`，审计留档落点写实。

## Impact

- `openspec/changes/2026-11-13-align-code-health-with-style/`：提案、任务与规约增量。
- `openspec/specs/code-health/spec.md`：归档时同步上述两条要求。
- `openspec/changes/archive/2026-11-08-code-health-reference-review/tasks.md`：关闭失效待办。
- `docs/specification/`、`docs/persistence/`、`scripts/`、源码：均不修改。
