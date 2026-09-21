# 修饰键栏方向键 DECCKM 感知

## 上下文

- 生产路径恒走可配置键栏（`TerminalScreen.kt:1014` 传非空 layout），DECCKM 感知的 `dispatchArrow` 只在不可达默认分支（`ModifierBar.kt:282-285`）。
- 可配置路径 `toolbarItemPresentation:878-881` 长按重复写死 `key.sequence`。
- `isAppCursorMode` 已接入键栏参数（`ModifierBar.kt:233`、`TerminalScreen.kt:1016`）。

## 任务

1. 可配置路径方向键点击与长按重复在触发时刻经 `isAppCursorMode` 编码（`TerminalInputEncoder.arrowSequence`），替代写死 `key.sequence`。
2. `ModifierBarRobolectricTest` 新增可配置路径两种模式用例。
3. 真机验证：`less`/`more` 分页器中键栏上下键翻页，普通 shell 中行为不变。
