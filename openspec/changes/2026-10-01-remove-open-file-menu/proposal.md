# 移除长按菜单“打开文件”选项

## Why

长按菜单的“打开文件”项按选中文本形态猜测用户意图并外跳应用，既非终端仿真必需，也与“菜单只做文本操作”的最小实现冲突。移除后菜单仅保留文本操作与 OSC 8 超链接跳转。

## What Changes

- `TerminalSurface` 选择菜单不再组装“打开文件”项；删除 `openSelectionAsFile`、`toastCannotOpenFile`、`isFilePathCandidate` 与 `MAX_SELECTION_ACTION_LENGTH`。
- 删除 `open_file`、`open_file_failed` 字符串；删除已无消费者的 `TerminalFileProvider`、清单 `provider` 项与 `res/xml/file_paths.xml`。
- `openspec/specs/text-selection` 删除文件相关条目与场景；`docs/specification/DESIGN.md` 删除打开文件条款（已获用户明确同意）。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 选择菜单内容：有内容区域条目为复制、分享、全选，另按 OSC 8 超链接附打开链接；不再含打开文件。

## Impact

- 影响 `android/app/src/main`（`ui/TerminalSurface.kt`、`util/TerminalFileProvider.kt`、`AndroidManifest.xml`、两处资源）、`openspec/specs/text-selection`、`docs/specification/DESIGN.md`（已获用户明确同意）。不改其他保护文件，不新增功能。
