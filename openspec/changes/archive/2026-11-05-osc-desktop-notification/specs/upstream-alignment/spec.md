## MODIFIED Requirements

### Requirement: 上游 API 取用判定登记

本仓以 Ghostty 为终端状态单一来源：上游已有的能力 MUST 直接取用，MUST NOT 重实现；
上游有而本仓不用的能力 MUST 在此登记「不采用」的 DESIGN 依据或「待办」的阻塞点，
MUST NOT 留空使后来者重复调研或把合规分叉移植回来。每项 MUST 给可复核依据
（上游源码位置、本仓文件行号或 DESIGN 条款）。

#### Scenario: 结论可脱离审计报告独立复核

- **WHEN** 复查某项上游 API 的取用与否
- **THEN** 该项结论与依据 MUST 能在本规范内读到，MUST NOT 需要先翻审计报告或归档变更

上游 API 取用判定如下（上游位置均指 `Cargo.toml` 所钉 rev 的 `crates/libghostty-vt/src`）。

| 上游能力 | 结论 | 可复核依据 |
| --- | --- | --- |
| `terminal.rs`：`on_clipboard_write` 必须 `reply` | 采用（已修） | 上游文档「返回前必须 `reply`，否则拒绝写入」；本仓 `internal.rs` 回调此前零 `reply`。现满通道回 `Busy` 否则 `Ok` |
| `style::Underline` 五形状判定 | 采用（已修） | 三处 `matches!` 收敛为 `has_visible_underline`/`is_double_underline` 同源函数 |
| `Terminal::vt_write` 二进制安全 | 采用（已修） | `pty_write` 曾把 `>0xF7` 改空格，破坏 Kitty 直接 RGB 等二进制载荷；现仅剔除 NUL，与 `vt_write` 同策略 |
| 调色板与字节转浮点 | 采用（已修） | 渲染侧 `CATPPUCCIN_MOCHA_BACKGROUND` 与手写除法已删，以 `theme` 单源（构建期 alacritty-theme）为唯一入口，按标识取用（`theme::default_dark`／`theme::by_name`）；中间 epoch 用过的 `GhosttyTerminal::default_palette` 已随「主题改按标识在原生单点解析」删除 |
| `selection` 交叉翻转归属 | 采用（已修） | 对标 termlib `applyHandleDrag` 五元组：回传新归属柄并收紧等值判据，调用方更新 `handleDragState` |
| `search::Search` 全文搜索 | 采用（2026 改判） | 自研 `regex` 搜索曾被判定合规，实为重复实现：软换行跨行、随输出增量同步、命中总数与导航滚动上游全已提供。现 VT 线程持有 `search::Search`（同生命周期的局部变量，非自引用结构），needle/总数/导航/视口命中均走上游，本仓只做「上游选区 → 按物理行网格区间」转换。代价：上游仅 ASCII 折叠、无区分大小写开关，`DESIGN.md` 修饰键栏节原「大小写匹配」与「使用库（如 `regex`）实现文本搜索」两条**已由维护者改写**（删去大小写匹配按钮、改写为使用 libghostty-vt Search，与上游口径一致）；`regex` 依赖随之删除。软换行/宽字符/CJK/真实空格的用例语义按上游口径重写，见 `text-search` 规约 |
| `selection/gesture` 手势、`select_line`/`select_output`、`format_selection_*` | 不采用 | 连击语义与 DESIGN.md 终端页面节的「不得支持 双击 三击 多击选择」「不得支持 双击 三击 多击选择」正面冲突，该族主体（拖动手势 + 边缘自动滚动）另有两条理由：其自动滚动按定时器在边缘区持续滚动，与 `text-selection` 的 Scenario「下缘拖动逐行滚动」（每次触点移动视口滚动 1 行）与「手指静止不滚动」相反；其手势状态若由 VT 线程持有，则每个触摸帧一次跨线程往返，使拖动快路径的无跨语言调用优势尽失，并与视图侧手势所有权形成双状态机。四项上游优势的覆盖情况：词派生、跟踪选区、随滚动跟随、复制格式化已走上游；宽字符吸附的事实取自上游网格单元宽度；边缘滚动节奏不采纳；单一来源已由 `terminal.rs` 承担。 |
| termlib `trimDetectedUrl`/括号计数/跨行拼接 | 不采用 | DESIGN「链接识别通过 `libghostty-vt` 查询，没有额外检查」即禁止移植；仅保留空白 trim 与协议白名单（属安全检查） |
| `on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report` 等回调 | 不采用 | Kotlin 侧零消费者；DESIGN「不做不必要功能」，有消费者时再注册。`on_bell` 曾被 7 处注释误记为已接线（`ghostty_terminal/commands.rs` 的 `RunConfig` 悬空附着于 `cell_size_px`、`types.rs` 的 `EVENT_CHANNEL_CAPACITY`、以及 `session/` 的 5 处收割路径文档），实际该通道只承载 OSC 52 剪贴板写入；注释已删除。`on_bell` 与 `on_desktop_notification` 现已单独接线，见下两行 |
| `Terminal::on_bell`（BEL 回调） | 采用（2026-11-03 改判） | `DESIGN.md` Shell 节要求 `bell-character` 三态与 termux-app 一致，响铃事件必须有来源。回调只置无锁标志（`ghostty_terminal/internal/run.rs` 的 `on_bell` 注册），每帧收割为 `Event::Bell { session_id }`（`android/ffi/render/pump.rs`），Kotlin 按 `bell-character` 三态限频执行（`runtime/BellPlayer.kt`），限频窗口内的重复响铃静默丢弃 |
| `Terminal::on_desktop_notification`（OSC 9/777 回调） | 采用（2026-11-05 改判） | `DESIGN.md` 软件节要求处理 OSC 9 等通知（前台 Toast、后台系统通知、点击切换会话），通知必须有来源。回调注册在 `ghostty_terminal/internal/run.rs`；负载是程序给出的原始字节（上游文档写明不保证 UTF-8），按替换字符口径解码后按会话归属投递；Kotlin 侧按应用前后台状态分派（`runtime/DesktopNotificationPlayer.kt`），规约见 `desktop-notification` |
| `track_grid_ref`、`width_px`/`height_px`、光标色实时查询、`set_mode`、`vt_write_until_ground` | 不采用 | 按需取用；现有轮询与原子镜像满足当前消费者，DESIGN 要求最小实现 |
| `key::Encoder` 按键编码 | 采用（已修） | Android 键码到上游 `Key` 为机械对应表（`keymap.rs`），桥形态与 `Query::EncodeMouseEvent` 同构（编码在 VT 线程）；`TerminalInputEncoder` 仅保留输入法文本路径 |
| `Terminal::paste` 粘贴 | 不采用 | 含换行文本返回 `Error::Rejected` 并要求确认框后 `allow_unsafe` 重试，PROHIBITED 明禁粘贴确认对话框；恒置 `allow_unsafe` 则检查失去意义 |
| `paste::encode` 粘贴编码 | 采用（已修） | 无终端依赖的纯函数：bracketed 标志取活体模式 2004，控制字节中化与括号包裹由上游承担；`encodePaste` 查询跑在 VT 线程，Kotlin 只做截断与字节流切块 |
| `paste::is_safe` | 不采用 | 其拒绝流程同样要求确认对话框，PROHIBITED 明禁 |
| `device_attributes` 回调 | 不采用 | 缺省 trampoline 已按默认属性应答（实测 DA1=`?62;22c`，`test_output_capture_da` 钉住）；自定义 132 列等属性需先验证 DECCOLM 通路，暂无消费者要求 |
| `xtversion` 回调 | 不采用 | 缺省回退 `libghostty` 已应答；自定义名字无功能增益 |
| `color_scheme` 回调 | 不采用 | 注册 None 等价于未注册；无 OS 级来源，不猜 |
| `set_terminfo_name` | 不采用 | 未设置则 TN 查询不报告；声称值需先审计能力一致性，暂无消费者要求 |
| Kitty 文件/共享内存介质 | 不采用 | 直传已覆盖主路径；文件介质是新能力面（沙箱路径裁决），需产品确认，另立变更 |
| `kitty::graphics::RustPngDecoder` | 采用（已修） | 本仓原先自写 60 行 `KittyPngDecoder`，注释称「上游 RustPngDecoder 无公开构造器」——该结论与上游源码不符：`pub const fn new()` 存在，且本仓已开 `png` 特性。自写版按未经上限校验的 PNG 头尺寸在全局堆分配，绕过的正是上游分配器上限 |
| `style::RgbColor::parse` / `perceived_luminance` | 采用（已修） | 主题色值解析与深浅分组原为本仓手写（`#RGB` 展开、Rec.601 加权 0.299/0.587/0.114）；两者都取自上游，阈值语义与 ghostty 一致 |
| `on_clipboard_read` | 不采用 | 上游要求在回调内同步应答（VT 流暂停）。本仓的应答链必须异步：剪贴板内容来自 Android 系统剪贴板，经 `pollEvent` 上报后再回写 PTY；若改走上游回调，VT 线程要阻塞到 Kotlin 应答，终端在应答期间冻结。识别仍需自研扫描（上游回调装上后才拦截得到），故净收益为负 |
| `fmt::Formatter` 取全量纯文本 | 不采用 | 文档写明只覆盖 active screen，而本仓 `ReadAllText` 的口径是「回滚 + 视口」；上游补回滚参数后才可能跟进 |

### Requirement: 六大上游对标结论按「能力 → 本仓现状 → 差距 → 阻塞层」登记

对标对象固定为六个：`sylirre/ghostty-android-terminal`、`termux/termux-app`、
`mrndstvndv/termux-ghostty`、`connectbot/termlib`、`libtermux/libtermux-android`、
`Uzaaft/libghostty-rs`（后者以 `Cargo.toml` 所钉 rev 为准）。每项 MUST 记录上游的
**实测做法**（文件与符号级），本仓现状，差距，以及差距可修的层；MUST NOT 以
「上游更好」这类概括代替定位。

#### Scenario: 复查结论可逐项复核

- **WHEN** 读到某项对标结论
- **THEN** 该项的上游做法、本仓现状、差距与阻塞层都能在表内读到，不需重新克隆上游

2026-10-12 复查台账（上游均取当日 `main`/master 浅克隆）：

| 能力 | 上游实测做法 | 本仓现状 | 差距 | 阻塞层 |
| --- | --- | --- | --- | --- |
| 全文搜索实现 | `ghostty-android-terminal` 在 `app/src/main/cpp/terminal_jni.c` 自研：`run_search` 全屏扫描、`search_cs` 保留区分大小写、`case_fold.h` 生成 BMP 折叠表、`search_dirty` 触发重扫、`MAX_MATCHES` 封顶；其注释写明「libghostty-vt has no built-in search」（所钉版本早于上游引入 search） | 走上游 `search::Search`（本规范「采用（2026 改判）」行） | 区分大小写：上游仅 ASCII 折叠、无开关 | 上游（`SearchOption` 只有 NEEDLE/SELECT_*，需上游新增才可能跟进） |
| 搜索命中高亮 | 同上：当前命中装为终端选区（`search reuses the selection slot`），高亮走选区通道 | 独立 `render_state.search_highlights` 通道，由原生按上游命中直接生成 | 多一条高亮通道 | 本仓（可选：复用选区槽可再删通道，代价是命中与选区互斥） |
| 搜索导航上下文 | 同上：导航从 Java 侧重跑扫描，陈旧屏幕坐标不需跟踪引用 | 导航走上游 `select_*`，视图侧把命中行居中 | 无 | 无 |
| 格子数据跨语言传输 | `termux-ghostty` 把脏行序列化为直连 JNI `ByteBuffer`（UTF-16 长度/折行标志/连续 cell 起点长度宽度/u64 样式/UTF-16 字符，8 字节对齐） | 网格数据全程在 Rust 内（最新帧邮箱 + 脏跟踪），不经 JNI | 不适用 | 无（我们的路径本就不跨 JNI，该优化无对象） |
| 修饰键栏 | `termux-app` 的 `termux-shared/.../extrakeys/`（`ExtraKeysInfo`/`ExtraKeysView`/`ExtraKeysConstants`/`SpecialButton`）含布局编辑器与按键配置 | 仅采纳默认布局与粘滞语义 | 布局编辑器 | PROHIBITED（明禁） |
| 选区拖动手柄 | `termlib` 的 `applyHandleDrag` 锚点语义 + 交叉翻转 | 已采纳（同源实现与单测） | 无 | 无 |
| Linux 用户态 SDK | `libtermux-android` 提供 `core`/`os`/`terminal-view`/`bootstrap-*`/`shizuku` 模块（README 自述实验阶段） | 自持 bootstrap 安装与终端视图 | Shizuku 集成 | PROHIBITED（明禁）；其余能力已由本仓自实现覆盖 |
| 其余 libghostty-rs 能力 | `gesture`/`select_output`/`format_selection_*`/`snapshot decoder`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report`/`device_attributes`/`xtversion`/`color_scheme`/`set_terminfo_name`/`track_grid_ref`/`width_px`/`height_px` | 均未采纳 | 无消费者 | 本仓（按需取用，见「上游 API 取用判定」表） |
| 桌面通知回调 | `terminal.rs:3475` 的 `on_desktop_notification` 经 C ABI 的 `GhosttyTerminalDesktopNotification` 回调暴露；上游 `osc9.zig` 与 `rxvt_extension.zig` 分别把 OSC 9 与 OSC 777 的 `notify` 归到同一条 `show_desktop_notification` 命令 | 已采纳（2026-11-05 改判）：注册上游回调，通知按会话归属投递，前台 Toast、后台系统通知、点击切换会话 | 无 | 无 |
| Kitty 图像 PNG 解码 | `kitty/graphics.rs:997` 的 `RustPngDecoder`（`pub const fn new`）经 `set_png_decoder` 注册；解码前先按 libghostty 分配器取定长缓冲，未受该上限约束的尺寸不会被 PNG 头放大 | 已采纳（本次改判）：删除本仓 60 行等价实现与 `png` 直接依赖 | 无 | 无 |
| 颜色语法与深浅判定 | `style.rs:390` `RgbColor::parse`（3/6/9/12 位 hex、X11 名、`rgb:`/`rgbi:`）与 `style.rs:415` `perceived_luminance`（文档写明「背景亮度超过 0.5 即视为浅色」） | 已采纳：主题色值与深浅分组都走上游，本仓不再手写 `#RGB` 展开与 Rec.601 公式 | 无 | 无 |
| 终端模式查询 | `terminal.rs:1380` `Mode::BRACKETED_PASTE`、`terminal.rs:1367` `Mode::FOCUS_EVENT` 具名常量 | 已采纳：`Query::ModeGet` 携带上游 `Mode`，删掉 `(u16, u8)` 裸号与各调用点的数字 | 无 | 无 |
| `fmt::Formatter` 全量纯文本 | `fmt.rs:163` `Formatter::new(terminal, opts)` 文档写明「a terminal's **active screen**」，只覆盖活动屏 | 不采纳：本仓的 `ReadAllText` 覆盖回滚 + 视口（`scrollback_len + grid_rows`），格式化器取不到已滚出的行 | 逐格 FFI 扫描（每格 `grid_ref`→`cell`→`codepoint`）在回滚填满时单趟可越过查询超时，靠 `all_text` 缓存兜住 | 上游（格式化器需补回滚范围参数） |
| `on_clipboard_read` 同步回调 | `terminal.rs:2754` 回调文档：应答须在回调返回前完成，回调期间 VT 流暂停；不装回调则 OSC 52 读取被忽略 | 不采纳：应答链是「VT 线程 → 会话队列 → `pollEvent` → Kotlin 读系统剪贴板 → JNI 回写 PTY」的异步链，改用回调须让 VT 线程阻塞等待 Android 侧剪贴板应答 | 读取请求的识别自研逐字节状态机（`output_processor.rs`），应答由本仓拼 OSC 52 帧 + base64 | 本仓（架构取舍：宁可不走上游回调，也不冻结 VT 线程） |

### Requirement: 不采用的上游回调不得留有通道描述

判定为不采用的上游回调 MUST NOT 在代码注释、结构体字段文档或常量说明中以「事件通道」
「每次振铃一个空消息」等形态描述为已存在的实现。若某回调已判定不采用而代码中仍见其
通道描述，MUST 删除该描述，MUST NOT 顺手接线回调——接线需先有 `DESIGN.md` 声明与
消费者，缺一即属未声明功能。

本仓的 OSC 回调通道有两条：剪贴板通道承载 OSC 52 剪贴板写入，通知通道承载 OSC 9/777
桌面通知。两条通道的容量常量与字段文档 MUST 只描述各自承载的回调。

#### Scenario: 悬空通道注释被删除

- **WHEN** 结构体中某字段的文档注释描述了一条并不存在的上游回调通道
- **THEN** 该描述被删除，相邻字段保留其自身文档，不因删除而并入邻字段

#### Scenario: 事件通道容量说明只写剪贴板

- **WHEN** 查阅剪贴板事件通道容量的说明
- **THEN** 说明只提到剪贴板写入，不出现未接线的其他回调

#### Scenario: 清理不以接线收尾

- **WHEN** 因注释失真而删除某回调的通道描述
- **THEN** 该回调仍保持未接线，且没有新增任何消费者或事件变体

### Requirement: OSC 覆盖逐项核对

外部终端生态的 OSC checklist（xterm/ECMA-48 常见段：0/1/2、4、7、9、10/11/12、
52、104、133、1337、777）MUST 在本规范内有一张逐项核对表，写明每一段在本仓的
行为归属（上游解析并观察 / 上游处理且有测试 / 默认色归内嵌方走原生 setter /
本仓收割应答 / 不接）与可复核依据（文件与行号）。表 MUST 覆盖各段而非概括表述；
上游不处理或本仓不消费的段 MUST 注明理由，MUST NOT 留空。

生态参照面为 `8dmusichannels-star/terminator` 的 OSC 注释清单（该仓 CI 虚假绿灯、
零测试资产，仅其 checklist 构成对照）；判定口径与「上游 API 取用判定登记」一致——
本仓不自建 OSC 解析器（`output_processor.rs` 模块文档），除 OSC 52 读取请求的
收割外不再自研识别。

#### Scenario: 某段 OSC 的行为可脱离代码检索直接读到

- **WHEN** 复查某个 OSC 段在本仓的行为
- **THEN** 在「OSC 覆盖逐项核对」表内读到该段的行为归属与文件行号依据，
      MUST NOT 需要先翻归档变更或全仓 grep

#### Scenario: 上游不处理的段注明归属

- **WHEN** 某 OSC 段的语义上游不处理（如默认色查询）
- **THEN** 表内 MUST 写明其归属（内嵌方）与本仓的置色入口（原生主题 setter），
      MUST NOT 记为「上游处理」

核对表如下。

| OSC 段 | 本仓行为 | 依据 |
| --- | --- | --- |
| 0 / 2（图标名 / 窗口标题） | 上游解析；标题经 `Query::Title` 与快照 `title` 观察 | `internal.rs` 标题查询与快照 title；`tests.rs` 的 OSC 0/2 标题可读用例（含跨分块重组） |
| 1（图标名查询） | 上游解析；本仓无消费者，不另设查询 | 标题查询已承载用途；`on_title_changed` 类回调不采用见「上游 API 取用判定登记」 |
| 4（调色板覆盖） | 上游处理且有测试钉住生效路径 | `internal.rs` 初始主题注释；`osc4_palette_override_reaches_dumped_grid` |
| 7（工作目录） | 上游解析，本仓不消费；透传行为有测试 | `output_processor.rs` 模块文档与 `osc7_and_osc8_pass_through`；`on_pwd_changed` 不采用见「上游 API 取用判定登记」 |
| 8（超链接） | 上游解析；本仓经 `hyperlink_at` 查询网格单元 URI | `internal.rs` `hyperlink_at_impl`（4KiB 内联缓冲 + 越界精确重读） |
| 9（iTerm2 通知） | 上游解析并观察：本仓注册 `on_desktop_notification` 回调，通知按会话归属投递 | `DESIGN.md` 软件节要求处理 OSC 9 通知（前台 Toast、后台系统通知、点击切换会话）；回调注册见 `ghostty_terminal/internal/run.rs`，分派见 `runtime/DesktopNotificationPlayer.kt`，规约见 `desktop-notification` |
| 10 / 11 / 12（默认前背景色 / 光标色查询与设置） | 上游不处理，默认值归内嵌方；本仓经原生主题 setter 置默认色 | `internal.rs` 初始主题段落三处注释（默认色归内嵌方、OSC 4 覆盖归上游、故走原生 setter 而非手写 OSC 序列） |
| 52（剪贴板读写） | 写走上游回调；读取请求由本仓扫描器收割并拼帧应答 | `upstream-alignment`「OSC 52 读取请求的收割与应答须对每个会话有界」；`output_processor.rs` `ReadScan` 状态机 |
| 104 / 110 / 111 / 112（色重置） | 上游处理；跨分块重组有测试 | `tests.rs` OSC color reset 跨分块用例 |
| 133（shell 集成标记） | 上游解析（若支持）；本仓不消费 | DESIGN 未声明 prompt 标记语义；`upstream-alignment` 预测线条目判「未声明，需先补 DESIGN」 |
| 777（rxvt 桌面通知） | 上游解析并观察：走上游 `on_desktop_notification` 回调（同 OSC 9） | 上游 `rxvt_extension.zig` 把 `777;notify;标题;正文` 归到 `show_desktop_notification`，与 OSC 9 同一条命令 |
| 1337（iTerm2 媒体 / 通知） | 不接：零消费者 | 上游 `iterm2.zig` 只实现 `Copy` 与 `CurrentDir`，通知类负载（`RequestAttention`）上游记为未实现；本仓不消费 |

#### Scenario: 新增 OSC 消费者时先补 DESIGN

- **WHEN** 拟为某个 OSC 段新增本仓消费者（如 cwd 展示、桌面通知）
- **THEN** MUST 先在 `DESIGN.md` 声明该功能再接线，
      MUST NOT 以「上游已解析」为由直接消费

### Requirement: 保留 JSON 的边界登记

经实测对照后保留 JSON 的边界逐项记录理由与替代方案的规模，避免后来者重复调研；
保留项的字段一经变更 MUST 同步其单测，MUST NOT 把解码失败改写为空值：

| 边界 | 形状 | 保留理由 | 直接量替代方案的实测规模 |
| --- | --- | --- | --- |
| `pollEvent` / `pollResult`（`event.rs`、`ffi.rs`） | 单次调用返回一个事件，3 种变体各自 2–3 个字段，`exit.code` 需表达 `null` 与 0 之别；剪贴板载荷长度不可预知 | 变体判别 + 「一次取出后即消费」的语义由一次 JSON 解码天然保证；拆成标量需跨调用保存槽位，而槽位要引入并发可见性与「取用顺序」两个新失败面 | 至少 1（判别）+ 4（文本/标识/码点/选择区）次调用，外加一处会话级槽位与其线程安全；剪贴板文本还须再加一段长度前缀与分段拉取协议 |
| `pollNotificationEvent`（通知队列） | 单次调用返回一个通知事件，含会话 id、标题、正文三个字段，负载长度不可预知 | 与事件队列同一套 `Event` 枚举、同一 Kotlin `PollEvent` 解码器；通知走独立队列只因后台泵不得取走退出事件，事件形状不变 | 若改为直接量，需新增「会话 id 数组 + 标题数组 + 正文数组」三段并行返回，三段长度必须同步，且 Kotlin 侧要重建同一 schema 镜像 |

#### Scenario: 保留项不得腐化

- **WHEN** 修改上述 JSON 边界的字段
- **THEN** 对应单测（`PollEventTest`、`ParseEventTest`）MUST 同步更新，
  且 MUST NOT 把反序列化失败改写为空值——空值与「真的没有」必须可区分

已按本标准收敛的形状（供对照，勿回流 JSON）：主题（`theme-configuration`：
`getThemeCatalog` 一次取回整个目录，`u16` 主题数 + 逐条「标志 + 名称长度 +
名称 + 54 字节颜色」）、字体信息（`getFontInfo` 的 `u32` 标志 + 三个 `f32` +
`u16` 名称数 + NUL 分隔 UTF-8 名称，CJK 状态按判别序号传递）、搜索结果
（`searchSelect` 扁平 `IntArray`，命中总数由 `searchSetNeedle` 返回）、会话列表
（`listSessions` 扁平 `LongArray`）、会话标题（`getSessionTitles` 的字符串数组，
按传入顺序对应）。
