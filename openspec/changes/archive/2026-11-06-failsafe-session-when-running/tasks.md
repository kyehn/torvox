# Tasks

## 一、实现

- [x] 1.1 `TerminalViewModel` 增补 Failsafe Intent 的处置决策
      （`decideFailsafeIntent`：只置标记 / 置标记并建会话 / 完全忽略）
- [x] 1.2 应用已启动时显式建出会话：置标记后经 `createSession()` 新建，
      Surface 未交付时挂起请求、交付后补建
- [x] 1.3 `MainActivity` 把 Intent 连同「是否新投递」交给 ViewModel
      （`onCreate` 用 `savedInstanceState == null` 判定，`onNewIntent` 恒为新投递）

## 二、验证

- [x] 2.1 单元测试：纯决策四种组合 + ViewModel 接线（已启动建会话、
      冷启动只置标记、重建既不置标记也不建）
- [x] 2.2 `:app:testDebugUnitTest` 全量通过（896 个用例，0 失败）
- [x] 2.3 `detekt`、`spotlessCheck` 通过
- [x] 2.4 `openspec validate --all --strict` 通过
- [x] 2.5 markdownlint 对本变更目录 0 issues
- [x] 2.6 semgrep 对改动文件无命中
