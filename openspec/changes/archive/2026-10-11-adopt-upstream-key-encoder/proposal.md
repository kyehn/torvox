# 改用上游键编码器

## Why

`openspec/specs/upstream-alignment` 的「上游 API 取用判定」把 `key::Encoder` 按键编码
登记为「待办」，阻塞点写的是「Android 键码到上游 `Key` 的映射裁定与 JNI 桥形态」。
本变更解除该阻塞并完成取用。

本轮核实的事实：

- `TerminalInputEncoder.kt` 318 行手写编码表覆盖 Tab/回车族/方向键/F1-F12/编辑键/
  修饰折叠与 `CSI 27`，全部在 Kotlin 侧算完再交 JNI；上游 `key::Encoder` 在
  `native/src` 的引用数为 0。
- 手写表只能表达「无修饰 + Shift + Alt + Ctrl」的 xterm 传统编码。以下四种终端
  状态在本仓**无法生效**，因为 Kotlin 侧根本没有它们的查询：
  - kitty 键盘协议（DESIGN 明确要求「支持连字、kitty 图像协议等特性」）；
  - `modifyOtherKeys`（xterm ctlseqs）；
  - backarrow key mode（DEC 私有模式 67）；
  - keypad 应用模式（DEC 私有模式 66）。
- DECCKM 是唯一被跟上的：Kotlin 经 `NativeBridge.getMode(1, 0)` 查
  `mode_get`，据此选 SS3/CSI。上游编码器用 `set_options_from_terminal` 一次性
  接管全部模式，本仓这条例外通道随之多余。

## What Changes

- 新增 `Query::EncodeKeyEvent`，在 VT 线程编码：与既有 `Query::EncodeMouseEvent`
  同构。`key::Encoder` 与 `key::Event` 由 VT 线程持有——`set_options_from_terminal`
  需要 `&Terminal`，而 `Terminal` 只存在于 VT 线程；编码在 VT 线程完成，字节经
  通道回传后写入 PTY。Kotlin 侧只提供键码、修饰位、文本与 unshifted 码点。
- 新增 Android 键码到上游 `Key` 的机械对应表：只做键名对应，无行为判断；
  `utf8` 文本按上游文档约束过滤（C0 控制字符与 U+007F 传 `None`，不传平台私有区
  码位）。修饰位经既有 `KeyModifiers.ghosttyMods` 转为上游 `key.Mods` 原始位。
- `TerminalInputEncoder` 收敛为上游编码的调用方：删手写转义序列表
  （`CSI_KEYS`、`csi27Sequence`、`arrowSequence`、`controlByteForKeyCode`、
  `escapeSequenceForKeyCode`、`modifierParam`）。保留 `encodeCommittedText`——
  输入法提交文本不经按键编码路径。
- 删除 `NativeBridge.getMode` 的 DECCKM 例外通道（实施时先核实是否另有消费者）。

### 上游编码实测（本轮探针，`set_options_from_terminal` 后）

| 输入 | 输出 |
| --- | --- |
| `A` 无修饰 | `a` |
| `A`+Ctrl | `0x01` |
| `A`+Alt | `ESC a` |
| `Enter` 无修饰 | `CR` |
| `Enter`+Shift | `ESC [ 27;2;13~` |
| `Tab` 无修饰 | `\t` |
| `Tab`+Shift | `ESC [ Z` |
| `Tab`+Alt | `ESC \t` |
| `ArrowUp` 无修饰 | `ESC [ A` |
| DECCKM 开后 `ArrowUp` | `ESC O A` |
| DECCKM 开后 `ArrowUp`+Ctrl | `ESC [ 1;5A` |
| backarrow 开后 `Backspace` | `0x08`（未开时 `0x7F`） |
| modifyOtherKeys=2 下 `A`+Ctrl | `ESC [ 27;5;97~` |
| kitty DISAMBIGUATE 下 `Escape` | `ESC [ 27u` |
| 单独 Ctrl / Shift / CapsLock | 空输出 |

`Tab` 的 Shift/Alt 分支与 `modifier-bar-sticky-encoding` 现有断言一致且上游同形，
该规范的行为要求继续成立，实现改由上游承载。`Enter`+Shift 走 `CSI 27;2;13~`
而非发 LF，与该规范「Shift+Enter 不发 LF」的理由一致（LF 在 raw 模式被读成 Ctrl+J）。

## Capabilities

### New Capabilities

- `key-input-encoding`: 按键编码的归属、模式适配、键码映射与输入法提交路径的边界

### Modified Capabilities

- `upstream-alignment`: `key::Encoder` 条目由「待办」改为「采用」并写明阻塞点解除方式

## Non-goals

- 不改输入法提交语义（`encodeCommittedText` 及其 CR 归一保持原样）。
- 不删除 `modifier-bar-arrow-encoding` 与 `modifier-bar-sticky-encoding` 的行为
  要求：它们继续成立，只是实现改由上游承载。

## Impact

- Kotlin：`ui/TerminalInputEncoder.kt`（删表、留映射与提交路径）、`ui/ModifierBar.kt`、
  `bridge/NativeBridge.kt`（新增 `encodeKeyEvent`，视核实结果删 `getMode`）
- native：`terminal/ghostty_terminal/{commands,internal,public_api,types}.rs`、
  `android/ffi.rs`
- 规范：`upstream-alignment`、`modifier-bar-arrow-encoding`（实现归属改指新规范）
- 测试：`TerminalInputEncoderTest` 的传统编码期望改由 native 侧承载，Kotlin 只留
  映射与输入法提交路径断言
