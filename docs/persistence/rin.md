# Rin 上游审查

- 上游：https://github.com/pavelc4/Rin @ `80bcec4`（2026-10-07）。
- 定位：Kotlin + Compose + Rust/JNI 的 Android 终端，主打兼容 Termux 包生态的 `rpkg`。

## 架构与数据流

- Rust 侧 `crates/rin-core` 用 `termwiz` 的 VT 解析与屏幕模型；`crates/rin-jni` 暴露 session、输入、resize 等 JNI 接口，并用句柄管理会话生命周期。
- Android 侧 `RinLib.kt`、`MainActivity.kt`、`SessionManager.kt`、`TerminalSession.kt` 与 Compose 的 `TerminalSurface`、`TerminalScreen`、`ExtraKeysBar`、`SessionDialog` 负责 UI；`TerminalSessionService.kt` 维持前台服务。
- `rpkg/src` 负责 Termux `.deb` 仓库索引、依赖解析、下载、校验与安装：`manager/install.rs`、`remove.rs`、`upgrade.rs` 与 `platform/android.rs` 构成包管理链；`doc/package-manager.md` 是行为权威说明。
- `RootHelper.kt`/`RinPermStorage.kt` 承担提权与权限状态；会话恢复与 extra keys 是常规 Android 终端做法。

## 已核实边界

- 无终端状态机测试：源码中只有 `rpkg` 的两个单元测试，`termwiz` 兼容性依赖上游版本且缺本地回归套件。
- `rpkg` 改写 Termux `.deb` 路径前缀以适配自身安装位置；上游包脚本假设 Termux 目录布局时容易失效，这是兼容生态的主要维护负担。
- root helper 与包安装是特权边界，README 已有安全提示；复用必须保留显式授权与最小暴露。

## 对 torvox 的取舍

- 不采纳包管理、root helper 与 `termwiz`；本地 `native/src/terminal/ghostty_terminal/public_api.rs` 已确定 Ghostty 解析路线。
- 只对照分层：Rust 核心持有 PTY 与会话、JNI 只传输入/resize；本地 `native/src/terminal/pty.rs` 的 `Pty` trait 与 `NativeBridge` 已覆盖同一边界。
- 缺少 VT 回归测试是反面提醒：本地 `key_encoding_tests.rs`、`output_processor.rs` 内的表驱动测试不能削减。
- 跳过：未运行 Gradle 与 `rpkg` 安装（需 Android SDK、root 环境）；风险是提权路径未复现。
