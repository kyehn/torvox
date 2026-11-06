# Design

## 决策

### 决策一：键栏的 `KEYBOARD` 与抽屉的键盘按钮共用同一动作

软键盘切换只有一套语义：`soft-keyboard-toggle-behaviour = enable/disable` 时翻转的是
「软键盘可用与否」，其余模式翻转当次可见性。抽屉里的键盘按钮已经在用这套语义，
键栏的 `KEYBOARD` 键若另写一份，两个入口对同一按键的判断就会分叉（例如一处禁用后
仍把键盘拉起）。

故接线只做一件事：把 `TerminalScreen` 里已有的切换动作交给键栏，不复制逻辑。

### 决策二：回调必填，让「忘了接线」变成编译错误

缺陷的成因是「有回调、无接线」：类型上完全允许，运行时却静默无反应。因此把
`onKeyboardClick` 在 `ModifierBar`、`ModifierBarActions` 与 `buildModifierBarActions`
三处都改为必填参数——任何一处新增构造点时，编译器会要求显式表态，缺接线不再是
「写不写都能过」的选择。

`ModifierBar` 的必填参数会波及既有单测的构造点（每个补一个空实现），这是有意的
成本：键栏的其它特殊键（CTRL/ALT/DRAWER/SCROLL/PASTE）同样各有归属，回调缺位时
表现同样是「键帽在、点了没反应」。

### 决策三：被记忆的回调经 `rememberUpdatedState` 承接最新实现

键栏动作集合被 `remember` 住（避免每次重组新建 lambda 而让键栏整棵子树重组），
而切换动作本身每次组合都会重建。若直接捕获，键栏拿到的是首次组合那一份：
`soft-keyboard-toggle-behaviour` 变化后，它读到的粘滞态是旧的。

故用 `rememberUpdatedState` 承接：记忆住的是稳定的委托，回调执行时读到的始终是最新
实现。这与 `ModifierBar` 内部对点击回调的既有做法一致。

## 风险

- 键栏多出一个必填参数：既有单测构造点需补空实现，无行为影响。
- `KEYBOARD` 键按下后会立刻切换软键盘：与 termux 一致，也与抽屉里的键盘按钮一致。
