# 配置文件

当前实现读取 `termux.properties` 中下列键；其余键与 `colors.properties`、`termux.float.properties`
一律不解析。终端配色只由内置主题决定，不存在用户颜色文件入口。

## 路径与读取

- 主路径：`/data/data/com.termux/files/home/.termux/termux.properties`
- 次路径：`/data/data/com.termux/files/home/.config/termux/termux.properties`
- 仅首个可读的常规文件生效，不合并；符号链接与非常规文件跳过；文件不存在时全部取默认值。
- 读取为 UTF-8；单文件超过 64 KiB 视为无效并跳过。
- 不创建、不写入、不修改任何文件；配置在启动首帧前与会话配置重下发时重读，不监听文件变化。

## 键

- `fullscreen`：隐藏状态栏，导航栏保持可见；默认 `false`。
- `ctrl-space-workaround`：部分输入法吞掉 Ctrl+空格时补发 NUL；仅终端页位于前台时生效；默认 `false`。
- `hide-soft-keyboard-on-startup`：启动时是否主动请求输入法。默认 `true`（不请求）；写 `false` 时在首帧后等待窗口取得焦点再请求一次，最多重试 2 秒。
- `extra-keys-text-all-caps`：修饰键栏键帽是否全大写；默认 `true`。
- `terminal-margin-horizontal`：终端左右边距（dp），范围 0–100；默认 3。
- `terminal-margin-vertical`：终端上下边距（dp），范围 0–100；默认 0。
- `terminal-cursor-style`：`block`/`bar`/`beam`/`underline`，非法回退 `block`；`beam` 与 `bar` 同为竖线。作为默认样式下发，程序用 DECSCUSR 指定过的样式仍然优先。
- `terminal-transcript-rows`：整数，范围 100–50000，非法回退 2000。
- `terminal-toolbar-height`：修饰键栏高度倍率，范围 0.4–3.0；默认 1.0。
- `extra-keys-style`：`default`/`arrows-only`/`arrows-all`/`all`/`none`；默认 `default`。
- `default-working-directory`：支持 `~`、`~/x`、相对路径与绝对路径；相对路径按家目录解析；尾部 `/` 归一（`/` 自身除外）；结果必须存在且可读，否则回退家目录。
- `back-key`：`back`/`escape`；`escape` 时返回键改发 ESC 且不退出应用；仅终端页位于前台时生效；默认 `back`。
- `volume-keys`：`virtual`/`volume`；`virtual` 时音量减作为一次性虚拟 Ctrl，音量加不拦截；仅终端页位于前台时生效；默认 `virtual`。
- `soft-keyboard-toggle-behaviour`：`show/hide`（默认，切换当次可见性）或 `enable/disable`（切换软键盘是否可用）；取值照 termux 上游原样，是斜杠而非连字符。禁用状态保存在组合状态中，不持久化到设置。