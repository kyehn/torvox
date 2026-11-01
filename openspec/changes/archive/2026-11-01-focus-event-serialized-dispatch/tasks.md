# Tasks

## 1. 焦点事件统一派发

- [x] 1.1 在 `TerminalRuntime` 新增单一派发入口 `dispatchFocusEvent(bridge, focused)`：
      经既有 `focusDispatcher` 派发，捕获异常并记日志，取消信号重抛
      （验证：`:app:compileDebugKotlin` 通过；派发体不含任何就地 `Bridge.focusEvent` 之外的
      会话锁交互）
- [x] 1.2 `focusChange` 改用该入口，删除就地 `scope.launch` 分支
      （验证：`:app:compileDebugKotlin` 通过；`focusChange` 内不再出现 `focusEvent` 字样）
- [x] 1.3 `switchSession` 阶段 3 的 focus-in 补发改用该入口
      （验证：`:app:compileDebugKotlin` 通过；该调用点不再出现 `target.bridge.focusEvent`）
- [x] 1.4 `activateReplacementSession` 的 focus-in 补发改用该入口
      （验证：`:app:compileDebugKotlin` 通过；该调用点不再出现 `bridge.focusEvent(true)`）

## 2. 门禁

- [x] 2.1 `.semgrep/rules.yml` 新增 Kotlin 规则 `no-inline-focus-event`：命中就地
      `$BRIDGE.focusEvent($FOCUSED)` 即判红，`dispatchFocusEvent` 函数体除外
      （验证：`semgrep scan --error --config .semgrep/rules.yml` 对当前代码 0 findings）
- [x] 2.2 验证该规则有判别力：临时把 `switchSession` 的补发改回就地调用，规则必须报出
      阻塞级 finding 且 semgrep 退出码非 0，随后恢复
      （验证：实测报出 1 条 blocking finding，`semgrep` 退出码 1）

## 3. 集成验证

- [x] 3.1 `:app:detekt`、`:app:spotlessCheck`、`:app:compileDebugKotlin`、
      `:app:testDebugUnitTest` 全绿
      （验证：Gradle 任务全部成功）
- [x] 3.2 `openspec validate --all --strict` 通过
      （验证：41 项 spec 与本变更一并通过）
- [x] 3.3 `markdownlint-cli2 "**/*.md"` 无告警
      （验证：0 issues）
