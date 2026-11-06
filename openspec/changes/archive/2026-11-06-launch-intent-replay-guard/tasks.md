# Tasks

## 一、实现

- [x] 1.1 `TerminalViewModel.requestSessionSwitch` 接收「是否新投递」：陈旧请求直接
      忽略，不切换也不写待补切的请求
- [x] 1.2 `MainActivity.handleLaunchIntent` 把 `freshIntent` 交给通知切换，并把
      「打开设置」的计数自增也置于同一门控之下
- [x] 1.3 设置覆盖层的开合状态改由 `rememberSaveable` 持有，跨 Activity 重建保持

## 二、验证

- [x] 2.1 单元测试：新投递即切换、新投递待 Surface 交付后补切、重建不切换、
      重建不留待补切请求（4 个用例）
- [x] 2.2 `:app:testDebugUnitTest` 全量通过（907 个用例，0 失败）
- [x] 2.3 `detekt`、`spotlessCheck` 通过
- [x] 2.4 `openspec validate --all --strict` 通过
- [x] 2.5 markdownlint 对本变更目录 0 issues
- [x] 2.6 semgrep 对改动文件无命中
