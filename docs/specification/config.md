# 配置文件

仅读取 `termux.properties` 中允许的三个键；其余键与 `colors.properties`、`termux.float.properties` 一律不解析。终端配色只由内置主题决定，不存在用户颜色文件入口。

## 路径与读取

- 主路径：`/data/data/com.termux/files/home/.termux/termux.properties`
- 次路径：`/data/data/com.termux/files/home/.config/termux/termux.properties`
- 仅首个可读的常规文件生效，不合并；符号链接与非常规文件跳过；文件不存在时全部取默认值。
- 读取为 UTF-8；单文件超过 64 KiB 视为无效并跳过。
- 不创建、不写入、不修改任何文件；配置在启动首帧前与会话配置重下发时重读，不监听文件变化。

## 键

- `default-working-directory`：目录必须存在且为目录，否则回退家目录。
- `terminal-transcript-rows`：整数，范围 100–50000，非法回退 2000。
- `terminal-cursor-style`：`block`/`bar`/`beam`/`underline`，非法回退 `block`。
