# 任务

## 一、缺陷核定

- [x] 确认 CI run `41414598205` 的失败用例为
      `SelectionEspressoTest#realLongPressKeepsMenuAndCopiesPressedLine`，
      失败信息为「真实长按松手后菜单必须留存并含「复制」」。
- [x] 确认该用例此前把 MotionEvent 投递给 `decorView`，落点取窗口几何中心。
- [x] 确认修饰键栏是覆盖在终端 Surface 之上的组合覆盖层
      （`TerminalScreen.kt` 的 `ModifierBarOverlay`，`align(BottomCenter)`）。
- [x] 确认启动期输入法自动弹出（`hide-soft-keyboard-on-startup` 默认 `false`）会把
      键栏抬到屏幕中部，窗口几何中心恰落在键栏上（运行期日志 `setImeInsetPx: 227`）。
- [x] 确认键栏先于 SurfaceView 命中手势，长按到不了终端表面：选区不建立、菜单不出现。

## 二、实现

- [x] `TestUtils.kt`：新增公共辅助 `hideImeAndAwaitHidden`——反复发隐藏指令并轮询
      `rootWindowInsets.isVisible(ime())` 确认确实收起（单次隐藏会被平台在收起动画
      期间再弹）。
- [x] `SelectionEspressoTest.kt`：落点改由网格单元格推出（`terminalCellSizePx`），
      手势改投递给终端表面（`findTerminalSurface` + `injectLongPress`）。
- [x] `SelectionEspressoTest.kt`：断言文案改用 `R.string.copy`，不再硬编码「复制」。

## 三、门禁

- [x] `compileDebugAndroidTestKotlin` 通过
- [x] `SelectionEspressoTest#realLongPressKeepsMenuAndCopiesPressedLine` 连接测试通过
- [x] `detekt` 通过
- [x] `spotlessCheck` 通过
- [x] `openspec validate --all --strict` 通过
