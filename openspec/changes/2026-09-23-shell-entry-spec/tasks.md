# Shell 启动入口

## 上下文

- `DESIGN.md :122`–`:126`（设置框）、`:188`–`:198`（Shell 节）为标准。
- native 侧已合规，只改 Kotlin/Compose 侧。

## 任务

- [ ] 3.1 Shell 框默认为空 + 保存按钮：`DEFAULT_SHELL` 改空，框显示空文本，加保存按钮点击写入，删 shell 防抖自动保存。
- [ ] 3.2 删除启动目录全链条：`StartDirInput`、`START_DIR` 键与 flow、`setStartDir`、`ConfigReads.startDir`；`workingDirectory` 恒为家目录。
- [ ] 3.3 简化前缀探测：`findPrefixShell` 仅 `isFile` 存在性检查，顺序不变；`resolveShell` 空即 `SystemDefault`。
- [ ] 3.4 `[Process completed]` 文本对齐 `:192` 示例。
- [ ] 3.5 验证（`cargo test`、`testDebugUnitTest`、门禁、release APK 设备验证）后 `openspec archive`。
