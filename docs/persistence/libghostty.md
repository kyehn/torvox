# libghostty-vt API 清单

## 0. 来源与口径

- 上游：<https://github.com/Uzaaft/libghostty-rs>，crate `libghostty-vt`（安全封装 `libghostty-vt-sys`，对 C 库 libghostty-vt 的 FFI 绑定）。
- 本项目经 `native/Cargo.toml` 的 `rev` 字段钉住 master（2026-11-25 对齐），特性 `log` + `png`，无本地补丁。
- 统计口径：对 `crates/libghostty-vt/src` 的 `pub` 声明做静态枚举（`pub(crate)`/`pub(super)`、`#[cfg(test)]` 模块、trait 实现与私有 `io` 模块不计；`pub use` 按导出名逐个计；inherent impl 的公开方法计入）。当前 pin 公开项 **835** 个。
- 本仓接线程度有两个可复现的口径（对 `native/src` 全部 `.rs` 计）：**显式 `use libghostty_vt::` 与全限定路径命中 41 项**（下限；代码经 `internal/mod.rs` 统一 `use` 后由 `super::` 再导出给兄弟模块，故系统性低估）；**上游公开声明名与本仓标识符同名 293 项**（上限，含 `rows`/`cols` 等同名成员，方法级项无独立路径，故系统性高估）。真实接线范围介于两者之间，逐项证据见 §2。
- 目标：只接入“本仓需要且能降低本地复杂度/提高正确性”的 API；不为覆盖率而接线，不引入会话持久化等被禁止能力（见 `docs/specification/PROHIBITED.md`）。

## 1. 模块总览（上游当前 pin）

| 模块 | 公开项 |
| --- | ---: |
| `effect` | 131 |
| `render` | 91 |
| `terminal` | 65 |
| `kitty::graphics` | 57 |
| `mode` | 53 |
| `color` | 53 |
| `selection::gesture` | 50 |
| `snapshot` | 41 |
| `key` | 36 |
| `screen` | 34 |
| `selection` | 32 |
| `mouse` | 30 |
| `(root)` | 30 |
| `search` | 24 |
| `fmt` | 19 |
| `grid_ref` | 14 |
| `build_info` | 12 |
| `osc` | 11 |
| `paste` | 9 |
| `sys` | 9 |
| `sgr` | 8 |
| `style` | 5 |
| `alloc` | 5 |
| `report` | 5 |
| `point` | 3 |
| `unicode` | 2 |
| `kitty` | 2 |
| `error` | 2 |
| `focus` | 2 |
| 合计 | 835 |

说明：`io` 与 `sys` 下的 `log`/`png`/`secure_random_source` 为私有模块，公开项经 `sys::{LOGGER, Level, Logger, PNG_DECODER, RustPngDecoder, ...}` 再导出，已并入 `sys` 计数；`screen::Cell` 现定义在 `screen.rs`，不再单列。旧版 `PointCoordinate`/`PointSpace` 对应 `point::Coordinate`/`point::Space`（本仓用别名适配）。

## 2. 已使用 API（按能力域）

调用点主要位于 `native/src/terminal/ghostty_terminal/internal/`（VT 主循环、命令、渲染、选区、Kitty）、`native/src/terminal/session/`、`native/src/terminal/test_helpers.rs`、`native/src/render/` 与 `native/src/android/ffi/`。`internal/mod.rs` 集中了大部分上游 `use`，兄弟模块经 `super::` 取用；其余直连点是各测试模块（`key`/`color::Rgb`）、`theme` 的 `color::Rgb` 与 `android/logging.rs` 的 `sys::Logger`/`sys::LOGGER` 接线。此处列出的都是源码中可验证的已接线项，不是上游清单。

### 2.1 终端与输出

- `Terminal::{new, vt_write, resize, reset, rows, cols, total_rows, scrollback_rows, cursor_x, cursor_y, title, pwd, active_screen, is_cursor_visible, is_mouse_tracking, scrollbar, mode, set_mode, set_default_fg_color, set_default_bg_color, set_default_cursor_color, set_default_color_palette, default_color_palette, color_palette, set_default_cursor_style, set_scrollback_max_lines, set_scrollback_max_bytes, scroll_viewport, grid_ref, point_from_grid_ref, kitty_graphics, set_selection, selection, on_pty_write, on_bell, on_clipboard_write, on_size}`。
- `terminal::{ScrollViewport, CursorStyle}`（滚动 Top/Bottom/Delta 与主题光标 Block/Bar/Underline）、`mode::{Mode, Kind}`（模式位查询/设置，含 `BRACKETED_PASTE`/`FOCUS_EVENT`/`SYNC_OUTPUT`/`COLOR_SCHEME_REPORT`；`Mode::new`/`Kind::Dec` 仅测试使用）、`report::SizeReportSize`（`on_size` 回调回答 XTWINOPS）。
- 剪贴板回调：`effect::ClipboardWrite` 及其 `location()`/`contents()`/`reply()` 方法、`effect::ClipboardLocation::{Standard, Primary, Selection}`、`effect::ClipboardWriteError::Busy`（通道满时回 `Err`，不静默）。`ClipboardContent`/`ClipboardContents` 两种类型不具名出现（contents 迭代产物直接按字段读 `mime`/`data`）。
- 默认色与光标：`set_default_fg_color`/`set_default_bg_color`（主题推送；上游不处理 OSC 10/11 默认色，调色板覆盖 OSC 4 由上游处理）、`set_default_cursor_color`/`set_default_cursor_style`、`set_default_color_palette`/`default_color_palette`/`color_palette`。

### 2.2 渲染快照与单元提取

- `render::{RenderState, Snapshot, RowIterator, CellIterator, ImageIterator, ImageProtocol}`：脏行走 `RowIteration::next_dirty`（约 10 处），单元颜色/样式/选中态经 `CellIteration::{fg_color, bg_color, style, is_selected}` 读出，`Snapshot::image_placements_generation` 用于帧同步戳。光标样式取自 `Snapshot::cursor_visual_style()` 返回的 `terminal::CursorStyle`（旧 `render::CursorVisualStyle` 已移除）。**`DirtyState`/`CellFg`/`CellBg`/`StyleSet`/`render::Cursor` 没有引用**——颜色与样式一律经迭代方法读出。
- `screen::{Screen, CellWide}`（活动屏判定、宽字符列宽；`Screen::Alternate` 判定于帧构建）、`style::{Style, StyleColor, Underline, Id}`（下划线形状直接按枚举判定，不再维护本地布尔包装；`style::Id` 用作样式缓存键）。
- `point::{Point, Coordinate, Space}`（本仓别名 `PointCoordinate`/`PointSpace`）用于 `grid_ref`、历史/视口寻址与坐标换算。
- 单元字形：`CellIter::codepoint()` 现返回 `char`，本仓以 `u32::from` 转成字形索引。

### 2.3 选区与搜索

- `selection::{Selection, Order, SelectWord}` 与 `Terminal::{select_word, select_all, set_selection}`：词选由 `Terminal::select_word(grid_ref)` 返回的 `SelectWord` builder 经 `finish()` 取边界（旧 `SelectWordOptions` 已移除），全选走 `select_all`；`Selection::new`+`to_ordered(Order::Forward)` 排序后用 `set_selection` 安装，矩形选区由本仓 `SELECTION_RECTANGLE`（`bool`）表达。
- `search::{Search, MatchBuffer, Scroll}` 与 `Search::snapshot` / `Snapshot::{selected_match, terminal}`：终端内增量搜索与高亮命中；命中集合以本仓 `SearchMatch` 镜像交 Kotlin 侧（无全量一次性取回，滚动状态走 `Scroll`）。
- 选区文本导出走 `fmt::Formatter`（见 2.6）。

### 2.4 输入、焦点、粘贴与上报

- `key::{Encoder, Event, Key, Action, Mods}`：按终端模式编码按键（`Encoder::set_options_from_terminal` 同步 DECCKM/keypad 等）。`Encoder::{set_kitty_flags, set_macos_option_as_alt, set_modify_other_keys_state_2}` 均未接线（未实现 kitty keyboard 协商；macOS Option→Alt 与 modifyOtherKeys 状态 2 无产品需求）。Android `KeyEvent` 不属上游，经本仓映射逐字段写 `key::Event`。
- `mouse::{Encoder, Event, Action, Button, Position, EncoderSize}`：鼠标编码跨事件复用，`Encoder::set_options_from_terminal` 同步跟踪模式与编码格式，`EncoderSize` 写入当前网格像素（尺寸写不进编码器时丢这一事件并记日志，不沿用过期尺寸）。`mouse::TrackingMode`/`Format` 不具名——二者取自终端活体状态，本仓不手选。
- `focus`：焦点开关由 `Mode::FOCUS_EVENT` 判定（会话输出路径直写子 PTY `CSI I`/`CSI O`，不经上游 `focus::Event::encode`）。
- `paste::encode`：括号化编码（含 `OutOfSpace` 扩容重试）。`paste::is_safe`/`paste::Options`/`Terminal::paste` 未接线的原因见 §3。
- `report::{ColorScheme, SizeReportSize}`：主题明暗变化主动上报 `CSI ? 997 ; 1|2 n`（`COLOR_SCHEME_REPORT` 模式开启时）；on_size 回答 XTWINOPS 尺寸查询。

### 2.5 Kitty 图形

- `Terminal::kitty_graphics` 现返回 `Result<Graphics>`（不再包 `Option`），取得 `kitty::graphics::{Graphics, Image}` 与 `PlacementIterator`，读取可见放置与图像元数据；`render::{ImageIterator, ImageProtocol}`（纹理传输协议判别与放置图迭代）与 `Snapshot::image_placements_generation`（帧构建同步戳）配套。
- PNG 解码器：库加载（`.init_array`）时经 `sys::PNG_DECODER.install_with(|| Box::leak(Box::new(sys::RustPngDecoder::new())))` 安装（旧的 `kitty::graphics::set_png_decoder` 已移除）。
- 像素格式：`ImageFormat::{Rgba, Rgb, Gray, GrayAlpha, Png}`。
- 上游不向内嵌方暴露 APC 命令解析类型（`Command`/`Response`/`CommandData`/`ControlData` 不是公共类型：kitty 指令随 VT 流由上游状态机处理）。本仓对 kitty 只做：清屏前撤下可见放置（`DELETE_VISIBLE_PLACEMENTS`/`d=I` 字节序列）、放置/图像采集、生成戳给帧构建。

### 2.6 其它

- 日志：`android/logging.rs` 实现 `sys::Logger`，在 `JNI_OnLoad` 时以 `sys::LOGGER.install(&GHOSTTY_LOGGER)` 安装（上游改为静态 `SysFunc` 槽位，旧的 `set_logger` 已移除）；消息经 Rust `log` facade 落到 Android logcat，`sys::Level` 用于级别映射。`sys::LogStderr`/`sys::TracingLogger` 未使用。
- `error::Error`：统一错误传播与日志。
- `fmt::Formatter`：选区复制时把选中范围格式化为纯文本导出（`set_selection`/`set_unwrap`/`set_trim` + `format_alloc`）。

## 3. 未使用但应关注的 API（结论与取舍）

| 模块/项 | 结论 | 理由 |
| --- | --- | --- |
| `report` 其余报告类型 | 暂不接入 | 当前仅需配色变化主动上报；其它报告由终端查询/应答路径按需补。 |
| `effect` 的设备属性与进度报告等类型 | 暂不接入 | 只需剪贴板回调与尺寸/配色上报，其余由上游状态机消费，直接解析会重复。 |
| `paste::is_safe`/`paste::Options`/`Terminal::paste` | 暂不接入 | `paste::encode` 已逐字节中化 NUL/ESC/DEL 等不安全控制字节为空格，bracketed 2004 的包裹与换行转换随终端活体模式生效；`is_safe` 是与终端状态无关的保守拒绝规则，接线收益不足。`Terminal::paste` 的拒绝流程要求确认对话框（PROHIBITED 明禁），故只取编码器。 |
| `focus::Event::encode` | 暂不替换 | 焦点序列固定、现有实现已正确且无额外状态；替换收益不足以引入接口改动。 |
| `grid_ref::TrackedGridRef` | 暂不接入 | 选区/搜索已由 `selection`+`search` 覆盖；渲染期 grid_ref 均为帧内瞬态引用，无跨帧跟踪需求。 |
| `terminal::Terminal::set_default_cursor_blink` | 不接入 | 光标闪烁由子进程 DECSCUSR 与上游默认决定，本仓主题层只下发样式，无独立「光标闪烁」设置项。 |
| `snapshot` | 不接入 | 会话/状态持久化在本仓被禁止，且与现有生命周期冲突。 |
| `osc`/`sgr` 低层解析类型 | 不直接接入 | `Terminal::vt_write` 与回调已覆盖所需语义；直接解析会重复上游状态机。 |
| `selection::gesture`、`build_info`、`alloc` 与 `sys` 其余项 | 不接入 | 无产品需求或已有等价实现；`sys` 仅用日志与 PNG 解码器安装点。 |
| `Terminal::memory_usage`、调试/诊断扩展 | 可按需 | 仅在出现内存诊断需求时接线，避免无谓探针。 |

## 4. 本次对齐带来的本地代码收敛

- 日志接线改为实现 `sys::Logger` 并调用 `sys::LOGGER.install`，不再使用已移除的 `set_logger`。
- PNG 解码器安装改走 `sys::PNG_DECODER.install_with`；`Terminal::kitty_graphics()` 改为直接处理 `Result<Graphics>`。
- `scroll_viewport` 改为处理 `Result`，错误显式记日志；越界滚动由上游静默钳制。
- 光标视觉样式改从 `Snapshot::cursor_visual_style()` 取 `terminal::CursorStyle`，不再依赖 `render::CursorVisualStyle`。
- 词选改用 `SelectWord` builder；`Search` 生命周期参数减为一；`CellIter::codepoint()` 返回 `char`，本仓按 `u32::from` 转换。

## 5. 维护约定

- 升级 pin 后：先 `git diff` 上游 `crates/libghostty-vt/src` 的 `pub` 面，再按「能消除本地重复/提高正确性」筛选接入点，最后同步本文件的 pin 日期、计数与结论。
- 禁止把「未使用项数量」当作目标去强行接线；禁止为使用 API 而绕开上游回调/状态机语义。
- 若后续引入搜索 UI、更多 Kitty 能力或鼠标手势，优先复用 `search`/`kitty::graphics`/`selection::gesture` 的既有抽象，并在此文档补充“已使用/暂不接入”的理由。
