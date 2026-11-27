# plyra-terminal 研究留档

## 定位

Plyra Terminal 是 GPL-3.0 的 Rust 工作区：桌面主目标是 macOS Apple Silicon，Android 目前是冒烟实现。工作区成员包括 `terminal-core`、`session-manager`、`terminal-render`、`system-api`、`mcp-client`、`privilege-policy`、`privilege-broker` 以及两个 app。它使用 vendored 的 `vt100`、`block`、`wgpu-hal`，终端解析在 macOS 上由 helper C 进程持有 libvterm 0.3.3，跨平台的便携后端才用 `portable-pty` 与 `vt100`。

## 关键实现与文件锚点

- `crates/terminal-core/src/lib.rs`：`MIN_COLS = 10`／`MAX_COLS = 240`／`MIN_ROWS = 4`／`MAX_ROWS = 100`／`MAX_EVENT_BYTES = 1_048_576`／`MAX_HISTORY_LINES = 2000`／`MAX_HISTORY_CELLS = 200_000`／`MAX_HISTORY_BYTES = 16 * 1024 * 1024`；`Screen::apply_helper_event`。
- `crates/terminal-core/src/history.rs`：`used_columns` 与 reflow 锚点。
- `crates/session-manager/src/lib.rs`：自有 PTY、输入队列、鼠标与焦点协议。
- `crates/terminal-render/src/lib.rs`：wgpu 管线与 WGSL 材质。
- `crates/{system-api,privilege-policy,privilege-broker}/`：能力绑定与一次性票据。
- `apps/android/`：API 26–34 的冒烟实现。
- `docs/ARCHITECTURE.md`：helper 持 libvterm 0.3.3 并经 JSON 事件投影。

## 终端核心

`terminal-core` 是 libvterm 屏幕协议的只显示投影：helper 发来的 JSON 事件经 `Screen::apply_helper_event` 校验后更新 `Screen`，事件无效时保持原子性，不会留下半更新状态。`Run`/`Line`/`Screen` 保存样式、宽度、软换行标记与 OSC 8 超链接；`used_columns` 区分真实写入的空格与右侧填充，`reflow_history` 处理列宽变化，`viewport` 生成视图副本。

边界有明确上限：列 10–240、行 4–100、单事件 1 MiB、历史 2000 行、历史 200000 cell、历史 16 MiB；超限按“保留最近行”修剪。核心不做执行、剪贴板、URL 打开或特权操作。`history.rs` 的测试覆盖 Unicode grapheme、超链接样式保持与 reflow 锚点一致性。

`session-manager` 管理自有 PTY、结构化 argv、输入队列、resize、鼠标与焦点协议；`terminal-render` 用 wgpu/Metal、glyphon/cosmic-text 与 WGSL 玻璃材质，把背景、前景、IME preedit 与 egui 控件合成一帧。`system-api` 与两个 privilege crate 把权限能力绑定到具体目标、发一次性确认票据，并在独立进程里做目标再校验与审计；插件跑在 Seatbelt 沙箱里。

## 事件驱动

正常事件循环在没有事件时休眠；终端输出、输入、布局变化与显式动画才请求帧。终端矩形只在拥有终端焦点时接收文本，对话框文本字段走 UI 输入路径；组合取消与迟到 commit 过滤防止文字跨 pane 焦点跳转。

## 已核实局限

- macOS 的屏幕来自 helper 进程的 JSON 协议，多一层序列化与校验；本地项目直接经 JNI 调 Rust，不需要这层，也不应 vendored 依赖。
- 安全模型（权限 broker、一次性票据、沙箱插件）远超出本仓规范，明确不考虑，不移植。
- Android 只是 smoke 实现，不能作为成熟 Android 终端参考。

## 与本仓对照

- `Screen::apply_helper_event` 的“先在边界校验、原子更新、超限保留最近”思路可用于本仓 JNI 批量查询的入参校验；但本仓的 cell 数据来自 Ghostty，不自建第二份屏幕协议。
- `used_columns` 区分 padding 与写入空格、`reflow_history` 保持软硬换行，可作为本仓回滚与复制行为的交叉检查。
- 事件驱动按需帧、焦点路由与组合取消过滤，能对照本仓 `FrameClock`/`RenderSupervisor` 与 IME 输入路径；权限与沙箱部分不实现。
