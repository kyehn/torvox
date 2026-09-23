# Shell 入口按规范重实现

## 背景

`DESIGN.md` Shell 启动入口节（`:122`–`:126`）与 Shell 节（`:188`–`:198`）为标准，现状多处相悖：Shell 设置默认值为 `/system/bin/sh` 预填（违 `:122` 未设置时为空）；无保存按钮，靠防抖自动写入（违 `:122` 提供保存按钮）；启动目录设置框、`START_DIR` 键、`setStartDir`、`workingDirectory` 自定义链条全套存在（违 `:126` 不提供启动目录设置与 `PROHIBITED.md :36`）；前缀探测要求 ELF 魔数或系统 shebang 且要求 `etc` 目录完整（违 `:188` 文件存在即启动，不检查文件权限）；`[Process completed]` 提示文本括号位置与 `:192` 示例不一致；`resolveShell` 对 `/system/bin/sh` 做特殊映射（违 `:125` 不特殊处理）。

native 侧已合规（`split_shell_entry` 空白切分透传参数、`build_env` 白名单、失败走 `[Process completed]` 无回退），本次只改 Kotlin/Compose 侧。

## 改动

- Shell 设置默认值改空，设置框显示空文本，`placeholder` 保留示例；加保存按钮，点击才写入，移除 shell 防抖自动保存。
- 删除启动目录全链条：`StartDirInput`、`START_DIR` 键与 flow、`setStartDir`、`ConfigReads.startDir`；`workingDirectory` 恒为家目录。
- `findPrefixShell` 简化为仅 `isFile` 存在性检查（bash→login 顺序不变）；`isElf`/`isSystemShellScript` 保留（安装器共用）。
- `resolveShell` 简化为空即 `SystemDefault`，其余原样 `Custom`，删 `/system/bin/sh` 特殊映射。
- `[Process completed]` 后缀对齐 `:192` 示例文本。
- failsafe 快捷方式与 native spawn 链不动（已合规）。

## 影响

- 测试：`SettingsRepositoryTest` 加 shell 默认空与保存用例；`cargo test` `testDebugUnitTest` 通过；release APK 设备验证（空设置回退链、自定义三项启动、启动目录项消失）。
- 不碰：native 侧、failsafe、ENV/mkshrc、回滚行数。
