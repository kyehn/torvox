# Proposal

## Why

`docs/STATUS.md` 把「侧边面板过宽，布局不合理，和规范不一致」列为未解决。宽度部分
已修（去掉 240dp 下限），布局部分却从未对齐：`docs/specification/DESIGN.md` 侧边面板
节规定「添加会话」位于会话列表底部、动作区为「重置终端 文本搜索 / 显示、隐藏输入法
设置」两行两列；实现把「新建会话」放在标题栏、动作顺序为「文本搜索 / 键盘 / 重置终端 /
设置」，两处都与规范相反。抽屉是切换会话的主要入口，入口位置与规范不符会把「先看
列表、再新建」的路径变成「先抬头找加号」，动作顺序错位也让「重置终端」这个最常用的
补救入口落在角落。

## What Changes

- 「新建会话」入口从标题栏移到会话列表底部，改为图标加文案的按钮。
- 动作区按规范重排为「重置终端 文本搜索 / 显示、隐藏输入法 设置」。
- 标题栏只保留「会话」标题，删除随之失效的回调参数。
- 补 Robolectric 用例钉住动作顺序与「新建会话」入口的位置。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-list`：新增「抽屉布局与规范一致」要求。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/SessionDrawer.kt`：抽屉布局重排。
- `android/app/src/main/res/values/strings.xml`：新增「新建会话」按钮文案。
- `android/app/src/test/java/terminal/emulator/ui/SessionDrawerNarrowTest.kt`：新增 2 例布局断言。
- `docs/STATUS.md`：该条标注收口。
