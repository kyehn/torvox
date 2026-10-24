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

终端模式 MUST NOT 经独立的模式查询通道暴露给按键编码：本仓不设 `getMode` 之类的
模式查询导出，MUST NOT 据「测试需要观察模式」新增一个无生产调用方的导出。模式是否
生效 MUST 经编码输出的变化观察——DECCKM 置位使方向键从 `CSI A` 换成 `SS3 A`，复位
再换回来。

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

#### Scenario: 模式经编码输出而非模式查询观察

- **WHEN** 检查终端模式的可观察面
- **THEN** 不存在独立的模式查询导出；DECCKM 置位后方向键编码为 `SS3 A`、复位后回到 `CSI A`

### Requirement: Android 键码到上游 Key 的映射单一且无行为判断

`KEYCODE_*` 到上游 `key::Key` 的映射 MUST 是唯一的机械对应表，MUST NOT 在映射中
按修饰键、终端状态或应用模式分支——那会使编码在两处发生。修饰位 MUST 经
`KeyModifiers.ghosttyMods` 转为上游 `key.Mods` 原始位，MUST NOT 与本仓内部的
`SHIFT/ALT/CTRL/META` 位值混用。

按键的 `utf8` 文本 MUST 按上游文档约束过滤：C0 控制字符（U+0000-U+001F、U+007F）
MUST 传 `None` 而非原样传入，MUST NOT 传平台功能键私有区码位。映射表中无对应的
Android 键码 MUST NOT 在原生侧构造上游 `Key`：该键码在 JNI 边界短路为空字节数组，
交宿主按 `unicodeChar` 与虚拟键盘映射表决定是否回退（见「编码器未覆盖的按键回退走
同一条写入出口」），原生不构造 `Key::Unidentified`、也不进编码器。

#### Scenario: 映射表给出上游 Key

- **WHEN** 以 `KEYCODE_DPAD_UP` 发起编码
- **THEN** 上游 `Key` 为 `ArrowUp`

#### Scenario: 修饰位取自统一入口

- **WHEN** 以 `META_SHIFT_ON | META_CTRL_ON` 发起编码
- **THEN** 上游 `Mods` 含 `SHIFT` 与 `CTRL`，不含 `ALT` 与 `SUPER`

#### Scenario: 未知键码不在原生侧产生 Key

- **WHEN** 以映射表未覆盖的 Android 键码发起编码
- **THEN** 原生返回空字节数组，不构造上游 `Key`，也不由编码器产出序列

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

### Requirement: 编码器未覆盖的按键回退走同一条写入出口

上游编码器未覆盖的键码（其 `Key` 无对应项，如 `KEYCODE_AT` 的 `@` 实为 Shift+2），
以及输入法在 `unicodeChar == 0` 时须由虚拟键盘映射表推导的字符，MUST 走与其余按键
**同一条**有序写入出口（宿主队列）并触发渲染唤醒钩子；原生侧 MUST 只做编码，MUST NOT
在其中同步写 PTY。该路径的调用点在 UI 线程上，而主端满时同步写最多等 5 秒——绕过队列
即每敲一次这类键冻住主线程（ANR），且回显要多等一个空闲闭锁才出现。

#### Scenario: 回退按键不阻塞 UI 线程

- **WHEN** 子进程不读 stdin，此时按下编码器未覆盖的键
- **THEN** 该按键经宿主写入队列送出，UI 线程不等待子进程消费

#### Scenario: Alt 前缀与普通字符同样直写

- **WHEN** 回退路径编码带 ESC 前缀的字符（Alt/Super）与不带前缀的普通字符
- **THEN** 两者都以原生给出的字节原样写入，不经 VT 引擎重排

### Requirement: 触摸手势在鼠标上报中转发为鼠标事件

子进程启用鼠标上报（DECSET 1000/1002/1003）时，触摸手势 MUST 经上游鼠标编码器
转发为鼠标事件，MUST NOT 落回本地手势语义（拉起键盘、滚动本地回滚）：轻点 MUST
编码为同一单元格的左键按下+抬起，拖动 MUST 按滚过的行转发为滚轮。转发与否 MUST
由编码器的输出决定——应用未启用上报时编码器返回空、不写 PTY，轻点照常走终端
自身的链接/选区/焦点路径，MUST NOT 由宿主另行猜测上报模式。

备用屏没有本地回滚，编码器不产出时拖动静默吞掉；主屏编码器不产出时拖动回落
为本地回滚。本次拖动已转发滚轮的手势，其惯性段 MUST 同样属于应用，
MUST NOT 让本地回滚接管。

#### Scenario: 轻点转发为同单元格左键

- **WHEN** 子进程已启用鼠标上报且用户轻点某单元格
- **THEN** PTY 收到该单元格的左键按下与抬起（SGR 下同坐标的 `M` 与 `m`），
      不拉起输入法

#### Scenario: 拖动转发为滚轮而非本地回滚

- **WHEN** 子进程已启用鼠标上报且用户拖动数行
- **THEN** 每滚过一行 PTY 收到一个滚轮事件（手指上移为滚轮上），本地回滚偏移不变

#### Scenario: 一次拖动事件的全部档位一次往返编完

- **WHEN** 单个 `onScroll` 事件滚过多行
- **THEN** 全部档位由一次原生调用编成（每档一对按下/抬起，坐标相同）并一次写入 PTY，
      档位数与滚过的行数一致，不逐档往返 VT 线程

#### Scenario: 上报状态由镜像读取

- **WHEN** 触摸拖动需要判断子进程是否启用鼠标上报
- **THEN** 判定取自 VT 线程维护的无锁镜像，不发起模式查询；镜像随 DECSET/DECRST
      装载与退出同步翻转

#### Scenario: 未启用上报时轻点仍具终端语义

- **WHEN** 子进程未启用鼠标上报且用户轻点
- **THEN** 不向 PTY 写入鼠标序列，轻点照常打开链接或聚焦并拉起输入法

#### Scenario: 转发过的手势其惯性不滚本地回滚

- **WHEN** 拖动已转发滚轮、手指以划动速度抬起
- **THEN** 惯性段不产生本地回滚位移

### Requirement: 焦点上报直写子 PTY 且只在子进程启用 1004 时发送

DECSET 1004 焦点事件的 `CSI I`/`CSI O` MUST 直写子 PTY，MUST NOT 进入 VT 引擎
（引擎的输出流解析器把 `CSI I` 当 CHT、把 `CSI O` 当非法 CSI，送入只会移动光标而非
通知应用）；且只在子进程确实启用了 1004 时发送（xterm 语义）。

模式查询的等待窗口 MUST 严格宽于 VT 线程的空闲 tick：VT 线程只在命令批尾与 tick 到点
时排空查询通道，窗口与之相等即与该 tick 赛跑，超时先到会被当成「子进程没开 1004」而
静默丢掉这一次焦点上报。

#### Scenario: 1004 未启用时静默

- **WHEN** 子进程未启用 1004，视图发生焦点变化
- **THEN** 不向子 PTY 写入任何字节

#### Scenario: 查询窗口宽于 VT 空闲 tick

- **WHEN** 比对焦点模式查询的等待窗口与 VT 线程的空闲 tick
- **THEN** 前者严格大于后者

### Requirement: Android 键码映射覆盖 Termux 键表

`KeyEvent` 键码到上游 `key::Key` 的映射 MUST 覆盖 Termux `KeyHandler` 的键集。
未映射的键退回按键文本；对没有文本的键（`KEYCODE_BREAK` 等）该回退为空，整个键
静默失效。映射只表达「是哪个键」，转义序列 MUST 仍由上游编码器按活体模式产出。

#### Scenario: Pause/Break 到达编码器

- **WHEN** 硬件键盘按下 Pause/Break（`KEYCODE_BREAK`）
- **THEN** 事件按 `key::Key::Pause` 编码发出，MUST NOT 因无文本回退而被丢弃
      （用例 `pause_break_key_maps_to_pause`）

#### Scenario: 系统导航键仍不进键表

- **WHEN** 按下返回键（`KEYCODE_BACK`）
- **THEN** 该键不属于终端键表，由宿主的手势与返回处理消费
