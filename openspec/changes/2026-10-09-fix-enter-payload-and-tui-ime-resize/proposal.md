## Why

三个缺陷，全部有代码级与真机实证，且都曾被「声称已解决」过：

1. **回车在 raw 模式应用里变成 `j`。** `TerminalInputEncoder` 无修饰回车发的是
   `"\n"`（LF 0x0A）。crossterm 在 raw 模式下明确把 0x0A 解成
   `b'\x01'..=b'\x1a'` 分支的 `Char('j') + CONTROL`（`parse.rs` L92-107 注释：
   *"\n = 0xA, which is also the keycode for Ctrl+J … we disable that"*），
   helix/vim/less 因此收到字母 `j`。真机（1080x2400 模拟器）以
   `stty raw -echo; dd bs=1 count=1 | od -An -tx1` 读到下发字节为 `0a`；
   ghostty `function_keys.zig` 与 termux `KeyHandler.getCode` 的裸回车都是 `"\r"`。
   2026-10-08 归档变更只把 `KEYCODE_DPAD_CENTER` 补进 `enterKeyCodes`——键码集合
   正确、载荷错误，缺陷因此原样存活，且被「已修复」的说法掩盖。

2. **全屏 TUI 在输入法弹出时下部永久不可见。** 网格尺寸此前
   `applyGridResize` 刻意不减输入法 inset（`adjustNothing` 下 Surface 尺寸不变），
   备用屏的位移又被 `computeImeSurfaceShift` 强制为 0（防顶部被推出屏幕），
   于是 helix 仍按整屏行数布局、收不到 SIGWINCH：键盘遮住的 26 行里包含状态行。
   这是「位移方案」在备用屏上的结构性失配——它只防遮挡，不产生 resize。
   真机日志：键盘弹出前 `45x48`，弹出后本应变为 `26x48`。

3. **字号调节条范围与实际可设置范围不一致。** Kotlin 侧有
   `NATIVE_FONT_SIZE_MAX_SP = 100f`，原生 `setFontSizeInPlace` 里另有
   `if !(4.0..=100.0).contains(&size) { return }`。两份魔数漂移时，用户拖到
   Termux 允许的字号（低密度设备可达 256sp）却被原生**静默丢弃**——症状正是
   「设置条范围和实际可设置范围不一致」。2026-10-08 的 `effectiveFontSizeMaxSp`
   取两者较小，正是为了掩盖这份重复；重复仍在，于是上界永远只是原生魔数的影子。

## What Changes

- 回车族的无修饰载荷由 LF 改为 CR(0x0D)，`Alt+回车` 由 `"\n"` 前缀 ESC 改为
  `ESC CR`（ghostty `modifyKeysNormal` / termux `"\033\r"`）；带 Ctrl/Alt 的
  `CSI 13;mod~` 不变。输入法唯一换行提交 `"\n"` 归一为 CR，多字符提交内的换行
  逐字保留（那是内容本身，改成 CR 会把多行粘贴粘成一行）。
- 编辑器属性补 `TYPE_TEXT_FLAG_MULTI_LINE`，并把 `IME_ACTION_NONE` 换成 termux
  同款的 `IME_FLAG_NO_FULLSCREEN`：termux 在该行注明 `IME_ACTION_NONE` 会让屏幕
  键盘无法输入换行（termux-app#221）。MULTI_LINE 是输入法回车走换行键而非编辑器
  动作的前提。
- 备用屏激活时把输入法遮挡高度计入网格高度：遮挡高度改由**平台 insets 派发**
  （`ViewCompat.setOnApplyWindowInsetsListener` 装在终端 Surface 上）维护，
  备用屏状态取运行期逐帧发布的流值，触发一次防抖 resize/SIGWINCH，
  全屏 TUI 按可见高度重绘；键盘已展开时启动 TUI 另有一次触发
  （`onAltScreenChanged`）。主屏不扣遮挡，仍走纯平移
  （TESTING.md 要求上移后底部像素完全相同）。
- 顺带修正输入法高度来源：原实现轮询 `DecorView.rootWindowInsets` 并与 Compose 的
  `WindowInsets.ime` 叶节点取最大值，二者在仪器化环境下均恒为 0（实测键盘高 883px
  时 DecorView 仍报 0），导致既有输入法跟随位移整体失效——仓库自带
  `ImePopupPixelInstrumentedTest` 三个用例在该环境下即以此判红（位移=0）。
- 字号可选区间只留 `SettingsRepository.fontSizeMaxSp`（Termux 256px 换算）一处定义，
  删除与原生守卫重复的 `NATIVE_FONT_SIZE_MAX_SP`；原生守卫上界改由图集边长推导
  （字形位图必须放进图集是唯一真实约束），越界改记错误日志而非静默丢弃。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `modifier-bar-sticky-encoding`：回车族载荷 MUST 为 CR，输入法唯一换行提交 MUST
  归一为 CR，多字符提交内的换行 MUST 保留。
- `render-stability`：备用屏下输入法遮挡 MUST 计入网格高度并触发 SIGWINCH，
  主屏位移语义不变。
- `font-selection`：用户可选字号上界 MUST 只有一处定义（Termux 像素上限换算），
  MUST NOT 与原生守卫重复；原生越界 MUST 留痕。

## Impact

- `ui/TerminalInputEncoder.kt`：回车载荷、输入法换行归一、删除恒为 false 的
  bracketed paste 分支。
- `input/TerminalEditorInfo.kt`：`inputType` / `imeOptions`。
- `ui/TerminalSurface.kt`、`ui/TerminalScreen.kt`：平台 insets 派发维护输入法遮挡高度、
  备用屏据此重排网格。
- `settings/SettingsRepository.kt`：删除重复常量与被替代的换算函数。
- `native/src/android/ffi.rs`：字号与光栅缩放守卫改记错误日志、字号上界改由图集推导。
- 测试：`TerminalInputEncoderTest`、`FontSizeRangeTest`、`TerminalSurfaceLogicTest`、
  `CoerceSpToPxScaleTest` 改为对照 Termux 的具体取值而非复述公式；
  `FontSizeReflowInstrumentedTest` 增加端点验收；新增
  `AltScreenImeReflowInstrumentedTest`。
