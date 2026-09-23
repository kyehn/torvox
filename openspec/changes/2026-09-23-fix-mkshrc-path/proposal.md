## Why

`docs/specification/DESIGN.md` Shell 节要求：`.mkshrc` 不得放在 `$HOME` 下，应用不该修改用户数据；设置节定义用户数据为 `/data/data/com.termux/files` 目录（除 Bootstrap 设置外不得修改）。当前实现两处违反：Kotlin `ensureMkshPromptRc()` 向 `$HOME/.mkshrc` 落盘短提示符 rc，native `build_env` 把 `ENV` 固定拼成 `$HOME/.mkshrc`——rc 位于用户数据树内。裁决：mkshrc 可以存在，但路径必须正确（不在 `$HOME`、不改用户数据）。

## What Changes

- Kotlin：`.mkshrc` 写入位置改为应用私有 `context.noBackupFilesDir/.mkshrc`（经 Context API 获取，非硬编码路径；在 `files/` 用户数据树之外）；`TerminalConfig` 与 JNI `initSession` 新增 `mkshrcPath` 参数传递该路径。
- native：`ShellEnv` 新增 `mkshrc_path: Option<String>`；`build_env` 仅在 `mkshrc_path` 为 `Some` 时注入 `ENV`，不再从 `home` 拼接（沿用 `prefix` 空→`None` 的既有 FFI 模式）。
- 旧 `$HOME/.mkshrc` 不主动删除：删除用户数据同样违反规范，且 `ENV` 指向新路径后旧文件自然失效（设备取证确认 Android mksh 交互 shell 仅经 `ENV` 加载 rc）。
- 同步更新直调 `initSession` 的 4 处测试调用点。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `shell-env`: `ENV` 值从 `$HOME/.mkshrc` 改为调用方显式传入的应用私有 `mkshrc` 路径；路径缺失时不再注入 `ENV`；路径约束（不在 `$HOME`、非用户数据树）来自 `DESIGN.md` Shell 节。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：`ensureMkshPromptRc` 写入路径、`buildConfig`/`buildFailsafeConfig` 构造 `TerminalConfig`。
- `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt`：`TerminalConfig` 新增字段。
- `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt`：`initSession` 新增参数。
- `native/src/android/ffi.rs`：`initSession` JNI 签名与 `ShellEnv` 构造。
- `native/src/terminal/shell_env.rs`：`ShellEnv` 新增字段与测试。
- `native/src/terminal/pty.rs`：`build_env` 注入逻辑与单测。
- 测试调用点：`NativeBridgeSmokeTest`、`ShellPtyInstrumentedTest`、`VtCorrectnessInstrumentedTest`、`CjkPresentSemanticsTest`。
- `docs/specification/DESIGN.md` 无需修改（约束已存在）。
