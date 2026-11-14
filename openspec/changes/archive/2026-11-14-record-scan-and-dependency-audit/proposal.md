# Proposal

## Why

`code-health` 能力把审计留档的落点写死在 `upstream-alignment` 的「实测基线」小节：
静态形状扫描与重复率的实测数字、扫描参数与逐处判定写在该处，依赖活跃度的核对结论
也 MUST 写入同一小节。

该小节当前只有重复率（`jscpd`）与禁改区门禁项的基线：

- **静态形状扫描**的逐处判定仍留在归档变更 `2026-11-05-footgun-triage` 的 `triage.md`
  里。那份留档记录的是当时的代码树（主源码集 81 个文件、10 处命中），此后字体管理被
  拆到 `FontManager.kt`、多处实现重构，命中位置与文件数都已过时；规约要求的落点没有
  这项记录。
- **依赖活跃度**的核对结论只在归档变更 `2026-11-05-code-health-audit` 的 `audit.md`
  里，采样于 2026-11-05，且漏了后加入的 `wgpu-types`；规约要求的落点同样没有这项记录。

两项都是「扫描命中必须逐条判定并留档」与「直接依赖必须处于活跃维护状态」的硬要求，
落点缺失即视为审查未完成。

## What Changes

- 在 `upstream-alignment` 的「实测基线」小节补写**静态形状扫描**的扫描参数、当日实测
  数字与 10 处命中的逐处判定（判据取各命中指向的陷阱文档，结论取现场代码）。
- 同节补写**依赖活跃度**核对：crates.io 侧 28 个直接依赖的累计下载量、近 90 天下载量
  与最近发布（当日采样），Maven/Google 侧 47 个直接依赖的最近发布事实，以及两仓库
  不公开下载量这一事实本身。
- 同节按 2026-11-14 重测结果刷新 `jscpd` 基线的文件数（克隆数、重复行数与重复率不变）。
- 位置表述用函数名与变量名，不用易变动的行号（遵 `docs/specification/STYLE.md` 文档节）。
- 不改任何源码行为，不改受保护文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `upstream-alignment`：「禁改区待维护者裁定项台账」的实测基线小节补入审计留档。

## Impact

- `openspec/changes/2026-11-14-record-scan-and-dependency-audit/`：提案、任务与规约增量。
- `openspec/specs/upstream-alignment/spec.md`：归档时同步上述补写内容。
- 源码、`docs/specification/`、受保护文件：均不修改。
