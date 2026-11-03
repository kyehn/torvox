# Proposal

## Why

ZTE 折叠屏外屏（472×819）的用户日志与 STATUS 挂账表明：输入法未弹出时，修饰键栏
底部存在一条「过高的空区域背景」。取证（`logcat_2026-11-03_17-29-27.txt` 等）显示
该设备 `navigationBars` 底部 inset 为 **0**（全面屏手势、无胶囊条），而键栏下仍有
背景色空带——空带高度即内容盒上 `safeGestures` 相对导航栏的**附加内缩**
（`TerminalScreen.kt` 曾按「导航栏与手势区较大者」内缩）。

旧判定（本能力「键栏位于系统底部手势区之上」Requirement）认为键位落入手势区会重现
「按下动画不复位」回归，故禁止。该前提已失效：`ExtraKeyButton` 的手势处理器已实现
`ACTION_CANCEL` 合成（`processCancel` 路径 + `isGestureCancelled`）与按下反馈延迟
（`PRESS_FEEDBACK_DELAY_MS` 覆盖从 `ACTION_DOWN` 到被认领的窗口），取消不再产生
任何按键输出或残留动画；且本能力的第一个 Requirement（触摸取消不触发按键）已把
该行为钉为规约。上游 termux 的 `ExtraKeysView` 同样不为手势区留白。

## What Changes

- 内容盒底部内缩**只按导航栏 inset**（`navigationBarsPadding`），移除
  `safeGestures` 附加内缩：导航栏 inset=0 的纯手势设备上键栏直落屏幕底边，
  空带消失；带胶囊条的设备上键栏贴导航栏上沿，附加空带同样消失。
- 键位因此可能落在系统手势区内：静止轻点不受影响（底部上滑需要位移才会被系统
  认领）；被认领时的 `ACTION_CANCEL` 由既有取消处理吞掉（本能力第一个
  Requirement 不变）。
- 输入法跟随口径随之对齐：键栏位移量是 `ime - navigationBars`，内容盒不再附加
  手势内缩后，键栏隐藏态贴屏幕底（或导航栏上沿）、弹出态底边恰在键盘顶边，
  不再残留手势区高度的缝隙。
- 手势区的触摸安全仍由取消链路保证，不以布局内缩承担。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `modifier-bar-gesture-cancel`：撤销（REMOVED）「键栏位于系统底部手势区之上」
  Requirement，新立（ADDED）「底部内缩只按导航栏 inset」——键栏底边允许落在
  手势区内，手势认领的取消由第一个 Requirement 的取消链路吸收；原「起滑点不落在
  任何修饰键按键上」的保证撤销，改为「起滑点命中按键时取消链路必须无输出」。

## 不做什么

- 不改 `ExtraKeyButton` 的取消处理与按下反馈延迟：它们已由本能力其余 Requirement
  与 `ModifierBarTest` 的 Cancel 用例覆盖，是本次布局改动的安全前提。
- 不改终端网格的键栏高度预留口径（`computeGridAvailableHeight`）：键栏自身高度
  不变，只是它相对屏幕底边的位置随导航栏 inset 移动。
- 不引入「按设备手势模式条件内缩」之类的分支：单一口径（只按导航栏）在两类
  导航模式下都成立，条件分支只会增加无法在模拟器验证的路径。
