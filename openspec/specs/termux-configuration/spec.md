# termux-configuration Specification

## Purpose

`~/.termux/termux.properties` 是 termux 生态里用户改配置的方式（`adb push`、SAF、
装包脚本都直接写文件，不经过本应用的设置页）。本能力定义该文件的读取时机、容错口径
与逐键语义。

该文件**不经过设置页**，因此没有「设置变了」这种事件可用：读取只能发生在
启动与会话配置重下发这两个时机。读失败 MUST 保留上一份可用配置——写到一半的文件
不该把界面打回全默认。

## Requirements

### Requirement: 读取路径与容错

解析器 MUST 依次尝试 `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties`，
前者不存在或不可读时回落后者。文件读取或解析失败 MUST 保留上一次成功的解析结果
并记录错误，MUST NOT 抛给调用方、MUST NOT 清空已有配置。

只认**常规文件**且 MUST NOT 跟随符号链接：该文件由外部写入，链接可以把读取带到
家目录之外，也可能指向设备；非常规文件（管道、目录）同样跳过。

读取 MUST 为 UTF-8；单文件超过 64 KiB MUST 视为无效并跳过（MUST NOT 读进内存）。
应用 MUST NOT 创建、写入或修改该文件。

`colors.properties` MUST NOT 被读取或解析，终端配色只来自内置主题。

#### Scenario: 文件写到一半

- **WHEN** `termux.properties` 被外部进程截断成半行
- **THEN** 界面沿用上一次成功解析的配置，日志里有一条解析错误，应用不崩溃

#### Scenario: 两份路径都存在

- **WHEN** `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties` 同时存在
- **THEN** 以前者为准，后者不参与解析

#### Scenario: 配置是符号链接

- **WHEN** `~/.termux/termux.properties` 是指向别处的符号链接
- **THEN** 该路径被跳过，改用次路径；次路径不存在时全部取默认值

#### Scenario: 配置过大

- **WHEN** `termux.properties` 超过 64 KiB
- **THEN** 该文件不被读取，日志里有一条体积超限的说明，配置取默认值

#### Scenario: colors.properties 不生效

- **WHEN** 用户写入 `~/.termux/colors.properties`
- **THEN** 终端配色仍取当前主题，该文件不影响任何颜色

### Requirement: properties 语法

解析器 MUST 支持 `key = value`、`#` 与 `!` 行注释、行尾续行符 `\`，
以及值内的 `\n` `\t` `\r` `\f` `\uXXXX` 转义。未知键 MUST 被忽略，
MUST NOT 使整份文件解析失败。

数值键越界 MUST 回退各自的默认值并记录错误，MUST NOT 钳到区间边界——
上游 `getDefaultIfNotInRange` 的口径是「没配好就用默认」。

#### Scenario: 未知键

- **WHEN** 文件里出现本应用不认识的键
- **THEN** 该行被忽略，其余键照常生效

#### Scenario: 数值越界

- **WHEN** `terminal-transcript-rows` 写成 99999（有效区间 100–50000）
- **THEN** 回滚行数取默认 2000，日志里有一条越界说明

### Requirement: 键栏布局由 extra-keys 决定

键栏的行数、键位与字形 MUST 完全由 `extra-keys` / `extra-keys-style` /
`extra-keys-text-all-caps` 决定，MUST NOT 另有写死的键位表。

`extra-keys` 解析出零行时 MUST NOT 画键栏，且网格 MUST NOT 为它预留高度：留着一条
点不出东西、也看不见内容的空白带，比没有键栏更糟。

#### Scenario: 自定义三行布局

- **WHEN** `extra-keys` 给出三行
- **THEN** 键栏画三行，网格恰好预留三行的高度，终端内容不被遮挡

#### Scenario: 显式清空

- **WHEN** `extra-keys = []`
- **THEN** 键栏整条不出现，终端占满可用高度

### Requirement: extra-keys-style 的字形档位

`extra-keys-style` MUST 接受 `default` / `arrows-only` / `arrows-all` / `all` / `none`，
字形集合逐项对齐 termux `ExtraKeysConstants`。

五档不是五套互不相干的外观，而是同一组字形表的嵌套裁剪：`none` 一个都不换，
`arrows-only` 只换方向键与横线，`default` 再加回车/Tab/退格等常用符号，
`arrows-all` 再加 Home/End/PgUp/PgDn，`all` 最后把 ESC/CTRL/ALT 换成 ISO 符号。

#### Scenario: 逐级加字形

- **WHEN** `extra-keys-style` 从 `none` 逐档改到 `all`
- **THEN** 每一档都在上一档基础上多出一组字形，`all` 一档含 ESC/CTRL/ALT 的 ISO 符号

### Requirement: 键栏中不可渲染的项

未知键名 MUST 被跳过并记警告，MUST NOT 当作字面量渲染。
键名 MUST 先套用 termux 上游的别名表（`ESCAPE`=`ESC`、`CONTROL`=`CTRL`、`SHFT`=`SHIFT`、
`RETURN`=`ENTER`、`LT`/`RT`/`DN`=方向键、`PAGEUP`/`PAGE UP`=`PGUP`、`DELETE`=`DEL`、
`BACKSPACE`=`BKSP`、`BACKSLASH`=`\` 等）：上游按同一张表归一，不套表则同一份 `extra-keys`
在两个应用里画出的键栏不一样。

`FN` 与 `SHIFT` 键 MUST NOT 渲染：`SHIFT` 由粘滞修饰栏承担，`FN` 在本应用没有二次映射。

对象形式的 `popup` MUST 被丢弃并记一条说明，主键本身照常保留。只有 `display`、没有
`key`/`macro` 的对象 MUST 被丢弃并记一条说明。只含坏字段的对象 MUST 只丢自己那一项，
MUST NOT 作废整行或整份布局。

`display` MUST 覆盖键帽文本（`extra-keys-text-all-caps` 照常作用于它），MUST NOT 改变键码、
特殊动作或长按内容。

#### Scenario: 未知键名不落到键栏

- **WHEN** `extra-keys` 里出现本应用不认识的键名
- **THEN** 该键不出现在键栏上，日志里有一条警告，同行其余按键照常渲染

#### Scenario: 别名键名与原名等价

- **WHEN** `extra-keys` 里写 `ESCAPE` 而非 `ESC`
- **THEN** 键栏画出一个与 `ESC` 等价的键，按下发出 ESC

#### Scenario: 空对象只丢自己

- **WHEN** `extra-keys = [['ESC',{popup:'|'},'TAB']]`
- **THEN** 键栏只有 ESC 与 TAB，日志里说明那一项被丢弃

#### Scenario: display 覆盖键帽

- **WHEN** `extra-keys = [[{key:'ESC',display:'quit'}]]` 且 `extra-keys-text-all-caps = false`
- **THEN** 键帽显示 `quit`，按下仍发出 ESC

### Requirement: 键栏布局的容错

可解析布局里的坏项 MUST NOT 让整份布局回落到默认：一个不认识的键不该作废用户配好的其余键位。

两个例外：语法上无法配平的整份文本 MUST 退回默认布局并记错误；「写了行、但每一行都被坏项
掏空」时 MUST 退回默认布局并记错误，因为键栏是抽屉与滚动锁定的唯一入口，空键栏会把它们
一起藏起来。显式写 `[]` 仍按「不要键栏」处理。

#### Scenario: 坏项不连坐

- **WHEN** `extra-keys` 的一行里有一个坏项与若干好项
- **THEN** 好项照常渲染，坏项被跳过

#### Scenario: 整行都被掏空时退回默认

- **WHEN** `extra-keys` 写了行，但每一行都被坏项掏空
- **THEN** 键栏退回默认布局，并记一条错误

### Requirement: extra-keys 的宏

`{macro: "..."}` MUST 被解析并播放：宏串按空白切分，`CTRL`/`ALT`/`SHIFT` 是修饰记号，
作用于其后第一个普通记号（用过即清），普通记号是键名或可编码的单字符——语义与 termux
上游 `TerminalExtraKeys` 一致。大写字母是字面量，MUST 补 Shift 以产出大写，MUST NOT
被当成小写字母发送。

普通记号 MUST 走原生键编码器，并连同平台文本一起下发：可打印键没有文本时编码器
不产生任何字节，Ctrl 折叠与 Alt 的 ESC 前缀也由编码器决定。

宏 MUST NOT 影响键栏上的粘滞 CTRL/ALT：宏自带修饰位，两者叠加会让语义与用户预期不符。

宏内出现 `FN` 或无法识别的记号时，该项 MUST NOT 渲染（只丢自己那一项），MUST NOT
发出与配置不符的半截按键序列。

#### Scenario: 宏按顺序发出

- **WHEN** `extra-keys` 里有一项 `{macro: "CTRL f d"}` 且用户点击它
- **THEN** 终端依次收到 Ctrl+f 与 d 两个按键

#### Scenario: 宏里的修饰只作用于一个键

- **WHEN** `extra-keys` 里有一项 `{macro: "CTRL f d"}`
- **THEN** `d` 不带 Ctrl

#### Scenario: 不可用的宏不渲染

- **WHEN** `extra-keys` 里有一项 `{macro: "FN f"}`
- **THEN** 该项不占位置也不可点，其余键位照常生效

### Requirement: default-working-directory 的解析

`default-working-directory` MUST 支持 `~`、`~/x`、相对路径与绝对路径；相对路径按
家目录解析。尾部的 `/` MUST 被归一（`/` 自身除外），否则会话抽屉会随 shell 的
OSC 7 上报在同一次会话里换名字。

解析结果 MUST 落在真实存在且可读的目录上，否则回退家目录：目录不存在或不可读时 PTY 会直接失败，
配置里的一个笔误不该让应用「打不开终端」。

#### Scenario: 工作目录不存在

- **WHEN** `default-working-directory` 指向一个不存在的路径
- **THEN** 会话仍能启动，工作目录是家目录

#### Scenario: 尾部斜杠被归一

- **WHEN** `default-working-directory` 写成 `~/work/`
- **THEN** 工作目录是 `~/work`，OSC 7 上报的名字不带尾斜杠

### Requirement: terminal-cursor-style 的取值

`terminal-cursor-style` MUST 接受 `block`/`bar`/`underline`，非法值 MUST 回退 `block`。
另 MUST 接受 `beam` 作为 `bar` 的别名（同为竖线）——termux 上游没有这个取值，它是本
应用对常见写法的兼容。

它 MUST 作为**默认**样式下发，程序用 DECSCUSR 指定过的样式仍然优先。

#### Scenario: beam 与 bar 同形

- **WHEN** `terminal-cursor-style = beam`
- **THEN** 光标画成竖线，与 `bar` 的效果相同

### Requirement: 键盘切换键的两种语义

`soft-keyboard-toggle-behaviour` MUST 支持 `show/hide`（默认，切换当次可见性）与
`enable/disable`（切换「软键盘是否可用」）——取值照 termux 上游原样，是斜杠而非连字符。

`enable/disable` 下禁用 MUST 收起正在显示的输入法，且此后的轻点 MUST NOT 拉起输入法
——**包括平台在焦点变化时自动弹键盘那条路径**：平台按 `onCheckIsTextEditor` 决定要不要
自动弹，禁用期间它 MUST 报否，否则轻点里那次 `requestFocus` 就足以把键盘带出来。

启用 MUST 只恢复可用性、不主动弹键盘：轻点终端即回来。

两种切换 MUST 各给一次提示：该模式在屏幕上不留任何痕迹，没有提示的话，同一次点击
在禁用与「坏了」之间无法区分。

#### Scenario: 禁用后轻点终端

- **WHEN** `soft-keyboard-toggle-behaviour = enable/disable`，用户按 KEYBOARD 键禁用软键盘
- **THEN** 正在显示的输入法立即收起；随后轻点终端只结束选区与滚动，不再弹出输入法

#### Scenario: 再按一次恢复

- **WHEN** 软键盘处于禁用状态，用户再按一次 KEYBOARD 键
- **THEN** 出现一次「已启用」提示，轻点终端可正常输入

### Requirement: 启动时的软键盘

`hide-soft-keyboard-on-startup = true`（本应用默认）MUST 表示启动时不主动请求输入法，
且平台按 `onCheckIsTextEditor` 自动弹键盘那条路径也不得在启动时把键盘带出来。

`false` MUST 在首帧后请求输入法，并把「本视图可接收文本输入」的答案置真——
否则焦点早已就位，平台不会再问，键盘永远不出现。请求 MUST 等窗口取得焦点后再执行；
窗口尚未取得焦点时 MUST 重试，最多 2 秒；成功或超时后 MUST 不再重试。

默认取 `true` 而非上游的 `false`，因为本应用启动时从不请求输入法，默认值必须反映
既有外观。

#### Scenario: 默认启动

- **WHEN** 没有写这个键，冷启动应用
- **THEN** 键盘保持隐藏，直到用户轻点终端

#### Scenario: 要求启动即弹

- **WHEN** `hide-soft-keyboard-on-startup = false`，冷启动
- **THEN** 终端拿到焦点后弹出一次输入法；用户收起后不会被再次弹回

### Requirement: 窗口与边距类键

下列键 MUST 按 `docs/specification/config.md` 的取值与默认值生效：

- `fullscreen`：布尔，默认 `false`；为真时隐藏状态栏。
- `use-fullscreen-workaround`：布尔，默认 `false`；为真时全屏下系统栏改为滑动临时唤出。
- `extra-keys-text-all-caps`：布尔，默认 `true`；键帽是否大写。
- `terminal-margin-horizontal`：整数 0–100（dp），默认 3。
- `terminal-margin-vertical`：整数 0–100（dp），默认 0。
- `terminal-toolbar-height`：浮点 0.4–3，默认 1；修饰键栏单键高度倍率。

#### Scenario: 全屏隐藏状态栏

- **WHEN** `fullscreen = true`
- **THEN** 状态栏隐藏，终端占满窗口

#### Scenario: 边距与倍率按配置生效

- **WHEN** `terminal-margin-horizontal = 10` 且 `terminal-toolbar-height = 1.5`
- **THEN** 终端内容左右内缩 10 dp，键栏单键高度为默认值的 1.5 倍

### Requirement: 返回键、音量键与 Ctrl+空格

`back-key` MUST 支持 `back`/`escape`（默认 `back`）：`escape` 时返回键改发 ESC。

`volume-keys` MUST 支持 `virtual`/`volume`（默认 `virtual`）：`virtual` 时音量减按下即置为
虚拟 Ctrl，按住期间持续生效、抬起时回收；`volume` 时不拦截，系统照常调节音量。外接字母
键盘发出的音量键 MUST NOT 拦截（termux 上游口径：完整键盘在场时不抢它的音量键）；音量加
MUST NOT 拦截——上游把它映射成虚拟 FN，本应用没有 FN 映射。

`ctrl-space-workaround` MUST 为布尔（默认 `false`）：为真时把 Ctrl+空格补发为 NUL。

#### Scenario: escape 模式下返回键发 ESC

- **WHEN** `back-key = escape` 且用户按系统返回键
- **THEN** 终端收到 ESC，而不是退出应用

#### Scenario: 外接字母键盘的音量键不被拦截

- **WHEN** 外接字母键盘在场，用户按它的音量减
- **THEN** 该按键不被应用拦截，交给键盘自身处理

#### Scenario: Ctrl+空格补发为 NUL

- **WHEN** `ctrl-space-workaround = true` 且用户按下 Ctrl+空格
- **THEN** 终端收到一个 NUL
