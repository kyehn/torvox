# 任务

## 1. 提案与规约

- [x] 1.1 变更提案与 spec delta 编写，`openspec validate` 通过
- [ ] 1.2 实现后归档变更

## 2. 代码删除

- [ ] 2.1 菜单不再组装打开文件项，删除 `openSelectionAsFile`、`toastCannotOpenFile`、`isFilePathCandidate`、`MAX_SELECTION_ACTION_LENGTH` 与 `FileProvider` 导入
- [ ] 2.2 删除 `open_file`、`open_file_failed` 字符串
- [ ] 2.3 删除 `TerminalFileProvider`、清单 `provider` 项、`file_paths.xml`，更新文档提供器注释
- [ ] 2.4 删除 `isFilePathCandidate` 相关单测，保留链接用例

## 3. 文档同步

- [ ] 3.1 `openspec/specs/text-selection` 主规约同步 delta
- [ ] 3.2 `docs/specification/DESIGN.md` 删除打开文件条款（已获用户明确同意）

## 4. 验证

- [ ] 4.1 `SelectionMenuActionsTest` 与相关单测通过，`lintDebug` 无新增问题
- [ ] 4.2 小步提交推送
