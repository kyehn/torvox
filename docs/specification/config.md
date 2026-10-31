# 配置文件

读取 `termux.properties` 中下列键；其余键与 `colors.properties`、`termux.float.properties`
一律不解析。终端配色只由内置主题决定，不存在用户颜色文件入口。

## 路径与读取

- 主路径：`/data/data/com.termux/files/home/.termux/termux.properties`
- 次路径：`/data/data/com.termux/files/home/.config/termux/termux.properties`
- 仅首个可读的常规文件生效，不合并；符号链接与非常规文件跳过；文件不存在时全部取默认值。
- 读取为 UTF-8；单文件超过 64 KiB 视为无效并跳过。
- 不创建、不写入、不修改任何文件；配置在启动首帧前与会话配置重下发时重读，不监听文件变化。

## 键

- `default-working-directory`：目录必须存在且可读，否则回退家目录；支持 `~`、`~/x`、相对/绝对路径，尾部 `/` 归一。
- `terminal-transcript-rows`：整数，范围 100–50000，非法回退 2000。
- `terminal-cursor-style`：`block`/`bar`/`beam`/`underline`，非法回退 `block`（`beam` 与 `bar` 同为竖线）。
- `fullscreen`：布尔，默认 `false`；为真时隐藏状态栏。
- `use-fullscreen-workaround`：布尔，默认 `false`；为真时全屏下系统栏改为滑动临时唤出。
- `ctrl-space-workaround`：布尔，默认 `false`；为真时补发 Ctrl+空格为 NUL。
- `hide-soft-keyboard-on-startup`：布尔，默认 `true`；为 `false` 时启动后请求一次输入法。
- `extra-keys-text-all-caps`：布尔，默认 `true`；键帽是否大写。
- `terminal-margin-horizontal`：整数 0–100（dp），默认 3。
- `terminal-margin-vertical`：整数 0–100（dp），默认 0。
- `terminal-toolbar-height`：浮点 0.4–3，默认 1；修饰键栏单键高度倍率。
- `extra-keys-style`：`default`/`arrows-only`/`arrows-all`/`all`/`none`，默认 `default`。
- `extra-keys`：键栏布局（行、键位、`display`/`popup`/`macro`），解析失败回退默认两行布局。
- `back-key`：`back`/`escape`，默认 `back`；`escape` 时返回键改发 ESC。
- `volume-keys`：`virtual`/`volume`，默认 `virtual`；`virtual` 时音量减作虚拟 Ctrl。
- `soft-keyboard-toggle-behaviour`：`show/hide`/`enable/disable`，默认 `show/hide`。
