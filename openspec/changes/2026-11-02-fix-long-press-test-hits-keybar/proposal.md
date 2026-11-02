# Proposal

## Why

CI run `41414598205` 的
`SelectionEspressoTest#realLongPressKeepsMenuAndCopiesPressedLine` 失败：

```text
AssertionError: 真实长按松手后菜单必须留存并含「复制」
```

该用例是走真实手势链的端到端断言（按住不动 → 松手 → 菜单留存 → 点「复制」→ 剪贴板
拿到被长按那一行的文本）。它此前把 MotionEvent 投递给 `decorView`，落点取**窗口几何
中心**。

窗口几何中心不是终端内容的位置：修饰键栏是覆盖在终端 Surface 之上的组合覆盖层
（`TerminalScreen.kt` 的 `ModifierBarOverlay`），启动期自动弹出的输入法（`hide-soft-keyboard-on-startup`
默认 `false`，对齐 termux 上游）会把键栏抬到屏幕中部，落点于是落在键栏上。键栏先于
SurfaceView 命中手势，长按**根本到不了终端表面**：选区不建立、菜单不出现。

失败信息却指向「菜单必须留存」，把根因（落点根本没进终端）掩盖成「菜单被错误收起」，
排障方向被带偏。

## What Changes

- 落点改由**网格单元格**推出：用运行期的行列数与单元格像素尺寸算出目标格子的中心，
  而不是用窗口几何中心。
- 手势改投递给**终端表面**（`findTerminalSurface`）而非 `decorView`，绕开键栏这层
  组合覆盖层的命中拦截，保留 SurfaceView 自身的手势与命中测试路径。
- 用例先收起输入法并确认已收起（新增公共辅助 `hideImeAndAwaitHidden`），使网格与
  屏幕坐标一一对应。
- 断言文案改用 `R.string.copy` 取本地化字符串，不再硬编码「复制」。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `instrumentation-harness`：把「手势类用例必须把事件投递给目标视图本身、落点必须
  由被测对象的实际布局推出」这条既有纪律的适用面由「读 Surface 像素」扩到「向终端
  注入真实手势」——此前该纪律只落在像素读取上，真实手势注入是漏网的。

## 不做什么

- 不改修饰键栏的覆盖层结构，也不改输入法的弹出策略：启动期自动弹键盘是产品既定
  行为（对齐 termux 上游），键栏覆盖层也是既定结构。用例应适应产品行为，而不是
  让产品迁就用例。
- 不删减该用例的断言强度：菜单留存、剪贴板内容两条断言都保留。
