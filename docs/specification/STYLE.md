# 样式指南

## Shell 脚本

所有 Shell 脚本均使用 Nushell（`.nu`），禁止使用 bash 或 sh。

- Shebang：`#!/usr/bin/env -S nix develop --command nu`
- 命名：`snake_case`

### 确定性脚本规则

环境是确定性的：SDK 路径、工具可用性与系统状态在运行时已固定，脚本须与此保持一致。

- 禁止 `do --ignore-errors`：让非零退出码自然传播。仅当失败本身即为预期状态时（如未连接设备时执行 `adb`），才可使用 `try/catch`，禁止用于掩盖错误。
- 禁止 `err> /dev/null` 或 `e>| null`：标准错误输出为诊断信息。若某条命令的错误信息被视为噪声，则说明该命令本身有误。
- 禁止使用 `which X` 查找工具：全部工具由 `nix develop` 保证可用。CI 中路径固定的 SDK 根目录请直接硬编码。

### 禁止模式

- 禁止 `else { print }` 形式的回退分支 — 错误应自然传播。
- 禁止使用 `| ignore` 压制预期失败 — 须显式处理错误。
- 禁止 `print "=== step_name ==="` 形式的步骤标签 — 输出仅保留结果。
- 禁止无意义输出：如 `print "Done!"`、`print "Boot verified"` 等。
- 禁止冗余的 `which X | length` 检查 — Shebang 已进入 `nix develop` 环境，无需重复探测。
- 禁止仅使用一次的中间变量别名（如 `let sdkmanager = ...`、`let adb = ...`）— 请直接使用路径。
- 禁止环境变量遮蔽：不要设置 `$env.AVD_DIR = $avd_home` — 请直接使用 `$env.ANDROID_AVD_HOME`。
- 对于必须存在的目录，禁止使用 `if ($dir | path exists)` 预检 — 让命令以清晰的错误直接失败。
- 对于应当存在的目录：须显式检查，若缺失则以非零状态退出。
- 禁止无助于提升清晰度的中间变量，如 `let start = ... let elapsed = ...`。
- 禁止使用 `$env.ANDROID_HOME/platform-tools/adb` 或硬编码路径调用二进制 — `adb`、`emulator`、`sdkmanager`、`avdmanager` 均来自 `nix` 开发环境（`android-tools` 包）。
- 禁止在 Nushell 脚本内部使用 `nu scripts/xxx.nu` 调用 — 请使用 `./scripts/xxx.nu`（依赖 Shebang）。
- 禁止在检查脚本中执行 `rustup target add` 或声明交叉编译目标 — 仅运行工作区测试。

### 风格规则

- 变量与函数一律使用完整描述性名称：禁止单字母变量（如 `s`、`p`、`w`、`h`、`t`、`e`），禁止缩写（如 `config` 而非 `cfg`、`background` 而非 `bg`、`application` 而非 `app`）。
- Nushell：使用 `is-not-empty` / `is-empty`，而非 `| length > 0` / `| length == 0`。

## Nix

全部环境管理均通过 Nix 完成，禁止使用系统 Shell 构建。

- 始终使用 `nix develop`。
- ShellHook 为主要机制；检查与格式化器在 `flake.nix` 中定义。

## GitHub Actions

- Action 版本：使用默认分支（`@main` 或 `@master`），而非标签。
- 例外：`reactivecircus/android-emulator-runner@v2` — `@main` 未包含已编译的 `node_modules`。
- 禁止设置步骤 `name`。
- 将相邻的 `run` 步骤合并为多行块。
- `||` 仅用于显式错误处理，禁止用于吞没错误。
- 任务命名：`kebab-case`。

## 通用

- 尽可能内联中间变量。
- 一题一档，避免重复。
