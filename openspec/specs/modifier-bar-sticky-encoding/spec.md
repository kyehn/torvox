# modifier-bar-sticky-encoding Specification

## Purpose

粘滞 CTRL/ALT 必须在所有输入路径一致生效。可配置键栏的普通按键（ESC、TAB、
HOME、END、PGUP、PGDN 等）此前走裸序列直写 PTY，不读粘滞态：点亮 CTRL 后点其他
键没有组合效果，且 Once 状态无人消费。键栏普通按键须与 IME、硬件路径同样经
`TerminalInputEncoder.encodeKeyEvent` 编码，并在发送后消费 Once 态。

## Requirements

### Requirement: 按键跟随粘滞修饰编码

可配置键栏普通按键点击时 MUST 读取当前 CTRL/ALT 态并经编码器编码；
无修饰时输出 MUST 与原序列一致；带修饰发送后 MUST 消费 Once 态。
硬件按键路径 MUST 同时把平台报告的 Shift 状态下传编码器：Shift 参与修饰键位计算
（Shift=1、Alt=2、Ctrl=4，无修饰为 1，与 xterm PC-Style Function Keys 的
`modifier` 参数一致），MUST NOT 在 `Bridge.processKeyEvent` 取掩码时被丢弃。

#### Scenario: 无修饰点 ESC

- **WHEN** 未点亮修饰且点击 ESC
- **THEN** 发送 `ESC` 单字节，不消费任何状态

#### Scenario: 点亮 CTRL 后点 C 相关键

- **WHEN** CTRL 为 Once 且点击可编码按键
- **THEN** 按编码器输出发送组合序列，且 CTRL 回到 Off

#### Scenario: Shift 参与修饰键位

- **WHEN** 硬件键盘按住 Shift 再按上键
- **THEN** PTY 收到 `CSI 1;2A`：修饰键位为 1+1(Shift)=2，
      MUST NOT 因 Shift 被丢弃而退化成裸 `CSI A`

### Requirement: 回车族载荷必须是 CR 而非 LF

回车键族（`KEYCODE_ENTER` / `KEYCODE_NUMPAD_ENTER` / `KEYCODE_DPAD_CENTER`）在
无修饰时 MUST 下发 CR(0x0D)，MUST NOT 下发 LF(0x0A)：raw 模式的 TUI 应用
（helix/vim/less）在 raw 模式下把 0x0A 解成 Ctrl+J 即字母 `j`
（crossterm `parse.rs`：*"\n = 0xA, which is also the keycode for Ctrl+J … When we
enter raw mode, we disable that"*），LF 因此表现为「回车变成 j」。带 Ctrl/Alt/Shift 的回车
MUST 按 ghostty `src/input/function_keys.zig` 的 Enter 条目上报 `CSI 27;mod;13~`
（Shift=2、Alt+Shift=4、Ctrl=5、Ctrl+Shift=6、Alt=7、Alt+Ctrl+Shift=8）；
仅 Alt 时 MUST 为 ESC 前缀加 CR。

Shift+Enter MUST NOT 走 LF：LF 在 raw 模式下同样被读成 Ctrl+J 即字母 j，
zed `("enter", Shift) => "\x0a"` 只适用于前台编辑器语义，终端下无效。

依据：ghostty `src/input/function_keys.zig` 的裸回车条目是 `"\r"`、Alt+回车是
`"\x1b\r"`（modifyKeysNormal）、Shift 回车是 `CSI 27;2;13~`、带 Ctrl 的是
`CSI 27;{mod};13~`；termux `KeyHandler.getCode` 的裸回车与 Alt+回车同样是
`"\r"` / `"\033\r"`。
（`CSI 13;mod~` 是 xterm 的传统 `modifyFunctionKeys` 形式，与 ghostty 的实际实现不符。）

#### Scenario: 无修饰回车下发 CR

- **WHEN** 用户按回车（硬件键、`KEYCODE_DPAD_CENTER` 或输入法回车）
- **THEN** PTY 收到的字节是 0x0D，在 raw 模式下被应用读作 Enter

#### Scenario: 平台报告 LF 时仍下发 CR

- **WHEN** 硬件回车事件的 `unicodeChar` 是 `\n`（小键盘回车与部分输入法的常见取值）
- **THEN** PTY 仍收到 0x0D：转义序列的优先级 MUST 高于 `unicodeChar`，
      否则可打印字符分支先执行就会把 LF 写进 PTY，raw 模式读作 Ctrl+J 即字母 j

#### Scenario: Alt 加回车是 ESC 前缀加 CR

- **WHEN** Alt 粘滞状态下按回车
- **THEN** PTY 收到 `ESC` + 0x0D

#### Scenario: Ctrl 加回车上报 CSI 27

- **WHEN** Ctrl 粘滞状态下按回车
- **THEN** PTY 收到 `CSI 27;5;13~`，与 ghostty 的 modifyOtherKeys 一致

#### Scenario: Shift 加回车上报 CSI 27 而非 LF

- **WHEN** 按住 Shift 再按回车
- **THEN** PTY 收到 `CSI 27;2;13~`，MUST NOT 收到 0x0A

### Requirement: 输入法换行提交归一为 CR

输入法提交文本中的**唯一**换行提交（`"\n"`，多行字段的回车键）MUST 归一为 CR(0x0D)，
修饰键状态 MUST NOT 影响归一结果（IME 只交出一个 `"\n"`，无从得知修饰键）。
多字符提交（拼音候选、滑行输入、输入法内部粘贴）内的换行 MUST 逐字保留：
那是内容本身，逐字换成 CR 会把多行文本粘成一行。

该归一只覆盖**孤立**换行提交。多字符提交在 helix 里仍是 Ctrl+J；粘贴路径
已实现 bracketed 包裹（`paste-encoding`），输入法提交路径不受影响，
本 MUST NOT 被解读为多行提交已修复。

编辑器属性 MUST 声明 `TYPE_TEXT_FLAG_MULTI_LINE`（回车键形态由此决定），
MUST NOT 使用 `IME_ACTION_NONE`：termux 在该处注明它会让屏幕键盘无法输入换行
（termux-app#221）。

#### Scenario: 输入法回车提交归一为 CR

- **WHEN** 输入法以 `commitText("\n")` 提交回车
- **THEN** PTY 收到 0x0D 而不是 0x0A

#### Scenario: 多字符提交内的换行保留

- **WHEN** 输入法提交 `"a\nb"`
- **THEN** PTY 原样收到 `a` LF `b`

### Requirement: Tab 的修饰分支按权威实现分流

Tab 的修饰编码 MUST 按下表分流（依据：xterm `ctlseqs.txt` 的 modifyOtherKeys 形态
`CSI 27;mod;9~` 与反 Tab `CSI Z`；ghostty `src/input/function_keys.zig` 的 Tab 条目；
zed `mappings/keys.rs` 的 `("tab", Shift) => "\x1b[Z"`；
wezterm `termwiz/src/input.rs` 的 Alt+Tab → `ESC TAB`）：

| 修饰 | 序列 |
| --- | --- |
| 无 | `HT`(0x09) |
| Shift | `CSI Z`（反 Tab，terminfo `kcbt`） |
| 仅 Alt | `ESC HT` |
| 含 Ctrl | `CSI 27;mod;9~` |
| Shift+Alt | `CSI 27;4;9~` |

MUST NOT 发出 `CSI 9;mod~`：该形态既不是 xterm 的 `CSI 27;mod;9~`，
也不是其 `formatOtherKeys` 变体 `CSI 9;mod u`，无任何参考实现使用。

#### Scenario: Shift 加 Tab 发反 Tab

- **WHEN** 按住 Shift 再按 Tab
- **THEN** PTY 收到 `CSI Z`

#### Scenario: Alt 加 Tab 是 ESC 前缀加 HT

- **WHEN** Alt 粘滞状态下按 Tab
- **THEN** PTY 收到 `ESC` + 0x09

#### Scenario: Ctrl 加 Tab 上报 CSI 27

- **WHEN** Ctrl 粘滞状态下按 Tab
- **THEN** PTY 收到 `CSI 27;5;9~`

### Requirement: CSI 功能键的修饰键位编码

带修饰（Ctrl/Alt/Shift 任一）的功能键 MUST 走 `CSI <intro>;<mod><final>` 形态，
无修饰时 MUST 保持原序列（F1–F4 为 `SS3 P/Q/R/S`，其余为裸 CSI/tilde 序列）。
应用的修饰键位 MUST 同一算式：`1 + (Shift?1:0) + (Alt?2:0) + (Ctrl?4:0)`，
与 xterm PC-Style Function Keys 的 modifier 参数、zed `modifier_code` 一致。
修饰后的方向键 MUST NOT 因 DECCKM 改用 SS3（xterm/ghostty/wezterm 的修饰条目一律 CSI）。

#### Scenario: Shift 加方向上键

- **WHEN** 按住 Shift 再按上键
- **THEN** PTY 收到 `CSI 1;2A`

#### Scenario: Shift 加 PageUp

- **WHEN** 按住 Shift 再按 PageUp
- **THEN** PTY 收到 `CSI 5;2~`

#### Scenario: Shift 加 F5

- **WHEN** 按住 Shift 再按 F5
- **THEN** PTY 收到 `CSI 15;2~`

#### Scenario: Ctrl 加 Alt 加左键叠加修饰位

- **WHEN** 同时按住 Ctrl 与 Alt 再按左键
- **THEN** PTY 收到 `CSI 1;7D`：1+2(Alt)+4(Ctrl)=7

#### Scenario: 应用光标模式下 Shift 加上键仍为 CSI

- **WHEN** 终端处于应用光标模式（DECCKM）且按住 Shift 再按上键
- **THEN** PTY 收到 `CSI 1;2A`，MUST NOT 收到 `SS3` 变体
