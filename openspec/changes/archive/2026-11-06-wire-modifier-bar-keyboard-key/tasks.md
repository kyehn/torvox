# Tasks

## 一、接线

- [x] 1.1 `TerminalScreen` 把键盘切换动作接入键栏动作集合，并传给 `ModifierBar`
- [x] 1.2 `ModifierBar`、`ModifierBarActions`、`buildModifierBarActions` 的
  `onKeyboardClick` 改为必填参数
- [x] 1.3 被记忆的回调经 `rememberUpdatedState` 承接最新实现

## 二、验证

- [x] 2.1 Robolectric 用例：`extra-keys` 的 `KEYBOARD` 键按下后触发切换回调
- [x] 2.2 `:app:testDebugUnitTest`、detekt、spotlessCheck 通过
- [x] 2.3 `openspec validate --all --strict` 通过
- [x] 2.4 `markdownlint-cli2` 对本变更目录 0 issues
