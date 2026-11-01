# 任务

## 一、缺陷核定

- [x] 确认缺陷在**解析器**而非配置本身：`extra-keys = [['ESC',{key:'TAB',}]]` 这一份
      完全可读的配置，改动前整份退回默认布局。
- [x] 定位根因：`ExtraKeysParser.parseKeyObject` 在对象内部用 `require`/`error` 抛出的
      语法错误（缺冒号、尾随逗号、空对象）一路冒泡到 `parse` 的兜底 catch，
      被判为「整份文本写坏」。
- [x] 对照规范：既有 Requirement「键栏布局的容错」已把例外限定为「语法上无法配平的
      整份文本」；一个对象少写冒号时整份文本仍配平，故属「坏项」，实现此前未兑现。
- [x] 对照上游：上游用的 JSON 解析器收尾随逗号（`{key:'X',}`）与空对象（`{}`），
      按同一口径实现，键名解析结果与上游一致。

## 二、实现

- [x] `Cursor.skipPastComposite(start)`：把光标移到从 `start` 起的那段复合值的闭合符
      **之后**，用于坏对象只丢自己那一项后的恢复。
- [x] `ExtraKeysParser.parseKey`：对象内部的语法错误（`IllegalArgumentException` /
      `IllegalStateException`）只丢这一项——同步光标、留下「该项被丢弃」的说明；
      结构本身配不平时异常继续冒泡，整份布局退回默认。
- [x] `ExtraKeysParser.parseKeyObject`：收下尾随逗号与空对象。尾随逗号记一条说明，
      键照常解析出来；空对象按「没有 `key`/`macro`」的既有口径丢弃。

## 三、回归钉

- [x] `a key object with a trailing comma keeps the key`：尾随逗号不丢键，且留说明。
- [x] `a key object missing its colon drops only itself`：缺冒号只丢这一项，其余键位照常。
- [x] `an empty key object drops only itself`：空对象只丢这一项，其余键位照常。
- [x] 既有 35 条解析用例全部仍通过（语义层面的坏项隔离与「整份写坏退回默认」未受影响）。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.settings.TermuxPropertiesParserTest"`
      → BUILD SUCCESSFUL（38 tests, 0 failed）。
- [x] `:app:testDebugUnitTest` 全量 → 见提交说明。
- [x] `detekt` 与 `spotlessCheck` → 见提交说明。
- [x] `openspec validate <change-id> --strict` → 通过。
