# mkshrc 路径修正

## 上下文

- `DESIGN.md` Shell 节：`.mkshrc` 不得放在 `$HOME` 下，应用不该修改用户数据；设置节：用户数据为 `/data/data/com.termux/files`（除 Bootstrap 设置外不得修改）。
- 现状：`ensureMkshPromptRc()` 写 `$HOME/.mkshrc`，`build_env` 注入 `ENV=$HOME/.mkshrc`，均落在用户数据树内。
- 路径只有 Kotlin（持有 `Context`）可知，native 侧无法自行推导应用私有目录。

## 任务

- [ ] 1.1 native `ShellEnv` 新增 `mkshrc_path: Option<String>`，默认 `None`；`build_env` 改为 `Some` 时注入 `ENV=mkshrc_path`、`None` 时不注入；`pty.rs` 单测改为断言传入路径、补 `None` 不注入用例。
- [ ] 1.2 Kotlin `TerminalRuntime`：`ensureMkshPromptRc` 改写 `applicationInfo.dataDir/.mkshrc`（文档路径）；`buildConfig`/`buildFailsafeConfig` 把该路径放入 `TerminalConfig`；`Bridge.kt` `TerminalConfig` 与 `NativeBridge.kt` `initSession` 新增 `mkshrcPath` 参数。
- [ ] 1.3 `ffi.rs` `initSession` 新增 `mkshrcPath` JString 参数并接入 `ShellEnv`（空串→`None`，沿用 `prefix` 模式）；同步 9 处测试调用点传空串。
- [ ] 1.4 验证：`cargo test --package native shell_env` `cargo test --package native pty` `./gradlew ':app:testDebugUnitTest'`；设备验证新路径 rc 经 `ENV` 加载出短提示符、`ENV` 不含 `home` 前缀。
- [ ] 1.5 实现验证后按实际补充本 change 的 specs 实现细节，`openspec archive` 同步主 spec 并归档。
