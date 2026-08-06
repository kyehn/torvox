# 深度研究：zed-android-port（GeneralKaos666/zed-android-port）

> 文档定位：供 torvox（Android 终端，Kotlin Compose + Rust native + wgpu + libghostty-vt）吸收参考的深度研究报告。
> 研究时间：2026-08-06
> 研究对象：`/home/runner/work/kudzu/kudzu/repositories/refs/zed-port`（2484 文件，Zed 编辑器完整 Android 移植，含终端、GPU 平台层、运行时适配层、Termux 捆绑、chroot 适配器）
> 重点阅读范围：`crates/terminal/`（完整）、`crates/gpui_android/`（完整）、`crates/zdroid_runtime/`（完整）、`crates/gpui_android/examples/zed_android/src/lib.rs`、`crates/util/src/env.rs`、`docs/workarounds/` 关键文档
> 行号以研究时的仓库快照为准；代码/标识符/路径保持原文。

---

## 1. 项目定位与整体架构

### 1.1 定位

**zed-android-port（内部代号 Zdroid）** 是 Zed 编辑器（`zed-industries/zed`，AGPL-3.0）的 Android 完整移植。它不是一个"终端 App"，而是一个**完整 IDE/编辑器**在 Android 上的运行：GPUI（自研 UI 框架）→ Android 平台层（`gpui_android`）→ 完整 workspace/editor/project 栈 → 集成终端（`crates/terminal`，基于 alacritty_terminal）。

对 torvox 的价值不在"编辑器"，而在三个被 Android 环境逼出来的子系统：

1. **终端层**（`crates/terminal`）：Zed 上游终端与 alacritty_terminal 的全部集成代码——按键→转义序列映射、鼠标协议、超链接检测、PTY 进程信息、终端设置——以及 Android 特有补丁（env overlay 消费、HOME 双指向）。
2. **平台层**（`crates/gpui_android`）：Android 上从零手写的输入/IME/触摸/多窗口/存储/运行时引导，记录了**每一个 Android 坑及其绕过方式**（SELinux、noexec、JNI 线程、软键盘、targetSdk=28 等）。
3. **运行时适配层**（`crates/zdroid_runtime` + `native/zd-runtime` + `termux-patches`）：把"在 Android 上跑 Linux 用户态"做成可插拔适配器（chroot / 自带 bootstrap / 外部 Termux），配套 `runtime.toml` 配置、`zd-exec` 通用 spawn 包装器、dpkg 路径重写补丁。这是 torvox 最值得吸收的部分。

### 1.2 顶层布局（与 torvox 相关的部分）

```
zed-port/
├── crates/
│   ├── terminal/            # 集成终端：alacritty_terminal 包装 + 全部输入/超链接/设置
│   │   └── src/
│   │       ├── terminal.rs           # 3635 行：TerminalBuilder / Terminal / 事件泵 / 输入输出
│   │       ├── pty_info.rs           # PTY 前台进程组 → 进程名/cwd/argv（sysinfo）
│   │       ├── terminal_hyperlinks.rs# URL 正则 + 路径正则 + OSC 8 超链接检测
│   │       ├── terminal_settings.rs  # TerminalSettings（Zed settings 体系）
│   │       └── mappings/             # keys.rs（按键→转义序）、mouse.rs（鼠标协议）、colors.rs
│   ├── gpui_android/       # GPUI 的 Android 平台实现（Platform trait）
│   │   ├── src/            # platform/window/events/ime/touch/multi_window/saf/storage/...
│   │   ├── examples/
│   │   │   └── zed_android/          # 应用入口（android_main / boot）
│   │   ├── native/zd-runtime/        # zd-exec / zd-runtime-sync shell 脚本族
│   │   ├── termux-patches/           # dpkg 等 Termux 用户态补丁
│   │   └── docs/workarounds/         # 80+ 篇"坑与绕过"文档（黄金资料）
│   ├── zdroid_runtime/    # 运行时适配器：chroot / bootstrap / external_termux
│   ├── util/              # src/env.rs：EnvOp + terminal_env_overlay（环境分层核心）
│   └── ...（editor/project/workspace 等上游 crates，非本报告范围）
├── docs/                   # 上游文档
└── assets/                 # 字体/图标（include_bytes! 进 .so）
```

### 1.3 关键架构决策（理解一切的前提）

| 决策 | 内容 | 后果 |
|---|---|---|
| **Termux 剥离（Phase 4/6/8 系列重构）** | 不再依赖用户安装 Termux，自带 bootstrap 用户态（`Dylanmurzello/zdroid-bootstrap` 仓库发布 tar 包，`install()` 时下载解压到 `$PREFIX=/data/data/com.zdroid/files/usr`） | 独立分发；`$PREFIX` 路径全部重定位 |
| **运行时适配器可插拔** | `runtime.toml` 选 chroot / bootstrap / external_termux 三种后端，统一 `RuntimeProvider` trait | 环境分层抽象化 |
| **targetSdk=28 冻结** | 必须 28（legacy storage + SELinux `untrusted_app_27` domain 允许 `execute_no_trans`），否则 `$PREFIX/bin/*` 全 EACCES | SELinux canary 守护（`termux_bootstrap.rs`） |
| **终端 env 不继承进程 env** | alacritty 的 PTY spawn 会用显式 env map **整体替换**继承 env | 催生 `util::env::EnvOp` overlay 注册机制（见 §6） |
| **HOME 双指向** | 进程 HOME=`data_path`（`dirs::home_dir()` 需要），终端子进程 HOME=`TERMUX__HOME=data_path/home` | `home-env-dual-pointing.md` |
| **ZED_BUILD_REMOTE_SERVER=never** | 禁用远程 server 源码构建回退（Android 上无 rustup/zig） | 远程开发三件套之一 |

### 1.4 终端渲染管线（了解 zed 终端如何与 alacritty 配合）

```
alacritty_terminal::EventLoop（后台线程，持有 pty fd）
        │  AlacTermEvent（pty 输出、bell、标题、光标等）
        ▼
ZedListener(UnboundedSender) ──► events: VecDeque<InternalEvent>（Terminal 内部队列）
        │  sync() 每帧/事件时 drain
        ▼
process_terminal_event(term: &mut Term<ZedListener>)
        │  term 是 Arc<FairMutex<Term>>
        ▼
make_content(term) ──► TerminalContent { cells: Vec<IndexedCell>, mode, cursor, ... }
        │
        ▼
terminal_view（GPUI 侧渲染：cell → 纹理/字模，滚动条、选择覆盖）
```

关键点：**Zed 不直接持有 alacritty 的 `Term` 渲染**，而是每帧把 `display_iter` 拷贝成 `IndexedCell` 快照（`make_content`，terminal.rs:1706），由 GPUI 场景图渲染。torvox 用 libghostty-vt snapshot 的方式与此同构（都走"VT 引擎 → 快照 → 自绘"），因此 zed 的**交互层代码（键鼠映射、超链接、选择）可以平移**，VT 引擎本身不用换。

---

## 2. crates/terminal 逐文件详解

依赖（`crates/terminal/Cargo.toml`）：`alacritty_terminal`（workspace 版，核心 VT/PTY）、`gpui`、`sysinfo`（进程信息）、`futures`/`async-channel`、`parking_lot`、`libc`、`itertools`、`percent-encoding`、`urlencoding`、`url`（经 util 传递）。**没有 tokio**——整个终端是同步 + 通道模型，对 torvox（也是同步 reader 线程模型）非常亲切。

### 2.1 terminal.rs（3635 行，核心文件）

#### 模块级 API 与动作（78-119 行）

`actions!(terminal, [...])` 声明 13 个动作：Clear、Copy、Paste、PasteText、ShowCharacterPalette、SearchTest、ScrollLineUp/Down、ScrollPageUp/Down、ScrollHalfPageUp/Down、ScrollToTop、ScrollToBottom、ToggleViMode、SelectAll。torvox 可对照这些动作清单查漏（滚动/清屏/全选/vi 模式是否都覆盖）。

#### `insert_zed_terminal_env`（123-161 行）★Android 关键

```rust
pub fn insert_zed_terminal_env(env: &mut HashMap<String, String>, version: &impl Display) {
    env.insert("ZED_TERM", "true");
    env.insert("TERM_PROGRAM", "zed");
    env.insert("TERM", "xterm-256color");
    env.insert("COLORTERM", "truecolor");
    env.insert("TERM_PROGRAM_VERSION", version);
    #[cfg(target_os = "android")]
    {
        // alacritty 的 PTY spawn 用显式 map 整体替换继承 env：
        for key in ["HOME", "PATH", "SHELL", "TMPDIR", "LANG"] {
            if let Ok(value) = std::env::var(key) { env.entry(key).or_insert(value); }
        }
        // 然后叠加运行时适配器注册的 overlay（Termux 变量、libtermux-exec、
        // CA bundle、HOME=TERMUX__HOME 覆盖……全部来自 adapter）
        for (key, op) in util::env::terminal_env_overlay() { ... }
    }
}
```

这就是"终端环境分层"的消费端：**PTY env = 进程 env 的五键子集 + 适配器 overlay**。chroot 模式 overlay 为空（env 由 chroot 边界处理）；bootstrap 模式 overlay 携带完整 Termux 形状。详细分析见 §6。

#### 事件类型（165-219 行）

- `Event`（向上，165）：TitleChanged、BreadcrumbsChanged、CloseTerminal、Wakeup、SelectionsChanged、ProcessOutputAvaliable 等 UI 层事件。
- `InternalEvent`（197）：`Resize(TerminalBounds, bool)`（bool=是否同步 PTY，dock 拖拽的像素级 resize 不同步）、`Clear`、`Scroll(AlacScroll)`、`ScrollToAlacPoint`、`SetSelection`、`UpdateSelection(Point<Pixels>)`、`FindHyperlink(Point<Pixels>, bool)`、`ProcessHyperlink((String, bool, Match), bool)`、`Copy(Option<bool>)`、`ToggleViMode`、`ViMotion(ViMotion)`、`MoveViCursorToAlacPoint`。

#### `ZedListener`（223-230）

`pub struct ZedListener(pub UnboundedSender<AlacTermEvent>)` 实现 `alacritty_terminal::event::EventListener`，是 alacritty EventLoop 与我们之间的唯一桥梁。alacritty 事件（pty 输出、bell、标题变化、光标形状变化、退出）都经它进入 `Terminal.events` 队列。

#### `TerminalBounds`（232-321）

`line_height/cell_width/bounds` 的包装，提供 `num_lines/num_columns/height/width/cell_width/line_height`，实现 `alacritty_terminal::grid::Dimensions`（296-307 行 `impl Dimensions for TerminalBounds`：`total_lines/screen_lines/columns`）。注意 278 行 `Default` 用调试常量（500×30px、5px cell）。

#### `TerminalError`（323-380）

`{ directory, program, args, title_override, source }`，`fmt_directory/fmt_shell` 生成用户可读错误（"spawn 失败：目录/程序/参数"）。

#### `TerminalBuilder`（383-506）★PTY spawn 入口

- `new_display_only`（389）：无 PTY 的纯显示终端（用于无 shell 环境，如无 runtime 时的占位）。
- `new`（472）：全参数构造——`working_directory`、`task: Option<SpawnInTerminal>`、`shell`、`env`、`cursor_shape`、`alternate_scroll`、`max_scroll_history_lines`、`path_hyperlink_regexes/timeout`、`is_remote_terminal`、`window_id`、`activation_script`、`path_style`。
- `ShellParams`（508-527）：`{ program, args, title_override }`。
- Shell 解析（529-565）：`Shell::System` 在非 Windows 下为 `None`（**让 alacritty 自己选系统 shell**）；`Shell::Program`/`WithArguments` 显式指定。`shell_kind` 用于 Windows 逃逸参数（`cfg!(windows)`）。
- **PTY options 组装（567-583）**：

```rust
alacritty_terminal::tty::Options {
    shell: alac_shell,                    // Option<tty::Shell>
    working_directory,                    // 由 WorkingDirectory 设置解析
    drain_on_exit: true,
    env: env.clone().into_iter().collect(), // 已含 §2.1 insert_zed_terminal_env 结果
    #[cfg(windows)] escape_args,
}
```

- 滚动历史（586-595）：任务是 `cargo build` 类时用 `MAX_SCROLL_HISTORY_LINES`，否则 `max_scroll_history_lines.unwrap_or(DEFAULT)`。
- `Config`（596-600）：`scrolling_history + default_cursor_style + Config::default()`。
- **`tty::new(&pty_options, TerminalBounds::default().into(), window_id)`（603）**：alacritty 内部完成 openpty/fork/exec。失败时 `bail!(TerminalError)`。
- 事件通道（617-624）：`unbounded()` 创建 events_tx/events_rx；`Term::new(config, &TerminalBounds::default(), ZedListener(events_tx))`；`AlternateScroll::Off` 时 `term.unset_private_mode(NamedPrivateMode::AlternateScroll)`（627-629）。
- 组装（631-640）：`Arc<FairMutex<Term>>`、`PtyProcessInfo::new(&pty)`、`EventLoop::new(term.clone(), ZedListener(events_tx), pty, drain_on_exit, ...)`。
- **Android 分支（约 640-700）**：terminal.rs 的 Android cfg 段处理 working_directory 默认（`data_path/home`）、shell 默认（adapter 的 `terminal_shell()`，bootstrap= `$PREFIX/bin/bash`）、`activation_script`（bootstrap 的 profile 注入），以及"无 runtime 时降级 display-only"。这是 torvox 拼装 spawn 参数时可对照的清单。
- `subscribe`（734-798）：把 alacritty EventLoop 的 task 挂到背景执行器，事件 rx 循环 push 进 `Terminal.events`。
- `resolve_path`（799-816，Windows only）：shell 程序路径解析。

#### `Terminal` 结构体（890-926）

字段（挑选重要的）：`term: Arc<FairMutex<Term<ZedListener>>>`、`terminal_type: TerminalType::{Pty { pty_tx, info }, DisplayOnly}`、`events: VecDeque<InternalEvent>`、`last_content: TerminalContent`、`template: TerminalBuilder`（记录 spawn 参数，供 `clone_builder` 分屏用）、`selection_phase: SelectionPhase`、`selection_head`、`matches: Vec<Match>`（搜索匹配）、`hovered_word`、`last_mouse`、`vi_mode_enabled`、`is_remote_terminal`、`activation_script`、`path_style`、`pty_info`、`task: Option<TaskState>`、`window_id`。

#### 生命周期

- `Drop for Terminal`（2510-2530）：`pty_tx.0.send(Msg::Shutdown)` → `info.terminate_child_process()` → 后台 100ms 后强制 kill 进程树。
- `process_event`（974-1052）：AlacTermEvent → InternalEvent 翻译 + 状态更新（标题、光标形状、bell、进程退出 → `register_terminal_exit`/`register_task_exit`、CloseTerminal）。
- `process_terminal_event`（1057-1263）：InternalEvent 的消费端——`Resize`（1065，先 clamp 最小尺寸、同步 PTY 时 `pty_tx.0.send(Msg::Resize)`、**总是** `term.resize` 保持网格跟随，matches 非空时发 Wakeup 重算）、`Clear`（1091，清 saved buffer + 上方行 + 复制当前行到顶部）、`Scroll`（1124，`term.scroll_display` + vi 光标跟随 + Linux 下写 PRIMARY 剪贴板）、`SetSelection`（1166）、`UpdateSelection`（1180）、`ScrollToAlacPoint`、`FindHyperlink`（1264）→ `process_hyperlink`（1264-1299，调用 terminal_hyperlinks 并 emit ProcessHyperlink）、`UpdateSelectedWord`（1300）等。

#### 输出快照（1706-1737）

`make_content(term, last_content)`：`term.renderable_content().display_iter` → `Vec<IndexedCell>`（带预分配 `size_hint`）、`selection_text`（有选择时才 `selection_to_string`，避免每帧字符串化）、mode、display_offset、cursor、cursor_char、scrolled_to_top/bottom。**性能细节：无选择时不生成 selection_text**——torvox 快照可借鉴。

#### 输入路径

- `input`（1537-1550）：编码输入（paste 用 bracketed paste 包装，`input.into_bytes()`）。
- `write_to_pty`（1523-1536）：`pty_tx.0.try_send(Msg::Input(...))`。
- `try_keystroke`（1647-1665）：**键盘入口**——先试 vi 模式（`vi_motion`/`toggle_vi_mode`），再 `to_esc_str(keystroke, &term.mode(), option_as_meta)` 生成转义序写入。返回 bool 表示是否被消费。
- `try_modifiers_change`（1666-1683）：修饰键变化 → 写入 `\x1b[27;m;n~`（CSI 27 修饰符编码）当程序请求时。
- `paste`（1685-1693）：bracketed paste `\x1b[200~...\x1b[201~`。
- `focus_in/focus_out`（1802-1812）：`TermMode::FOCUS_IN_OUT` 时写 `\x1b[I`/`\x1b[O`。
- `mouse_changed`（1814+）：鼠标位置变化去重，配合 hovered word 刷新。

#### 搜索 / 滚动 / 选择 API（供 UI 调用）

- `total_lines`（1375）/`viewport_lines`（1379）/`scrolled_to_top|bottom`（1476/1480）/`scroll_line_up`（1436）…`scroll_to_bottom`（1471）/`set_size`（1485，clamp + `term.resize` + 同步 PTY + 重算 matches）/`activate_match`（1387）/`select_matches`（1400）/`select_all`（1415）/`copy`（1428）/`clear`（1432）。
- `get_content`（1739-1744）：整屏 `bounds_to_string`；`last_n_non_empty_lines`（1746-1800）：**按逻辑行（WRAPLINE 合并）取最后 N 行非空**——`find_logical_line_start`（1771）+`construct_logical_line`（1784）+`process_line`（1793）。这是"终端输出给 AI 取上下文"的现成实现，torvox 的 MCP `terminal_get` 类工具可直接照搬。
- vi 模式：`toggle_vi_mode`（1555）、`vi_motion`（1559-1646，完整 ViMotion 匹配表）、`vi_mode_enabled`（2399）。

#### 任务（Task）支持（939-1052、2433-2508）

`TaskState { spawned_task, exit_status }`、`TaskStatus`；`task_summary`（2434）用 `⏵ Task \`label\` finished with exit code: N` / `terminated by signal: N` 格式输出摘要；`append_text_to_term`（2498，**unsafe**）：PTY 死亡后直接向网格追加文本的"少公开 API"路径（文档详述了 `term.input` 不改行号、`\n\r` 被忽略等怪癖，附 SAFETY 说明）。任务摘要模式（命令 + 退出码 + 时间）对 torvox 的"命令完成回调"UI 有参考价值。

#### 其他

- `clone_builder`（2403）：分屏克隆 spawn 参数。
- 测试（2668-3635）：`init_ctrl_click_hyperlink_test`（2717）、鼠标→单元格转换属性测试（3044-3176）、浮点精度测试（3607）。测试模式可抄：**鼠标坐标→cell 映射用 proptest 随机化**。

### 2.2 pty_info.rs（221 行）★进程信息

- `ProcessIdGetter`（17-53）：`tcgetpgrp(pty fd)` 拿**前台进程组** PID（负=错误、0=还没设置前台组，此时回退到 `pty.child().id()`，即 alacritty 记录的 shell 直接子进程）。Windows 用 `GetProcessId`（70-83）。
- `ProcessInfo { name, cwd, argv }`（86-91）。
- `PtyProcessInfo`（94-221）：`sysinfo::System`（RwLock）+ `ProcessRefreshKind`（只刷 cmd/cwd/exe，`UpdateKind::Always`）+ `current: RwLock<Option<ProcessInfo>>` + `task: Mutex<Option<Task<()>>>`。`refresh` 在后台线程跑：`system.refresh_specifics(pid, refresh_kind)`，把 name/cwd/argv 读进 `current`。`terminate_child_process`（约 165-216）：`kill(pid, SIGKILL)` + 2 秒后 `kill(-pid, SIGKILL)`（**进程组**）。
- 用途：Zed 用它显示"终端里正在跑什么"（标题栏/进程状态）与关闭时杀进程树。

torvox 对照：torvox 的 session 只保存 `child_pid`（`PtyPair::child_pid`），没有前台进程组跟踪、没有 cwd/argv 读取、kill 只杀单进程。**这是可吸收项**（见 §9.3）。

### 2.3 terminal_hyperlinks.rs（约 2700 行，核心 440 行）★超链接

- `URL_REGEX`（20）：`(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`']+` —— 20 种协议前缀 + 排除控制字符/空白/引号/括号/`^` 等。
- `RegexSearches`（25-67）：`url_regex: RegexSearch`（alacritty 的 `term::search::RegexSearch`，**逐行反向/正向搜索**）+ `path_hyperlink_regexes: Vec<Regex>`（用户配置，terminal_settings 的 `path_hyperlink_regexes`，编译失败 warn 并跳过）+ `path_hyperlink_timeout`。
- `find_from_grid_point<T: EventListener>`（69-162）★核心入口：给定网格点，按优先级：
  1. **OSC 8 链接**（76-101）：`grid.index(point).hyperlink()`，向两边扩展（`sub/add` 直到 hyperlink 不同），返回 `(url, true, match)`。
  2. **URL 正则**（103-116）：`line_search_left/right` 限定行范围 → `RegexIter::new(...).find(|rm| rm.contains(&point))` → `sanitize_url_punctuation`。
  3. **路径正则**（118+）：`path_match(...)`。
- `try_osc8_url_to_path`（163-176）：OSC 8 URI 转本地路径（`file://` scheme 解码）。
- `sanitize_url_punctuation`（177-230）：**去尾标点**（`,` `;` `:` `!` `?` `'` `"` `)` `]` `}` 等），`first_unbalanced_open_paren`（231-254）处理不配对括号。
- `path_match`（255-441）：**路径超链接核心**——把 `PathWithPosition`（`/path/to/file.rs:123:45`）的解析逻辑搬上网格：从点出发左右扩展合法路径字符，再尝试解析 `:行[:列]` 后缀；处理 Windows UNC/盘符、宽字符占位（`WIDE_CHAR_SPACERS`，21-23 行 flags）、包裹引号、`(file.rs)` 括号内路径。返回值含 `(path, is_url, match)`。
- 测试（442+）：`test_url_regex`、`test_url_parentheses_sanitization`、`simple_with_descriptions`、`colons_galore`、`quotes_and_brackets`、`trailing_punctuation`、`issue_alacritty_8586`、`issue_12338`、`issue_40202`、`issue_28194`、`issue_50531`、`issue_46795`、`path_with_position_parse_str`、`alacritty_bugs_with_two_columns`、`invalid_row_column_should_be_part_of_path`、`many_trailing_colons_should_be_parsed_as_part_of_the_path`、`parens_in_filename`、`default_prompts`、`unc` 等。**这些测试名就是需求清单**：两列宽字符、无效行列号、多冒号、括号文件名、Windows UNC、默认提示符。

torvox 对照：torvox 有 OSC 8 解析（`osc_handler.rs` 产生 `HyperlinkEvent`，ghostty_terminal cell 带 `uri`），但**没有**：URL/路径正则检测、尾标点清理、`:行:列` 定位、点击打开（Kotlin 侧 `Bridge.kt` 的 `openUrl` 目前只被 MCP `open_url` 工具触发）。zed 这套是"可点击终端"的完整实现，且**算法与 VT 引擎解耦**（只依赖 `term.grid()`、`line_search_left/right`、`RegexIter`——alacritty 特有 API，但 torvox 的 ghostty grid 提供了等价物：行访问 + 单元格 flags + 字符）。

### 2.4 terminal_settings.rs（186 行）

`TerminalSettings`（24-56）字段全清单（Zed settings 体系，serde + JsonSchema + `RegisterSetting`）：

```rust
shell: Shell, working_directory: WorkingDirectory, font_size: Option<Pixels>,
font_family: Option<FontFamilyName>, font_fallbacks, font_features, font_weight,
line_height: TerminalLineHeight, env: HashMap<String, String>,        // ★用户自定义 env
cursor_shape: CursorShape, blinking: TerminalBlink, alternate_scroll: AlternateScroll,
option_as_meta: bool, copy_on_select: bool, keep_selection_on_copy: bool,
button: bool, dock: TerminalDockPosition, flexible: bool,
default_width: Pixels, default_height: Pixels, detect_venv: VenvSettings,
max_scroll_history_lines: Option<usize>, scroll_multiplier: f32,
toolbar: Toolbar { breadcrumbs: bool }, scrollbar: ScrollbarSettings,
minimum_contrast: f32, path_hyperlink_regexes: Vec<String>,
path_hyperlink_timeout_ms: u64, show_count_badge: bool, bell: TerminalBell,
```

- `from_settings`（82-176）：**项目级设置合并**（`project_content.merge_from_option(content.project.terminal)`——允许仓库里放 `.zed/settings.json` 的 terminal 子集），其余取用户级。
- 注意 `env` 字段：用户可在 settings.json 里给终端加环境变量（`"terminal": {"env": {...}}`）——与 overlay 机制叠加（overlay 后应用，见 §6）。

torvox 对照：torvox 的 Kotlin `SettingsRepository` 覆盖了字体/主题/光标闪烁/滚动，但缺 `env` 注入、`option_as_meta`（Alt 键映射）、`alternate_scroll`、`copy_on_select`、`path_hyperlink_regexes`、`minimum_contrast`。字段语义可逐个平移。

### 2.5 mappings/（键鼠颜色映射）★纯算法，可整体搬运

#### keys.rs（417 行）——GPUI Keystroke → 终端转义序列

- `AlacModifiers`（7-44）：五态枚举 `None/Alt/Ctrl/Shift/CtrlShift/Other`，`new(keystroke)` 只看 alt/control/shift/platform 四键位。
- `to_esc_str(keystroke, mode: &TermMode, option_as_meta: bool) -> Option<Cow<'static, str>>`（46-329）：
  - **手写表**（54-105）：tab/escape/enter（含 Shift=LF、Alt=ESC+CR）/backspace（Ctrl=`\x08`、Alt=`\x1b\x7f`）/space（Ctrl=`\x00`）/home/end/up/down/right/left（**按 `TermMode::APP_CURSOR` 分 `\x1bO` vs `\x1b[` 两套**，68-79）/insert/delete/pageup/pagedown/f1-f20（85-104）。
  - **Caret 记法**（107-207）：Ctrl+a..z → `\x01..\x1a`，Ctrl+Shift+A..Z 也映射到 caret 码（108 行 `("A", CtrlShift) => "\x01"`）；Ctrl+`[` `\` `]` `^` `_` `?` `@` 等特殊字符映射。
  - **修饰键组合**（208+）：Alt+字母 → `ESC + 字符`；Alt+特殊键 → `ESC + 序列`；Ctrl+数字键 → `CSI 27;5;code~` 形式；`option_as_meta` 为真时 Option 键当作 Meta（生产 `\x1b` 前缀）。
  - `modifier_code`（约 380-405）：计算 CSI 27 的修饰码（1=shift, 2=alt, 3=shift+alt, 4=ctrl, ...）。
  - 含 `KeyCode`→`key` 字符串归一化（`keystroke.key` 与 gpui 命名一致）。
- 测试（330-417）：`alt-a`/`shift-ctrl-alt-a` 的 modifier_code 断言等。

**这是 torch 级参考价值**：一张完整的"Android 外接键盘/软键盘 → VT 输入"映射表（xterm 兼容），不依赖 Zed，纯函数。torvox 目前的按键路径（Kotlin `TerminalKeyHandler` → `writeKey`）逐键特判，缺大量修饰键组合。

#### mouse.rs（319 行）——GPUI 鼠标 → alacritty 鼠标协议

- `MouseFormat`（13-28）：按 `TermMode::SGR_MOUSE` / `UTF8_MOUSE` 选 Sgr / Normal(true/false)。
- `AlacMouseButton`（30-80）：按钮编码表（Left=0、Middle=1、Right=2、Move=32..35、ScrollUp/Down=64/65）+ 与 gpui `MouseButton` 的转换（**注意 58-59 行：gpui Right ↔ alac 中键、gpui Middle ↔ alac 右键的错位映射是 alacritty 协议约定**）。
- `scroll_report`（82-100）：`TermMode::MOUSE_MODE` 时把滚轮转成重复 N 次的滚轮报告（`repeat(report).take(scroll_lines)`）。
- 其余：`grid_point`（物理像素→网格点）、`mouse_button_report`（309 附近：SGR `\x1b[<b;x;y[mM]` 与普通 `\x1b[Mbbbxy` 两种格式）、`mouse_moved_report`、`alt_scroll`（alt+滚轮 → 半页滚动）。
- 测试：`scroll_report_repeats_for_negative_scroll_lines`（108）等。

torvox 对照：torvox 的鼠标几乎不存在（Android 触屏为主，无鼠标协议上报；`MouseFormat` 不需要）。**但**：若 torvox 将来支持蓝牙鼠标/触控板（Kotlin 已透传部分鼠标事件），SGR 上报格式可直接抄。

#### colors.rs（11 行）

`to_alac_rgb(color: impl Into<Rgba>) -> AlacRgb`：GPUI 的预乘 alpha 颜色 → alacritty `Rgb`，**关键细节：手动做 alpha 合成 `r*color.a`**（gpui Rgba 是预乘格式，直接取 r 会偏暗）。torvox 的 theme→snapshot 颜色转换可借鉴这条注释。

---

## 3. crates/gpui_android 平台层逐文件详解

依赖（Cargo.toml）：`android-activity 0.6`（game-activity feature）、`jni 0.21`、`ndk 0.9`（nativewindow + api-level-26）、`gpui_wgpu`（wgpu 渲染）、`calloop`（事件循环）、`ureq`（轻量 HTTP）、`zip 2`。

### 3.1 gpui_android.rs（入口，113 行）

- `run<A, F>(android_app, assets, on_finish_launching)`（99-113）：`updater::register_android_app` 存档 app 引用（Activity 重建幂等）→ `AndroidPlatform::new(android_app, false)` → `gpui::Application::with_platform(platform).with_assets(assets).run(...)`。
- 三个设置原子注入函数（40-60）：`set_on_screen_keyboard_enabled` / `set_trackpad_mode_enabled` / `set_programming_extras_row_enabled`——把用户设置推进 `ime.rs` 的 `AtomicBool`，**必须在无 Pane 渲染时也生效**（onboarding 阶段没有 Pane 渲染路径），所以从 `cx.observe_global::<SettingsStore>` 钩子调用而非渲染路径。这是"设置 → 平台运行时"的架构样板。

### 3.2 platform.rs（1530 行）★主循环

- **帧率控制**（28-119）：`AChoreographer` FFI 直连（`[link(name = "android")]`，避免 JNI 每 vsync 一跳）；`set_native_window_frame_rate`（83）：`dlsym` 懒解析 `ANativeWindow_setFrameRate`（API 30+，minSdk 26 不能直接链接），目标 120Hz，**注释警告传 0.0 会让三星自适应面板掉到 30Hz**。
- `AndroidCommon`（176-246）：`{ window, extra_windows, active_window, callbacks, appearance, background_executor, foreground_executor, text_system, gpu_context, running, ... }`；`AndroidCommon::new`（247）里初始化 `AndroidDispatcher`（calloop 驱动）、`AndroidTextSystem`（cosmic-text）、`GpuContext`。
- `AndroidPlatform`（324-336）：`{ common: RefCell<AndroidCommon>, android_app }`。
- `compute_scale_factor`（376-381）：`density/160.0`（160dpi=1x），clamp ≥1。
- `compute_launch_bounds`（343-370）：逻辑像素 → 设备像素，居中。
- `open_extra_window`（393-472）：第二个及以后的窗口 → 启动 `ExtraWindowActivity`（JNI Intent），500ms 超时等 `surfaceCreated`，注册表跟踪 Activity 引用（`mark_window_registered` 必须在启动 Activity **前**标记，防 onCreate 竞态）。
- **事件泵**（474-1037）：
  - `drain_extra_window_events`（474）/`drain_captured_pointer_events`（620）/`drain_ime_events`（668）：三个 mpsc 通道，分别承接 Kotlin UI 线程 → 游戏线程的事件（**JNI 回调在 UI 线程、gpui 状态在游戏线程，全部走通道跨界**）。
  - `reconcile_ime_visibility`（692）/`reconcile_ime_for_window`（709）/`tick_ime_target_and_selection`（778）：IME 目标类型（TextField/Multiline/None）探测 + 软键盘显隐对账（设置门控）。
  - `tick_trackpad_mode_active`（889）/`tick_extras_row_enabled`（914）/`tick_soft_keyboard_setting`（946）/`tick_soft_keyboard_visibility`（972）：把 Rust 侧原子设置**只在状态变化时** JNI 推给 Kotlin（`call_activity_set_*`，1458/1511）。
  - `drain_input_events`（998-1038）：NDK 输入队列——`KeyEvent` → `events::translate_key_event`；`MotionEvent` → `events::translate_motion_event`（**物理像素 → 逻辑像素除以 scale_factor**）。
- `handle_main_event`（1040-1098）：`InitWindow` → `attach_surface`；`TerminateWindow` → `detach_surface`；`WindowResized`/`ConfigChanged` → `resize_surface`（ConfigChanged 还刷新 scale_factor + appearance）；`RedrawNeeded` → `refresh`；`Destroy` → running=false。
- `Platform::run`（1114-1224）：调 `on_finish_launching()` → 事件循环：`android_app.poll_events` + drain 各通道 + `callbacks`（`app.run` 的回调执行）。
- 其余 Platform trait 桩：`restart/activate/hide/hide_other_apps/unhide_other_apps` 全空（1243-1247）、`keyboard_layout`（1442，`AndroidKeyboardLayout`）、`keyboard_mapper`（1445，`DummyKeyboardMapper`——**Android 不做键盘布局映射**，键码直接翻译）、剪贴板（1416-1421，JNI 桥）。

### 3.3 window.rs（827+ 行）

- `AndroidRawWindow`（34）：raw-window-handle 包装。
- `AndroidWindowState`（71-188）：`{ window_handle, bounds, scale_factor, appearance, last_input_was_touch: AtomicBool, clicks: click_track, ... }`。
- `AndroidWindowStatePtr`（189）：`Rc<RefCell<AndroidWindowState>>` 指针（非 Send，跨线程全靠通道）。
- `attach_surface`（212-339）：ANativeWindow → wgpu surface + `set_native_window_frame_rate`；`detach_surface`（340）；`resize_surface`（365）；`refresh`（397，request_redraw）；`notify_active_status_change`（430）；`handle_input`（442，PlatformInput → gpui）。
- `AndroidWindow`（469-827）：`new`（483）、`draw`（776，`Scene` → wgpu 提交）、`update_ime_position`（827，空实现）、`toggle_soft_keyboard`（747）、`soft_keyboard_visible`（768）、`trackpad_mode_enabled`（772）。

### 3.4 events/（输入翻译）

- `events.rs`（271 行）：`translate_motion_event`（65-271）——`source::classify` 分流：**Finger → touch.rs 状态机**（100-102），Mouse/Stylus/Touchpad → 匹配 `MotionAction`：Down（106-119，非左键 latch）、Move（含 button_state 合成 MouseDown/Move/Up）、Up、Scroll（`wheel_scroll`）、Hover。`translate_extra_motion_event`（约 190-271）：ExtraWindow 的 JNI 原始字段版（`JAVA_ACTION_*` 常量 45-53 手写，因为 `MotionAction` 构造器私有）。
- `events/keyboard.rs`（263 行）★硬件键盘：`translate_key_event`（18-41）→ `KeyDown{keystroke, is_held: repeat_count>0, prefer_character_input: false}`；`translate_extra_key_event`（55-87，JNI 原始字段版）；`modifiers_from_meta`（89-97，MetaState 位域 → gpui Modifiers）；`is_modifier_key`（105）；`build_keystroke`（113-263）：`named_key` 表（AKEYCODE → gpui 命名键，含 "back" → backspace 之类 Android 特色）+ `lowercased_key`（字母数字）+ 大写/shift 组合键生成（249-262，shift 与字符反转义）。**注意 line 6 注释：IME/软键盘合成字符在别处（ime.rs），硬件键路径不含 ACTION_MULTIPLE**。
- `events/mouse.rs`：`ANDROID_BUTTON_*` 位常量（19-27）、`button_from_state`（34-43，优先级 primary > tertiary > secondary > back > forward）。
- `events/click_track.rs`：`next_click_count`（双击/三击判定，位置+时间窗口）、`mark_non_primary_down`。
- `events/source.rs`：`classify`（SOURCE_MOUSE / SOURCE_TOUCHSCREEN / SOURCE_STYLUS / SOURCE_TOUCHPAD）。

### 3.5 touch.rs（1208+ 行）★触摸状态机

`TouchEvent`（90）/`TouchAction`（99：Down/Move/Up/Cancel）/`TouchPointer`（119：id+x/y+pressure+size）/`dispatch_primary`（125）/`dispatch_extra`（148，JNI 字段版）/`GesturePhase`（263：Idle/Possible/Scroll/Selection/...）/`PointerState`（298）→ **完整手势状态机**：单击/长按/拖拽/双指滚动/双指捏合缩放/双指右击（`two-finger right-click`）/`TrackpadGesturePhase`（841）/`TrackpadTouchState`（880）：**虚拟触控板模式**（`on_event`，902-1208）——把屏幕当触控板，手指移动映射为指针移动，双指滚动 → 滚轮、双指点击 → 右击，支持 `trackpad_mode_enabled` 门控与 `invert_scroll`。这是"手机当触控板用"的完整实现（约 300 行）。

torvox 对照：torvox 的触摸在 Kotlin Compose 侧（`TerminalGestureDetector` 等），长按菜单/选择拖拽已实现，但没有双指右击、没有虚拟触控板、没有点击计数跟踪。zed 的双指右击合成（Up(Left)+Down(Right)+Up(Right)，events.rs 36-38 注释）与手势状态机是 Kotlin 侧可参考的规格说明。

### 3.6 captured_pointer.rs（53KB）★指针捕获

`MainActivity.requestPointerCapture()` 成功后，触控板原始事件（`SOURCE_TOUCHPAD` + `AXIS_RELATIVE_X/Y` + `ACTION_BUTTON_PRESS/RELEASE`）经 `OnCapturedPointerListener` → mpsc → 游戏线程合成 gpui MouseMove/Down/Up/Scroll（自维护光标位置，系统光标隐藏）。含桌面级手势（滚动/缩放/拖拽对应表）。**结论：Android 上做"鼠标级指针"是可行的，路线是 requestPointerCapture + 相对坐标 + 自己维护光标**。torvox 若要支持外接鼠标精确指针（而非触屏模拟），这是唯一完整参考实现。

### 3.7 ime.rs（约 750 行）★软键盘/输入法

- 原子门控（30-103）：`soft_keyboard_visible`/`on_screen_keyboard_enabled`/`trackpad_mode_enabled`/`invert_scroll_enabled`/`ime_route_as_keys`（IME 文本是否当按键路由）。
- `ImeTargetKind`（143-161）：`TextField/Multiline/None`，`probe_target_kind(handler)`（163）用 `PlatformInputHandler` 的 API 探测（`bounds`/`marked_text_range` 等启发式），`restart_input_for_kind`（180）在种类变化时重启输入连接。
- `ImeEvent`（226-263）：`CommitText/SetComposingText/DeleteSurrounding/KeyDown/KeyUp/...`，`init_event_channel`（264）mpsc；`apply_event`（595-694）游戏线程消费：Composing 文本 → `PlatformInput::Text` 或按 `ime_route_as_keys` 转 `KeyDown`；`show_keyboard/hide_keyboard/toggle_keyboard`（695-725）JNI 调 `MainActivity.showIme()/hideIme()`。
- 配套 Kotlin：`ZdroidInputConnection.kt`（BaseInputConnection 子类，每个 IME 回调 JNI 进 Rust）、`ImeHostView.kt`。
- **架构要点**：IME 事件 UI 线程 → mpsc → 游戏线程（与 multi_window/captured_pointer 同模式）；`reconcile_ime_visibility` 在 platform.rs 每帧对账（目标有焦点且设置允许 → show）。

torvox 对照：torvox 用 Android 标准 `InputConnection` + `adjustNothing`（tech-stack.md），IME 在 Kotlin 侧。zed 的"探测目标种类 + 对账显隐 + 路由开关"规格仍可借鉴（尤其 `ime_route_as_keys`：**软键盘回车/文本直接当键发**）。

### 3.8 multi_window.rs（33KB）+ saf.rs + storage.rs

- `multi_window.rs`：`ExtraWindowActivity` 全生命周期（create_extra_window_blocking 阻塞等 surfaceCreated、500ms 超时、Activity 引用注册表、`finishAndRemoveTask`、`nativeOnExtraKeyEvent` JNI 桥）。Android 自由窗口（DeX/桌面模式）规格。
- `saf.rs`：SAF 拾取器 → POSIX 路径翻译（`content://...tree/primary%3A...` → `/storage/emulated/0/...`），**非 primary 卷不支持**（15 行注释明说）。
- `storage.rs`：`request_once`（34-54，`requestStoragePermissions()` JNI，targetSdk=28 需要运行时权限；失败不锁 OnceLock 以便重试）；`setup_user_symlinks`（81-375）：**Termux 风格 `~/storage/{shared,dcim,downloads,documents,movies,music,pictures,podcasts,external-N}` 符号链接**（镜像 `termux-setup-storage`），幂等重建，跳过 `/storage/emulated|self`。torvox 直接可用（如果走 com.termux 身份则 Termux 已有，无需重复；若自建 bundle 则需要）。

### 3.9 termux_bootstrap.rs（SELinux canary，30 行）

`check_selinux_domain`：读 `/proc/self/attr/current`，若不含 `untrusted_app_27`/`untrusted_app_25` 则 error 日志（targetSdk=28 对应的 domain），提示 build.gradle.kts 别把 targetSdk 升回 29+（否则 `execute_no_trans` 失效、`$PREFIX/bin/*` 全 EACCES）。**5 行代码防一类灾难**，注释里完整记录 Phase 6/8a/8b 演进史。

### 3.10 zd_exec_install.rs（264 行）★二进制安装

`ensure_installed(android_app, data_path)`（54-118）：APK asset `zd-exec`（Gradle `buildZdExec` 任务用 `cargo ndk build --release -p zdroid_runtime --bin zd-exec` 预编译进 APK）→ 按**字节长度比对**决定是否重抽取 → staging 写 + chmod 0755 + rename 原子替换 → 清扫旧路径孤儿。`install_runtime_symlinks`（约 120-264）：在 `<data>/files/zd-runtime/` 下为每个工具建符号链接（zd-runtime-sync 的 Rust 化），含 `SYMLINKS.txt` 式清单处理、损坏链接清扫。**设计要点（21-27 行注释）：zd-exec 与编辑器 Rust 代码同版本节奏，所以随 APK 而非 bootstrap zip 分发；用户态（libc/coreutils）独立版本节奏**。

### 3.11 其他模块（一行为准）

- `updater.rs`（16KB）：GitHub Releases 查版本 → 下载 APK → `MainActivity.launchPackageInstaller`（**选 ureq 而非 reqwest**：无 tokio、冷启动路径只跑一次）。
- `dns_bridge.rs`（3.5KB）：**Bun 编译的 CLI 硬编码 `/etc/resolv.conf`**（musl/c-ares），Android 没有该文件 → 把系统 DNS 写成 `/sdcard/.zed/r`（16 字节槽位，deep-walk 十六进制补丁把二进制里的路径改短后指向这里）。这是"在 Android 上跑桌面工具链"的极致细节。
- `dispatcher.rs`：calloop 驱动的 `PlatformDispatcher`（最小 2 线程，timer 队列）。
- `display.rs`：`AndroidDisplay`（唯一 display）。
- `clipboard.rs`/`cursor.rs`/`splash.rs`/`askpass_install.rs`：JNI 剪贴板、光标样式（Android 无系统光标，多为空）、启动屏、SSH askpass 安装。
- `frame_timing.rs`：帧时间统计。

---

## 4. examples/zed_android/src/lib.rs（1650 行）——应用引导层

`android_main`（103-319）全流程：`android_logger` 初始化（**屏蔽 android-activity 0.6.1 每 vsync 一次的 "Spurious ALOOPER_POLL_CALLBACK" error 日志**，109-119 注释）→ `data_path` 计算（`/data/data/com.zdroid/files`）→ `active_provider(data_path)` 构建运行时适配器 → **注册 env overlay**（`util::env::register_terminal_env_overlay(provider.env_for_zed_process(...))`，把适配器给出的 Termux 形状写进进程 env）→ `register_npm_libtermux_exec_path` → `zd_exec_install::ensure_installed`（抽 zd-exec + 建 zd-runtime 符号链接）→ `storage::request_once`（存储权限）→ `termux_bootstrap::check_selinux_domain` → askpass 安装 → `gpui_android::run(app, assets::Assets, boot)`。

`boot(cx)`（约 450-1649）要点：

1. `BUNDLED_FONTS`（92-101）：**8 个字体文件 `include_bytes!` 进 .so**（Lilex 4 权重 + IBM Plex Sans 4 权重）——不经过 AAssetManager，mmap 直读 rodata，省掉抽取/解压步。torvox 字体打包可参考（torvox 目前用系统字体 + 可选 ttf）。
2. `run_update_check`（345-500）：更新检查（GitHub tag 比对 + 用户确认 + 下载 + 安装器）。
3. **`auto_update::Check` 动作拦截**（735-745）：必须注册在 `auto_update::init` **之前**（gpui 冒泡阶段动作分发遇第一个不 `cx.propagate()` 的监听者即停，先注册者先跑）；`ZED_UPDATE_EXPLANATION` 环境变量（707-712）抑制上游轮询。
4. `NodeRuntime`（768-798）：PATH 查找（`ignore_system_version` 设置）+ 托管下载兜底；`observe_global::<SettingsStore>` 推送 node 选项。
5. `watch_settings_files`（805-813）：**磁盘 settings.json 热重载**。
6. Android 输入设置原子注入（816+，见 §3.1）。
7. 全量 `*::init(cx)` 清单（1098-1204）：go_to_line/file_finder/outline/project_panel/tasks_ui/image_viewer/csv_preview/svg_preview/markdown_preview/.../git_graph/extensions_ui/keymap_editor/which_key/settings_ui/terminal_view/onboarding。**skip 清单**（1094-1097 注释）：audio/call/livekit、agent_ui/copilot、debugger_ui/repl、auto_update、telemetry、extension_host。**`language_model::init` 必须注册**（1120-1125：git_panel 首绘读 `GlobalLanguageModelRegistry`，不注册直接 panic）。
8. **`OpenSettings` 动作拦截**（1165-1188）：无 `zd-runtime.toml` 时路由到 runtime picker（必须注册在 `settings_ui::init` 前）。
9. `reload_zdroid_keymaps`（1211-1220）：keymap 加载必须**最后**跑（`load_asset_allow_partial_failure` 会静默丢弃未注册 action 的绑定），且 `VimModeSetting` 变化时重载。
10. **`Open` 动作拦截**（1519-1619）：SAF 拾取路径 → 优先 `MultiWorkspace::open_project`，回退 `Workspace::open_paths`（首启 welcome 屏没有 MultiWorkspace）。
11. 首启决策（1632-1646）：`KeyValueStore` 读 `onboarding::FIRST_OPEN` → 首启 `show_onboarding_view`，否则 `workspace::open_new`。

**可吸收结论**：这套引导代码本身（Zed 特化）不适用于 torvox，但其中 3 个模式是通用的：①动作/设置拦截的**注册顺序敏感性**（先注册者优先，遇不 propagate 即停）；②`include_bytes!` 字体打包；③设置→平台原子注入用 `observe_global` 而非渲染路径。

---

## 5. crates/zdroid_runtime——运行时适配层（Termux 捆绑 / chroot 适配器）★最值得吸收

### 5.1 port.rs——`RuntimeProvider` 端口（合约）

- `SpawnRequest`（23-58）：`{ program: String, args: Vec<OsString>, cwd: Option<PathBuf>, env: HashMap<String, OsString>, interactive: bool, stdio: [RawFd; 3] }`——**cwd/env/stdio 三要素 + interactive 标志**（决定是否分配控制 tty / setsid / 登录 shell）。
- `SpawnHandle`（63-71）：`wait() -> i32`（-1=信号杀）、`kill()`。
- `RuntimeProvider` trait（75-250）完整方法：

```rust
fn id(&self) -> RuntimeId;
fn health_check(&self) -> HealthStatus;                    // <1s ping
fn install(&self, progress: &mut dyn ProgressSink) -> Result<()>;  // 幂等
fn uninstall(&self) -> Result<()>;
fn spawn(&self, req: SpawnRequest) -> Result<Box<dyn SpawnHandle>>;
fn requires_restart_on_switch(&self) -> bool { true }       // 切换需重启
fn env_for_zed_process(&self, data_path: &Path) -> Vec<(String, EnvOp)>;  // ★进程 env
fn env_for_terminal(&self, data_path: &Path) -> Vec<(String, EnvOp)>;     // ★终端 env
fn terminal_shell(&self, data_path: &Path) -> Option<PathBuf>;            // ★$SHELL
fn workspace_root(&self, data_path: &Path) -> Option<PathBuf>;            // ★home/workspace
fn npm_libtermux_exec_path(&self, data_path: &Path) -> Option<PathBuf>;   // ★LD_PRELOAD
```

**这就是"终端环境分层"的完整抽象**：环境怎么来（进程 env / 终端 env / shell 路径 / home）全部由适配器回答，编辑器零特判。torvox 若要支持"自带 Termux / 外部 Termux / chroot"三种形态，照抄这个 trait 是最短路径。

### 5.2 config.rs——runtime.toml

```toml
[runtime]
type = "chroot"          # chroot | bootstrap | external_termux
[chroot]
root = "/data/local/nhsystem/kali-arm64"
home_bind = "/zed"
spawnd_socket = "/dev/socket/zd-spawn"
su_path = "/product/bin/su"
[bootstrap]
prefix = "/data/data/com.zdroid/files/usr"
proot_rootfs = ""        # 空 = bare 模式
release_repo = "Dylanmurzello/zdroid-bootstrap"
[external_termux]
package = "com.termux"
prefix = "/data/data/com.termux/files/usr"
```

`RuntimeFile::load → resolve() → adapters::for_config()`（config.rs:27-34 注释的加载流）。`RuntimeId`（45-62）带 `display_name`（设置 UI 用）。

### 5.3 adapters/bootstrap.rs——自带 Termux 用户态 ★

- 两种模式（1-17 注释）：**Bare**（默认，直接 `$PREFIX/bin/<target>`，bionic 二进制，零开销）与 **Proot**（`proot -r <proot_rootfs>`，glibc 工具，~10-15ms/syscall）。
- `resolve_target`（39-45）：`$PREFIX/bin` → `$PREFIX/sbin` 查找。
- `build_base_command`（89-140+）：**`cmd.env_clear()` + 显式 env 注入**（102-107）——bootstrap spawn 也是整体替换 env；stdio fd 必须 `dup`（113-120 注释：不 dup 的话 fd 被 move 进 child、请求作用域结束时关闭，破坏调用者视角）。
- `env_for_zed_process`（约 315-384）★env 全清单：

```rust
HOME        = data_path/home          // 进程侧 HOME（双指向的进程端）
PREFIX      = <prefix>
TERMUX__ROOTFS / TERMUX__PREFIX / TERMUX__HOME   // Termux 兼容三件套
TERMUX_APP__PACKAGE_NAME = "com.zdroid"          // ★patched dpkg 路径重写开关
TMPDIR      = $PREFIX/tmp
TERM        = xterm-256color
LANG        = en_US.UTF-8
COLORTERM   = truecolor
ZED_BUILD_REMOTE_SERVER = never
LD_PRELOAD  = <Remove>                 // ★清掉 Termux 的 profile.d 注入，防污染远端 SSH
PATH        = $PREFIX/.zed/bin:$PREFIX/bin:$PATH   // 前端加载自管工具
SHELL       = $PREFIX/bin/bash
```

- `env_for_terminal`（386-434）：在进程 env 基础上追加 `SSL_CERT_FILE`/`CURL_CA_BUNDLE`（`$PREFIX/etc/tls/cert.pem`，**cargo/npm 证书错误的根因**，419-431）。
- `terminal_shell`（436）：`$PREFIX/bin/bash`；`workspace_root`（440）：`data_path/home`；`npm_libtermux_exec_path`（447）：`$PREFIX/lib/libtermux-exec.so`（唯一有它的适配器）。
- 注释宝藏（341-358）：HOME 为什么指向 termux_home 而非 data_path——`git config --global` 读 `~/.gitconfig` 的路径一致性；Zed 自身数据目录已由 `paths::set_custom_data_dir(env_root)` 钉死，不受 HOME 影响。

### 5.4 adapters/chroot.rs——zd-spawnd 守护进程模式

- 协议（8-11、30-43）：Unix socket + `MAGIC=0x5A445350("ZDSP")` + `VERSION=1` + `FLAG_INTERACTIVE`；`send_request`（84-160+）：头（magic/version/flags/lengths）+ prog/cwd/argv/envp 字节流 + `sendmsg(SCM_RIGHTS)` 传 stdio fd。`read_spawned_response`（357-380）/`read_exited_response`（385-399）解析 daemon 回包（errno 转错误链）。
- **v1.1.6 对称 bind 挂载**（89-103、310-327 注释）：host `/data/data/com.zdroid/files` 在 chroot 内同名挂载 + rootfs 里 `/data/user/0/com.zdroid -> /data/data/com.zdroid` 符号链接 → **host 路径在 chroot 内 inode 相同，cwd/argv/env 无需翻译**。仅存的翻译：`strip_zd_runtime`（343-352）——zd-runtime 符号链接目标（bionic zd-exec）在 glibc rootfs 里没有 `/system/bin/linker64`，必须剥成裸程序名让 chroot 内 PATH 解析。这是"Android 路径双命名空间"问题（`/data/user/0` vs `/data/data`）的实战解法。
- `sanitize_env_for_chroot`（约 240-308）：剥离 host 形状 env → chroot 原生 `PATH=/usr/local/sbin:...`、`HOME=/root`、`USER/LOGNAME=root`、白名单 `PASSTHROUGH` 透传（TERM、COLORTERM、LANG 等）、`INIT_PWD`（NetHunter profile.d 钩子）。
- `env_for_zed_process`（约 520-660）：与 bootstrap 同形状但无 libtermux-exec。

### 5.5 adapters/external_termux.rs——桥接已装 Termux

`RUN_COMMAND_ACTION = "com.termux.RUN_COMMAND"`（27）、extras 常量表（35-52，PATH/ARGUMENTS/WORKDIR/BACKGROUND/SESSION_ACTION/RESULT_PENDING_INTENT）。**现状：结构完整但 spawn 未实现**（104-118：Intent 不带 fd，需要 Termux 侧 `zd-bridge` 辅助进程 + 抽象 socket 对，TODO 中）。`terminal_shell` 用 `/system/bin/sh`（163-168）。用户需手动 `allow-external-apps=true` + 授予 `com.termux.permission.RUN_COMMAND`。

### 5.6 adapters/bootstrap_install.rs——bootstrap 下载安装 ★

- `RELEASE_ASSET_NAME = "bootstrap-aarch64.zip"`（33）+ 404 时 GitHub API 枚举兜底（35-41 前缀/后缀匹配）。
- `SYMLINKS_ENTRY = "SYMLINKS.txt"`（47）：**zip 内的符号链接清单**（`名字←目标`，48 行 `SYMLINKS_DELIM`）——Android 的 zip 解压不保留 symlink/mode，抽取后重放。torvox 若自建 bootstrap 打包，必须实现这个（Termux 官方 bootstrap 的 symlink 也靠类似清单）。
- `VERSION_FILE = ".bootstrap-version"`（53）：版本哨兵，tag 比对决定是否重抽。
- `install`（56-112）：下载 → `extract_into_staging`（staging 目录）→ `swap_staging_into_prefix`（原子换入）→ 写哨兵。`resolve_latest_tag`（114+）：`/releases/latest` 302 Location 解析 tag，**一次 HTTP、不依赖 api.github.com、不依赖 asset 名**。
- `health.rs`：`HealthStatus::{Healthy, NotInstalled{hint}, Misconfigured{reason}, Failed{error}}` + `ProgressSink::{step, progress, warn}`——适配器健康状态 → UI 红黄绿点。torvox 的 bootstrap 安装流程（TerminalRuntime.kt 已有 `BootstrapOrchestrator`）可对照升级。

### 5.7 bin/zd-exec.rs——通用 spawn 包装器 ★

- 三种调用形态（5-27 注释）：**符号链接**（`zd-runtime/<name>` → 目标即 argv[0] basename）、**shell 模式**（alacritty 以 `$SHELL` 执行：`["zd-exec"]` 或 `["zd-exec","-c",...]` → 转发给 bash，`-l` 登录标志 65-72）、**直接调用**（`zd-exec <prog> [args]`）。
- `interactive = std::io::stdin().is_terminal()`（105）——**tty 判定即 interactive 标志**，适配器据此决定 setsid/TIOCSCTTY。优雅。
- 退出码语义（117-128）：`code` 或 `128+signum`（bash 惯例）。
- `RUNTIME_TOML` 硬编码（49）`/data/data/com.zdroid/files/usr/etc/zd-runtime.toml`——包装器无 PATH 发现能力，确定性优先。
- 无 `su` 回退（29-32 注释）：**回退 su 是 per-spawn fork-bomb 回归的根源**，宁可失败带提示。

### 5.8 native/zd-runtime/（shell 脚本族，历史形态）

`zd-runtime-sync`：遍历 rootfs `PATH` 目录为每个二进制建 `$PREFIX/zd-runtime/` 符号链接；`zd-runtime.conf.example`：chroot/proot 配置模板。README（1-77）记录 chroot 调用链：`busybox_nh chroot → /usr/bin/sudo -E PATH=... /bin/bash -c "cd ...; exec target"`，tty 时 `setsid -c`（TIOCSCTTY）。**这些已被 Rust 化（zdroid_runtime）取代，作为演进史参考**。

---

## 6. 终端环境分层专题（env overlay）——torvox 第一吸收目标

### 6.1 机制全链路

```
启动（lib.rs::android_main）
  └─ active_provider() ──► BootstrapAdapter（默认）
       └─ provider.env_for_zed_process(data_path) ──► Vec<(String, EnvOp)>
            └─ util::env::register_terminal_env_overlay(ops)   [OnceLock，重复注册 warn]
                 （同时 env_for_terminal 与 terminal_shell/workspace_root 由 provider 持有）

任意时刻（terminal.rs::insert_zed_terminal_env，仅 Android cfg）
  └─ util::env::terminal_env_overlay() ──► 逐条 Set/Remove 应用到 pty env map
```

- `EnvOp`（util/src/env.rs:19-25）：`Set(OsString) | Remove`——一个变量一条 op，无第三种形态。
- `TERMINAL_ENV_OVERLAY: OnceLock<Vec<(String, EnvOp)>>`（36）：**OnceLock 而非 RwLock**——注册一次、只读消费，Activity 重建 re-entry 安全（42-46 注释：二次注册 warn 丢弃）。
- 放 `util` 而非 `terminal`/`zdroid_runtime` 的原因（1-5、33-35 注释）：**避免适配器 crate 依赖编辑器终端栈**（依赖方向：zdroid_runtime → util ← terminal）。
- `NPM_LIBTERMUX_EXEC_PATH`（86-97）：同样 OnceLock，供 editor 代码在 spawn 时给 bionic CLI 加 `LD_PRELOAD`。

### 6.2 为什么需要它（Android 特有）

1. alacritty `tty::new` 的 PTY env 是**显式 map 整体替换**（terminal.rs:133-135 注释），不是继承——进程 env 里就算设了 Termux 变量也到不了终端子进程。
2. 进程 env 与终端 env **必须不同**：进程 HOME=data_path（`dirs::home_dir()` 不能 panic）、终端 HOME=termux_home（bash `~/projects` 语义）——`home-env-dual-pointing.md`。
3. 不同运行时后端（chroot/bootstrap/external Termux）需要**不同形状**的终端 env——硬编码任何一份都会锁死另一种。

### 6.3 分层总结（torvox 可直接套用的模型）

| 层 | 内容 | 来源 |
|---|---|---|
| L0 通用 | ZED_TERM/TERM_PROGRAM/TERM/COLORTERM/TERM_PROGRAM_VERSION | `insert_zed_terminal_env` |
| L1 进程子集 | HOME/PATH/SHELL/TMPDIR/LANG（进程 env 现值） | terminal.rs Android cfg |
| L2 用户设置 | `terminal.env`（settings.json） | TerminalSettings |
| L3 适配器 overlay | TERMUX__*/PREFIX/SSL_CERT_FILE/CURL_CA_BUNDLE/LD_PRELOAD 等 | `util::env::terminal_env_overlay()` |
| L4 shell 登录 | profile.d/zed-init.sh 再改 HOME/PATH | bootstrap 用户态 |

应用顺序 L0→L1→L2→L3（terminal.rs:144-159：先五键 or_insert，再 overlay 覆盖）。

### 6.4 对 torvox 的映射

torvox 现状（`native/src/terminal/shell_env.rs`）：`ShellEnv { home, user, path, working_directory, prefix, extra }`，`build_env`（pty.rs）写死 `LD_PRELOAD=$PREFIX/lib/libtermux-exec.so`、`TMPDIR=/data/local/tmp`（或 prefix/tmp）、`TERM=xterm-256color` 等，全部**编译期常量**；Kotlin 侧 `TerminalRuntime.kt` 从 `SettingsRepository` 读 shell/prefix。torvox 没有"适配器"概念，`applicationId=com.termux` 意味着它**永远只能打外部 Termux 一种牌**（§9 展开）。

---

## 7. termux-patches 与 docs/workarounds 精选

### 7.1 termux-patches/

`dpkg/lib-dpkg-tarfn.c.patch`（3.9KB）+ `dpkg/src-deb-extract.c.patch`（2.4KB）：**dpkg 解包时把 `/data/data/com.termux/` 开头的路径改写为当前包名**（配合 `TERMUX_APP__PACKAGE_NAME=com.zdroid` env，bootstrap.rs:365-370）。让 `pkg install <上游 .deb>` 在重定位 PREFIX 下直接可用——deb 里硬编码的 termux 路径被运行时重写。torvox 用 com.termux 身份则无需此补丁；若自建 bundle 则需要等价物。

### 7.2 docs/workarounds/（80+ 篇，黄金资料）

已深读三篇：

1. **home-env-dual-pointing.md**：进程 HOME 与终端 HOME 分离的完整论证 + 回归失败模式（§6.2）。
2. **terminal-home-override.md**：指向 home-env-dual-pointing 的 stub（注意：仓库自身文档也有 TODO 未完成态）。
3. **remote-development-on-android.md**：远程开发三件套——`ZED_BUILD_REMOTE_SERVER=never`（源码构建回退不可行）、auto_update 初始化但轮询抑制（`ZED_UPDATE_EXPLANATION`）、**SSH transport env 过滤**（`build_command_posix` 剥离 LD_PRELOAD/HOME/TMPDIR/PREFIX/SSL_CERT_FILE/CURL_CA_BUNDLE/TERMUX__*/TERMUX_APP__*，防 Android 形状 env 污染远端 Linux，83-88 行）；外加 `ReleaseChannel::Dev → Ok(None)` 静态漏斗补丁与 CDN URL 的 `Nightly.dev_name()` 替换（Dev 通道名进不了 CDN）。**torvox 若做 SSH 集成（无），但"子进程 env 跨界污染"问题在 torvox 的 MCP server 上同样存在**（MCP server 继承 Termux env 再 exec 用户工具——污染面小，因为不跨网络）。

其余值得按需翻阅的索引（文件名即摘要）：`android-noexec-mount`（/storage FUSE noexec）、`deferred-musl-subprocess-ld-preload`（musl 二进制 LD_PRELOAD 方案）、`jni-exception-clear-after-error`、`jvm-clipboard-stack-overflow`（JNI 递归）、`miui-aggressive-task-killing`（MIUI 杀后台）、`saf-picker-empty-on-termux-presence`（SAF 与 Termux 文件提供者冲突）、`termux-storage-symlinks`、`targetsdk-28-execve`、`two-finger-rightclick`、`wgpu-device-lost-recovery`、`zed-documents-provider`（DocumentsProvider 集成）、`sdcard-dot-zed-namespace`、`gitconfig-safe-directory`。

### 7.3 文档工程本身的价值

- 每篇 workaround 固定结构：**Status / Phase / Files / Problem / Constraint / Solution / Failure mode if regressed / See also**——"回归失败模式"段落是别处没有的工程纪律。
- 大重构（Termux-divestment Phase 1-10）以 commit/phase 编号沉淀，代码注释互相引用（`memory/project_runtime_swap_architecture.md` 等）。
- **对 torvox 的启示**：torvox docs/ 已有 lessons/ 与 adr/，可增加"workarounds/"目录承接 Android 平台坑（如 adjustNothing IME、com.termux 身份陷阱），并强制"Failure mode if regressed"字段。

---

## 8. 功能对比：torvox vs zed-port（逐项）

| # | 功能 | zed-port（参考实现） | torvox（现状） | 结论 |
|---|---|---|---|---|
| 1 | **终端环境分层（适配器抽象）** | `RuntimeProvider` trait + `runtime.toml` 三后端 + `EnvOp` overlay（§5/§6） | 无。`ShellEnv` 静态结构 + pty.rs 编译期常量；`applicationId=com.termux` 绑定外部 Termux | **无** → 最高优先级吸收 |
| 2 | **Termux 捆绑（自带用户态）** | `BootstrapAdapter` + `bootstrap_install.rs`（GitHub release 下载、SYMLINKS.txt 重放、版本哨兵、原子换入） | 有雏形：Kotlin `BootstrapOrchestrator` 下载 Termux bootstrap（settings 里 "Bootstrap" 段，`bootstrapUrl` 等）；但**无 SYMLINKS 重放/版本哨兵/原子换入的 Rust 化实现**（至少 native 侧没有） | 部分 → 吸收 Rust 化安装器 |
| 3 | **chroot 适配器（root 设备）** | `ChrootAdapter` + zd-spawnd socket + SCM_RIGHTS + 对称 bind 路径哲学（§5.4） | **无**（`applicationId=com.termux` 下 chroot 需要 Termux 的 proot-distro，torvox 未做） | **无** → 记录（非必须，量大） |
| 4 | **外部 Termux 桥接（RUN_COMMAND Intent）** | `ExternalTermuxAdapter`（结构完整，spawn 未实现） | **以 com.termux 身份直读其数据**（更彻底的形态：无 Intent 开销，但有签名/身份耦合风险） | 参考（torvox 形态更优，无需吸收） |
| 5 | **PTY spawn** | alacritty `tty::new`（openpty/fork/exec 封装） | 自研 `PtyPair`（nix fork+posix_openpt+execvp，异步信号安全纪律） | 平级；**吸收点**：stdio dup 纪律、interactive 判定、TMPDIR/SELinux 细节 |
| 6 | **前台进程组跟踪** | `ProcessIdGetter`（tcgetpgrp + 回退）+ `PtyProcessInfo`（sysinfo 读 cwd/argv） | 仅 `child_pid`；kill 单进程 | **无** → 吸收（关闭会话杀进程树、显示当前命令） |
| 7 | **按键 → VT 转义映射** | `mappings/keys.rs` 全表（caret 记法、修饰键、APP_CURSOR 双套、f1-f20、CSI 27） | 部分：Kotlin `TerminalKeyHandler` 逐键特判（回车/退格/方向/功能键/粘贴），**修饰键组合不全**（Alt+字母、Ctrl+Shift+字母、Ctrl+数字 CSI 27 缺失或零散） | 部分 → **吸收**（纯函数表，可直接移植为 Rust 或规格） |
| 8 | **鼠标协议上报（SGR/普通）** | `mappings/mouse.rs`（SGR 格式、move 按钮、scroll report 重复） | **无**（无鼠标协议上报；Kotlin 侧鼠标事件零散透传） | **无** → 记录（蓝牙鼠标用户需要时再吸收） |
| 9 | **颜色映射** | `to_alac_rgb`（alpha 合成细节） | theme → `CellStyle`（Kotlin 侧色板） | 平级；记录 alpha 预乘陷阱 |
| 10 | **URL 超链接（正则检测）** | `terminal_hyperlinks.rs`：URL_REGEX（20 协议）+ 尾标点清理 + 括号平衡 | **无**（OSC 8 有解析，但无 URL 正则检测） | **无** → **吸收** |
| 11 | **路径超链接（file.rs:1:23）** | `path_match`（PathWithPosition、宽字符、UNC、括号文件名） | **无** | **无** → **吸收** |
| 12 | **OSC 8 解析** | alacritty_terminal 内置 + `find_from_grid_point` 扩展 | 有（osc_handler.rs `HyperlinkEvent`、ghostty cell 带 uri） | 已有，平级 |
| 13 | **超链接点击打开** | `Event::ProcessHyperlink` → 编辑器打开/浏览器 | 有（`PollEvent.OpenUrl` → `ACTION_VIEW`），但仅限 MCP open_url 触发 | 部分 → 补"点击/长按触发" |
| 14 | **终端设置** | `TerminalSettings` 30+ 字段（env 注入、option_as_meta、alternate_scroll、copy_on_select、hyperlink regexes、minimum_contrast、bell…） | Kotlin `SettingsRepository`：字体/字号/主题/光标闪烁/滚动行数/shell/前缀等 | 部分 → 按字段清单查漏 |
| 15 | **软键盘 IME** | Rust 侧 InputConnection 宿主 + 目标种类探测 + 对账显隐 + `ime_route_as_keys` | Kotlin 标准 InputConnection + `adjustNothing` | torvox 形态更简单且成熟，**无需吸收**；借鉴 `ime_route_as_keys` 思路 |
| 16 | **触摸手势** | 双指右击合成、点击计数、虚拟触控板模式（trackpad SM） | 单指/双指滚动/长按选择/捏合缩放（Kotlin） | 部分 → 双指右击规格可抄 |
| 17 | **指针捕获（鼠标级光标）** | `captured_pointer.rs`（requestPointerCapture + 相对坐标 + 自维护光标） | **无** | **无** → 记录 |
| 18 | **滚动/搜索** | `scroll_*`/`activate_match`/`select_matches`/`last_n_non_empty_lines` | 滚动有、搜索有（`searchAllInScrollback`） | 已有；吸收"逻辑行合并取上下文" |
| 19 | **vi 模式** | 完整 ViMotion 表 | **无** | 记录（torvox 无此需求） |
| 20 | **会话/进程任务摘要** | `task_summary`/`append_text_to_term`（任务退出码回显） | 无任务概念（单会话终端） | 记录 |
| 21 | **多窗口/自由窗口** | ExtraWindowActivity 全链路 | 单 Activity | 记录 |
| 22 | **存储权限 + ~/storage 链接** | `storage.rs`（运行时权限 + termux-setup-storage 镜像） | com.termux 身份下 Termux 已做 | 无需求；自建 bundle 时参考 |
| 23 | **SELinux 守护** | `termux_bootstrap.rs` canary + targetSdk=28 纪律 | **无此问题**（com.termux 的 targetSdk 已是 28 且 domain 已就绪） | 无需求，但**文档化**（若换 applicationId 必读） |
| 24 | **Bun/musl CLI 兼容（resolv.conf、libtermux-exec）** | `dns_bridge.rs` + `npm_libtermux_exec_path` + 十六进制路径补丁 | pty.rs 已 LD_PRELOAD libtermux-exec（Termux 惯例） | 已有大部分；dns_bridge 是增量 |
| 25 | **应用内更新** | `updater.rs`（GitHub + 系统安装器） | 无（APK 分发） | 记录 |
| 26 | **远程开发（SSH）** | 完整 remote 栈 + env 过滤 | **无**（MCP server 是本地工具） | 记录；"env 跨界污染"教训通用 |
| 27 | **MCP** | 无（Zed 是 AI 编辑器但 Android 侧未提 MCP） | 有（tower-mcp + axum + Unix socket + stdio） | torvox 独有优势 |

### 8.1 对比总结

- **torvox 已领先**：外部 Termux 直连形态（#4）、MCP（#27）、Kotlin 侧 IME/触摸成熟度（#15/#16）。
- **torvox 缺失且值得吸收**（按优先级）：终端环境分层（#1）→ URL/路径超链接（#10/#11）→ 前台进程组跟踪（#6）→ 按键映射补全（#7）→ bootstrap 安装器 Rust 化（#2）。
- **记录备用**：chroot（#3）、鼠标协议（#8）、指针捕获（#17）、vi 模式（#19）、多窗口（#21）。

---

## 9. 依赖分析与可吸收清单

### 9.1 依赖分析：能否用于 torvox？

**torvox 不能直接依赖 zed-port 的任何 crate**（`terminal`/`gpui_android`/`zdroid_runtime` 都深度依赖 `gpui`、`collections`、`settings`、`theme`、`task` 等 Zed 内部 crate，且 `gpui_android` 是 `#![cfg(target_os="android")]` 平台实现，不是库）。但：

- **不先进激进**：`alacritty_terminal`、`sysinfo`、`regex`、`url`、`zip`、`ureq`、`parking_lot` 全是稳定生态位依赖，无 nightly、无构建魔法。torvox 的 `#![deny(unsafe_code)]`（terminal 模块）与"库中无 anyhow"约束下，**算法层（§9.3 的 A/B/C/E）可零风险移植**。
- **alacritty_terminal 本身**：torvox 已选 libghostty-vt（ADR-0002 定案，VT5xx 更现代），**不应回退**。移植的是"包装层的交互算法"，不是 VT 引擎。
- **sysinfo**：轻量（torvox 无 tokio 依赖，sysinfo 纯同步，兼容）。
- 唯一要小心的依赖：`mappings/keys.rs` 的输入类型是 `gpui::Keystroke`——移植到 torvox 时需替换为等价结构（`key: String + modifiers: {shift,control,alt,super,function}`，torvox 的 keymap.rs 已有近似类型）。

### 9.2 依赖形态建议

| 建议 | 说明 |
|---|---|
| 纯函数模块直接内联 | keys 映射表、URL/path 超链接算法、colors 转换——搬进 `native/src/terminal/` 现有模块，无新依赖 |
| `sysinfo` 新增依赖（可选） | 若要进程 cwd/argv（§9.3-C），`sysinfo = "0.3x"` 单依赖即可 |
| `regex` 新增依赖 | torvox 目前无 regex crate；URL/path 检测需要（可换 `regex-lite` 控体积） |
| `zip` 新增依赖（可选） | bootstrap 安装器 Rust 化时用（torvox 现有下载在 Kotlin 侧，可保持 Kotlin） |

### 9.3 可吸收到 torvox 的具体内容（含代码注释建议）

#### A. 终端环境分层（第一优先）——按 zed 的 `util::env` 模式

```rust
// native/src/terminal/shell_env.rs 改造建议（保留现有 ShellEnv，追加 overlay 层）

/// 一个 env 变更原语。与 zed `util::env::EnvOp` 相同：只有 Set/Remove 两种形态，
/// 无第三种（无 "append/prepend" —— PATH 前缀合并由调用方拼好字符串再 Set）。
///
/// 参考：zed-port crates/util/src/env.rs:19（EnvOp）、
///       crates/zdroid_runtime/src/port.rs:75（RuntimeProvider::env_for_terminal）
#[derive(Debug, Clone)]
pub enum EnvOp {
    Set(std::ffi::OsString),
    Remove,
}

/// 终端 spawn 时的环境分层，应用顺序固定：
///   1. 通用变量（TERM/COLORTERM/TERM_PROGRAM…）       —— 对应 zed terminal.rs:123 insert_zed_terminal_env
///   2. 进程 env 的少量子集（HOME/PATH/SHELL/TMPDIR/LANG）—— 对应 terminal.rs:145（Android cfg）
///   3. 用户设置 env（Kotlin settings 传入）             —— 对应 TerminalSettings.env（terminal_settings.rs:34）
///   4. 运行时适配器 overlay（Termux 变量、LD_PRELOAD、CA bundle）
///      —— 对应 zed util::env::terminal_env_overlay()（env.rs:52）
///
/// 注意：为什么 PTY 子进程 env 不能直接继承进程 env？
/// zed 的教训（terminal.rs:133-135 注释）：alacritty 的 tty::new 用显式 map
/// 整体替换继承 env；torvox 的 build_env（pty.rs）同样是构造式而非继承式，
/// 因此"进程 env 里设了变量≠终端里有"，必须显式分层。
pub fn build_terminal_env(overlay: &[(String, EnvOp)], user_env: &[(String, String)]) -> Vec<(String, OsString)> { ... }
```

（torvox 现无"适配器"概念，最小改造：`ShellEnv` 增加 `overlay: Vec<(String, EnvOp)>` 字段，Kotlin `initSession` 传参或 native 侧 `TerminalRuntime` 构造时注入；`applicationId=com.termux` 的默认 overlay = 现有常量集，未来切自建 bundle 只需换 overlay，不动 spawn 代码。）

#### B. 前台进程组跟踪（关闭会话杀进程树）

```rust
// native/src/terminal/pty.rs 建议新增（对照 zed pty_info.rs:29-53）

/// 终端"当前前台进程"的 pid：tcgetpgrp(master_fd) 返回 PTY 上前台进程组。
/// 返回 0 = 尚无前台组（shell 启动瞬间）；负 = 错误。
/// 回退：无前台组时用 shell 直接子进程（fork 时记录的 child_pid）。
/// 杀进程树时先 kill(pid) 再 kill(-pid)（进程组），zed 在 2 秒后补发组信号
/// （pty_info.rs terminate_child_process）。
fn foreground_pid(&self) -> Option<nix::unistd::Pid> { ... }
```

用途：①会话关闭时杀干净（torvox 现只 `child.kill()` 单进程，`top`/`vim` 等会残留）；②UI 显示"当前运行命令"。

#### C. URL/路径超链接（第二优先，纯算法）

```rust
// native/src/terminal/ 新模块 hyperlinks.rs 建议（对照 zed terminal_hyperlinks.rs）

/// URL 检测正则：zed 的 URL_REGEX（terminal_hyperlinks.rs:20）——
/// 20 种协议前缀，排除控制字符/空白/`<>"'{}^` 与反引号。
/// 注意排除集里有 `{-}\^` 这种"括号到反引号区间"写法，移植时保持等价。
pub const URL_REGEX: &str = r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`']+"#;

/// 去尾标点（sanitize_url_punctuation，terminal_hyperlinks.rs:177）：
/// 从匹配末尾循环剥 `,.;:!?'"` 与 `)]}`，但用 first_unbalanced_open_paren
/// （:231）保护不配对括号（"see (here)." 的 '.' 该剥，"(foo)" 的 ')' 不该剥）。
fn sanitize_url(url: &str) -> &str { ... }
```

- ghostty 的 grid API 差异：zed 用 `term.line_search_left/right` + `RegexIter`（alacritty 特有），torvox 需要用自己的方式拿"整行文本 + 每格字符/宽度/flags"再跑正则（ghostty snapshot 有 `cells: &[CellData]`，可行）。
- 路径超链接（`path_match`，:255）依赖 `util::paths::PathWithPosition`（`:行:列` 解析）——torvox 可先做 URL 部分，路径部分等"点击打开文件"有 UI 承载时再移植。
- 打开动作：复用 torvox 现有 `Event::OpenUrl`（ffi.rs:1803）→ Kotlin `ACTION_VIEW`（TerminalRuntime.kt:1235-1239 已有），只需把触发源从 MCP 扩到"长按/点击检测到超链接"。

#### D. 按键映射补全（第三优先，纯函数表）

- 把 zed `mappings/keys.rs` 的映射表转成 torvox 内部格式（`key+modifiers → String`），重点补：Ctrl+a..z caret 记法全表（:107-207）、Ctrl+Shift+字母、Alt+字母（ESC 前缀）、Ctrl+数字/标点（CSI 27;5;n~）、方向键的 APP_CURSOR 双套（:68-79）。
- torvox 的 `TerminalKeyHandler`（Kotlin）目前做不完这些——**建议在 Rust 侧实现**（ghostty_terminal/keymap.rs 已有键码→符号基础），Kotlin 只传原始键事件。
- `option_as_meta` 设置字段也一并引入（终端设置对比 #14）。

#### E. bootstrap 安装器 Rust 化（第四优先，若走自建 bundle）

- 对照 `bootstrap_install.rs`：`SYMLINKS.txt` 重放（:47-48）、`.bootstrap-version` 哨兵（:53）、staging 原子换入（:91-96）、`/releases/latest` 302 解析 tag（:114+）。
- torvox 现状（Kotlin `BootstrapOrchestrator`）若已能工作则保持；缺"符号链接重放"最值得补（Termux bootstrap 的 symlink 结构在 Android zip 解压下会丢）。

#### F. 文档与测试纪律（贯穿）

- 每个吸收点写"回归失败模式"段落（zed workarounds 格式）。
- 超链接/键映射移植时带上 zed 的测试用例清单（issue_12338 等）作为验收基线。
- `terminal_hyperlinks.rs` 的测试名即需求（§2.3 结尾列表），可转成 torvox 的 `vt_conformance.rs` 风格测试。

### 9.4 不建议吸收（架构不匹配）

| 内容 | 原因 |
|---|---|
| `TerminalSettings` 的 Zed settings 体系（serde+RegisterSetting+项目级合并） | torvox 是 Kotlin DataStore 设置，重造服务端式设置栈不划算；只抄字段语义 |
| gpui_android 平台层（IME/touch/multi_window/captured_pointer） | torvox 的 Kotlin 侧实现已成熟且更贴合 Compose；平台层与 gpui 强耦合 |
| alacritty_terminal 引擎 | ADR-0002 已定 libghostty-vt，不回退 |
| zd-exec/RuntimeProvider 全量 | 除非 torvox 决定支持"非 com.termux 身份"，否则只需 EnvOp 分层这一半 |

---

## 10. 项目文档吸收价值

1. **docs/workarounds/ 目录模式**：80+ 篇"坑与绕过"，固定模板含 **Failure mode if regressed**。torvox 已有 lessons/、adr/，建议新增 workarounds/ 承接 Android 平台坑（IME adjustNothing 的坑、com.termux 身份被其他 App 占用、MIUI 杀后台等）。
2. **Phase 重构命名法**：Termux-divestment Phase 1-10 让演进史可追溯；代码注释里引用 `memory/*.md` 与 phase 编号。torvox 的 openspec/ 已承担类似职责，可借鉴"代码注释 ↔ 文档互相引用"的密度。
3. **SELinux/targetSdk 纪律**：5 行 canary + 注释即文档（termux_bootstrap.rs）——"一个常量防一类灾难"的最小可维护模式。
4. **BACKLOG.md + UPSTREAM_MERGE.md**：移植项目与上游的合并纪律（上游 merge 演练、backlog 分阶段）——torvox 的 libghostty-vt 也是 vendored 上游，可借鉴其合并策略文档（UPSTREAM_MERGE.md 4KB 含演练流程）。
5. **README 级架构图**：zdroid_runtime.rs 头注释的 ASCII 架构图（Zed → PATH → zd-runtime → zd-exec → 适配器），比独立文档更不易腐烂。

---

## 11. 核心发现摘要（一页版）

1. **zed-port 是 Zed 编辑器完整 Android 移植**，其终端（alacritty_terminal 包装）、平台层（gpui_android）、运行时适配层（zdroid_runtime）三块对 torvox 有高吸收价值；VT 引擎与 UI 框架不可吸收（架构定案不同）。
2. **最有价值的单一概念是"终端环境分层"**：`util::env::EnvOp`（Set/Remove）+ `RuntimeProvider::env_for_terminal` + OnceLock 注册 + PTY spawn 端显式应用。它解决了"Android 上 PTY 子进程 env 无法继承进程 env + 不同运行时后端需要不同 env 形状 + 进程 HOME 与终端 HOME 必须分离"三个问题。torvox 现为编译期常量写死（LD_PRELOAD/TMPDIR/TERM），改造成本极低。
3. **超链接是第二大吸收点**：zed 的 URL_REGEX（20 协议）+ 尾标点清理 + 括号平衡 + 路径 `:行:列` 定位 + OSC 8 扩展，算法与 VT 引擎解耦、测试完备；torvox 仅有 OSC 8 解析与 MCP 触发的打开，缺正则检测与点击交互。
4. **前台进程组跟踪**（tcgetpgrp + sysinfo cwd/argv + 组杀）是 torvox 会话管理的真实缺口（现仅单进程 kill）。
5. **按键映射表**（caret 记法、APP_CURSOR 双套、CSI 27 修饰码）纯函数可整体移植，补齐 torvox 修饰键组合缺失。
6. **chroot 适配器**（zd-spawnd + SCM_RIGHTS + 对称 bind）是"Android 上跑完整 Linux 发行版"的成熟方案，torvox 记录备用；**外部 Termux 直连形态（com.termux 身份）torvox 已优于 zed 的 Intent 桥接方案**。
7. **平台层硬核细节**（targetSdk=28 + SELinux canary、noexec、JNI 线程跨界通道、软键盘对账、120Hz Choreographer）值得按需翻阅 workarounds/ 文档，不建议直接吸收代码。
8. **工程纪律层面**：workarounds 文档模板（含"回归失败模式"）、Phase 重构命名、注释↔文档互引，是 torvox 文档体系可立即采纳的模式。

---

## 附录 A：研究过程文件清单（已读）

- crates/terminal/src/{terminal.rs, pty_info.rs, terminal_hyperlinks.rs, terminal_settings.rs, mappings/{keys,mouse,colors,mod}.rs, Cargo.toml}
- crates/gpui_android/src/{gpui_android.rs, platform.rs, window.rs, events.rs, events/{keyboard,mouse,click_track,source}.rs, ime.rs, touch.rs, captured_pointer.rs, multi_window.rs, saf.rs, storage.rs, termux_bootstrap.rs, zd_exec_install.rs, updater.rs, dns_bridge.rs, dispatcher.rs, display.rs, clipboard.rs, cursor.rs, splash.rs, askpass_install.rs, frame_timing.rs, Cargo.toml}
- crates/gpui_android/examples/zed_android/src/lib.rs（1650 行全文）
- crates/zdroid_runtime/src/{zdroid_runtime.rs, port.rs, config.rs, health.rs, setup.rs, adapters.rs, adapters/{bootstrap,bootstrap_install,chroot,external_termux}.rs, bin/zd-exec.rs}
- crates/util/src/env.rs；crates/gpui_android/native/zd-runtime/{README.md, zd-exec, zd-runtime-sync, zd-runtime-hook, 99-zd-runtime, zd-runtime.conf.example}
- crates/gpui_android/docs/workarounds/{home-env-dual-pointing, terminal-home-override, remote-development-on-android}.md（其余 80+ 篇按索引登记）
- crates/gpui_android/termux-patches/dpkg/*.patch
- torvox 侧：Cargo.toml、docs/tech-stack.md、native/src/terminal/{pty.rs, session.rs, shell_env.rs, osc_handler.rs}、native/src/android/ffi.rs、docs/ 目录索引

## 附录 B：术语对照

| zed-port 术语 | 含义 | torvox 对应 |
|---|---|---|
| TerminalBuilder / Terminal | 终端 spawn 参数 / 运行时实体 | TerminalRuntime（Kotlin）+ Session（Rust） |
| InternalEvent | 终端内部事件队列（resize/scroll/selection/hyperlink） | PollEvent（ffi.rs） |
| TerminalContent | 每帧渲染快照（IndexedCell 列表） | RenderSnapshot（ghostty） |
| EnvOp / terminal_env_overlay | 环境变更原语 / 运行时 overlay | （无）→ 待吸收 |
| RuntimeProvider / adapter | 运行时后端（chroot/bootstrap/external Termux） | （无）→ 待吸收 |
| zd-exec | 通用 spawn 包装器（PATH 符号链接入口） | （无） |
| workarounds/ | Android 坑与绕过文档 | lessons/（部分对应） |


