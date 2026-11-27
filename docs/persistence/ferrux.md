# ferrux 上游审查

- 上游：https://github.com/FerraStack/ferrux @ `69d3a04`（2026-11-21）；核心 Apache-2.0。
- 定位：Rust 工作区 + Kotlin 应用，提供终端核心、PTY、会话、JNI、AIDL 跨应用 API 与 bootstrap 安装器。

## 架构与数据流

- `crates/ferrux-term` 是 `alacritty_terminal` grid/parser 的薄封装（明确不用其 `tty` 模块以避免进程级 `SIGCHLD` handler）：`Terminal::feed` 解析、`snapshot()` 返回渲染友好副本、`take_responses()` 返回设备应答、`drain_events()` 返回 title/bell/OSC 52 事件。
- `snapshot.rs` 把颜色解析为 RGB、预先应用 reverse video、标记 `WIDE`/`WIDE_SPACER`/`COMBINING`/`WRAPPED`/`SELECTED`/`HYPERLINK`/`URL`；flag 位与 Kotlin `SnapshotLayout` 是显式 FFI 契约。
- 默认 `WORD_SEPARATORS` 去掉 `:`，使长按可选中完整 URL 或 `src/main.rs:10:5`；`MAX_CLIPBOARD_BYTES` 为 1 MiB。
- `ferrux-pty` 管 PTY，`ferrux-session` 管会话生命周期，`ferrux-jni-core`/`ferrux-android` 管 JNI，`ferrux-env` 管环境包与 bootstrap；Android 侧 `SessionManager.kt`、`FerruxService.kt` 与 AIDL `ApiRegistry.kt` 提供跨应用 API 与所有权模型。
- `android/terminal-view` 的 `TerminalView`、`TerminalInputConnection`、`TerminalAccessibility`、`ScreenText` 把快照绘制、IME 与无障碍读出独立成模块。

## 已核实边界

- `Snapshot::cell`/`row`/`combining_at` 用索引直取，不做边界检查；当前 JNI 以整块快照传输，调用方仍需保证索引合法。
- bootstrap 下载链路是供应链信任点；AIDL 扩大可调用面，`Ownership` 语义一旦松动就是跨应用越权。
- 模块数量多：只取终端核心会失去 API 层，只取 API 层又带入会话所有权模型，不能按“复制一个目录”的方式复用。

## 对 torvox 的取舍

- 参考其显式错误模型：跨边界调用返回类型化错误而非静默失败；本地 `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` 已用直接 `external fun`，不引入 AIDL。
- 参考 `TerminalAccessibility` 的组织方式；本地规范明确禁止无障碍朗读，只保留模块边界知识，不实现功能。
- `ferrux-term` 的 `Snapshot` 字段与本地 `native/src/render/cell_builder.rs` 的 `CellData` 可对照（颜色预解析、宽字符标记、软换行标记）；解析引擎不同，不移植代码。
- 跳过：未运行 Gradle/Rust 测试与 bootstrap 安装（需 Android SDK 与网络）；风险是跨应用 API 行为只按源码与测试文件核对。
