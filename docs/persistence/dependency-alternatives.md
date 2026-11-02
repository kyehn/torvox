# 外部依赖替代评估

## 来源与口径

对 `native/`（Rust，43583 行）与 `android/app/src/main`（Kotlin，24872 行）逐文件盘点自实现代码，对照 `Cargo.toml` / `Cargo.lock`（268 个包）与 `android/app/build.gradle.kts` 现有依赖，逐项评估「外部库替代 vs 保留自实现」。库的质量按用户要求过滤：crates.io 下载量、最近更新日期（核实于 2026-11-02）。

口径与 `DESIGN.md` 一致：多使用库、绝不重复造轮子；但「不重复造轮子」只针对**本仓能力面内已有等价物**或**成熟外部库可覆盖**的部分，不针对有实测理由的自实现（下文逐项给出理由）。

## 已接入的外部依赖（无需动作）

- `libghostty-vt`（git master rev）：VT 解析、终端状态、按键/鼠标/粘贴编码、kitty 图形与占位符、搜索、选择——本仓不自建 VT。
- `cosmic-text` + `swash` + `fontdb`：整形与光栅化。`cosmic-text 0.19` 经 `swash 0.2.10`（harfrust/skrifa）整形，`fontdb 0.23` 管字体库。
- `wgpu 30` + `wgpu-types` + `etagere` + `raw-window-handle`：Vulkan 渲染与字形图集分配。
- `nix 0.31`：PTY、termios、fork、信号、poll。
- `roxmltree`：`fonts.xml` 解析。`toml` + `serde`：主题文件。
- `flume`：查询通道与 VT 事件选择。`lru`：字形/整形缓存。`bytemuck`：单元帧上传。`base64`：kitty 图形介质。`foldhash`：单元实例键。`pollster`：wgpu 初始化。`jni`、`log`、`parking_lot`、`thiserror`、`unicode-script`、`libc`。
- Kotlin 侧：`okhttp`（下载）、`kotlinx-serialization-json`（事件 JSON）、`kotlinx-coroutines`、`datastore-preferences`、Hilt、Compose、`kotlinx-collections-immutable`、LeakCanary（debug）、`mockwebserver`/`mockk`/`turbine`/`robolectric`（测试）。

## 评估为「应接入外部库」的项

### Rust：`unicode-width` 替代手写的宽字符宽度判定

- 现状：`unicode-width 0.2.3` 已在 `Cargo.lock`（被 `codespan-reporting` 传递依赖），但 `native/Cargo.toml` 未直接声明；`unicode-segmentation 1.13.3` 同样已在锁文件（`cosmic-text` 传递）。本仓宽字符列宽来自 Ghostty 网格（`CellData.width`），无自实现——**此项实为误报，保留以说明排查过程**。
- 结论：无需接入。列宽唯一真源是 Ghostty，符合 `DESIGN.md`「终端状态单一来源」。

### Rust：`sys-locale` 替代手写 locale 读取

- 现状：`native/src/render/font/font_db.rs` 的 `current_locale()` 由 JNI 侧注入（Kotlin `Locale.getDefault()`），Rust 侧只缓存字符串。
- 评估：`sys-locale 0.3.2`（2024-11 更新，4097 万总下载、近 12 月 1387 万次）在 Android 上读 `LocaleList` 需要 JNI，与本仓「Kotlin 注入」路径等价，且本仓实现只有几行。**不接入**：无收益，且 `cosmic-text` 已把 `sys-locale` 带进依赖树（锁文件已含），再直接依赖它只是显式化，不减少代码。

### Kotlin：`java.util.zip` 已覆盖归档解压——确认无自实现

- 现状：`BootstrapInstaller.extractZip` 用 `ZipInputStream`（平台 API），zip 炸弹防护（逐条目与总量双上限）、滑移路径防护（`Paths.get(name).normalize()` 逐段检查）、`SYMLINKS.txt`/`EXECUTABLES.txt` 有界读取均为本仓安全语义，不是 zip 解析。
- 结论：**无需外部库**。平台 `java.util.zip` 即为成熟实现；自实现的部分是安全策略而非格式解析。

## 评估为「不应接入外部库」的自实现（附理由）

以下各项是本次盘点的重点：它们看起来像「可替代的自实现」，但逐项核实后均有实测理由。

### PTY：`native/src/terminal/pty.rs`（832 行）不自建，`portable-pty` 不适用

- `portable-pty 0.9.0`（wezterm 作者，2025-02 更新，2555 万总下载）质量合格，但不适配本仓三个硬约束：
  1. 其 `CommandBuilder` 在 fork 后子进程路径使用 `std::process::Command`（内部堆分配），与本仓「fork 后只调异步信号安全函数」的实测要求冲突（`pty.rs` 的 `PtyPair::spawn` 注释明确禁止子进程分配；`is_pie`/`read_shebang_interpreter` 全部在 fork 前完成）。
  2. 不提供 execve 失败回报管道——本仓用 errno 管道区分「exec 成功」与「退出码撞车」（`read_exec_failure_errno`），portable-pty 无此能力。
  3. `openpty` 的 FD_CLOEXEC 修复、`setsid` 孤儿检测、`PR_SET_PDEATHSIG` 规避均为本仓实测驱动（注释记录了设备上的具体故障）。
- 结论：保留自实现。`nix`（已接入）提供系统调用薄封装，本仓 PTY 层是薄封装之上的编排，不是 VT 那样的「重复造解析器」。

### 事件队列：`native/src/event.rs`（344 行）不可由 `flume` 替代

- 现状：已有 `flume` 用于命令/查询通道；事件队列是 `Mutex<VecDeque>` + 淘汰策略。
- 理由：淘汰语义是需求本身——`Exit` 与 `ClipboardRead` 不可淘汰（丢失 `Exit` 永久泄漏会话、丢失 `ClipboardRead` 让远端把空剪贴板当答案），满队列时淘汰最旧其余事件、只剩受保护事件时丢弃新事件。`flume` 的 bounded 策略是丢弃**新**的，无法表达该优先级。
- 结论：保留。外部通道库不承担业务淘汰策略。

### 限频闸门：`native/src/slow_log.rs`（273 行）不接 `ratelimit_meter`

- `ratelimit_meter 5.0.0` 最后更新 2019-09（七年未动），近 12 月下载 5721 次——按用户「长期未更新、下载量过少」的双重标准直接排除。
- 本仓实现是 CAS 单调时钟闸门（`should_log_at` 可注入时刻以便测试），语义简单且被 `logcat-hygiene` 规范钉住。
- 结论：保留。

### 搜索防抖：`SearchDebouncer.kt`（57 行）不换协程 `debounce`

- `SettingsRepository` 已用 `Flow.debounce`（URL 输入框）。文本搜索的防抖要求「静默期后只跑最后一次」且可精确取消，`SearchDebouncer` 经 `DebounceScheduler` 抽象在 JVM 单测中用假调度器覆盖，Handler 实现只有 15 行。
- 结论：保留（既有 Flow 路径已覆盖另一场景，两处语义不同）。

### 惯性滚动：`TerminalSurface.kt` 的 fling（`flingOffsetPx` 等）不换 `OverScroller`

- 注释记录实测：厂商 ROM 改写 `OverScroller` 样条（日志 `fling, after Optimize … force Finished`），一次猛甩折叠成一帧落点跳十几行。自建指数衰减模型是纯函数、跨设备一致、可单测。
- 结论：保留。这是「平台库不可靠」的典型反例。

### BiDi/Arabic：`PaintRenderer` 式整段整形不接入 `unicode-bidi`

- `unicode-bidi 0.3.18` 已在锁文件（cosmic-text 传递，5.6 亿下载、活跃维护）。但本仓渲染是 wgpu 单元格实例化（`cell_builder.rs`），`DESIGN.md` 明令「不做跨单元连字」；阿拉伯连接天然跨单元格，接入等于重写渲染管线。
- 结论：不接入（与 `upstream-alignment` 台账 RTL/BiDi 行一致）。

### 字体库：`font_db.rs`（906 行）不换 `fontdb` 的系统扫描

- `fontdb::Database::load_system_fonts()` 只在宿主测试路径使用；Android 生产路径走本仓的 `fonts.xml` 解析（`roxmltree`），因为 DESIGN 要求「fonts.xml 是唯一来源、不得硬编码字体名」。`fontdb` 的 Android 系统扫描不读 `fonts.xml` 的 `lang` 回退块，无法支撑 CJK 回退分片。
- 结论：保留。`roxmltree` 已承担 XML 解析，自实现的是 Android 字体配置语义（族/别名/语言回退/符号层），无外部库覆盖。

### OSC 52 读取收割：`output_processor.rs`（577 行）不接库

- 上游 Ghostty 明确不支持 OSC 52 读取请求（FR-036），本仓只在 PTY 字节流上做有界状态机（64 字节暂存上限）拦截 `ESC ] 52 ; … ; ?` 并应答。这是「上游缺口的最小补丁」，不是通用解析器。
- 结论：保留。

### `unicode_placement.rs`（163 行）不换库

- kitty 图形协议的 Unicode 虚拟占位符（U+10EEEE 编码图像/放置 id）是协议实现，上游只提供解码 API；占位符编解码属协议语义的一部分。
- 结论：保留。

### IME 契约：`input/ComposingBaseline.kt`（146 行）、`ComposingDiff.kt`（92 行）、`ImeCursorContract.kt`（103 行）

- 组字基线、最长公共前缀差分、光标锚点折算是输入法交互的状态机，`updateSelection` 经 `javap` 实测为框架 @hide 不可覆写（见 `upstream-alignment` 台账 charon 行）。无外部库覆盖 Android IME 契约。
- 结论：保留。

### `TermuxProperties.kt`（790 行）的 properties 解析器已改用 JDK 平台库

- 此前是自研解析器，理由是「`java.util.Properties` 会做 ISO-8859-1 转码，破坏非 ASCII 键值（中文路径）」。实测该理由不成立：转码只发生在 `load(InputStream)`，`load(Reader)` 按字符读，传 `StringReader` 即无转码。
- 差分与模糊测试（40 万次随机输入 + 真实 termux.properties 语料）确认 JDK 解析器语义更贴近 termux：空白分隔的 `key value`、注释行以 `\` 结尾不吞下一行等场景，自研实现都判错。
- 现状：`PropertiesFile` 委托 `java.util.Properties.load(StringReader)`，用 `RecordingProperties` 记录写入顺序与重复键；非法 `\u` 转义先降级成字面量，保持「写坏一个键只废掉那个键」的口径。
- `extra-keys` 的迷你 JSON 解析器仍是自研：termux 语法允许单引号与裸键名，标准 JSON 不接受。
- 结论：properties 解析改用 JDK；extra-keys 解析保留自研。

### `TerminalKeyCatalog.kt`（168 行）与 `TermuxMacro.kt`

- 键名表逐项对齐 termux-app `ExtraKeysConstants.PRIMARY_KEY_CODES_FOR_STRINGS`，别名表对齐 `CONTROL_CHARS_ALIASES`，宏语义对齐 `TerminalExtraKeys.onExtraKeyButtonClick`。这是配置兼容面，不是通用键位库。
- 结论：保留。

### `isElf` / `isSystemShellScript`（TerminalRuntime）与 `is_pie` / `read_shebang_interpreter`（pty/child.rs）

- ELF 魔数与 shebang 读取各十几行，两侧（Kotlin 安装校验、Rust 启动派发）用途不同：Kotlin 侧验安装结果，Rust 侧在 fork 前决定 linker 桥接。`is_pie` 读 ELF 头 `e_type`。
- 结论：保留。体量与风险都不足以引入 `goblin`/`object` 等解析库（且两者均非 Android 场景的主流选择）。

## 结论

- **没有发现应接入而未接入的外部库**。所有「看起来像重复造轮子」的点，核实后要么已有外部依赖覆盖（VT、整形、光栅化、zip、JSON、TOML、XML、HTTP），要么是平台库不可靠（fling）、要么是协议/业务语义本身（事件淘汰、OSC 52 补丁、termux 配置方言、IME 契约），要么受实测约束（fork 后分配禁令、execve errno 回报）。
- 本次盘点已落地一处替换：termux.properties 的解析改用 JDK `java.util.Properties`（平台库），自研解析器删除。
- 候选库中唯一质量合格且可替代的 `portable-pty`，因 fork 后分配禁令与 execve 回报管道两项硬约束不适用。
- 候选库中明确不合格：`ratelimit_meter`（2019 年停更、下载量低）。
- 本仓依赖策略已符合「尽量使用外部依赖、不自实现」：VT/整形/渲染/PTY 系统调用/归档/JSON/序列化/properties 解析全部外部化；自实现集中在「上游缺口的最小程序」「平台不可靠处的纯函数模型」「配置方言忠实解析」三类，每类都有实测或规范依据。

## 复核口径

- 依赖清单：`native/Cargo.toml`、`native/Cargo.lock`（268 包）、`android/app/build.gradle.kts`。
- 库质量核实：crates.io API（`portable-pty` 0.9.0 / 2025-02、`unicode-width` 0.2.3 / 2026-10、`unicode-script` 0.5.8 / 2025-12、`unicode-segmentation` 1.13.3 / 2026-06、`unicode-bidi` 0.3.18 / 2024-12、`sys-locale` 0.3.2 / 2024-11、`fontdb` 0.24 / 2026-07、`cosmic-text` 0.19 / 2026-04、`ratelimit_meter` 5.0.0 / 2019-09）。
- 自实现盘点：`native/src/terminal/pty.rs`（spawn/wait_writable/exec 回报管道）、`native/src/event.rs`（EventQueue 淘汰）、`native/src/slow_log.rs`（CAS 闸门）、`native/src/terminal/output_processor.rs`（ReadScan 状态机）、`native/src/render/font/font_db.rs`（fonts.xml 语义）、`android/.../ui/TerminalSurface.kt`（fling 物理）、`android/.../ui/SearchDebouncer.kt`、`android/.../settings/TermuxProperties.kt`（PropertiesFile/ExtraKeysParser）、`android/.../settings/TermuxMacro.kt`、`android/.../input/ComposingBaseline.kt`、`android/.../runtime/TerminalRuntime.kt`（isElf/isSystemShellScript）、`android/.../installer/BootstrapInstaller.kt`（zip 安全策略）。
