# key-input-encoding Specification

## Purpose

硬件按键到终端转义序列的编码归属与模式适配。以 Ghostty 为终端状态的单一来源，
按键编码 MUST 取用上游 `key::Encoder`，本仓 MUST NOT 自维护按键转义序列表。
编码在 VT 线程完成：上游编码器的模式选项须从活体 `Terminal` 读取，而 `Terminal`
只存在于 VT 线程。

## Requirements

### Requirement: 按键编码取用上游编码器

硬件按键事件（键盘、修饰键栏的方向键与功能键）到转义序列的编码 MUST 经上游
`key::Encoder`，编码前 MUST 调用 `set_options_from_terminal` 使编码器选项取自
终端活体状态（DECCKM、keypad 应用模式、backarrow key mode、`modifyOtherKeys`、
kitty 键盘协议标志）。本仓 MUST NOT 自维护功能键、编辑键、方向键的转义序列表，
MUST NOT 用 Kotlin 侧的模式查询结果替代上游的模式读取。

编码 MUST 与既有鼠标编码同构地落在 VT 线程：编码器与事件对象由 VT 线程持有，
`Query` 携带键码、修饰位与文本，编码字节经通道回传后写入 PTY。

`NativeBridge.getMode` 模式查询通道 MUST NOT 出现在按键编码路径上。经核实它仍有
仪器化测试消费者（`VtCorrectnessInstrumentedTest` 断言 DECCKM 经该通道可见），
故该通道为测试与诊断用途而保留，MUST NOT 据「已无人调用」将其删除。

#### Scenario: 无修饰字母按 xterm 传统编码

- **WHEN** 按下 `A`，终端处于默认状态
- **THEN** PTY 收到单字节 `a`

#### Scenario: Ctrl 折叠为控制字节

- **WHEN** 按下 `A` 且 Ctrl 生效
- **THEN** PTY 收到 `0x01`

#### Scenario: Alt 前置 ESC

- **WHEN** 按下 `A` 且 Alt 生效
- **THEN** PTY 收到 `ESC` 紧跟 `a`

#### Scenario: 应用光标模式下的方向键为 SS3

- **WHEN** 终端已 `ESC [ ? 1 h`（DECCKM）且按下 `ArrowUp`
- **THEN** PTY 收到 `ESC O A`

#### Scenario: 应用光标模式下的带修饰方向键仍为 CSI

- **WHEN** 终端已 `ESC [ ? 1 h` 且 `ArrowUp` 叠加 Ctrl
- **THEN** PTY 收到 `ESC [ 1;5A`

#### Scenario: backarrow key mode 改变退格字节

- **WHEN** 终端已 `ESC [ ? 67 h` 且按下退格键
- **THEN** PTY 收到 `0x08`；未开启时收到 `0x7F`

#### Scenario: 修饰功能键走 CSI 修饰参数

- **WHEN** 终端已 `ESC [ > 4;2 m`（`modifyOtherKeys`=2）且 `A` 叠加 Ctrl
- **THEN** PTY 收到 `ESC [ 27;5;97~`

#### Scenario: 纯修饰键不产生输出

- **WHEN** 单独按下 Shift、Ctrl 或 CapsLock
- **THEN** 不向 PTY 写入任何字节

#### Scenario: 模式查询通道不在编码路径上

- **WHEN** 检查 `NativeBridge.getMode` 的调用点
- **THEN** 按键编码路径上不存在该调用；其消费者只有仪器化测试

### Requirement: Android 键码到上游 Key 的映射单一且无行为判断

`KEYCODE_*` 到上游 `key::Key` 的映射 MUST 是唯一的机械对应表，MUST NOT 在映射中
按修饰键、终端状态或应用模式分支——那会使编码在两处发生。修饰位 MUST 经
`KeyModifiers.ghosttyMods` 转为上游 `key.Mods` 原始位，MUST NOT 与本仓内部的
`SHIFT/ALT/CTRL/META` 位值混用。

按键的 `utf8` 文本 MUST 按上游文档约束过滤：C0 控制字符（U+0000-U+001F、U+007F）
MUST 传 `None` 而非原样传入，MUST NOT 传平台功能键私有区码位。映射表中无对应的
Android 键码 MUST 映射为上游 `Key::Unidentified`，由上游编码器判定其是否产出序列。

#### Scenario: 映射表给出上游 Key

- **WHEN** 以 `KEYCODE_DPAD_UP` 发起编码
- **THEN** 上游 `Key` 为 `ArrowUp`

#### Scenario: 修饰位取自统一入口

- **WHEN** 以 `META_SHIFT_ON | META_CTRL_ON` 发起编码
- **THEN** 上游 `Mods` 含 `SHIFT` 与 `CTRL`，不含 `ALT` 与 `SUPER`

#### Scenario: 未知键码不静默产生字符

- **WHEN** 以映射表未覆盖的 Android 键码发起编码且无 `unicodeChar`
- **THEN** 上游 `Key` 为 `Unidentified` 且不向 PTY 写入字节

### Requirement: 输入法提交文本不经按键编码路径

输入法 `commitText` 与组字增量提交的文本 MUST NOT 经按键编码路径：那是文本语义，
不是按键语义——编码器按逻辑键与修饰位派生序列，无法表达输入法提交的既成文本。
该路径保留其既有契约（孤立换行归一为 CR、多字符提交逐字保留），并 MUST NOT 因
本规范而改变。

#### Scenario: 输入法回车仍归一为 CR

- **WHEN** 输入法以 `commitText("\n")` 提交回车
- **THEN** PTY 收到 `0x0D`

#### Scenario: 多字符提交内的换行保留

- **WHEN** 输入法提交 `"a\nb"`
- **THEN** PTY 原样收到 `a` LF `b`
