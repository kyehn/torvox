# Proposal

## Why

全量 `:app:connectedDebugAndroidTest`（231 例）连跑两次，各有一次红灯，两处都
不是产品缺陷，而是**量具的时序与口径**问题：

1. `KittyGraphicsPlacementTest.kitty_image_moves_up_with_scrolled_content` 断言
   「图像随内容上移 3 行＝51px」，实测 68px（4 行）。滚动命令经 shell 执行，
   命令回显与提示符额外占行，滚动量根本不是 3 行——期望值写死，判据失效。
2. `UiAutomatorTest.typingViaSystemKeyboardReacts` 报「搜索框未出现」。该用例
   单跑三次必绿、全量套件两次都红。logcat 显示：抽屉按钮点击后 408ms 就查到了
   `SearchButton` 节点（抽屉滑入动画未完），无障碍树给出的节点边界落后于实际
   绘制位置，随后的点击落到遮罩上，抽屉被点关、搜索框从未出现。

两处都靠改用例解决，且改法必须保住判别力：Kitty 仍要判「图像随内容移动」，
UiAutomator 仍要判「真实系统键盘的输入到达应用」。修法与取证过程本身是可复用
的知识（下次再有人怀疑这两条用例「不稳定」，不必从头取证），故留档。

## What Changes

- 新增留档 `test-timing.md`：两条用例的红灯根因、取证证据与判据口径。
- 不改任何产品行为：两处改动都只落在 `androidTest` 源集内。

## Capabilities

无规范增量：本变更只留取证与口径说明，不改变任何已声明的行为要求。

## Impact

- `openspec/changes/2026-10-23-instrumented-test-timing/test-timing.md`（新增）
- `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsPlacementTest.kt`
- `android/app/src/androidTest/java/terminal/emulator/ui/UiAutomatorTest.kt`
