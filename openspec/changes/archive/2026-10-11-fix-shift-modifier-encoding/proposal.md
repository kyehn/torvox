# Proposal

## Why

对照四个权威实现逐条核对本仓 `TerminalInputEncoder` 时发现：修饰键位掩码自
`KeyModifiers.fromKeyEvent` 采集后，在 `Bridge.processKeyEvent` 只取出 CTRL/ALT 两位，
Shift 在键盘编码路径被整体丢弃。其后果是 Shift+Tab 不发反 Tab（`CSI Z`）、
Shift+方向/Home/End/PgUp/PgDn/Insert/Delete 不发 `CSI 1;2X`——四项参考实现
（termux `KeyHandler`、zed `mappings/keys.rs`、wezterm `termwiz/src/input.rs`、
ghostty `src/input/function_keys.zig`）对这四项的期望完全一致，本仓两项都没实现。
同一处代码还存在注释与行为矛盾：Tab 分支注释写"按住 Alt 时，xterm 发送 ESC TAB"，
实际发出的却是 `CSI 9;3~`；Ctrl+Tab 发的 `CSI 9;5~` 同样不是 xterm `ctlseqs.txt`
规定的任何一种形态（xterm 规定 `CSI 27;5;9~`）。

## What Changes

- `KeyModifiers` 的 Shift 位传入 `TerminalInputEncoder.encodeKeyEvent`，新增
  `shiftActive` 参数；`Bridge.processKeyEvent` 从掩码取 Shift 并下传。
- `modifierParam` 纳入 Shift 位（Shift=1、Alt=2、Ctrl=4，无修饰为 1），
  与 xterm PC-Style Function Keys 表一致（zed `modifier_code` 同一算式）。
- CSI 功能键编码收敛为**单一来源表**（`CSI_KEYS`），替掉现状
  `escapeSequenceForKeyCode` 与 `csiSequenceWithModifier` 两份平行表
  （F1–F12、方向、Home/End、Insert/Delete、PgUp/PgDn 各写两遍）。
- Tab 分支按参考实现分流：Shift → `CSI Z`；仅 Alt → `ESC TAB`；
  带 Ctrl → `CSI 27;mod;9~`（ghostty 与 xterm `ctlseqs.txt` 一致，
  替掉现状未定义的 `CSI 9;mod~`）。
- 回车族补 Shift 分支：`CSI 27;2;13~`（ghostty 同形），与既有 Ctrl 分支同一形态。
- **BREAKING**（对外行为）：Alt+Tab 由 `CSI 9;3~` 改为 `ESC TAB`；
  Ctrl+Tab 由 `CSI 9;5~` 改为 `CSI 27;5;9~`；Shift+Enter 由 `CR` 改为 `CSI 27;2;13~`。
  三者的旧值均不是任何参考实现的合法形态或与该参考冲突。

### New Capabilities

无（属既有 `modifier-bar-sticky-encoding` 的行为补全）。

### Modified Capabilities

- `modifier-bar-sticky-encoding`：把 Shift 修饰纳入键盘编码契约，并订正
  Tab/Enter 的修饰分支期望值。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt`：编码表重构 + Shift 支持。
- `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt`：下传 Shift 位。
- `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`：调用点补参（键栏无 Shift，恒 false）。
- `android/app/src/test/java/terminal/emulator/ui/TerminalInputEncoderTest.kt`：新增 Shift 用例，
  订正 Alt+Tab / Ctrl+Tab / Shift+Enter 三条既有断言。
- Rust 侧无改动：`text_utils::encode_modifiers` 只服务可打印字符（Shift 已由平台并入字符），
  引入恒无效的 Shift 位是死代码。
- 无新增依赖，无二进制资产，无语料文件。
