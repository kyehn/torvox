# libghostty-vt API 清单

## 0. 来源与口径

- 上游：<https://github.com/Uzaaft/libghostty-rs>，crate `libghostty-vt`（安全封装 `libghostty-vt-sys`，对 C 库 libghostty-vt 的 FFI 绑定）。
- 本项目经 `native/Cargo.toml` 跟踪 master，当前 pin：`483ea3d5247b214060da58ef6037a95f30ae89fb`（HEAD，2026-04-21），特性 `log` + `png`，无本地补丁。
- 统计口径：对 `crates/libghostty-vt/src` 的 `pub` 项做静态枚举（不含 `pub(crate)`、trait 实现、测试模块）。本 rev 公开项 **789** 个；本仓 `native/src` 实际引用到的上游名 **约 180 个**（按标识符/路径证据，含同名类型的方法调用），其余为未接线能力。
- 目标：只接入“本仓需要且能降低本地复杂度/提高正确性”的 API；不为覆盖率而接线，不引入会话持久化等被禁止能力（见 `docs/specification/PROHIBITED.md`）。

## 1. 模块总览（上游 483ea3d）

| 模块 | 公开项 |
| --- | ---: |
| `terminal` | 85 |
| `render` | 92 |
| `kitty::graphics` | 60 |
| `effect` | 104 |
| `mode` | 50 |
| `selection::gesture` | 50 |
| `color` | 41 |
| `key` | 34 |
| `mouse` | 30 |
| `selection` | 30 |
| `screen` | 29 |
| `search` | 26 |
| `fmt` | 20 |
| `grid_ref` | 11 |
| `osc` | 11 |
| `sgr` | 8 |
| `paste` | 7 |
| `report` | 5 |
| `style` | 5 |
| `log` | 4 |
| `point` | 3 |
| `focus` | 2 |
| `kitty` | 2 |
| `screen::cell` | 2 |
| `error` | 2 |
| `alloc` | 5 |
| `build_info` | 1 |
| `sys` | 1 |
| `snapshot` | 36 |
| `unicode` | 2 |
| `(root)` | 33 |
| 合计 | 789 |

说明：`point`/`color`/`effect`/`mode`/`report` 等在本 rev 已拆为独立模块并由根模块再导出；与旧版 `PointCoordinate`/`PointSpace` 对应的现名为 `point::Coordinate`、`point::Space`（本仓以别名 `PointCoordinate`/`PointSpace` 适配）。

## 2. 已使用 API（按能力域）

调用点主要位于 `native/src/terminal/ghostty_terminal/internal/`（VT 主循环、命令、渲染、选区、Kitty）、`native/src/terminal/session/`、`native/src/terminal/test_helpers.rs`、`native/src/render/context/renderer.rs`、`native/src/android/ffi/session.rs` 等。

### 2.1 终端与输出

- `Terminal::{new, vt_write, resize, reset, rows, cols, total_rows, scrollback_rows, cursor_x, cursor_y, title, pwd, active_screen, is_cursor_visible, is_mouse_tracking, scrollbar, mode, set_mode, set_default_* , set_scrollback_max_lines, set_scrollback_max_bytes, scroll_viewport, grid_ref, point_from_grid_ref, kitty_graphics, set_selection, selection, on_pty_write, on_bell, on_clipboard_write, on_size}`。
- `ScrollViewport::{Top,Bottom,Delta}`、`Scrollbar`、`Mode`（含 `BRACKETED_PASTE`/`FOCUS_EVENT`/`SYNC_OUTPUT`/`COLOR_SCHEME_REPORT` 等查询与设置）、`SizeReportSize`。
- 剪贴板回调类型：`ClipboardWrite`、`ClipboardWrite::reply`、`ClipboardLocation`、`ClipboardContent`/`ClipboardContents`、`ClipboardWriteError::Busy`。
- 默认色与光标：`set_default_fg/bg/cursor_color`、`set_default_color_palette`、`default_color_palette`、`color_palette`、`set_default_cursor_style`、`terminal::CursorStyle`。

### 2.2 渲染快照与单元提取

- `render::RenderState`、`render::Snapshot`、`render::RowIterator`/`Row`、`render::CellIterator`/`Cell`、`render::Cursor`、`render::CursorVisualStyle`、`render::Dirty`/`render::DirtyState`、`render::Color`/`CellFg/Bg`、`render::StyleSet` 及快照查询（`update/clean`、`cursor`、`selection`、`viewport`、`row/cell` 迭代与颜色/样式读出）。
- `screen::Screen`（活动屏判定）、`screen::Cell`、`style::Style`/`style::Underline`（下划线形状直接按枚举判定，不再维护本地布尔包装）。
- `point::{Point, Coordinate, Space}`（本仓别名 `PointCoordinate`/`PointSpace`）用于 `grid_ref`、历史/视口寻址与坐标换算。

### 2.3 选区、搜索与格式化

- `selection::{Selection, ScreenSelection, ScreenPoint, Point as SelectionPoint, Drag, Pin}` 用于安装/跟踪终端原生选区。
- `search::{Search, SearchOptions, Active, SelectedMatch, Page, PageOptions, Viewport, Match}` 与 `GridRef` 路径用于搜索结果定位与页提取。

### 2.4 输入、焦点、粘贴与上报

- `key::{Encoder, Event, Key, Action, Mods, OptionSet, OptionAsAlt, KittyFlags, KeyEvent}`：按终端模式编码按键。
- `mouse::{Encoder, Event, Action, Button, Position, MouseEvent}` 与 `terminal::Terminal::encode_mouse` 路径。
- `focus`：焦点开关由 `Mode::FOCUS_EVENT` 判定（`CSI I/O` 序列由输入层按协议发送）。
- `paste::{Options, check_safe, encode}`：安全检查与括号化编码（含 `OutOfSpace` 扩容重试）。
- `report::ColorScheme::{Dark,Light}::encode`：在 `COLOR_SCHEME_REPORT` 模式开启时，用上游编码器生成 `CSI ? 997 ; 1|2 n` 主动上报，避免手写字面量。

### 2.5 Kitty 图形

- `Terminal::kitty_graphics`、`kitty::graphics::{KittyGraphics, Command, Response, CommandData, ControlData, Command as wire command helpers}`：协议解析/存储查询与应答；渲染侧读取 `ImageStorage`/图像与放置信息生成纹理上传。
- `render::ImagePlacement` 等放置元数据用于帧构建。

### 2.6 其它

- `log`：`set_log_level` / `disable` / `LogLevel` 接线到 Rust 日志后端。
- `error::Error`：统一错误传播与日志。
- `fmt`（子集）：格式化/提取能力按需用于文本导出路径（以源码实际引用为准）。

## 3. 未使用但应关注的 API（结论与取舍）

| 模块/项 | 结论 | 理由 |
| --- | --- | --- |
| `report` 其余报告类型 | 暂不接入 | 当前仅需配色变化主动上报；其它报告由终端查询/应答路径按需补。 |
| `focus::Event::encode` | 暂不替换 | 焦点序列固定、现有实现已正确且无额外状态；替换收益不足以引入接口改动。 |
| `grid_ref::TrackedGridRef` | 暂不接入 | 选区/搜索已由 `selection`+`search` 覆盖，未见需要持久跟踪引用的场景。 |
| `snapshot` | 不接入 | 会话/状态持久化在本仓被禁止，且与现有生命周期冲突。 |
| `osc`/`sgr` 低层解析类型 | 不直接接入 | `Terminal::vt_write` 与回调已覆盖所需语义；直接解析会重复上游状态机。 |
| `selection::gesture`、`build_info`、`alloc`、`unicode` 大部分 | 不接入 | 无产品需求或已有等价实现。 |
| `Terminal::memory_usage`、调试/诊断扩展 | 可按需 | 仅在出现内存诊断需求时接线，避免无谓探针。 |

## 4. 本次对齐带来的本地代码收敛

- 配色变化上报改用 `report::ColorScheme::encode`，移除手写 `CSI ? 997 ; 1|2 n` 字面量构造。
- 下划线判定改为直接使用 `style::Underline` 枚举比较，删除 `has_visible_underline` / `is_double_underline` 两个本地包装函数。

## 5. 维护约定

- 升级 pin 后：先 `git diff` 上游 `crates/libghostty-vt/src` 的 `pub` 面，再按「能消除本地重复/提高正确性」筛选接入点，最后同步本文件的 rev、计数与结论。
- 禁止把「未使用项数量」当作目标去强行接线；禁止为使用 API 而绕开上游回调/状态机语义。
- 若后续引入搜索 UI、更多 Kitty 能力或鼠标手势，优先复用 `search`/`kitty::graphics`/`selection::gesture` 的既有抽象，并在此文档补充“已使用/暂不接入”的理由。
