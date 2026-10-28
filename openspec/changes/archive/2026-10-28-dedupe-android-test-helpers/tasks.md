# 任务

- [x] 建 change（proposal/design/tasks，skip_specs：纯重构无行为变化）
- [x] `TestUtils.kt` 新增 `requireBridge()`、`pumpedTerminalText()`、
      `onUiThread(block)`、`tapToFocus(surface)`
- [x] 9 个测试类改用共享助手（删掉各自的 `bridge()` / `currentText()` /
      `pumpAndText()` / `onUiThread()` / `tapToFocus()` 副本）
- [x] `AltScreenImeReflowInstrumentedTest` 抽出 5 个共用块
- [x] `SelectionDragQuantifiedTest` 抽出 `longPressAndReadSelection()`
- [x] `DocumentsProviderInstrumentedTest` 抽出 `registerListingObserver()`
- [x] jscpd 复查：`--min-lines 20 --min-tokens 100` 全仓 0 克隆
- [x] Kotlin 门禁：`:app:detekt`、`:app:spotlessCheck`、
      `:app:compileDebugAndroidTestKotlin`
- [x] 模拟器仪器测试跑通受影响用例
- [x] `openspec validate --all --strict` 通过
