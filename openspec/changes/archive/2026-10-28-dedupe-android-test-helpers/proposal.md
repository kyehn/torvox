# Proposal

## Why

仪器测试层逐份复制同一批「读桥、取文本、拿焦点、等就绪」样板，jscpd 在
`--min-lines 20 --min-tokens 100` 下报出 2 处克隆（42 行），降到
`--min-lines 12 --min-tokens 60` 则有 25 处（365 行）：

- `private fun bridge()` 在 7 个测试类里各写一遍（取活动会话的桥，取不到即抛）。
- `private fun currentText()` / `private fun pumpAndText()` 在 4 个测试类里各写
  一遍（先泵送 `pollEvent` 再读全量文本）。
- `setUp()` 里的「等会话 → 轮询等桥孵化 → 断言桥就绪」在 5 个测试类里各写
  一遍，而 `TestUtils.awaitBridge()` 早已提供同一件事。
- `private fun <T> onUiThread(block)` 与 `private fun tapToFocus(surface)` 在两个
  输入法延迟用例里各写一遍（逐字相同）。
- `AltScreenImeReflowInstrumentedTest` 内两处「网格复原 + 列数未变」断言块、
  三处「进入备用屏并等发布」块与三处 `finally` 收尾块逐份重复。
- `SelectionDragQuantifiedTest` 两处「长按 → 等菜单 → 读选区」前置块重复。
- `DocumentsProviderInstrumentedTest` 两处目录观察者登记块重复。

样板一旦分叉，失败信息就各说各话：同一处「桥没孵化」在不同类里给出不同消息，
排查时得先认出这是同一件事。

## What Changes

- 在 `androidTest/.../TestUtils.kt` 落四个共享助手：`requireBridge()`、
  `pumpedTerminalText()`、`onUiThread(block)`、`tapToFocus(surface)`，并让
  `awaitBridge()` 成为唯一的桥就绪门槛。
- 9 个测试类删掉各自的 `bridge()` / `currentText()` / `pumpAndText()` /
  `onUiThread()` / `tapToFocus()` 私有副本，改用共享助手。
- `AltScreenImeReflowInstrumentedTest` 抽出 `enterAltScreenAndAwaitPublished()`、
  `showImeAndAwaitShrink()`、`hideIme()`、`leaveAltScreenAndRestoreRows()`、
  `assertGridRestored()`。
- `SelectionDragQuantifiedTest` 抽出 `longPressAndReadSelection()`；
  `DocumentsProviderInstrumentedTest` 抽出 `registerListingObserver()`。
- 断言消息与判定条件逐字保留（共享助手的消息与被替换的副本一致）。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无：本次只改测试代码的组织方式，不改任何可观察行为。

## Impact

- `android/app/src/androidTest/java/terminal/emulator/TestUtils.kt`：新增共享助手。
- `android/app/src/androidTest/java/terminal/emulator/` 下 9 个测试类与 2 个
  用例类：改用共享助手。
- 不改 `android/app/src/main/`（产品代码）、`docs/specification/`、
  `docs/STATUS.md` 等保护文件。
- 不引入新依赖。
