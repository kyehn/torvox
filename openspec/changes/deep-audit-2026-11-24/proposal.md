## Why

本轮对仓库进行逐子系统深度审查，确认现行实现的正确性与可维护性，并把审查证据固化到 openspec，避免后续重复排查。审查发现两处确实需要修复的缺陷：安装器诊断文本截断可能切断 UTF-16 代理对；openspec 中存在被项目规则禁止的模糊分类目录 `upstream-alignment/`。

## What Changes

- 安装器诊断文本截断统一改用 `truncateTextAtCharBoundary`，避免切断代理对产生无效字符串。
- 删除 `openspec/specs/upstream-alignment/` 及其全部引用行，恢复现行规范目录整洁。
- 记录本轮深度审查覆盖的子系统、验证命令与结果，作为后续回归基线。

## Capabilities

本变更为缺陷修复与文档整理，不改变任何规范级行为：`skip_specs: true`。

## Impact

- `android/app/src/main/java/terminal/emulator/installer/SecondStageRunner.kt`
- `android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt`
- `openspec/specs/` 与相关引用文档
