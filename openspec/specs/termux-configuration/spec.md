# termux-configuration Specification

## Purpose

`~/.termux/termux.properties` 与 `~/.termux/colors.properties` 是 termux 生态里用户
改配置的方式（`adb push`、SAF、装包脚本都直接写文件，不经过本应用的设置页）。
本能力定义这两份文件的读取时机、容错口径与逐键语义。

两份文件都**不经过设置页**，因此没有「设置变了」这种事件可用：读取只能发生在
启动与会话配置重下发这两个时机。读失败 MUST 保留上一份可用配置——写到一半的文件
不该把界面打回全默认。

## Requirements

### Requirement: 读取路径与容错

解析器 MUST 依次尝试 `~/.termux/<文件名>` 与 `~/.config/termux/<文件名>`，
前者不存在或不可读时回落后者。任一文件读取或解析失败 MUST 保留上一次成功的解析结果
并记录错误，MUST NOT 抛给调用方、MUST NOT 清空已有配置。

#### Scenario: 文件写到一半

- **WHEN** `termux.properties` 被外部进程截断成半行
- **THEN** 界面沿用上一次成功解析的配置，日志里有一条解析错误，应用不崩溃

#### Scenario: 两份路径都存在

- **WHEN** `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties` 同时存在
- **THEN** 以前者为准，后者不参与解析

### Requirement: properties 语法

解析器 MUST 支持 `key = value`、`#` 与 `!` 行注释、行尾续行符 `\`，
以及值内的 `\n` `\t` `\r` `\f` `\uXXXX` 转义。未知键 MUST 被忽略，
MUST NOT 使整份文件解析失败。数值键 MUST 钳到各自区间，越界值取边界。

#### Scenario: 未知键

- **WHEN** 文件里出现本应用不认识的键
- **THEN** 该行被忽略，其余键照常生效

### Requirement: colors.properties 覆盖

`colors.properties` 的 19 个颜色槽（背景、前景、光标、16 个 ANSI）MUST 以
「覆盖槽位」形式下发：未出现的槽用 `-1` 表示「沿用主题原色」，由原生按基底主题补齐。

Kotlin 侧 MUST NOT 拼出完整调色板。主题目录的打包数据不含光标色，在 Kotlin 侧拼
完整调色板会缺项，表现为「只改了背景，光标却变白」。

覆盖 MUST 压在**基底主题**之上，MUST NOT 压在上一次覆盖的结果之上——否则连续两次
覆盖会把上一次的结果当成新基底累积。

#### Scenario: 只改背景

- **WHEN** `colors.properties` 只写了 `background`
- **THEN** 背景换色，前景、光标与 16 个 ANSI 色保持当前主题原色

#### Scenario: 换主题后覆盖失效

- **WHEN** 用户在设置页换了一个主题，而 `colors.properties` 已不存在
- **THEN** 终端回到新主题的原色，上一次的覆盖不残留

### Requirement: 键栏由 extra-keys 决定

键栏的行数、键位与字形 MUST 完全由 `extra-keys` / `extra-keys-style` /
`extra-keys-text-all-caps` 决定，MUST NOT 另有写死的键位表。

`extra-keys` 解析出零行时 MUST NOT 画键栏，且网格 MUST NOT 为它预留高度。

未知键名 MUST 被跳过并记警告，MUST NOT 当作字面量渲染（那会让用户点出一串
无意义的字符）。`FN` 未实现，MUST NOT 渲染。

#### Scenario: 自定义三行布局

- **WHEN** `extra-keys` 给出三行
- **THEN** 键栏画三行，网格恰好预留三行的高度，终端内容不被遮挡

#### Scenario: 显式清空

- **WHEN** `extra-keys = []`
- **THEN** 键栏整条不出现，终端占满可用高度

### Requirement: 工作目录、铃音与光标

- `default-working-directory` MUST 支持 `~`、`~/x`、相对路径与绝对路径；
  尾部的 `/` MUST 被归一（`/` 自身除外），否则会话抽屉会随 shell 的 OSC 7 上报
  在同一次会话里换名字。
- `bell-character` MUST 支持 `vibrate`（默认）/`beep`/`ignore`；连续响铃 MUST 有
  最小间隔，避免每秒刷新的程序把手机震成蜂鸣器。
- `terminal-cursor-style` MUST 作为**默认**样式下发，程序用 DECSCUSR 指定过的样式
  仍然优先。
- `terminal-cursor-blink-rate` 为 0 时 MUST NOT 产生任何补帧；非 0 时相位 MUST 由
  时基计算而 MUST NOT 由帧计数累加——空闲省电会让帧间隔抖动到秒级。

#### Scenario: 静止终端里光标照常闪

- **WHEN** 终端没有任何输出，`terminal-cursor-blink-rate` 为 500
- **THEN** 光标每 500ms 翻转一次，画面随之重绘

### Requirement: 键盘切换键的两种语义

`soft-keyboard-toggle-behaviour` MUST 支持 `show-hide`（默认，切换当次可见性）与
`enable-disable`（切换「软键盘是否可用」）。

`enable-disable` 下禁用 MUST 收起正在显示的输入法，且此后的轻点 MUST NOT 拉起输入法
——**包括平台在焦点变化时自动弹键盘那条路径**：平台按 `onCheckIsTextEditor` 决定要不要
自动弹，禁用期间它 MUST 报否，否则轻点里那次 `requestFocus` 就足以把键盘带出来。

启用 MUST 只恢复可用性、不主动弹键盘：轻点终端即回来。

两种切换 MUST 各给一次提示：该模式在屏幕上不留任何痕迹，没有提示的话，同一次点击
在禁用与「坏了」之间无法区分。

#### Scenario: 禁用后轻点终端

- **WHEN** `soft-keyboard-toggle-behaviour = enable-disable`，用户按 KEYBOARD 键禁用软键盘
- **THEN** 正在显示的输入法立即收起；随后轻点终端只结束选区与滚动，不再弹出输入法

#### Scenario: 再按一次恢复

- **WHEN** 软键盘处于禁用状态，用户再按一次 KEYBOARD 键
- **THEN** 出现一次「已启用」提示，轻点终端可正常输入

### Requirement: 明确不实现的键

以下键 MUST 被解析但不改变行为，理由记录在
`openspec/changes/2026-10-30-termux-properties-and-colors/proposal.md`：

- `hide-soft-keyboard-on-startup`
- `disable-terminal-session-change-toast`
