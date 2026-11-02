# 任务

## 一、缺陷核定

- [x] 确认缺陷在**解析器**而非配置本身：`extra-keys = [[{key:'TAB',popup:{key:'X' bad}},'CTRL']]`
      这一份完全可读的配置，改动前整份退回默认布局。
- [x] 定位根因：`ExtraKeysParser.parsePopup` 直接调用 `parseKeyObject`，后者在对象内部
      抛出的语法错误（缺逗号、缺冒号）冒泡到顶层 `parseKey` 的兜底 catch，被判为
      「整个按键对象写坏」，主键随之消失；该行被掏空后又触发「每行都空 → 整份退回默认」。
- [x] 对照规范：既有 Requirement「键栏布局的容错」把坏项定义为「按键对象内部的语法错误」
      并要求只丢该对象那一项；「extra-keys 的 popup 语义」要求次级键不可用时 MUST NOT
      丢弃主键。popup 对象自己就是按键对象，实现此前未兑现。

## 二、实现

- [x] `ExtraKeysParser.parsePopup`：对象形式的 popup 单独捕获语法错误，把光标同步到该
      popup 对象的闭合符之后，只丢这个 popup。
- [x] 新增 `ExtraKeysParser.dropMalformedPopup`：留下「该 popup 对象被丢弃」的说明
      （说明必须可读——静默丢弃会让用户无从定位是长按动作写坏了）。
- [x] 对象本身配不平时 `skipPastComposite` 仍抛出，整份布局退回默认并记错误（既有边界
      不变）。

## 三、回归钉

- [x] `a malformed popup object drops only the popup`：popup 对象缺逗号时，主键 TAB 与
      CTRL 照常解析，坏 popup 被丢弃且留下含「popup」的说明。
- [x] 既有 39 条解析用例全部仍通过（popup 的语义口径、对象语法错误只丢自己、
      「整份写坏退回默认」均未受影响）。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.settings.TermuxPropertiesParserTest"`
      → BUILD SUCCESSFUL（39 tests, 0 failed）。
- [x] `:app:testDebugUnitTest` 全量 → BUILD SUCCESSFUL（826 tests, 0 failed）。
- [x] `detekt` 与 `spotlessCheck` → BUILD SUCCESSFUL。
- [x] `openspec validate 2026-11-02-fix-extra-keys-popup-recovery --strict` → 通过。
