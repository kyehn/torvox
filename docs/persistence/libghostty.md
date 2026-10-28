# libghostty-vt API 清单

## 0. 来源与口径

上游：<https://github.com/Uzaaft/libghostty-rs>，crate `libghostty-vt`（对 C 库 `libghostty-vt-sys` 的安全 Rust 绑定）。本项目经 `native/Cargo.toml` 跟踪其 master 修订，开启 `log` 与 `png` 两个特性，无本地补丁。

`crates/libghostty-vt/src` 对外公开项 718 个，本项目调用 203 个，其余 515 个未使用。

- 已使用：本项目源码（含测试）出现具名引用、构造或方法调用。经字段访问隐含使用的返回类型（`PlacementRenderInfo`、`RowSelection`、`CursorViewport`）计入已使用。
- 枚举变体随父类型计为一项；`pub const` 常量与 `pub use` 再导出别名各计一项。
- `pub(crate)` 内部助手与 `From`/`TryFrom`/`Iterator`/`Display`/`Debug` 等 trait 实现不是公开 API，不在清单内；上游自身测试模块不计入。

## 1. 模块总览

| 模块 | 公开项 | 已使用 | 未使用 |
| --- | --- | --- | --- |
| `terminal` | 225 | 46 | 179 |
| `key` | 36 | 13 | 23 |
| `mouse` | 30 | 14 | 16 |
| `paste` | 9 | 1 | 8 |
| `render` | 63 | 26 | 37 |
| `screen` | 37 | 11 | 26 |
| `search` | 26 | 15 | 11 |
| `selection` | 38 | 12 | 26 |
| `style` | 45 | 27 | 18 |
| `fmt` | 26 | 8 | 18 |
| `kitty::graphics` | 60 | 26 | 34 |
| `kitty` | 2 | 0 | 2 |
| `log` | 5 | 3 | 2 |
| `error` | 2 | 1 | 1 |
| `alloc` | 5 | 0 | 5 |
| `build_info` | 11 | 0 | 11 |
| `focus` | 2 | 0 | 2 |
| `osc` | 9 | 0 | 9 |
| `sgr` | 8 | 0 | 8 |
| `snapshot` | 27 | 0 | 27 |
| `unicode` | 2 | 0 | 2 |
| `selection::gesture` | 50 | 0 | 50 |
| 合计 | 718 | 203 | 515 |

调用点集中在 `native/src/terminal/ghostty_terminal/internal.rs`（VT 线程主循环）、`public_api.rs`、`commands.rs`、`theme.rs`、`session.rs` 与 `render/context.rs`；其余模块全未接线。

## 2. 已使用 API

### 2.1 `terminal`（46 项）

下表为 `Terminal` 的方法与相关类型；模式常量只列已用的三个，其余 39 个见 3.1。

| 项 | 说明 |
| --- | --- |
| `ClipboardContent` | 单条负载（`data`/`mime` 字段） |
| `ClipboardContents` | 负载集合 |
| `ClipboardLocation` | 选择器枚举（`Standard`/`Primary`/`Selection`） |
| `ClipboardWriteError` | 写入拒绝原因（仅用 `Busy`） |
| `Mode` | 终端模式号与 DEC/ANSI 分类的载体 |
| `ModeKind` | `Dec`/`Ansi`，模式种类 |
| `Point` | 网格坐标寻址：`Screen`/`Viewport`/`History`/`Active` 四种空间 |
| `PointCoordinate` | 行列坐标值 |
| `PointSpace` | 格引用反解时的坐标空间（仅用 `Screen`） |
| `ScrollViewport` | 视口滚动量（仅用 `Delta`） |
| `SizeReportSize` | `on_size` 应答的行列与单元格像素载荷 |
| `Terminal` | 终端实例，VT 解析与终端状态的单一来源 |
| `ClipboardWrite::contents` | 迭代负载的 MIME 与字节 |
| `ClipboardWrite::location` | 目标选择器：剪贴板/主选区/次选区 |
| `ClipboardWrite::reply` | 回调返回前必须应答，否则写入被拒 |
| `Mode::BRACKETED_PASTE` | 模式 2004，粘贴括号化判定 |
| `Mode::FOCUS_EVENT` | 模式 1004，焦点上报判定 |
| `Mode::SYNC_OUTPUT` | 模式 2026，同步输出（脏帧门控）判定 |
| `Mode::new` | 以「编号 + 种类」构造模式 |
| `Terminal::active_screen` | 活动屏查询，备用屏判定 |
| `Terminal::color_palette` | 生效的 256 色调色板（含 OSC 4 覆盖） |
| `Terminal::cols` | 当前网格列数 |
| `Terminal::cursor_x` | 活动区光标列 |
| `Terminal::cursor_y` | 活动区光标行 |
| `Terminal::default_color_palette` | 读取默认调色板以覆写 |
| `Terminal::grid_ref` | 取网格格引用，读单元与超链接的统一入口 |
| `Terminal::is_cursor_visible` | 光标可见性 |
| `Terminal::mode` | 查询单个终端模式开关 |
| `Terminal::new` | 以行列创建终端，VT 线程入口 |
| `Terminal::on_clipboard_write` | OSC 52/1337/5522 剪贴板写入回调 |
| `Terminal::on_pty_write` | 终端应答（DECRPM、DSR、DA）回写 PTY 的回调 |
| `Terminal::on_size` | XTWINOPS 尺寸查询（CSI 14/16/18 t）应答回调 |
| `Terminal::point_from_grid_ref` | 格引用反解为绝对网格坐标 |
| `Terminal::reset` | RIS 全重置，清空回滚与状态 |
| `Terminal::resize` | 重调网格尺寸与单元格像素几何 |
| `Terminal::rows` | 当前网格行数 |
| `Terminal::scroll_viewport` | 按 `ScrollViewport::Delta` 增量滚动视口 |
| `Terminal::scrollback_rows` | 回滚行数 |
| `Terminal::set_default_bg_color` | 推送默认背景色 |
| `Terminal::set_default_color_palette` | 推送 16 色 ANSI 调色板 |
| `Terminal::set_default_fg_color` | 推送默认前景色 |
| `Terminal::set_scrollback_max_bytes` | 解除字节预算，让行数成为唯一回滚约束 |
| `Terminal::set_scrollback_max_lines` | 设置回滚行数预算 |
| `Terminal::title` | 窗口标题，会话列表显示 |
| `Terminal::total_rows` | 含回滚的总行数 |
| `Terminal::vt_write` | 喂入 PTY 输出字节，驱动全部 VT 解析 |

### 2.2 `key`（13 项）

输入是 Android 侧的键码 + 修饰位 + 平台文本，经 VT 线程编码为转义序列。

| 项 | 说明 |
| --- | --- |
| `Action` | 按键动作（仅用 `Press`） |
| `Key` | 逻辑键枚举 |
| `Mods` | 修饰位标志（仅用 `SHIFT`/`ALT`/`CTRL`） |
| `Encoder::encode_to_vec` | 编码为转义序列字节 |
| `Encoder::new` | 建键盘编码器，逐事件复用 |
| `Encoder::set_options_from_terminal` | 从终端同步 DECCKM、keypad、modifyOtherKeys、kitty 协议等选项 |
| `Event::new` | 建按键事件，逐事件复用 |
| `Event::set_action` | 设动作 |
| `Event::set_consumed_mods` | 标记已被文本消费的修饰位，避免重复叠加 |
| `Event::set_key` | 设逻辑键 |
| `Event::set_mods` | 设修饰位 |
| `Event::set_unshifted_codepoint` | 设无修饰码点，缺省显式写 0 防跨事件残留 |
| `Event::set_utf8` | 设平台生成的文本（Shift+A → `A`） |

### 2.3 `mouse`（14 项）

鼠标上报的像素位置配渲染器的实时单元格尺寸，编码出 SGR/X10 序列。

| 项 | 说明 |
| --- | --- |
| `Action` | 鼠标动作（`Press`/`Release`/`Motion`） |
| `Button` | 按键（左/右/中/四/五） |
| `EncoderSize` | 屏幕与单元格像素尺寸、内边距 |
| `Position` | 像素坐标 |
| `Encoder::encode_to_vec` | 编码为上报序列 |
| `Encoder::new` | 建鼠标编码器，逐事件复用 |
| `Encoder::set_any_button_pressed` | 视口外 motion 仅在有键按下时上报 |
| `Encoder::set_options_from_terminal` | 从终端同步跟踪模式与上报格式 |
| `Encoder::set_size` | 设置像素→单元映射几何 |
| `Event::new` | 建鼠标事件，逐事件复用 |
| `Event::set_action` | 设动作 |
| `Event::set_button` | 设按键 |
| `Event::set_mods` | 设修饰位 |
| `Event::set_position` | 设像素位置 |

### 2.4 `paste`（1 项）

粘贴取活体模式 2004 决定是否括号化。

| 项 | 说明 |
| --- | --- |
| `encode` | 粘贴编码：括号化包裹、换行转回车、控制字节中化，输出缓冲按 `OutOfSpace{required}` 精确扩容 |

### 2.5 `render`（26 项）

帧数据主路径：`RenderState::update` 出快照，行迭代器 + 单元迭代器逐格读取；行级脏跟踪驱动跨帧行缓存复用——干净行整段复制上一帧，省逐单元 FFI 往返（命中的前提是同一 `RenderState` 跨帧复用且逐帧标记已消费，口径见 `grid-extraction-incremental` 能力）。

| 项 | 说明 |
| --- | --- |
| `CellIterator` | 单元迭代器 |
| `CursorViewport` | 光标视口坐标返回值 |
| `CursorVisualStyle` | 光标视觉样式枚举（`Block`/`Bar`/`Underline`） |
| `RenderState` | 渲染状态宿主，产出逐帧快照 |
| `RowIterator` | 可见行迭代器 |
| `RowSelection` | 行选区范围返回值（`start_x`/`end_x` 字段） |
| `Snapshot` | 逐帧渲染快照 |
| `CellIteration::bg_color` | 解出的背景色 |
| `CellIteration::fg_color` | 解出的前景色 |
| `CellIteration::graphemes` | 字素簇续接码位 |
| `CellIteration::next` | 取下一单元 |
| `CellIteration::raw_cell` | 原始单元（码点、宽度、样式号） |
| `CellIteration::style` | 解出的样式（SGR 属性） |
| `CellIterator::new` | 创建单元迭代器 |
| `CellIterator::update` | 绑定行 |
| `RenderState::new` | 创建渲染状态 |
| `RenderState::update` | 把终端状态更新为快照 |
| `RowIteration::dirty` | 该行是否脏——行级脏判定，干净行整段复制上一帧缓存 |
| `RowIteration::next` | 取下一行 |
| `RowIteration::selection` | 该行的终端持有选区范围 |
| `RowIterator::new` | 创建行迭代器 |
| `RowIterator::update` | 绑定快照 |
| `Snapshot::clean` | 标记快照已消费——整帧抽取完成后调用，与行缓存复用配对 |
| `Snapshot::cursor_viewport` | 光标视口坐标（滚动后与活动区坐标分叉，必须用这个） |
| `Snapshot::cursor_visible` | 快照内的光标可见性 |
| `Snapshot::cursor_visual_style` | 光标视觉样式 |

### 2.6 `screen`（11 项）

不经迭代器的按格查询（全量文本、宽字符尾格、超链接命中、选区端点）。

| 项 | 说明 |
| --- | --- |
| `Cell` | 原始单元 |
| `CellWide` | 宽度枚举：`Wide`/`SpacerTail`/`SpacerHead` |
| `GridRef` | 网格格引用 |
| `Screen` | 主屏/备用屏（仅用 `Alternate`） |
| `Cell::codepoint` | 主码点 |
| `Cell::has_hyperlink` | 该格是否带 OSC 8 链接 |
| `Cell::style_id` | 样式号，供连续同样式单元缓存复用 |
| `Cell::wide` | 宽度类型：常规/宽/spacer |
| `GridRef::cell` | 取该格单元 |
| `GridRef::hyperlink_uri` | OSC 8 超链接 URI，按 `OutOfSpace{required}` 二次精确读取 |
| `GridRef::style` | 取该格样式 |

### 2.7 `search`（15 项）

搜索随 VT 线程存活，`run` 之后按快照读命中。

| 项 | 说明 |
| --- | --- |
| `MatchBuffer` | 命中读出的可复用缓冲 |
| `Search` | 搜索器，随 VT 线程存活并跨查询复用索引状态 |
| `Snapshot` | 搜索快照 |
| `MatchBuffer::new` | 构造命中读出缓冲 |
| `Search::new` | 以终端创建搜索器 |
| `Search::run` | 同步跑完搜索 |
| `Search::select_next` | 朝更旧内容切换选中命中，含视口滚动 |
| `Search::select_prev` | 朝更新内容切换选中命中 |
| `Search::selected_index` | 当前选中序号 |
| `Search::set_needle` | 设置 needle |
| `Search::snapshot` | 取命中快照 |
| `Search::total_matches` | 命中总数 |
| `Snapshot::selected_match` | 选中命中的选区 |
| `Snapshot::terminal` | 快照所属终端 |
| `Snapshot::viewport_matches` | 视口内全部命中 |

### 2.8 `selection`（12 项）

选区一律装进终端成为跟踪引用，随滚动/输出/重排跟随。

| 项 | 说明 |
| --- | --- |
| `Order` | 取序方向（仅用 `Forward`） |
| `SelectWordOptions` | 词选选项 |
| `Selection` | 线性选区（`rectangle` 恒 false） |
| `SelectWordOptions::new` | 以格引用构造词选选项 |
| `Selection::end` | 终点格引用 |
| `Selection::new` | 以起止格引用构造 |
| `Selection::start` | 起点格引用 |
| `Selection::to_ordered` | 按 `Order` 取序，抹平拖动方向 |
| `Terminal::select_all` | 全选可选内容（不含尾部空行空列） |
| `Terminal::select_word` | 上游词边界语义派生选区 |
| `Terminal::selection` | 读当前活动选区 |
| `Terminal::set_selection` | 安装/清除终端持有的跟踪选区 |

### 2.9 `style`（27 项）

样式与颜色从 `render::CellIteration` 直读，主题色经 setter 推送。

| 项 | 说明 |
| --- | --- |
| `Id` | 样式号，连续同样式单元的缓存键 |
| `Palette` | 256 色调色板 |
| `PaletteIndex` | 调色板索引，含 16 个 ANSI 具名常量 |
| `RgbColor` | RGB 颜色（`r`/`g`/`b` 字段） |
| `Style` | 解出的单元样式（SGR 属性全集） |
| `StyleColor` | 颜色表达：`Rgb`/`Palette`/`None` |
| `Underline` | 下划线形状：`Single`/`Double`/`Curly`/`Dashed`/`Dotted` |
| `Palette::get` | 按索引读调色板项 |
| `Palette::set` | 按索引写调色板项 |
| `PaletteIndex::BLACK` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BLUE` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_BLACK` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_BLUE` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_CYAN` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_GREEN` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_MAGENTA` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_RED` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_WHITE` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::BRIGHT_YELLOW` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::CYAN` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::GREEN` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::MAGENTA` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::RED` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::WHITE` | ANSI 调色板索引，主题 16 色经它写入 |
| `PaletteIndex::YELLOW` | ANSI 调色板索引，主题 16 色经它写入 |
| `RgbColor::parse` | 解析十六进制颜色串，主题文件唯一色值入口 |
| `RgbColor::perceived_luminance` | 感知亮度，决定选选用前景/背景反色 |

### 2.10 `fmt`（8 项）

选区复制文本的唯一出口。

| 项 | 说明 |
| --- | --- |
| `Formatter` | 换行感知的选区文本格式化器 |
| `FormatterOptions` | 格式化选项 |
| `Formatter::format_alloc` | 格式化为字节 |
| `Formatter::new` | 以终端与选项创建 |
| `FormatterOptions::new` | 默认选项 |
| `FormatterOptions::with_selection` | 绑定选区 |
| `FormatterOptions::with_trim` | 去尾部空白 |
| `FormatterOptions::with_unwrap` | 拼接软换行行且不带换行符 |

### 2.11 `kitty::graphics`（26 项）

图像在进程内解码为 RGBA8，放置几何一次取全。

| 项 | 说明 |
| --- | --- |
| `Graphics` | Kitty 图像存储 |
| `Image` | 单幅图像 |
| `ImageFormat` | 像素格式枚举：`Rgba`/`Rgb`/`Gray`/`GrayAlpha`/`Png` |
| `PlacementIteration` | 放置迭代句柄 |
| `PlacementIterator` | 放置迭代器 |
| `PlacementRenderInfo` | 渲染几何返回值（视口可见性、像素/源矩形、行列） |
| `RustPngDecoder` | 上游内置 PNG 解码器 |
| `set_png_decoder` | 注册进程内 PNG 解码器（线程局部） |
| `Graphics::generation` | 存储生成戳，不变则跳过放置查询 |
| `Graphics::image` | 按图像号取图像 |
| `Image::data` | 像素字节 |
| `Image::format` | 像素格式 |
| `Image::height` | 图像高 |
| `Image::width` | 图像宽 |
| `PlacementIteration::image_id` | 所属图像号 |
| `PlacementIteration::is_virtual` | 是否虚拟占位（跳过，交给字形管线） |
| `PlacementIteration::next` | 取下一放置 |
| `PlacementIteration::placement_render_info` | 一次取全渲染几何 |
| `PlacementIteration::x_offset` | 单元格内水平像素偏移 |
| `PlacementIteration::y_offset` | 单元格内垂直像素偏移 |
| `PlacementIteration::z` | 叠加次序 |
| `PlacementIterator::new` | 创建放置迭代器 |
| `PlacementIterator::update` | 绑定图像存储 |
| `RustPngDecoder::new` | 构造内置解码器 |
| `Terminal::kitty_graphics` | 取 Kitty 图像存储 |
| `Terminal::set_kitty_image_storage_limit` | 图像存储总量上限（本仓设 64 MiB，恰为上游默认值） |

### 2.12 `log`（3 项）

上游内部日志（VT 解析错误、分配失败、状态机异常）经同一 logcat sink 输出。

| 项 | 说明 |
| --- | --- |
| `Level` | 日志级别（`Error`/`Warning`/`Info`/`Debug`） |
| `Logger` | 回调 trait；`log` 特性提供 `impl<L: log::Log> Logger`，故可直接复用本仓 logcat sink |
| `set_logger` | 安装日志回调，未装则上游内部日志全部静默丢弃 |

### 2.13 `error`（1 项）

错误一律显式处理：精确扩容、出声后丢弃、或让查询回退到默认值。

| 项 | 说明 |
| --- | --- |
| `Error` | 错误枚举；`OutOfSpace{required}` 用于超链接 URI 与粘贴编码的精确二次分配 |

## 3. 未使用 API

515 项未用（72%）经聚类验证为正常构成：整域禁用 117 项（`selection::gesture` 50 + `snapshot` 27 + `osc`/`sgr` 17 + `build_info` 11 + `alloc` 5 + 其余 7，均对应 PROHIBITED/DESIGN 禁项或引擎已内化）、枚举常量族 84 项（`Mode` 39 + 设备属性应答族 45，清单型常量按需取用）、同一能力的替代表达 24 项（`*_with_alloc` 分配器变体、`*_buf`/`format_len` 缓冲变体——已用等价形态）、功能域内可选方法/选项 290 项（完整桌面嵌壳方的可选面）。对照：上游官方示例 `ghostling_rs`（约 1kLoC 的完整终端）消费约 50 项，本仓 203 项是其 4 倍。718 项是上游「完整能力菜单」而非「预期全量消费」。

### 3.1 `terminal`（179 项）

配置类 setter 与查询类 getter 全部未用，上游默认值即满足需求；设备属性应答族备而未用的原因是本项目不需要向被测程序谎报能力。

| 项 | 说明 |
| --- | --- |
| `ClipboardMime` | 应答内的 MIME 条目 |
| `ClipboardRead` | 剪贴板读取请求（OSC 5522） |
| `ClipboardReadError` | 读取失败原因 |
| `ClipboardReplyContent` | 读取应答内容 |
| `ClipboardWrite` | 剪贴板写入请求（已用，此处仅列未用方法） |
| `ColorScheme` | 配色方案（明/暗） |
| `CompressionActivity` | 回滚压缩活跃度 |
| `CompressionMode` | 回滚压缩模式 |
| `CompressionResult` | 回滚压缩结果 |
| `ConformanceLevel` | 一致性级别常量族（VT100…VT525、LEVEL_2…5） |
| `CursorStyle` | 上游光标样式枚举 |
| `DesktopNotification` | OSC 9/777 桌面通知 |
| `DeviceAttributeFeature` | 设备特性位常量族 |
| `DeviceAttributes` | 设备属性应答组合 |
| `DeviceType` | 设备型号常量族（VT100…VT525） |
| `PrimaryDeviceAttributes` | 主设备属性（DA1） |
| `ProgressReport` | OSC 9;4 进度上报 |
| `ProgressState` | 进度状态枚举 |
| `Scrollbar` | 滚动条状态 |
| `SecondaryDeviceAttributes` | 次设备属性（DA2） |
| `TertiaryDeviceAttributes` | 三次设备属性（DA3） |
| `ClipboardMime::new` | 构造 MIME 条目 |
| `ClipboardRead::can_remember` | 授权可否被记忆 |
| `ClipboardRead::granted` | 此前是否已授权 |
| `ClipboardRead::list` | 负载列表 |
| `ClipboardRead::location` | 请求目标选择器 |
| `ClipboardRead::mimes` | 可提供的 MIME 列表 |
| `ClipboardRead::name` | 请求方程序名 |
| `ClipboardRead::reply` | 应答读取请求 |
| `ClipboardReplyContent::new` | 构造读取应答内容 |
| `ClipboardWrite::can_remember` | 授权可否被记忆 |
| `ClipboardWrite::granted` | 此前是否已授权 |
| `ClipboardWrite::name` | 写入方程序名 |
| `ColorScheme::encode_report` | 编码配色方案上报序列 |
| `ConformanceLevel::LEVEL_2` | 第 2 级扩展 |
| `ConformanceLevel::LEVEL_3` | 第 3 级扩展 |
| `ConformanceLevel::LEVEL_4` | 第 4 级扩展 |
| `ConformanceLevel::LEVEL_5` | 第 5 级扩展 |
| `ConformanceLevel::VT100` | VT100 级别 |
| `ConformanceLevel::VT101` | VT101 级别 |
| `ConformanceLevel::VT102` | VT102 级别 |
| `ConformanceLevel::VT125` | VT125 级别 |
| `ConformanceLevel::VT131` | VT131 级别 |
| `ConformanceLevel::VT132` | VT132 级别 |
| `ConformanceLevel::VT220` | VT220 级别 |
| `ConformanceLevel::VT240` | VT240 级别 |
| `ConformanceLevel::VT320` | VT320 级别 |
| `ConformanceLevel::VT340` | VT340 级别 |
| `ConformanceLevel::VT420` | VT420 级别 |
| `ConformanceLevel::VT510` | VT510 级别 |
| `ConformanceLevel::VT520` | VT520 级别 |
| `ConformanceLevel::VT525` | VT525 级别 |
| `DesktopNotification::body` | 通知正文 |
| `DesktopNotification::title` | 通知标题 |
| `DeviceAttributeFeature::ANSI_COLOR` | 支持 ANSI 颜色 |
| `DeviceAttributeFeature::ANSI_TEXT_LOCATOR` | 支持 ANSI 文本定位 |
| `DeviceAttributeFeature::CLIPBOARD` | 支持剪贴板 |
| `DeviceAttributeFeature::COLUMNS_132` | 支持 132 列 |
| `DeviceAttributeFeature::HORIZONTAL_SCROLLING` | 支持水平滚动 |
| `DeviceAttributeFeature::LOCATOR` | 支持定位器（鼠标） |
| `DeviceAttributeFeature::NATIONAL_REPLACEMENT` | 支持国别字符替换 |
| `DeviceAttributeFeature::PRINTER` | 支持打印机端口 |
| `DeviceAttributeFeature::RECTANGULAR_EDITING` | 支持矩形编辑 |
| `DeviceAttributeFeature::REGIS` | 支持 ReGIS 图形 |
| `DeviceAttributeFeature::SELECTIVE_ERASE` | 支持选择性擦除 |
| `DeviceAttributeFeature::SIXEL` | 支持 Sixel |
| `DeviceAttributeFeature::TECHNICAL_CHARACTERS` | 支持技术字符集 |
| `DeviceAttributeFeature::TERMINAL_STATE` | 支持终端状态查询 |
| `DeviceAttributeFeature::USER_DEFINED_KEYS` | 支持用户自定义键 |
| `DeviceAttributeFeature::WINDOWING` | 支持窗口操作 |
| `DeviceType::VT100` | 报称 VT100 |
| `DeviceType::VT220` | 报称 VT220 |
| `DeviceType::VT240` | 报称 VT240 |
| `DeviceType::VT320` | 报称 VT320 |
| `DeviceType::VT330` | 报称 VT330 |
| `DeviceType::VT340` | 报称 VT340 |
| `DeviceType::VT382` | 报称 VT382 |
| `DeviceType::VT420` | 报称 VT420 |
| `DeviceType::VT510` | 报称 VT510 |
| `DeviceType::VT520` | 报称 VT520 |
| `DeviceType::VT525` | 报称 VT525 |
| `Mode::ALT_ESC_PREFIX` | DEC 1036，Alt 发 ESC 前缀 |
| `Mode::ALT_SCREEN` | DEC 1047，备用屏（备用屏判定走 `active_screen`） |
| `Mode::ALT_SCREEN_LEGACY` | DEC 47，旧式备用屏 |
| `Mode::ALT_SCREEN_SAVE` | DEC 1049，备用屏保存光标 |
| `Mode::ALT_SCROLL` | DEC 1007，备用屏滚轮 |
| `Mode::ALT_SENDS_ESC` | DEC 1039，Alt 发 ESC |
| `Mode::ANY_MOUSE` | DEC 1003，任意移动鼠标跟踪 |
| `Mode::AUTOREPEAT` | DEC 8，自动重复 |
| `Mode::BUTTON_MOUSE` | DEC 1002，按键事件鼠标跟踪 |
| `Mode::COLOR_SCHEME_REPORT` | DEC 2031，配色方案上报 |
| `Mode::CURSOR_BLINKING` | DEC 12，光标闪烁（PROHIBITED 禁止设置） |
| `Mode::CURSOR_VISIBLE` | DEC 25，光标可见 |
| `Mode::DECCKM` | DEC 1，应用光标键 |
| `Mode::ENABLE_MODE3` | DEC 40，132 列模式使能 |
| `Mode::GRAPHEME_CLUSTER` | DEC 2027，字素簇模式 |
| `Mode::INSERT` | ANSI 4，插入模式 |
| `Mode::IN_BAND_RESIZE` | DEC 2048，带内尺寸上报 |
| `Mode::KAM` | ANSI 2，键盘动作锁定 |
| `Mode::KEYPAD_KEYS` | DEC 66，应用小键盘 |
| `Mode::LEFT_RIGHT_MARGIN` | DEC 69，左右边距 |
| `Mode::LINEFEED` | ANSI 20，换行兼回车 |
| `Mode::NORMAL_MOUSE` | DEC 1000，常规鼠标跟踪 |
| `Mode::NUMLOCK_KEYPAD` | DEC 1035，忽略 numlock（默认开） |
| `Mode::ORIGIN` | DEC 6，原点模式 |
| `Mode::PASTE_EVENTS` | DEC 5522，Kitty 粘贴事件 |
| `Mode::REVERSE_COLORS` | DEC 5，反色 |
| `Mode::REVERSE_WRAP` | DEC 45，反向折行 |
| `Mode::REVERSE_WRAP_EXT` | DEC 1045，扩展反向折行 |
| `Mode::SAVE_CURSOR` | DEC 1048，保存光标 |
| `Mode::SGR_MOUSE` | DEC 1006，SGR 鼠标扩展坐标 |
| `Mode::SGR_PIXELS_MOUSE` | DEC 1016，SGR 像素鼠标 |
| `Mode::SLOW_SCROLL` | DEC 4，慢速滚动 |
| `Mode::SRM` | ANSI 12，本地回显 |
| `Mode::URXVT_MOUSE` | DEC 1015，urxvt 鼠标扩展 |
| `Mode::UTF8_MOUSE` | DEC 1005，UTF-8 鼠标扩展坐标 |
| `Mode::VISIBILITY_REPORT` | DEC 2033，可见性上报 |
| `Mode::WRAPAROUND` | DEC 7，自动折行 |
| `Mode::X10_MOUSE` | DEC 9，X10 鼠标跟踪 |
| `Mode::_132_COLUMN` | DEC 3，132 列 |
| `Mode::kind` | 模式种类回读 |
| `Mode::value` | 模式号回读 |
| `PrimaryDeviceAttributes::new` | 构造主设备属性 |
| `ProgressReport::progress` | 进度百分比 |
| `ProgressReport::state` | 进度状态 |
| `Terminal::bg_color` | OSC 11 背景色 |
| `Terminal::clipboard_write_max_bytes` | 剪贴板写入上限读取 |
| `Terminal::compress` | 回滚压缩：仅改存储表示，不动逻辑内容与回滚上限（见 §5） |
| `Terminal::compression_activity` | 回滚压缩的活跃度 |
| `Terminal::continuation_alloc` | 把续传缓冲分配为字节 |
| `Terminal::continuation_buf` | 把续传缓冲拷入定长缓冲 |
| `Terminal::continuation_max_bytes` | 跨调用续传缓冲上限 |
| `Terminal::continuation_write` | 把续传缓冲写入 `Write` |
| `Terminal::cursor_color` | OSC 12 光标色 |
| `Terminal::cursor_style` | 光标样式（视觉样式改由快照读取） |
| `Terminal::default_bg_color` | 默认背景色读取 |
| `Terminal::default_cursor_color` | 默认光标色读取 |
| `Terminal::default_fg_color` | 默认前景色读取 |
| `Terminal::fg_color` | OSC 10 前景色 |
| `Terminal::height_px` | 终端像素高 |
| `Terminal::is_cursor_at_prompt` | 光标是否停在语义提示处 |
| `Terminal::is_cursor_pending_wrap` | 光标是否待折行 |
| `Terminal::is_mouse_tracking` | 鼠标跟踪是否开启（编码器自行同步） |
| `Terminal::is_vt_ground` | 当前是否处于 ground 状态 |
| `Terminal::kitty_keyboard_flags` | kitty 键盘协议标志读取 |
| `Terminal::mouse_shape` | OSC 22 设置的鼠标形状 |
| `Terminal::new_with_alloc` | 自定义分配器创建终端 |
| `Terminal::on_bell` | 振铃回调 |
| `Terminal::on_clipboard_read` | 剪贴板读取回调 |
| `Terminal::on_color_scheme` | 配色方案变化回调 |
| `Terminal::on_desktop_notification` | 桌面通知回调 |
| `Terminal::on_device_attributes` | 设备属性查询回调 |
| `Terminal::on_enquiry` | ENQ 询问回调 |
| `Terminal::on_progress_report` | 进度上报回调 |
| `Terminal::on_pwd_changed` | 工作目录变化回调 |
| `Terminal::on_render_hold` | 渲染保持回调（模式 2026 配套；本仓在帧发布层门控，见 §5 五次复核） |
| `Terminal::on_title_changed` | 标题变化回调 |
| `Terminal::on_xtversion` | XTGETTCAP/版本查询回调 |
| `Terminal::pwd` | OSC 7/1337 上报的工作目录 |
| `Terminal::scrollback_max_bytes` | 回滚字节上限读取 |
| `Terminal::scrollback_max_lines` | 回滚行数上限读取 |
| `Terminal::scrollbar` | 滚动条状态 |
| `Terminal::set_apc_max_bytes` | APC 序列上限（上游默认：kitty 65 MiB、glyph 1 MiB，两协议默认全开） |
| `Terminal::set_clipboard_write_max_bytes` | 设置剪贴板写入上限 |
| `Terminal::set_continuation_max_bytes` | 设置续传缓冲上限（默认禁用；仅服务快照重放，本仓无持久化故不启用） |
| `Terminal::set_default_cursor_blink` | 设置默认光标闪烁 |
| `Terminal::set_default_cursor_color` | 设置默认光标色 |
| `Terminal::set_default_cursor_style` | 设置默认光标样式 |
| `Terminal::set_default_mode` | 设置模式的默认值（reset 后恢复） |
| `Terminal::set_glyph_protocol_enabled` | 开关字形协议（Sixel/Kitty） |
| `Terminal::set_mode` | 直接设置单个模式 |
| `Terminal::set_resize_pull_scrollback` | 增高时是否从回滚拉取内容 |
| `Terminal::set_terminfo_name` | 设置应答用的 terminfo 名 |
| `Terminal::set_title_report_enabled` | 开关标题上报 |
| `Terminal::track_grid_ref` | 跨滚动自动跟踪的格引用 |
| `Terminal::viewport_active` | 视口是否贴在底部 |
| `Terminal::vt_processing_error` | VT 解析是否出现过错误 |
| `Terminal::vt_write_until_ground` | 写入至解析回到 ground 状态即停 |
| `Terminal::width_px` | 终端像素宽 |

### 3.2 `key`（23 项）

键盘编码器的全部选项由 `set_options_from_terminal` 一次同步，九个单独 setter 因此不调用。

| 项 | 说明 |
| --- | --- |
| `Encoder` | 按键编码器（类型本身） |
| `Event` | 按键/鼠标事件（类型本身） |
| `KittyKeyFlags` | kitty 键盘协议标志位 |
| `OptionAsAlt` | Option 作为 Alt 的策略枚举 |
| `Encoder::encode` | 编码并写入 `Write` |
| `Encoder::new_with_alloc` | 自定义分配器创建编码器 |
| `Encoder::set_alt_esc_prefix` | 单独设 Alt ESC 前缀 |
| `Encoder::set_backarrow_key_mode` | 单独设退格键行为 |
| `Encoder::set_cursor_key_application` | 单独设应用光标键 |
| `Encoder::set_ignore_keypad_with_numlock` | 单独设忽略 numlock |
| `Encoder::set_keypad_key_application` | 单独设应用小键盘 |
| `Encoder::set_kitty_flags` | 单独设 kitty 键盘标志 |
| `Encoder::set_macos_option_as_alt` | 单独设 macOS Option 行为 |
| `Encoder::set_modify_other_keys_state_2` | 单独设 modifyOtherKeys-2 |
| `Event::action` | 动作回读 |
| `Event::consumed_mods` | 已消费修饰位回读 |
| `Event::is_composing` | 读取组字中标记 |
| `Event::key` | 逻辑键回读 |
| `Event::mods` | 修饰位回读 |
| `Event::new_with_alloc` | 自定义分配器创建事件 |
| `Event::set_composing` | 标记组字中 |
| `Event::unshifted_codepoint` | 无修饰码点回读 |
| `Event::utf8` | 文本回读 |

### 3.3 `mouse`（16 项）

鼠标编码器的全部选项同样由 `set_options_from_terminal` 一次同步。

| 项 | 说明 |
| --- | --- |
| `Encoder` | 按键编码器（类型本身） |
| `Event` | 按键/鼠标事件（类型本身） |
| `Format` | 输出格式枚举 |
| `Shape` | OSC 22 鼠标形状枚举 |
| `TrackingMode` | 鼠标跟踪模式枚举 |
| `Encoder::encode` | 编码并写入 `Write` |
| `Encoder::new_with_alloc` | 自定义分配器创建编码器 |
| `Encoder::reset` | 重置编码器状态 |
| `Encoder::set_format` | 单独设上报格式 |
| `Encoder::set_track_last_cell` | 单独设末格跟踪 |
| `Encoder::set_tracking_mode` | 单独设跟踪模式 |
| `Event::action` | 动作回读 |
| `Event::button` | 按键回读 |
| `Event::mods` | 修饰位回读 |
| `Event::new_with_alloc` | 自定义分配器创建事件 |
| `Event::position` | 位置回读 |

### 3.4 `paste`（8 项）

终端驱动的粘贴带确认对话框流程，PROHIBITED 明禁，故只用底层的 `encode`。

| 项 | 说明 |
| --- | --- |
| `Options` | 粘贴选项 |
| `Source` | 粘贴来源枚举 |
| `is_safe` | 粘贴内容安全性预检 |
| `Options::new` | 构造粘贴选项 |
| `Options::with_allow_unsafe` | 允许不安全的粘贴内容 |
| `Options::with_location` | 设目标选择器 |
| `Options::with_source` | 设来源 |
| `Terminal::paste` | 终端驱动的粘贴（拒绝流程需确认对话框，PROHIBITED 明禁） |

### 3.5 `render`（37 项）

行级脏判定（`RowIteration::dirty`）与快照已消费标记（`Snapshot::clean`）已接线（见 §2.5），此表不再含这两项。剩余未用项是完整渲染器才需要的附加段：分段更新（`begin_update`/`Update::end`）服务「解析与渲染并发消费同一快照」的嵌壳方，本仓 VT 线程独占解析与抽取，无并发消费者；`next_dirty`/`set_dirty` 假设消费者按脏行跳跃推进，与本仓行缓存按行索引对齐（`next()` 保持全行序列）不匹配；过扫（`Overscan`）服务像素级滚动的行边界补齐，本仓滚动按整行推进。

| 项 | 说明 |
| --- | --- |
| `CellIteration` | 单元迭代句柄 |
| `Colors` | 调色板快照 |
| `Cursor` | 光标（位置/可见性/样式/颜色） |
| `Dirty` | 脏标记枚举 |
| `Overscan` | 过扫行数值 |
| `RowId` | 行标识类型 |
| `RowIteration` | 行迭代句柄 |
| `Update` | 分段更新句柄 |
| `CellIteration::graphemes_buf` | 字素簇写入定长缓冲 |
| `CellIteration::graphemes_len` | 字素簇码位数 |
| `CellIteration::graphemes_utf8` | 字素簇的 UTF-8 形式 |
| `CellIteration::has_styling` | 是否有非默认样式 |
| `CellIteration::is_selected` | 是否选中 |
| `CellIteration::select` | 选中该单元 |
| `CellIterator::new_with_alloc` | 自定义分配器创建 |
| `RenderState::begin_update` | 开始分段更新 |
| `RenderState::new_with_alloc` | 自定义分配器创建 |
| `RenderState::overscan_request` | 查询建议过扫行数 |
| `RenderState::set_overscan` | 设置过扫行数 |
| `RowIteration::cells_raw` | 整行单元原始切片 |
| `RowIteration::id` | 行标识 |
| `RowIteration::next_dirty` | 下一个脏行 |
| `RowIteration::raw_row` | 原始行 |
| `RowIteration::set_dirty` | 置脏 |
| `RowIteration::viewport_y` | 视口内行号 |
| `RowIterator::new_with_alloc` | 自定义分配器创建 |
| `Snapshot::colors` | 调色板快照 |
| `Snapshot::cols` | 快照列数 |
| `Snapshot::cursor` | 光标结构体 |
| `Snapshot::cursor_blinking` | 光标是否闪烁 |
| `Snapshot::cursor_color` | 光标色 |
| `Snapshot::cursor_password_input` | 是否密码输入模式 |
| `Snapshot::dirty` | 脏标记 |
| `Snapshot::overscan` | 快照过扫行数 |
| `Snapshot::rows` | 快照行数 |
| `Snapshot::set_dirty` | 置脏 |
| `Update::end` | 结束分段更新 |

### 3.6 `screen`（26 项）

单元与样式经 `render::CellIteration` 读取，其 `raw_cell` 正是 `screen::Cell`；原始行与跟踪引用无调用方。

| 项 | 说明 |
| --- | --- |
| `CellContentTag` | 单元内容标签枚举 |
| `CellSemanticContent` | 单元语义内容枚举 |
| `Row` | 原始行 |
| `RowSemanticPrompt` | 行语义提示枚举 |
| `TrackedGridRef` | 跨滚动跟踪格引用 |
| `Cell::bg_color_palette` | 背景色的调色板读法 |
| `Cell::bg_color_rgb` | 背景色的 RGB 读法 |
| `Cell::content_tag` | 内容标签 |
| `Cell::has_styling` | 是否含非默认样式 |
| `Cell::has_text` | 是否含文本 |
| `Cell::is_protected` | 是否受保护（DECSED/DECSEL 不擦除） |
| `Cell::semantic_content` | 语义内容 |
| `GridRef::graphemes` | 该格字素簇 |
| `GridRef::row` | 取该格所在原始行 |
| `Row::has_grapheme_cluster` | 含字素簇 |
| `Row::has_hyperlink` | 含超链接 |
| `Row::has_kitty_virtual_placeholder` | 含 Kitty 虚拟占位 |
| `Row::is_dirty` | 行是否脏 |
| `Row::is_styled` | 含非默认样式 |
| `Row::is_wrap_continuation` | 软换行续行 |
| `Row::is_wrapped` | 软换行起始行 |
| `Row::semantic_prompt` | 行语义提示 |
| `TrackedGridRef::has_value` | 跟踪是否仍有效 |
| `TrackedGridRef::point` | 取当前坐标 |
| `TrackedGridRef::set` | 重设跟踪点 |
| `TrackedGridRef::snapshot` | 取当前码点 |

### 3.7 `search`（11 项）

`run` 已封装 `feed`+`tick`，滚动由选中命中时上游自动完成，全部命中也够用。

| 项 | 说明 |
| --- | --- |
| `Matches` | 命中集合类型 |
| `Scroll` | 滚动策略枚举 |
| `Status` | 搜索状态枚举 |
| `Search::feed` | 分步喂入内容 |
| `Search::needle` | 当前 needle 回读 |
| `Search::new_with_alloc` | 自定义分配器创建 |
| `Search::scroll` | 滚动策略回读 |
| `Search::set_scroll` | 手动设滚动策略 |
| `Search::status` | 搜索状态机状态 |
| `Search::tick` | 分步推进搜索 |
| `Snapshot::matches` | 快照内全部命中 |

### 3.8 `selection`（26 项）

长按只需词选与全选两类派生，行选/输出选/词间选与格式化家族无调用方。

| 项 | 说明 |
| --- | --- |
| `Adjustment` | 调整方向枚举 |
| `FormatOptions` | 选区格式化选项 |
| `SelectLineOptions` | 行选选项 |
| `SelectWordBetweenOptions` | 词间选选项 |
| `gesture` | 手势子模块（见 selection::gesture 一节） |
| `FormatOptions::new` | 构造格式化选项 |
| `FormatOptions::with_emit_format` | 附加格式信息 |
| `FormatOptions::with_selection` | 绑定选区 |
| `FormatOptions::with_trim` | 去尾部空白 |
| `FormatOptions::with_unwrap` | 拼接软换行行 |
| `SelectLineOptions::new` | 构造行选选项 |
| `SelectLineOptions::with_semantic_prompt_boundary` | 行选是否以语义提示为界 |
| `SelectLineOptions::with_whitespace` | 行选是否含空白行 |
| `SelectWordBetweenOptions::new` | 构造词间选选项 |
| `SelectWordBetweenOptions::with_boundary_codepoints` | 自定义词间边界码位 |
| `SelectWordOptions::with_boundary_codepoints` | 自定义词边界码位（用上游默认） |
| `Selection::adjust` | 按方向调整选区 |
| `Selection::contains` | 是否包含某点 |
| `Selection::equals` | 是否与另一选区相等 |
| `Selection::is_rectangle` | 是否矩形选区 |
| `Selection::order` | 取当前方向 |
| `Terminal::format_selection_alloc` | 格式化选区为字节 |
| `Terminal::format_selection_buf` | 格式化选区入定长缓冲 |
| `Terminal::select_line` | 按行派生选区 |
| `Terminal::select_output` | 按命令输出派生选区 |
| `Terminal::select_word_between` | 按词间边界派生选区 |

### 3.9 `style`（18 项）

主题构建期从 alacritty-theme 仓库读取色值，X11 颜色名解析与掩码无需上游参与。

| 项 | 说明 |
| --- | --- |
| `PaletteMask` | 调色板掩码 |
| `X11ColorName` | 单个 X11 颜色名 |
| `X11ColorNames` | X11 颜色名表 |
| `X11ColorNamesIter` | X11 颜色名迭代器 |
| `Palette::generate` | 生成默认调色板 |
| `Palette::parse_palette_entry` | 解析单个调色板项 |
| `PaletteMask::is_set` | 该索引是否已置位 |
| `PaletteMask::new` | 构造掩码 |
| `PaletteMask::set` | 置位 |
| `RgbColor::contrast` | 两色对比度 |
| `RgbColor::luminance` | 相对亮度 |
| `RgbColor::parse_x11_color` | 解析 X11 颜色名 |
| `Style::is_default` | 是否默认样式 |
| `X11ColorNames::get` | 按下标取 |
| `X11ColorNames::is_empty` | 表是否空 |
| `X11ColorNames::iter` | 迭代 |
| `X11ColorNames::len` | 表长 |
| `X11ColorNames::new` | 构造颜色名表 |

### 3.10 `fmt`（18 项）

纯文本提取只需 `unwrap`/`trim`/`selection` 三个选项。

| 项 | 说明 |
| --- | --- |
| `Format` | 输出格式枚举 |
| `Formatter::format_buf` | 格式化写入定长缓冲 |
| `Formatter::format_len` | 仅算格式化后长度 |
| `Formatter::format_write` | 格式化写入 `Write` |
| `Formatter::new_with_alloc` | 自定义分配器创建 |
| `FormatterOptions::with_charsets` | 附带字符集 |
| `FormatterOptions::with_cursor` | 附带光标样式 |
| `FormatterOptions::with_format` | 设输出格式 |
| `FormatterOptions::with_hyperlink` | 附带超链接 |
| `FormatterOptions::with_keyboard` | 附带键盘协议 |
| `FormatterOptions::with_kitty_keyboard` | 附带 kitty 键盘标志 |
| `FormatterOptions::with_modes` | 附带模式列表 |
| `FormatterOptions::with_palette` | 附带调色板 |
| `FormatterOptions::with_protection` | 附带保护状态 |
| `FormatterOptions::with_pwd` | 附带工作目录 |
| `FormatterOptions::with_scrolling_region` | 附带滚动区域 |
| `FormatterOptions::with_style` | 附带默认样式 |
| `FormatterOptions::with_tabstops` | 附带制表位 |

### 3.11 `kitty::graphics`（34 项）

图像只可能来自 PTY 字节流内的 APC 载荷，三条读取路径开关（文件、共享内存、临时目录）保持上游默认「全部禁止」；分项几何读取由 `placement_render_info` 一次取全替代。

| 项 | 说明 |
| --- | --- |
| `Compression` | 图像压缩枚举 |
| `DecodePng` | 自定义 PNG 解码器 trait |
| `DecodedImage` | 解码结果（尺寸 + RGBA） |
| `GridSize` | 网格尺寸类型 |
| `Layer` | 图像分层枚举 |
| `PixelSize` | 像素尺寸类型 |
| `SourceRect` | 源矩形类型 |
| `ViewportPos` | 视口位置类型 |
| `Image::compression` | 图像压缩方式 |
| `Image::generation` | 图像生成戳 |
| `Image::id` | 图像号 |
| `Image::number` | 显示序号 |
| `PlacementIteration::columns` | 占列数 |
| `PlacementIteration::grid_size` | 网格尺寸 |
| `PlacementIteration::pixel_size` | 像素尺寸 |
| `PlacementIteration::placement_id` | 放置号 |
| `PlacementIteration::rect` | 完整矩形 |
| `PlacementIteration::rows` | 占行数 |
| `PlacementIteration::set_layer` | 设分层 |
| `PlacementIteration::source_height` | 源矩形高 |
| `PlacementIteration::source_rect` | 源矩形 |
| `PlacementIteration::source_width` | 源矩形宽 |
| `PlacementIteration::source_x` | 源矩形 x |
| `PlacementIteration::source_y` | 源矩形 y |
| `PlacementIteration::viewport_pos` | 视口位置 |
| `PlacementIterator::new_with_alloc` | 自定义分配器创建 |
| `Terminal::is_kitty_image_from_file_allowed` | 是否允许从文件读图像 |
| `Terminal::is_kitty_image_from_shared_mem_allowed` | 是否允许从共享内存读图像 |
| `Terminal::kitty_image_storage_limit` | 图像存储上限读取 |
| `Terminal::kitty_image_temp_file_dir` | 图像临时文件目录 |
| `Terminal::set_apc_max_bytes_kitty` | Kitty 的 APC 上限 |
| `Terminal::set_kitty_image_from_file_allowed` | 设置允许从文件读图像 |
| `Terminal::set_kitty_image_from_shared_mem_allowed` | 设置允许从共享内存读图像 |
| `Terminal::set_kitty_image_temp_file_dir` | 设置图像临时文件目录 |

### 3.12 `alloc`（5 项）

默认分配器满足单终端进程。

| 项 | 说明 |
| --- | --- |
| `Allocator` | 自定义分配器句柄 |
| `Bytes` | 分配器产出的字节缓冲 |
| `Allocator::GLOBAL` | 全局分配器常量 |
| `Bytes::new` | 按长度分配字节缓冲 |
| `Bytes::new_with_alloc` | 自定义分配器分配字节缓冲 |

### 3.13 `build_info`（11 项）

本项目不需要向上层报告上游构建能力。

| 项 | 说明 |
| --- | --- |
| `OptimizeMode` | 优化模式枚举 |
| `build_version` | 构建号 |
| `major_version` | 主版本号 |
| `minor_version` | 次版本号 |
| `optimize_mode` | 优化模式 |
| `patch_version` | 补丁版本号 |
| `pre_version` | 预发布标识 |
| `supports_kitty_graphics` | 是否编译进 Kitty 图像 |
| `supports_simd` | 是否启用 SIMD |
| `supports_tmux_control_mode` | 是否编译进 tmux 控制模式 |
| `version_string` | 完整版本串 |

### 3.14 `error`（1 项）

`Result<T>` 别名未直接书写，错误一律写作 `Error`。

| 项 | 说明 |
| --- | --- |
| `Result` | `Result<T, Error>` 别名 |

### 3.15 `focus`（2 项）

焦点上报序列必须直写子 PTY，送入 VT 引擎会被当作 CHT（光标水平制表）。序列本身是定长三字节（模式 1004 规范的 CSI I / CSI O），`session.rs` 的 `focus_event_bytes` 以字面量直写并经测试钉住（模式 1004 门控）；`Event::encode` 编码同一字节且多一次 FFI 往返，无净收益。

| 项 | 说明 |
| --- | --- |
| `Event` | 按键/鼠标事件（类型本身） |
| `Event::encode` | 把焦点事件编码为转义序列 |

### 3.16 `kitty`（2 项）

`graphics` 子模块本身与其 `Graphics` 再导出；实际使用 `kitty::graphics::*` 路径。

| 项 | 说明 |
| --- | --- |
| `graphics` | Kitty 图像子模块 |
| `graphics::Graphics` | `Graphics` 的再导出 |

### 3.17 `log`（2 项）

本仓有自己的 logcat sink，不需要上游的 stderr 与 tracing 回调。

| 项 | 说明 |
| --- | --- |
| `LogStderr` | 内置 stderr 日志回调 |
| `TracingLogger` | `tracing` 适配回调 |

### 3.18 `osc`（9 项）

剪贴板写与读走上游回调，其余 OSC 段由 VT 引擎自行消化。

| 项 | 说明 |
| --- | --- |
| `Command` | 解析出的 OSC 命令 |
| `CommandType` | OSC 命令类型枚举 |
| `Parser` | 解析器（类型本身） |
| `Command::command_type` | 命令类型 |
| `Parser::end` | 结束一次 OSC 解析并取命令 |
| `Parser::new` | 创建解析器 |
| `Parser::new_with_alloc` | 自定义分配器创建解析器 |
| `Parser::next_byte` | 逐字节喂入 OSC 解析 |
| `Parser::reset` | 重置 SGR 解析器 |

### 3.19 `sgr`（8 项）

SGR 属性经 `render::CellIteration::style` 直读。

| 项 | 说明 |
| --- | --- |
| `Attribute` | 解析出的 SGR 属性 |
| `Parser` | 解析器（类型本身） |
| `Unknown` | 未识别的 SGR 属性 |
| `Parser::new` | 创建解析器 |
| `Parser::new_with_alloc` | 自定义分配器创建解析器 |
| `Parser::next` | 取出下一个 SGR 属性 |
| `Parser::reset` | 重置 SGR 解析器 |
| `Parser::set_params` | 设置参数 |

### 3.20 `snapshot`（27 项）

会话数据持久化在 PROHIBITED 中禁止，快照编解码无用武之地。

| 项 | 说明 |
| --- | --- |
| `Decoder` | 快照解码器 |
| `IncrementalDecoder` | 增量解码器 |
| `Progress` | 解码进度 |
| `Decoder::decode` | 喂入字节解码 |
| `Decoder::history_rows_alternate` | 备用屏历史行数 |
| `Decoder::history_rows_primary` | 主屏历史行数 |
| `Decoder::max_continuation_bytes` | 续传上限 |
| `Decoder::new` | 创建解码器 |
| `Decoder::new_buf` | 以定长缓冲创建 |
| `Decoder::new_buf_with_alloc` | 定长缓冲 + 自定义分配器 |
| `Decoder::new_with_alloc` | 自定义分配器创建 |
| `Decoder::ready` | 是否已就绪出终端 |
| `Decoder::retain_continuation` | 是否保留续传数据 |
| `Decoder::set_max_continuation_bytes` | 设续传上限 |
| `Decoder::set_retain_continuation` | 设是否保留续传 |
| `Decoder::source_offset` | 已消费的源字节偏移 |
| `IncrementalDecoder::into_terminal` | 取出终端 |
| `IncrementalDecoder::next` | 推进一步 |
| `IncrementalDecoder::terminal` | 取终端 |
| `IncrementalDecoder::terminal_mut` | 取可变终端 |
| `Progress::as_decoder` | 转回解码器 |
| `Progress::remaining` | 剩余字节 |
| `Progress::rows` | 已恢复行数 |
| `Progress::screen` | 当前屏 |
| `Terminal::encode_snapshot` | 把终端编码为快照字节 |
| `Terminal::encode_snapshot_alloc` | 编码快照为分配字节 |
| `Terminal::encode_snapshot_buf` | 编码快照入定长缓冲 |

### 3.21 `unicode`（2 项）

单元宽度由 `Cell::wide()` 给出，字形宽度由字体管线负责。

| 项 | 说明 |
| --- | --- |
| `codepoint_width` | 码点显示宽度 |
| `grapheme_width` | 字素簇显示宽度 |

### 3.22 `selection::gesture`（50 项）

本项目手势由视图侧自建：连击语义与禁止双击三击多击冲突，边缘滚动节奏与上游定时自动滚动冲突（依据见 §5）。宽字符吸附的事实取自上游网格单元宽度，交叉交换归属对标 termlib 的 `applyHandleDrag`，逐行滚动为自建策略。

| 项 | 说明 |
| --- | --- |
| `Autoscroll` | 自动滚动状态枚举 |
| `AutoscrollTickEvent` | 自动滚动计时事件 |
| `Behavior` | 手势行为枚举 |
| `Behaviors` | 手势行为表 |
| `DeepPressEvent` | 深按事件 |
| `DragEvent` | 拖动事件 |
| `Geometry` | 手势几何再导出 |
| `Gesture` | 手势状态机 |
| `PressEvent` | 按下事件 |
| `ReleaseEvent` | 释放事件 |
| `AutoscrollTickEvent::apply` | 应用到选区 |
| `AutoscrollTickEvent::new` | 构造计时事件 |
| `AutoscrollTickEvent::new_with_alloc` | 自定义分配器创建 |
| `AutoscrollTickEvent::set_position` | 设位置 |
| `AutoscrollTickEvent::set_rectangle` | 设可视矩形 |
| `AutoscrollTickEvent::set_word_boundary_codepoints` | 设词边界码位 |
| `Behaviors::new` | 构造行为表 |
| `Behaviors::with_double_click_behavior` | 设双击行为 |
| `Behaviors::with_single_click_behavior` | 设单击行为 |
| `Behaviors::with_triple_click_behavior` | 设三击行为 |
| `DeepPressEvent::apply` | 应用到选区 |
| `DeepPressEvent::new` | 构造深按事件 |
| `DeepPressEvent::new_with_alloc` | 自定义分配器创建 |
| `DeepPressEvent::set_word_boundary_codepoints` | 设词边界码位 |
| `DragEvent::apply` | 应用到选区 |
| `DragEvent::new` | 构造拖动事件 |
| `DragEvent::new_with_alloc` | 自定义分配器创建 |
| `DragEvent::set_position` | 设位置 |
| `DragEvent::set_rectangle` | 设可视矩形 |
| `DragEvent::set_word_boundary_codepoints` | 设词边界码位 |
| `Gesture::anchor` | 锚点 |
| `Gesture::autoscroll` | 自动滚动状态 |
| `Gesture::behavior` | 当前行为 |
| `Gesture::click_count` | 连击次数 |
| `Gesture::dragged` | 是否已拖动 |
| `Gesture::new` | 创建手势状态机 |
| `Gesture::new_with_alloc` | 自定义分配器创建 |
| `Gesture::reset` | 重置手势状态 |
| `PressEvent::apply` | 应用到选区 |
| `PressEvent::new` | 构造按下事件 |
| `PressEvent::new_with_alloc` | 自定义分配器创建 |
| `PressEvent::set_behaviors` | 设行为表 |
| `PressEvent::set_position` | 设位置 |
| `PressEvent::set_repeat_distance` | 设重复距离阈值 |
| `PressEvent::set_repeat_interval` | 设重复间隔 |
| `PressEvent::set_time` | 设时间戳 |
| `PressEvent::set_word_boundary_codepoints` | 设词边界码位 |
| `ReleaseEvent::apply` | 应用到选区 |
| `ReleaseEvent::new` | 构造释放事件 |
| `ReleaseEvent::new_with_alloc` | 自定义分配器创建 |

## 4. 上游家族与替换性

`libghostty` 家族实测两层并存，本仓只用无头的那一层：

- **`libghostty-vt`**（本仓使用）：`GhosttyLibVt.zig` → `lib_vt.zig`，头
  `include/ghostty/vt*.h`，无头、跨平台（含 Android）、C ABI；本仓经
  `libghostty-rs` 使用，`native/Cargo.toml` 跟踪其 master 修订并开启
  `log`/`png`，无本地补丁。
- **完整 `libghostty`**：`GhosttyLib.zig` → `main_c.zig`，头 `include/ghostty.h`
  （`ghostty_app_t`/`config_t`/`surface_t`/`inspector_t`、剪贴板、检查器），
  胖静态库 + pkg-config，为移植「应用运行时」而设。其 `ghostty_platform_e`
  仅 `INVALID/MACOS/IOS`，**对 Android 无收益**，故不接入。

换核的可行性：本仓确切约束下无「更完整又能直接换」者（对照结论保留在
`openspec/specs/upstream-alignment`）。

## 5. 观察

- `Terminal::on_bell` 未接线。曾有 7 处注释描述一条不存在的 BEL 通道（`commands.rs` 的 `RunConfig`、`types.rs` 的 `EVENT_CHANNEL_CAPACITY`、`session.rs` 的 5 处收割路径文档），已随归档变更 `2026-10-12-audit-research-residual-gaps` 的任务 3.1/3.2 删除；这些位置现只描述 OSC 52 剪贴板写入。`session.rs:829` 的 `0x07` 是 OSC 52 应答的 **BEL 终止符**（兼容 xterm），是真实协议内容，不在其列。
- `selection::gesture` 一族不采用，理由不止连击计数：其自动滚动按定时器在边缘区持续滚动，与本仓「每次触点移动滚一行、静止不滚」冲突；其手势状态若由 VT 线程持有，则每个触摸帧都需跨线程往返，使拖动快路径的无跨语言调用优势尽失，并与视图侧手势所有权形成双状态机。词派生、跟踪选区、随滚动跟随与复制格式化仍走上游，见 §2.8。
- OSC 52 的读取请求（`?`）上游明确忽略，仍由本仓 `output_processor` 的最小扫描器拦截；因此 `on_clipboard_read` 与整个 `ClipboardRead` 族不必接线。
- 鼠标与键盘编码器的所有单选项 setter 都不单独调用，选项一律经 `set_options_from_terminal` 从终端整体同步，这是编码正确的唯一入口。
- 光标视觉样式取 `render::Snapshot::cursor_visual_style` 而非 `Terminal::cursor_style`，因为快照路径与 CellData 同源，两条路径不会分叉。
- 未使用的 515 项中，多数是上游为通用嵌壳方提供的可选能力（设备属性应答族、桌面通知、进度上报、快照持久化、手势状态机、自定义分配器）。剩余的配置型 setter 均为上游默认值即足够；需要限流或改协议行为时再启用，届时须先补 DESIGN 声明。
- 2026-10-20 全量复核：公开项总数 718 与源码精确吻合（含 `terminal.rs`/`render.rs` 各 2 个 `pub use` 再导出；`io.rs` 为 `pub(crate)` 不计入）；抽查已用项均有真实调用点、抽查未用项确实未接线，本清单分类可信。
- 同轮复核现有使用无错误：回滚须先 `set_scrollback_max_bytes(None)` 再设行数（上游字节预算默认先于行数触发），`internal.rs` 顺序正确；KGP 存储上限 64 MiB 恰为上游默认值；APC 载荷上游默认 kitty 65 MiB、glyph 1 MiB 且两协议默认全开，大图传输无隐藏截断；光标未知样式回退块、焦点序列直写 PTY 两处实现与文档论断一致。`Terminal::compress` 的条目说明原写成「丢弃最旧内容」，与上游「压缩只改存储表示、从不动逻辑内容或回滚上限」矛盾，已修正；该 API 未用合理——上游注明须与写入/渲染/搜索串行，本仓 2000 行预算内存可控。
- 二次复核（2026-10-20 晚）：上游 master 领先本仓钉住 rev 仅 1 个提交（2026-10-19，`libghostty-vt-sys` 构建脚本允许经 `ZIG` 环境变量选择 Zig 版本，只改 README 与 build.rs），不触碰 `crates/libghostty-vt/src`，公开面 718 项不变。本次修正两处分类滞后：`RowIteration::dirty` 与 `Snapshot::clean` 随行级脏跟踪接线已实际使用（`internal.rs` 的行级脏判定与整帧抽取后标记），自未用表移入 §2.5，计数订正为已用 203 / 未用 515。候选「可用未用」API 逐项核查（`on_title_changed`/`next_dirty`/`begin_update`/`Update::end`/`viewport_active`/`is_cursor_pending_wrap`/`vt_write_until_ground`/`is_vt_ground`/`track_grid_ref`/`overscan` 家族/`cursor_blinking`/`cursor_password_input`/`set_resize_pull_scrollback`）：生产代码零命中且各有不采用的架构理由（拉取模式、行缓存按行索引对齐、VT 线程独占解析、整行滚动），维持未用。`focus::Event::encode` 判定记入 §3.15：与手写字面量同字节，多一次 FFI 往返，无净收益。
- 三次复核（2026-10-20 深夜，逐项全覆盖）：已用 203 项逐一对照上游 doc comment 核查调用语义，**全部符合，无一使用不当**。逐项核实的关键约束：`set_png_decoder` 上游明文要求与 terminal 同线程，本仓在 `run_inner`（VT 线程）注册与 `vt_write` 同线程；`on_clipboard_write` 的 `reply` 义务两条路径都履行（成功 `Ok`、通道满 `Busy`），`try_send` 永不阻塞 VT 线程；`key::Event` 跨事件复用时 `set_unshifted_codepoint` 显式写 `'\0'` 防残留（上游无清零入口）、`set_consumed_mods` 避免文本消费后修饰位重复叠加；编码器选项一律每事件前 `set_options_from_terminal` 整体同步；`Snapshot::clean` 在整帧抽取完成后才调用；`set_logger` 实读上游源码为进程全局（`RwLock` + `sys_set`，非线程局部），本仓经 `log::logger()` 桥接与上游文档示例一致；`style_id` 行内连续同样式缓存、Kitty `generation()` 存储戳跳过、paste `OutOfSpace{required}` 精确扩容、`to_ordered(Forward)` 抹平拖动方向、search `MatchBuffer` 跨查询复用——均与上游语义一致。未用 515 项换角度全面复查（「本仓已有对应功能但自建」的交叉项逐一深挖）：光标色为本轮唯一新识别候选——上游 `Snapshot::cursor_color` 返回 OSC 12 运行时覆盖色，本仓 `build_cursor` 恒取主题 `palette.cursor`；判定为一致的设计立场而非遗漏：bg/fg/cursor 三色以主题为单一来源（`Terminal::bg_color`/`fg_color` 的 OSC 10/11 覆盖读取同样不消费），DESIGN 光标恒块状不闪烁，alacritty 主题文件自带 cursor 色且本仓解析链路完整。`selection_background` 由搜索高亮（`ffi.rs`）与选区反显（VT 烘焙进 CellData）消费，不经上游选区颜色路径，合理。`pending_wrap` 渲染、`on_enquiry`、OSC 12 消费均零命中，无功能缺口。结论：无应接线而漏接的 API，无需任何代码改动。
- 四次复核（2026-10-21，针对「未用比例异常」的构成分析）：515 项未用聚类为四类（见 §3 引言）——整域禁用 117（22.7%，对应 PROHIBITED/DESIGN 禁项或引擎已内化）、枚举常量族 84（16.3%，`Mode` 42 取 3、设备属性应答族 45，清单型常量按需取用）、替代表达 24（4.7%，`*_with_alloc`/`*_buf` 已用等价形态）、域内可选面 290（56.3%，完整桌面嵌壳方才需要），分模块账目自洽。对照证据：上游官方示例 `ghostling_rs`（约 1kLoC 完整终端）实读消费约 50 项（use 导入 27 类型 + 方法入口），且其消费的设备属性应答族恰是本仓判定不采用的域；本仓 203 项为其 4 倍。结论：72% 未用是通用 VT 引擎绑定库的正常形态，非异常信号。本轮逐家族「成对 API 只用一半」检查发现**一处使用不完整（唯一缺口）**：`Mode::SYNC_OUTPUT`（已用）与 `on_render_hold`（未用）成对只用一半——`internal.rs` 查询模式 2026 写入 `GridSnapshot.sync_active`，但该字段生产侧零消费（`take_snapshot` 生产调用为零、渲染主路径走 CellData 通道、渲染/JNI/Kotlin/tests 对 `sync_active` 全零命中），查询结果写后无人读，BSU/ESU 同步更新期间渲染无帧门控（可能呈现中间帧）；上游为这正是提供 `on_render_hold` 回调（`held: bool`，文档明言可自由忽略）。处置二选一待规格裁定：接线渲染门控，或移除该查询；两者都须先过 openspec 流程，本轮只记录不改代码。其余成对检查全部合适：`Selection::adjust` 未用合理（命令通道每事件携带完整坐标，`Selection::new` 重建 + `to_ordered` 比增量调整更直接）；glyph 协议维持默认全开合理（上游绑定层渲染面无 glossary 出口，本仓无消费路径，关闭亦无渲染差异）。
- 五次复核（2026-10-21，处置四次复核留下的唯一成对缺口）：不复用 `on_render_hold` 回调——本仓渲染由宿主线程驱动、VT 线程内没有可捕获的帧，回调语义无法完整复刻。改为在帧发布层门控：`internal.rs` 的 VT 循环在批次尾部（与 50ms 空闲 tick 两处发布点）查询 `Mode::SYNC_OUTPUT`，保持期间不构建、不发布 CellData，屏幕停留在此前的完整帧；ESU 的写入重新置脏并在同一批次发布最终帧。引擎没有时钟，故设 1s 超时（上游推荐值）：超时后 `set_mode(SYNC_OUTPUT, false)` 强制解锁并发布当前帧，避免程序崩溃导致画面永久冻结。`GridSnapshot.sync_active`（写后无人读）随之删除。回归测试 `synchronized_output_defers_frame_until_release` 与 `synchronized_output_hold_times_out_without_release` 钉住两条语义；`on_render_hold` 保持未用，但不再构成功能缺口。
