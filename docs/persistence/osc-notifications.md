# OSC 通知与响铃实现调研

## 来源与口径

- 主源码：termux-app master、ghostty-org/ghostty main、Uzaaft/libghostty-rs（锁定 rev 见 `native/Cargo.toml`）、connectbot/connectbot 与 connectbot/termlib main。
- 官方文档：iTerm2 escape codes、kitty desktop-notifications、WezTerm notification_handling、Ghostty docs/vt/osc/9、Windows Terminal progress-bar tutorial。
- 自动化矩阵：terminfo.dev extensions/notifications-osc-9（探测 14 个后端）。
- 本文件是研究记录与外部实现对比，不是行为规范；行为规范以 `docs/specification/` 与 `openspec/specs/` 为准。本仓尚未在 `DESIGN.md` 声明 OSC 通知功能，本文只记录外部实现与可能的接线路径，不构成实现承诺。

## termux-app

### OSC 分发

`TerminalEmulator.doOsc()` 的 `switch (value)` 全部分支：0/1/2 标题、4/10/11/12/104/110/111/112/119 调色板与颜色（含 `?` 查询）、52 剪贴板、1337 iTerm2 图像与 `ReportCellSize` 应答；default 走 `unknownParameter(value)`。**无 case 9、777、99、9;4**，全仓 grep "OSC 9" 无命中。

### 响铃（Bell）

`TerminalEmulator` 处理 BEL（case 7）：在 OSC 序列内 BEL 终止字符串；否则调 `mSession.onBell()` → `TerminalSessionClient.onBell()` → `TermuxTerminalSessionActivityClient.onBell()`：先检查 Activity 可见性，不可见直接返回；可见时按 `bell-character` 属性三态处理——`vibrate`（默认，`BellHandler` 振动 50ms，`MIN_PAUSE` 150ms 限频）、`beep`（`bell.ogg` 经 SoundPool 播放）、`ignore`。

### 通知设施与 OSC 无关

`termux-shared` 的 `NotificationUtils`/`TermuxNotificationUtils` 是前台服务与会话状态通知（NotificationChannel 构建器）；CLI 通知由独立 APK termux-api 的 `termux-notification` 命令提供。termux-app 本身不支持 OSC 触发的通知。

## 协议谱系

| 序列 | 来源 | 格式 | 内容 |
| --- | --- | --- | --- |
| `OSC 9;msg` | iTerm2（2010 年前） | `ESC]9;消息 BEL/ST` | 通知正文，无标题 |
| `OSC 9;4;st;pr` | ConEmu | st：0 停/1 百分比/2 错误/3 不确定/4 暂停；pr 为 0–100 | 标签页进度条（iTerm2 文档亦收录） |
| `OSC 777;notify;title;body` | rxvt-unicode | 结构化标题+正文 | foot/Ghostty/WezTerm 支持 |
| `OSC 99;meta;payload` | kitty | 分块协议：`i` 标识、`d` 完成、`p=title/body/icon/buttons`、`a=report/focus` 点击回报、`p=close/alive` 关闭/存活查询、`u` 紧急度、`w` 自动过期；块 ≤2048B（编码 ≤4096B） | 最完整，可更新/关闭通知 |
| ConEmu 其余 `OSC 9;x` | ConEmu | 9;1 睡眠、9;2 消息框、9;3 标签标题、9;9 cwd、9;10/11/12 窗口状态、9;11 注释、9;12 提示符标记 | 与 OSC 9 通知**编号冲突** |

冲突警示：OSC 9 的负载若以"数字;"开头会与 ConEmu 扩展混淆。Ghostty 官方文档要求标题不应以数字加分号开头；其解析器把无效 ConEmu 序列静默降级为 OSC 9 通知，但不建议依赖此行为。

## 各终端实现对照

### Ghostty

本仓 VT 解析器的上层参考，全链路（主源码核实）：

1. 解析：`src/terminal/osc/parsers/osc9.zig` 先按 ConEmu 扩展分流（9;1/9;2/9;3/9;4/9;10/11/12 等），其余按 iTerm2 通知：`show_desktop_notification{title="", body=msg}`；`rxvt_extension.zig` 解析 `777;notify;Title;Body` → 同一 Command；`kitty_desktop_notification.zig` 解析 OSC 99。
2. 命令：`src/terminal/osc.zig` 的 `show_desktop_notification: struct {title, body}`；另有 `conemu_progress`/`progress_report`（9;4）。
3. 回调：`src/terminal/stream_terminal.zig` 的 Handler 暴露 `desktop_notification` 与 `progress_report` 回调（OSC 9 或 OSC 777）。
4. 应用层：`src/apprt/action.zig` 的 `DesktopNotification` action → 各平台 surface；GTK 实现在 `src/apprt/gtk/desktop_notifications.zig`，用 GIO `gio.Notification`（走 org.freedesktop.Notifications），标题为空时显示 "Ghostty"，默认动作为 `app.present-surface`（点击通知聚焦来源 surface），按 hash 去重替换，跟踪上限 64 条，窗口聚焦时 3 秒自动过期（与 macOS 一致）。
5. 配置开关：`desktop-notifications`（默认 true）、`progress-style`（默认 true）。
6. 嵌入 ABI：`src/terminal/c/terminal.zig` 导出 `GhosttyTerminalDesktopNotification` 结构与 `Effects.desktop_notification` 回调——Ghostty 本身就是把通知委托给嵌入方的嵌入式库设计。
7. 长度限制：`src/apprt/surface.zig` 的 `DesktopNotification` 固定缓冲 title[63]、body[255]。

### iTerm2

规范起源。`OSC 9;消息` 发通知；`OSC 9;4;st;pr` 控制进度条（st 0 停/1 百分比/2 错误/3 不确定/4 暂停，pr 为 0–100）。OSC 9 通知自 2010 年前已存在。

### kitty

OSC 99 完整协议（见上表），含分块、点击回报（`a=report` 回发 `OSC 99;i=<id>`）、关闭/存活事件、图标（名称或 Base64 PNG）、按钮、声音、紧急度；0.24.0+ 也接受旧式 OSC 9 与 OSC 777（二手来源 termnotify README）。提供 `kitten notify` CLI。

### WezTerm

配置项 `notification_handling`（20240127 版起）：`AlwaysShow`/`NeverShow`/`SuppressFromFocusedPane`/`SuppressFromFocusedTab`/`SuppressFromFocusedWindow`——聚焦抑制策略是其特色；触发序列为 `OSC 777;notify;title;body` 与 `OSC 9;message`（官方文档）。

### ConnectBot（Android）

termlib（Compose + libvterm JNI）`TerminalEmulator` 回调集：`onBell`、`onClipboardCopy`（OSC 52）、`onProgressChange`（OSC 9;4 进度）——**无桌面通知回调**，即不支持 OSC 9/777 通知。应用层 `TerminalManager.onBell`：UI 可见且为当前会话 → `playBeep()`；否则 `sendActivityNotification(host)`（后台会话 bell 才发通知，带主机信息，经 `ConnectionNotifier` 的 NotificationCompat/通道管理）。设置项：bell 开关、音量、振动、通知开关。

### 其他

- **foot**：仅 `OSC 777;notify`（issue #224 讨论、PR #236 实现），不支持 OSC 9。
- **VTE**（GNOME Terminal/Tilix/Terminator）：`vte_terminal_set_enable_legacy_osc777` opt-in 启用 777（发行版补丁差异大）；忽略 OSC 9。
- **Windows Terminal**：官方文档支持 `OSC 9;4` 进度条（ConEmu 序列）；源码中 OSC 9 仅见 9;9（cwd），未见 9/777 通知分发。
- **tmux**：原生不支持（功能请求 issue #5136 仍开放），需 passthrough 绕行——SSH+tmux 场景通知会丢失。
- **Alacritty**：解析器接受 OSC 9 但未从公共 Rust API 暴露（terminfo.dev 自动化矩阵标记 partial）。
- **xterm.js / vt100.js**：不支持。

支持矩阵（terminfo.dev 自动化探测，OSC 9）：iTerm2、Ghostty、kitty、VS Code、Warp、Cursor、Terminal.app、vterm.js 支持；Alacritty、WezTerm headless 部分支持（解析了但 API 不暴露）；xterm.js、vt100.js 不支持。

## 对本仓的落地含义

**现状**：本仓已处理 OSC 0/2（标题）、7（cwd）、8（链接）、52（剪贴板，见 `native/src/terminal/ghostty_terminal/internal/run.rs` 的 `register_clipboard_write_callback`），但全仓无 `desktop_notification` 接线——OSC 9/777/99 目前被 libghostty-vt 静默忽略。BEL 响铃已按 termux 语义接线：`on_bell` 回调 → `Event::Bell` → `runtime/BellPlayer.kt` 三态执行（`vibrate`/`beep`/`ignore`），记录见 `docs/STATUS.md`。

**可行性（已核实锁定 rev 同样具备）**：`crates/libghostty-vt/src/terminal.rs` 提供：

- `DesktopNotification { title: &[u8], body: &[u8] }`（借用语义、仅回调期内有效、不保证 UTF-8）
- `Terminal::on_desktop_notification(...)`（OSC 9 或 OSC 777）
- `Terminal::on_progress_report(...)`（OSC 9;4）
- 绑定侧测试 `desktop_notifications_are_not_assumed_to_be_utf8` 证明回调通路可用
- 注：OSC 99 在绑定中解析为 `CommandType::KittyDesktopNotification`（osc.rs），但**未发现暴露给嵌入方的回调**——如需 kitty 协议须上游扩展。

**建议接线形状**（沿用 OSC 52 既有模式）：VT 线程注册 `on_desktop_notification` → 有界 flume 通道（`try_send`，满则丢弃+日志）→ JNI 事件 → Kotlin 侧 `NotificationManager`。需要配套的策略层：

1. 负载治理：标题/正文长度截断（参照 `truncate_clipboard_payload` 与 Ghostty 的 63/255 上限）、频率限制（参照 BellHandler 的 150ms 限频与窗口限频模式）。
2. 聚焦抑制：应用在前台/会话可见时不弹通知（WezTerm `SuppressFromFocused*`、ConnectBot 后台才通知、Ghostty 聚焦时 3 秒过期）。
3. 用户开关：参照 Ghostty `desktop-notifications` 配置与 termux `bell-character` 三态。
4. Android 特有：Android 13+ 需 `POST_NOTIFICATIONS` 运行时权限；独立 notification channel；点击通知可带回源会话（参照 Ghostty `app.present-surface` 与 ConnectBot 的 host 附加）。
5. 协议取舍：优先 `OSC 9`（事实标准，iTerm2/Ghostty/kitty/WezTerm/VS Code/Warp 都支持）+ `OSC 777;notify`（带标题，foot/Ghostty/WezTerm 收）；`OSC 9;4` 进度可顺带接（libghostty-vt 已有 `on_progress_report`，termlib 先例）；`OSC 99` 成本高且当前绑定未暴露，建议后续。
6. 安全：远程程序任意触发通知 → 长度上限、频率限制、聚焦抑制、开关默认值取舍。

## 响铃（Bell）与通知呈现对照

### Bell 处理

| 终端 | BEL 处理 | 可配置项 |
| --- | --- | --- |
| Ghostty | 默认 attention 请求（dock 弹跳/taskbar 高亮/GNOME 通知）+ 标题 🔔；可选 system（系统响铃函数）、audio（自定义声音文件）、border（边框高亮） | `bell-features`（system/audio/attention/title/border，默认 attention+title）、`bell-audio-path`、`bell-audio-volume` |
| iTerm2 | 可听响铃 + 可视铃（全屏闪烁）+ 标签页铃图标 + Notification Center 警报（系统通知） | Silence bell、Flash visual bell、Show bell icon in tabs、Notification Center alerts |
| kitty | 可听响铃（系统铃声，默认开，`enable_audio_bell`）+ 可视铃（`visual_bell_duration` 闪屏，默认 0 关闭）+ 窗口提醒（dock 弹跳/taskbar 闪烁）+ 标签铃符号（🔔）+ macOS dock 徽标 + `command_on_bell`（自定义命令）+ `bell_path`（自定义声音） | `enable_audio_bell`、`visual_bell_duration`/`visual_bell_color`、`window_alert_on_bell`、`bell_on_tab`、`command_on_bell`、`bell_path`、`linux_bell_theme` |
| WezTerm | 可听响铃（SystemBeep 默认，Wayland 无声音；Disabled 关闭）+ 可视铃（背景色闪烁，fade_in/out、颜色、光标 flare）+ `bell` 事件（Lua） | `audible_bell`、`visual_bell` |
| Windows Terminal | 可听响铃 / 窗口闪烁 / 任务栏闪烁 / 全部 / 无（`bellStyle`，默认 audible）+ 自定义铃声文件（`bellSound`） | `bellStyle`、`bellSound` |
| Alacritty | 可视铃（animation/duration/color，默认 duration 0 关闭）+ `command`（响铃时执行程序）；无内置可听铃 | `bell.animation`/`duration`/`color`/`command` |
| GNOME Terminal/VTE | 可听响铃（VTE `audible-bell` 属性，profile "Terminal bell" 开关）；可视铃/urgency 依赖 terminfo flash 与 WM | `audible-bell` |
| xterm | 可听系统响铃（默认）+ 可视铃（`visualBell` 资源）+ urgency hint（`bellIsUrgent`） | `visualBell`、`bellIsUrgent` X resources |
| urxvt | 可听响铃（默认）+ urgency hint（`urgentOnBell`） | `urgentOnBell`、`visualBell` X resources |
| VS Code 终端 | 可视铃（终端名旁铃图标）+ TerminalBell 无障碍信号（声音/播报，可配置） | TerminalBell 无障碍信号；`terminal.integrated.enableBell`（仅可视） |
| foot | 无 bell 配置项（BEL 默认无显式处理） | — |
| termux-app | vibrate（默认，50ms，限频 150ms）/ beep（`bell.ogg`）/ ignore，仅 Activity 可见时 | `bell-character` |
| ConnectBot | 前台 beep；后台 → 系统通知（NotificationCompat，含主机） | bell 开关/音量/振动/通知开关 |
| 本仓 | 忽略 BEL（`DESIGN.md` 明示），不注册 `on_bell` | — |

### 通知呈现形式

| 终端 | OSC 通知呈现 |
| --- | --- |
| Ghostty | 系统通知（GIO Notification → org.freedesktop.Notifications / macOS UNUserNotificationCenter）；点击聚焦来源 surface；另有 in-app toast（Ghostty 自身消息） |
| iTerm2 | macOS Notification Center 警报（系统）；bell 也可触发 Notification Center alerts |
| kitty | 系统通知（OS 通知 API）；OSC 99 支持点击回报/关闭/存活查询 |
| WezTerm | 系统 toast（OS 通知 API），`notification_handling` 配置聚焦抑制 |
| foot | 系统通知（系统通知机制） |
| VTE/GNOME | VTE 无内置 OSC 通知；OSC 777 仅 opt-in legacy API（发行版差异） |
| Windows Terminal | OSC 9;4 → 标签页进度条（终端内）；无 OSC 9/777 系统通知 |
| VS Code 终端 | OSC 9 → 系统通知（Electron Notification） |
| termux-app | 无 OSC 通知；通知仅 termux-api CLI 或前台服务 |
| ConnectBot | 无 OSC 通知；bell 后台 → 系统通知 |

## 相关现有条目

- `docs/persistence/terminal-protocols.md`：OSC 段已登记 `9 / 1337 / 777` 为"不接：零消费者"，且"将来计划"登记了桌面通知/进度上报条目；本文件提供外部实现对比与接线路径。
- `docs/persistence/termux-app.md`：`bell-character` 属性三态（vibrate/beep/ignore）与 `NotificationUtils` 说明。
- `docs/persistence/libghostty.md`：`DesktopNotification`/`on_desktop_notification`/`on_progress_report`/`on_bell` 列入未使用 API 表；本文件补充外部实现对比。

## 来源

主源码：termux-app master、ghostty-org/ghostty main、Uzaaft/libghostty-rs（锁定 rev 见 `native/Cargo.toml`）、connectbot/connectbot 与 connectbot/termlib main。官方文档：iTerm2 escape codes、kitty desktop-notifications、WezTerm notification_handling、Ghostty docs/vt/osc/9、Windows Terminal progress-bar tutorial。自动化矩阵：terminfo.dev extensions/notifications-osc-9。二手参考：termnotify README（kitty 对旧式 OSC 9/777 的支持）、vucica 博客（hterm OSC 9 growl 通知）、tmux issue #5136、foot issue #224/PR #236。
