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
  `ESC CR`（ghostty `modifyKeysNormal` / termux `"\033\r"`）；带 Ctrl/Alt 的编码
  改为 `CSI 27;mod;13~`（ghostty 的 modifyOtherKeys 形式；此前写的 `CSI 13;mod~`
  是 xterm 的 `modifyFunctionKeys`，与 ghostty 实现不符）。输入法唯一换行提交 `"\n"`
  归一为 CR，多字符提交内的换行逐字保留——只归一孤立换行，多行提交在 helix 里
  仍是 Ctrl+J，直到粘贴路径真正实现 bracketed paste。
- 编辑器属性补 `TYPE_TEXT_FLAG_MULTI_LINE`，并把 `IME_ACTION_NONE` 换成 termux
  同款的 `IME_FLAG_NO_FULLSCREEN`：termux 在该行注明 `IME_ACTION_NONE` 会让屏幕
  键盘无法输入换行（termux-app#221）。MULTI_LINE 是输入法回车走换行键而非编辑器
  动作的前提。
- 备用屏激活时把输入法遮挡高度计入网格高度：遮挡高度改由**平台 insets 派发**
  （`ViewCompat.setOnApplyWindowInsetsListener` 装在终端 Surface 上）维护，
  备用屏状态取运行期逐帧发布的流值，触发一次防抖 resize/SIGWINCH，
  全屏 TUI 按可见高度重绘；键盘已展开时启动 TUI 由备用屏状态翻转这条订阅补一次
  触发，离开备用屏时同样补一次（否则网格停留在被输入法缩小后的行数）。主屏不扣
  遮挡，仍走纯平移（TESTING.md 要求上移后底部像素完全相同）。
- 顺带修正输入法高度来源：原实现轮询 `DecorView.rootWindowInsets` 并与 Compose 的
  `WindowInsets.ime` 叶节点取最大值，而这条合成通道的写入发生在 insets 遍历内，
  不保证被组合观察到（实测每 300ms 写一次组合状态，20 次才换来一次重组），
  位移因此从未发生——仓库自带 `ImePopupPixelInstrumentedTest` 三个用例即以此判红
  （位移=0）。
- 输入法跟随位移改由终端 Surface 自身的 `translationY` 承担，不再经组合容器平移：
  组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer 帧回调里跑；
  主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时重组滞后可达十几秒（实测每
  300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与键栏
  纹丝不动。键栏是组合覆盖层，读同一个 `imeInsetFlow` 上移。
- 字号可选区间只留 `SettingsRepository.fontSizeMaxSp`（Termux 256px 换算）一处定义，
  删除与原生守卫重复的 `NATIVE_FONT_SIZE_MAX_SP`；原生守卫上界改由图集边长推导
  （字形位图必须放进图集是唯一真实约束），越界改记错误日志而非静默丢弃；上界推导
  抽为纯函数并由测试钉住「原生上界不小于可选上界」。

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
  备用屏据此重排网格，主屏位移改由 Surface 的 `translationY` 承担。
- `settings/SettingsRepository.kt`：删除重复常量与被替代的换算函数。
- `native/src/android/ffi.rs`：字号与光栅缩放守卫改记错误日志、字号上界改由图集推导
  并抽为纯函数。
- 测试：`TerminalInputEncoderTest`、`FontSizeRangeTest`、`TerminalSurfaceLogicTest`、
  `CoerceSpToPxScaleTest` 改为对照 Termux 的具体取值而非复述公式；
  `FontSizeReflowInstrumentedTest` 增加端点验收；新增
  `AltScreenImeReflowInstrumentedTest` 与 `font_size_cap_tests`。

## 上游测试资产调研结论

对比 kitty / rio / wezterm / alacritty / ghostty / xterm / foot / contour / esctest2
九项，结论是**不引入任何上游测试资产**：

- 唯一在设计上不可能自验证的体系是 esctest2（559 用例，测试方法自带断言，期望值
  来自 DEC 手册而非任何实现）。但它被 torvox 固定的 ghostty `22d13172`（2026-08-06）
  硬阻塞：`DECRQCRA` 支持于 2026-10-01 的 `9272a2f7`，不是该 tag 的祖先，而
  esctest 的 `--xterm-checksum=411` 强依赖它。直接接入会产出大面积假阳性，需要一份
  50+ 条的已知失败白名单。
- 其余上游语料均已被消费或清理：alacritty `ref/` 期望值由 alacritty 自己的
  `--ref-test` 生成（自验证），`be2e3a9e` 逐条记录 14/45 的差异且全部落在
  libghostty-vt 与 alacritty 之间；wezterm 的 556K `test-data/` 无任何程序消费者。
- 可低成本采纳的两项已落地：`keymap` 由抽样 5~13 个 Android 键码改为全表断言；
  `FontSizeRangeTest` 与新增的 `font_size_cap_tests` 都改为对照外部常量（Termux 的
  4dip/256px/步长 2、图集边长）而非复述实现公式。
