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

### Requirement: 键栏布局固定

`extra-keys` MUST NOT 被读取或解析；键栏固定为 2 行 7 列：
`ESC DRAWER SCROLL HOME ↑ END PGUP` / `TAB CTRL ALT ← ↓ → PGDN`。
`DRAWER` 与 `SCROLL` 是本应用的固定入口，分别打开会话抽屉与锁定滚动位置。

字形 MUST 由 `extra-keys-style` 与 `extra-keys-text-all-caps` 决定；两者只改变键帽文本，
MUST NOT 改变键位或键码。

#### Scenario: 固定布局

- **WHEN** 解析任意 `termux.properties`
- **THEN** 键栏仍为固定 2 行 7 列，`extra-keys` 不影响布局

### Requirement: extra-keys-style 的字形档位

`extra-keys-style` MUST 接受 `default` / `arrows-only` / `arrows-all` / `all` / `none`，
字形集合逐项对齐 termux `ExtraKeysConstants`。

五档不是五套互不相干的外观，而是同一组字形表的嵌套裁剪：`none` 一个都不换，
`arrows-only` 只换方向键与横线，`default` 再加回车/Tab/退格等常用符号，
`arrows-all` 再加 Home/End/PgUp/PgDn，`all` 最后把 ESC/CTRL/ALT 换成 ISO 符号。

#### Scenario: 逐级加字形

- **WHEN** `extra-keys-style` 从 `none` 逐档改到 `all`
- **THEN** 每一档都在上一档基础上多出一组字形，`all` 一档含 ESC/CTRL/ALT 的 ISO 符号

### Requirement: default-working-directory 的解析

`default-working-directory` MUST 支持 `~`、`~/x`、相对路径与绝对路径；相对路径按
家目录解析。尾部的 `/` MUST 被归一（`/` 自身除外），否则会话抽屉会随 shell 的
OSC 7 上报在同一次会话里换名字。

解析结果 MUST 落在真实存在的目录上，否则回退家目录：目录不存在时 PTY 会直接失败，
配置里的一个笔误不该让应用「打不开终端」。

#### Scenario: 工作目录不存在

- **WHEN** `default-working-directory` 指向一个不存在的路径
- **THEN** 会话仍能启动，工作目录是家目录

#### Scenario: 尾部斜杠被归一

- **WHEN** `default-working-directory` 写成 `~/work/`
- **THEN** 工作目录是 `~/work`，OSC 7 上报的名字不带尾斜杠

### Requirement: terminal-cursor-style 的取值

`terminal-cursor-style` MUST 接受 `block`/`bar`/`beam`/`underline`（`beam` 与 `bar`
同为竖线），非法值 MUST 回退 `block`。

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
