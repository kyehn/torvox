# 终端协议支持现状、缺口与计划

## 来源与口径

- 本项目（torvox）：`native/src/` 全部 Rust 源码、`native/Cargo.toml`、`docs/specification/`、`openspec/specs/`。
- VT 引擎：`libghostty-vt`（`Uzaaft/libghostty-rs`，`native/Cargo.toml` 钉的 rev），其 Rust 绑定源码位于本地 cargo 缓存 `~/.cargo/git/checkouts/libghostty-rs-*/<rev>/crates/libghostty-vt/src`。
- termux-app：`github-releases/v0.119.0-beta.3`（`DESIGN.md` 指定的参照基准，实测提交 2025-05-23）与 master（实测提交 2026-09-26）两个浅克隆。
- ghostty-android-terminal：master 浅克隆（实测提交 2026-10-30），同为 libghostty-vt 路线，作对照。

本文档是研究记录与计划登记，不是行为规范；行为规范以 `docs/specification/` 与 `openspec/specs/` 为准。

## 架构前提

本项目的 VT 解析与终端状态 100% 由上游 `libghostty-vt`（Ghostty 终端仿真库）承担，`native/src/terminal/ghostty_terminal/` 只是接线与查询封装（`internal/run.rs` 的 `configure_terminal` 注册回调，`internal/dispatch.rs` 处理查询）。因此「本项目支持哪些协议」= Ghostty VT 核支持的协议，减去 `openspec/specs/upstream-alignment/spec.md`「上游 API 取用判定登记」中明确判为不采用的回调。

环境声明（`docs/specification/DESIGN.md` Shell 节）：`TERM=xterm-256color`、`COLORTERM=truecolor`。

## 本项目支持的协议

### 基础 VT / ANSI / DEC

| 协议 | 作用 |
| --- | --- |
| ECMA-48 / ISO-6429（CSI/ESC） | 光标移动（CUU/CUD/CUF/CUB/CUP）、擦除（ED/EL）、滚动、插入删除行/字符（IL/DL/ICH/DCH）、制表符等全部基础序列 |
| ANSI.SYS / xterm 兼容 SGR | 16 色 / 256 色 / 24 位真彩色，下划线样式（含 kitty 下划线子参数），斜体、删除线、闪烁、反显 |
| DEC 私有模式（DECSET/DECRST） | DECCKM 应用光标键、DECCOLM 132 列、DECSCNM 反显、DECOM 原点、DECAWM 自动换行、DECSTBM 滚动区域、DECLRMM 左右边距、DECSACE、DECSC/DECRC 光标保存恢复、47/1047/1049 备用屏、1048、66 应用小键盘、69、1035/1036/1039 |
| 字符集换组（G0/G1） | DEC 特殊图形字符（线框表）、ISO 646 变体 |
| 回滚 / reflow / resize | 缩窄重排保留软换行文本；备用屏不重排（`tests/mouse_osc.rs` 的 `shrink_reflow_keeps_wrapped_text`、`shrink_alt_screen_clips_without_reflow` 钉住） |
| 设备属性应答 | DA1（默认应答 `?62;22c`，VT220 一致性等级，`test_helpers.rs` 的 `test_output_capture_da` 钉住）、DA2、DA3、DSR、DECRPM、XTVERSION、XTWINOPS（CSI 14/16/18t，走 `on_size` 回调）、DECRQSS、DCS `+q` termcap/terminfo 查询 |

### OSC 操作系统命令

解析归属见 `openspec/specs/upstream-alignment/spec.md` 的「OSC 覆盖逐项核对」表。

| OSC 段 | 作用 | 本项目状态 |
| --- | --- | --- |
| 0 / 2 | 设置窗口标题/图标名 | 上游解析，`Query::Title` 与快照 `title` 观察（会话列表标题） |
| 1 | 图标名查询 | 上游解析，本仓无消费者 |
| 4 | 调色板覆盖 | 上游处理，`osc4_palette_override_reaches_dumped_grid` 钉住 |
| 7 | 工作目录（`file://` URI） | 上游解析，`Query::Pwd` 消费（会话 pwd 展示）；`tests/osc_state.rs` 的 `osc7_pwd_is_tracked_verbatim` 钉住 |
| 8 | 超链接 | 上游解析，`commands.rs` 的 `hyperlink_at` 查询网格单元 URI → 长按选区「打开链接」（`text-selection` 规约） |
| 9 | ConEmu CurrentDir | 上游 pwd 解析的来源之一 |
| 10 / 11 / 12 | 默认前背景色/光标色设置与查询 | 上游不处理，默认值归内嵌方；本仓经原生主题 setter 置默认色，查询以主题色应答（`tests/osc_state.rs` 的 `osc11_reports_theme_background`） |
| 22 | 鼠标指针形状 | 上游解析（`mouse::Shape`） |
| 52 | 剪贴板读写 | 写走上游 `on_clipboard_write` 回调；读取请求由本仓 `output_processor.rs` 的 `ReadScan` 状态机收割并拼帧应答（FR-036） |
| 104 / 110 / 111 / 112 | 颜色重置 | 上游处理，跨分块重组有测试 |
| 133 | shell 集成标记（prompt 起止） | 上游解析，本仓不消费（DESIGN 未声明 prompt 标记语义） |
| 7501 | 程序状态上报 | 上游解析（`ProgramStatus`） |
| 9 / 1337 / 777 | iTerm2 通知 / 媒体 / 进度 | 不接：零消费者（upstream-alignment「OSC 覆盖逐项核对」表） |

### Kitty 系协议

- **Kitty 图形协议（APC `ESC_G`）**：完整支持。direct / file / shared memory / temp file 四种介质全部启用（`internal/run.rs` 的 `configure_terminal` 中 `set_kitty_image_from_file_allowed` 等四处配置）；PNG 载荷解码用上游 `kitty::graphics::RustPngDecoder`，进程加载时经 `.init_array` 单次安装（`ghostty_terminal/mod.rs` 的 `install_png_decoder`）；能力探测应答回显被查询的图像 id（`openspec/specs/kitty-graphics/spec.md` 钉住，yazi 等客户端据此判定支持）；z-index 分层、动画（generation 戳）、存储上限（`set_kitty_image_storage_limit`）；**Unicode 虚拟占位符**（U+10EEEE 以 fg 颜色编码图像 id、underline 颜色编码放置 id，组合记号编码图像行列）由上游解码为放置；本仓只在文本层跳过该码位（`ghostty_terminal/types.rs` 的 `KITTY_PLACEHOLDER`，判定在 `render/cell_builder.rs` 的两处构建路径），否则会画出豆腐块盖住图像。
- **Kitty 键盘协议**：支持。上游 `key::Encoder` 实现全部五档标志（DISAMBIGUATE / REPORT_EVENTS / REPORT_ALTERNATES / REPORT_ALL / REPORT_ASSOCIATED）；编码选项经 `set_options_from_terminal` 从活体终端读 `kitty_keyboard_flags`（`internal/dispatch.rs` 的 `Query::EncodeKeyEvent` 分支），终端侧 DECSET 推送的标志自动生效。
- **Kitty 颜色协议**：上游 OSC 解析器支持（`CommandType::KittyColorProtocol`）。
- **Kitty 剪贴板协议 / DnD / 文本尺寸 / 桌面通知**：上游 OSC 解析器可识别（`osc.rs` 的 `CommandType` 枚举），本仓无消费者。

### 输入编码协议（VT 线程统一编码）

- **鼠标协议**：跟踪模式 X10（9）/ Normal（1000）/ Button-event（1002）/ Any-event（1003）四种；输出格式 X10 / UTF-8（1005）/ SGR（1006）/ urxvt（1015）/ SGR-Pixels（1016）五种（上游 `mouse.rs` 头部明示）。触摸拖动按终端当前模式经 `Query::EncodeMouseEvent` 转发；`alt_screen_active` 与 `mouse_tracking_active` 两个无锁镜像让 UI 线程免查询（`ghostty_terminal/mod.rs` 字段文档）。
- **焦点上报（DECSET 1004）**：支持。`session/output.rs` 的 `focus_event_bytes` 按活体模式判定，仅在子进程启用 1004 时直写 `CSI I` / `CSI O` 到子 PTY（不送入 VT 引擎）。
- **Bracketed Paste（2004）**：支持。粘贴编码走上游 `paste::encode`，bracketed 标志取自模式 2004 活体状态（`internal/dispatch.rs` 的 `Query::EncodePaste` 分支），启用时包裹 `ESC[200~…ESC[201~` 并保留换行。
- **传统按键编码**：xterm `CSI 27;mod;code~`、PC 风格功能键表、SS3 应用光标键、modifyOtherKeys（`CSI > 4;2 m`）全部由上游编码器按终端模式派生；本仓不维护按键序列表（`openspec/specs/key-input-encoding/spec.md`）。

### 其他现代协议

- **Synchronized Output（DECSET 2026）**：支持。`internal/mod.rs` 的 `synchronized_output_hold` 在 BSU/ESU 之间不发布中间帧；引擎无时钟，程序崩溃或忘记 ESU 时由本仓 1 秒超时强制解锁复位（上游推荐口径）。
- **Grapheme Cluster（2027）**：上游 Mode 常量具备；渲染侧 `internal/rendering.rs` 经 `cell.graphemes()` 读组合记号（`types.rs` 的 `MAX_GRAPHEME_CLUSTERS = 8`），支持单码位整形与复杂文种叠加；不做跨单元连字（DESIGN 明示）。
- **Color Scheme Report（2031）/ Visibility Report（2033）/ In-Band Resize（2048）/ Paste Events（5522）**：上游 Mode 常量具备（`terminal.rs` 的 `Mode` 枚举）。
- **全文搜索**：上游 `search::Search`（回滚 + 视口、软换行跨行、命中总数与导航滚动），本仓只做上游选区到物理行网格区间的转换。
- **文本选择**：上游 `selection`（词派生、宽字符吸附、交叉翻转归属）。

### 明确不接线（规范已登记，非缺口）

`on_title_changed`/`on_pwd_changed`（改用 Query 拉取）、`on_render_hold`（自建 2026 门控）、`on_desktop_notification`/`on_progress_report`、`on_clipboard_read`（OSC 52 读走异步自研链，避免冻结 VT 线程）、`on_device_attributes`/`on_xtversion`（缺省应答已够）、`set_terminfo_name`、`gesture`/`select_output`/`format_selection_*`/`snapshot decoder` 等。依据：`openspec/specs/upstream-alignment/spec.md`「上游 API 取用判定登记」。`on_bell` 原判不接线（2026-11-03 已改判接线：无锁标志 + 每帧收割 `Event::Bell`，Kotlin 按 `bell-character` 三态执行，见 `docs/persistence/osc-notifications.md`）。

## 每个协议的作用（速查）

- **CSI/ESC/SGR**：终端控制的母语——光标、擦除、滚动、颜色、属性。
- **DECSET/DECRST**：开关终端模式（备用屏、应用光标键、鼠标、粘贴括号、同步输出等）。
- **OSC 0/1/2**：标题/图标名（会话列表显示）。
- **OSC 4/10/11/12/104/110-112**：调色板与默认色（主题应用、深浅色判断）。
- **OSC 7 / OSC 9 / OSC 1337 CurrentDir**：工作目录上报（会话 pwd）。
- **OSC 8**：超链接（文本可点击跳转）。
- **OSC 52**：剪贴板读写（终端↔系统剪贴板）。
- **OSC 133**：shell 集成（命令边界、退出状态着色）。
- **Kitty 图形协议**：终端内显示真实位图（yazi 图片预览、icat、mpv `--vo=kitty`）。
- **Kitty 键盘协议**：无歧义上报全部按键（含修饰键释放、全部功能键、关联文本）。
- **Kitty 颜色协议**：真彩色与渐变的高级颜色交互。
- **Synchronized Output（2026）**：程序批量重绘时终端暂停渲染，避免闪烁撕裂。
- **Bracketed Paste（2004）**：区分粘贴与手工输入，防止粘贴代码被误执行。
- **焦点上报（1004）**：通知应用窗口聚焦/失焦。
- **鼠标协议（1000/1002/1003 + 1005/1006/1015/1016）**：让 vim/tmux/htop 等接收点击与拖拽。
- **DA/DSR/DECRPM/XTVERSION/XTWINOPS/DECRQSS**：程序探测终端能力与尺寸的问答对。
- **Grapheme Cluster（2027）**：复杂文种（组合字符）整簇移动光标。

## 缺口分析：还有哪些应该支持

按 DESIGN「不做不必要功能」「终端状态单一来源」与 PROHIBITED 分三类。

### 上游 ghostty VT 核本身没有的协议（要支持必须自研或换核，违 DESIGN）

- **Sixel 图像协议**（DCS `q`）：termux-app master 已支持，ghostty 不支持。本仓图像通道走 Kitty 图形协议。
- **iTerm2 图像协议（OSC 1337 `File=`）**：termux-app master 已支持（`ITermImage.java`），ghostty 不支持。
- **Regis、Tektronix 4010/4014**：`upstream-alignment`「能力裁定补齐行台账」已判「未声明，不立项（Ghostty 无该协议）」。

### 上游解析了但本仓不消费的（接线即可，但须先补 DESIGN 声明）

- **OSC 133 shell 集成标记**（prompt 分段、命令退出码着色）：上游解析，本仓不消费。
- **OSC 9 / 1337 / 777 桌面通知与进度上报**：上游 `on_desktop_notification`/`on_progress_report` 回调存在，本仓判零消费者不接线。
- **OSC 1337 CurrentDir**：上游 pwd 解析的来源之一（`terminal.rs` 的 `on_pwd_changed` 文档），目前本仓只展示 OSC 7 来源的 pwd。

### 绑定层缺口

- **DECRQCRA 矩形校验和**（`AssertScreenCharsInRectEqual` 的底层）：esctest2 语料实测阻塞点——上游终端已具备分发与校验和计算，但 `libghostty-vt-sys` 绑定层未暴露（`upstream-alignment` 语料判定表已登记）。

### 对照结论

从协议覆盖看，本项目借助 Ghostty 核已覆盖现代终端全部主流协议（kitty 图形/键盘、同步输出、OSC 8 超链接、OSC 7、全格式鼠标、bracketed paste、焦点上报），比 termux-app 更完整。真正的缺口是 sixel/iTerm2 图像——但补它们意味着违反「不重复造轮子 / 单一来源」原则，属于产品决策而非工程缺口。

## termux-app 支持的协议（实测）

termux-app 是自研 VT 引擎（Java，`terminal-emulator` 模块），与本仓的 ghostty 核路线完全不同。

### 支持

- **基础 VT/ANSI/DEC**：CSI/ESC/SGR 全量、DECSET/DECRST、DECSTBM 滚动区域、DECLRMM、DECSACE、IRM 插入模式、DECAWM、字符集换组、DECSC/DECRC。
- **设备应答**：DA1 回 `?64;1;2;4;6;9;15;18;21;22c`（自报 vt420 class 64）；DA2 回 `>41;320;0c`；DECRQSS（`DCS $ q "p`）回 DECSCL `64;1"p`；DCS `+q` termcap/terminfo 查询应答（`Co=256`、`TN=xterm`，走 `KeyHandler.getCodeFromTermcap`）。
- **OSC**：0/1/2 标题、4 调色板、10/11/12 默认色设置与查询应答（`rgb:RRRR/GGGG/BBBB`）、52 剪贴板写（读不支持）、104/110/111/112 色重置、119（空实现）、1337 iTerm2 图像（仅 master：`ITermImage.java`，支持 `File=`/`MultipartFile=`/`FilePart=`/`FileEnd=` 分片与 `ReportCellSize` 应答）。
- **鼠标**：1000/1002/1003 跟踪模式（内部位）；输出格式仅 SGR（1006）与 X10 两种，1005/1015/1016 记入 case 但不改变输出格式（`sendMouseEvent` 只有 SGR 与 X10 两分支）。
- **Bracketed Paste（2004）**：支持（`paste()` 包裹 `ESC[200~`）。
- **光标样式**（DECSCUSR）、响铃处理、132 列模式的边距/清屏副作用。
- **Sixel**：仅 master（2026-09）新增——`TerminalSixel.java` + `TerminalBitmap.java` + `TerminalRenderer` 渲染；本仓基准版 v0.119.0-beta.3 没有（grep 零命中）。

### 不支持（与本仓的关键差距）

- **Kitty 图形协议**：不支持。`ApcTest.java` 注释自证——APC 被静默吞掉，yazi 的 `ESC_G…a=q` 探测得不到应答。
- **Kitty 键盘协议**：不支持（`KeyHandler` 只有传统 `CSI 27;mod;code~` 编码，`transformForModifiers` 只处理 shift/alt/ctrl 组合）。
- **modifyOtherKeys**：明确忽略（master 代码 `(ignored) CSI > MODIFY RESOURCE`）。
- **Synchronized Output（2026）**：不支持。
- **OSC 8 超链接**：不支持（`doOscSetTextParameters` 无 case 8；它的链接识别是扫描纯文本裸 URL，与 DESIGN「不扫描裸 URL」相反）。
- **OSC 7 工作目录**：不支持（无 case 7）。
- **焦点上报（1004）**：内部位标注 NOT implemented。
- **OSC 9/133 通知与 shell 集成**、**BiDi/RTL**、**Grapheme Cluster 模式**：不支持。

### ghostty-android-terminal 对照（同为 libghostty 路线）

- 注册 `GHOSTTY_TERMINAL_OPT_WRITE_PTY`/`BELL`/`TITLE_CHANGED`/`SIZE` 四个回调 + kitty 图存储上限（`terminal_jni.c`）。
- kitty 图形协议：PNG 解码 shim（`png_decode.c`）+ Unicode 占位符自研解码（`kitty_unicode.c`，与 ghostty `graphics_unicode.zig` 同源）。
- DEC mode 2027（Grapheme Cluster）有用户开关并粘滞重断言（`want_grapheme_cluster`，RIS 重置后重新置位）。
- 鼠标编码器经 `ghostty_mouse_encoder_setopt_from_terminal` 从终端状态取模式与格式。
- 搜索自建（其钉定的上游版本早于 search 模块引入）；本仓已改走上游 `search::Search`。
- 与本仓一样不支持 sixel/iTerm2 图像（ghostty 核无该协议）。

## 将来计划

以下按「依赖层」排序，每项注明前置条件。按 DESIGN 与 STYLE，落地前必须先在 `DESIGN.md` 声明对应功能，未声明不立项。

### 近期（接线级，前置是 DESIGN 声明 + 消费者）

- **OSC 133 shell 集成标记消费**：上游已解析，接线即可。前置：DESIGN 声明 prompt 分段/退出状态着色语义；消费者为「命令状态高亮」类 UI。
- **OSC 1337 CurrentDir 纳入 pwd 展示**：上游 pwd 解析已覆盖 OSC 9/1337，只需在会话 pwd 展示侧确认多来源口径。前置：DESIGN 声明 pwd 多来源优先级。
- **桌面通知/进度上报（OSC 9/1337/777）**：上游 `on_desktop_notification`/`on_progress_report` 回调已存在。前置：DESIGN 声明通知语义（Android 通知栏形态），并与已接线的 BEL 响铃（`bell-character` 三态）区分职责。外部实现对比、协议细节与接线路径见 `docs/persistence/osc-notifications.md`。
- **跟随上游 master rev**：`upstream-alignment` 要求依赖跟踪 master。计划内例行动作：升级 `native/Cargo.toml` 钉定的 rev，全量 `cargo test` + JNI 驱动的 JVM 测试保持全绿，并把「落后多少」登记进 `upstream-alignment`（提交日期与提交数差口径，不用 hash）。

### 中期（绑定层/上游依赖）

- **DECRQCRA 校验和**：阻塞在 `libghostty-vt-sys` 绑定层未暴露。上游终端已具备分发与计算。计划：向上游提绑定补丁或等上游发布；落地后 vttest 系语料的 `AssertScreenCharsInRectEqual` 类断言才具备引入前提。
- **区分大小写搜索**：上游 `Search` 仅 ASCII 折叠、无开关（`upstream-alignment` 对标表已登记）。计划：推动上游 `SearchOption` 扩展；本仓不改自研（违单一来源）。

### 长期（产品决策，违「单一来源」原则，需维护者裁定）

- **Sixel 图像协议**：termux-app master 已支持，ghostty 核没有。补齐意味着自研 VT 组件或换核，撞 DESIGN「不重复实现 Ghostty 已有功能」与「终端状态单一来源」。计划：仅当产品明确要求对齐 termux 图像生态（sixel 系工具链）时立项，形态为独立可选模块而非替换 VT 核。
- **iTerm2 图像协议（OSC 1337 `File=`）**：同上，termux-app master 支持而 ghostty 核不支持。计划：与 sixel 同批裁定。
- **BiDi/RTL**：上游 ghostty 核不输出 bidi 整形（gat 亦明确不支持）。计划：依赖上游或换核，未声明不立项（`upstream-alignment` 能力裁定表已判）。

### 不纳入计划（明禁或已判不立项）

- 会话持久化/恢复、粘贴确认对话框、自定义终端主题、修饰键栏布局编辑器、Model Context Protocol、termux-api 等：PROHIBITED 明禁。
- Tektronix 4010/4014、Regis、5250：未声明，不立项（ghostty 核无该协议）。
- WebSocket/网络终端形态、分屏平铺、ZMODEM 文件传输：未声明，不立项。

## 复核口径

- termux-app 结论基于 v0.119.0-beta.3 与 master 两版实测克隆；版本前进后结论可能变化，复核时重跑 grep（sixel/kitty/1337/OSC case 分支）而非沿用本文数字。
- 本项目结论基于 `native/Cargo.toml` 钉定的 libghostty-vt rev 的绑定源码与 `internal/run.rs` 的实际接线；rev 升级后须重核「上游 API 取用判定登记」。
