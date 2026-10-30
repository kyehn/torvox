# 接入 termux.properties 与 colors.properties

## Why

`docs/specification/PROHIBITED.md` 把「解析 `~/.termux/termux.properties`、
`~/.config/termux/termux.properties`、`colors.properties`、`termux.float.properties`」
列为禁止项。本轮用户明确要求实现这些解析，且要求「不只是开关，而是实际功能」。
按 `AGENTS.md`「不得以规范文档反驳用户提示」，本轮以用户指令为准，
`termux.properties` 与 `colors.properties` 两份文件改为可解析并生效。

`PROHIBITED.md` 本身是受保护文件，本轮未改动；该条目与实际行为已不一致，
需要在获得明确同意后由用户决定删除或改写。

`termux.float.properties` 不在本轮范围内，仍属禁止。

另两条既有条目与本轮相邻，处理方式如下（两处规范均未改动）：

- `DESIGN.md`「输入光标为方块样式（高度、宽度等均参考 Termux），不闪烁」：
  `terminal-cursor-blink-rate` 的默认值取 **0**，不闪，故默认外观与该条一致；
  闪烁只在用户自己往文件里写间隔后出现。
- `PROHIBITED.md`「光标闪烁开关」「光标闪烁速度（提供调节条）」：本仓不提供任何
  开关或调节条，闪烁只能由 `termux.properties` 控制，仍在禁止范围之内。

## What Changes

### 配置读取

- 新增 `TermuxPropertiesRepository`：主路径 `~/.termux/`，兜底 `~/.config/termux/`；
  启动与每次 `applySettings` 重读。读取或解析失败保留上一份可用配置并记日志——
  写到一半的配置文件不该把界面打回全默认。
- `termux.properties` 覆盖 22 个键；`colors.properties` 覆盖 19 个颜色槽。

### 逐键语义

| 键 | 落地 |
| --- | --- |
| `extra-keys` | 键栏布局完全由配置决定（`buildExtraKeySlots`） |
| `extra-keys-style` | 只改字形，不改键位与 testTag |
| `extra-keys-text-all-caps` | 只影响字母 |
| `soft-keyboard-toggle-behaviour` | `enable-disable` 翻转「软键盘可用与否」：禁用时轻点不再拉起输入法 |
| `terminal-toolbar-height` | 单键高度缩放；网格预留随之变化 |
| `terminal-margin-horizontal` / `-vertical` | 内缩 SurfaceView，网格随尺寸收窄 |
| `terminal-cursor-style` | 推给 VT 作**默认**样式，程序 DECSCUSR 仍可覆盖 |
| `terminal-cursor-blink-rate` | 原生按时基算相位，宿主按间隔补帧 |
| `fullscreen` | 隐藏状态栏 |
| `use-fullscreen-workaround` | 决定唤出后是临时栏还是默认行为 |
| `night-mode` | 三态压在应用日夜判定之上，视图与原生同源 |
| `bell-character` | `on_bell` → 事件 → 震动/提示音/忽略 |
| `default-working-directory` | `~`、相对、绝对三种写法 |
| `back-key` | `escape` 时返回键发 ESC |
| `volume-keys` | `virtual` 时音量减当虚拟 Ctrl |
| `ctrl-space-workaround` | 补发 NUL |
| `shortcut.create/next/previous-session` | `Ctrl+Alt+<char>` 切换会话 |
| `disable-hardware-keyboard-shortcuts` | 关掉上一条 |
| `colors.properties` | 19 槽覆盖，`-1` = 沿用主题原色 |

### 明确不实现的两项

- **`hide-soft-keyboard-on-startup`**：本应用启动时本就不弹软键盘（唯一的弹出路径是
  用户轻点终端手势），故 `true` 的语义已天然成立；`false` 需要启动即请求 IME，
  而 Android 12+ 会静默拒绝非受信任手势之外的 `SHOW_IMPLICIT`。
- **`disable-terminal-session-change-toast`**：本应用没有会话切换 Toast，也没有
  对应的提示通道，该键被解析但不改变任何行为。

## Impact

- 新增能力：`openspec/specs/termux-configuration/`
- 受影响规范：`modifier-bar-height-reservation`（预留高度改为数据驱动）、
  `cursor-rendering`（新增闪烁与默认样式）
- 新增依赖：无
- 新增加载点：`TerminalRuntime.start()` 与 `applySettings()` 各一次文件读取
