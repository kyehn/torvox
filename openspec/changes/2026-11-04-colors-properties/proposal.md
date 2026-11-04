# Proposal

## Why

`~/.termux/colors.properties` 是 termux 生态里改终端配色的既有手段：用户用
`adb push` 或 SAF 写一个文件即可改背景、前景、光标与十六色，不必进应用设置页。
本仓此前把它列为禁止实现（`docs/specification/PROHIBITED.md` 与
`openspec/specs/prohibited-features`），于是用户在 termux 里沿用的习惯在本应用里
无声失效，也没有任何提示说明原因。

上游 termux v0.119.0-beta.3 的实现已查清（`TerminalColorScheme.updateWith` 与
`TermuxTerminalSessionActivityClient.checkForFontAndColors`）：

- 它**不是完整主题**，而是覆盖：先 `reset()` 回内置方案，再逐键覆盖 `mDefaultColors`，
  未写的键保持内置值。
- 键只有 `foreground`、`background`、`cursor` 与 `color<N>`；`color256`/`257`/`258`
  是前三个键的别名（上游把三者写进同一个调色板数组）。
- 色值按 XQueryColor 语法解析（`TerminalColors.parse`）：`#RGB`、`#RRGGBB`、
  `#RRRGGGBBB`、`#RRRRGGGGBBBB`、`rgb:R/G/B`，alpha 恒为不透明。
- 未写 `cursor` 时按背景的感知亮度（HSP，阈值 130）取白或黑。

本仓与上游的差别在于：本仓有 13 套可选主题（上游只有一份写死的方案），因此覆盖必须
**落在主题之上**，而不是替代主题。

## What Changes

- 解除 `colors.properties` 的禁止项（`termux.float.properties` 仍禁止）。
- 新增 `colors-configuration` 能力：启动时读一次 `~/.termux/colors.properties`
  （上游只有这一条路径），按上游的键与色值语法解析成对当前主题的部分覆盖。
- 覆盖与主题一起下发、在下发前合并：未覆盖的槽沿用主题原色，选区底色无对应键故原样
  保留；覆盖后的背景参与明暗上报与光标色自适应。
- `color16`..`color255`（256 色立方体）本仓不支持——调色板只有 16 个 ANSI 槽，
  256 色由 `libghostty-vt` 按 xterm 标准生成；此类键按「不支持的键记警告并忽略」处理。

## Impact

- 规格：新增 `colors-configuration`；`prohibited-features` 与 `termux-configuration`
  各修改一条 Requirement，去掉 `colors.properties` 的禁止表述。
- 文档：`docs/specification/PROHIBITED.md` 与 `docs/specification/CONFIG.md`。
- 代码：Kotlin 侧新增 `ColorsProperties`（解析）与文件读取；Rust 侧新增调色板覆盖
  通道（`Palette::with_overrides`）并随 `setTheme` 下发；JNI `setTheme` 增加覆盖数组
  形参。
