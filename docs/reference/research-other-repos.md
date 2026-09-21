# 深度研究：其余参考仓库综合（16 个）

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/`（depth 1）
> 本文件覆盖：ghostling、osmosis、ply、fission、wgpu-example、zed-port、sushi-ssh、moke、redterm、terminator、onecode、cpmdroid、neotermux、termux-kotlin、termx、reterminal
> 各仓库单独研究文档：termux-app、ghostty-android、warp、termlib、haven、gnome-console、shashlik、zelland/wgpu-in-app（`research_zelland_wgpu.md`）

---

## 1. ghostling（libghostty C API 最小演示）— 高价值

**定位**：Ghostty 官方 libghostty C API 的最小终端演示（单 C 文件 + Raylib 渲染）。

**功能**（`main.c`）：
- `libghostty` C API 调用链：`ghostty_terminal_new` → `ghostty_terminal_write` → 快照读取 → 渲染
- 单线程（libghostty-vt 支持多线程，演示用单线程）
- Raylib 2D 渲染（非 GPU 直渲）
- 警告注释：演示目的，未全面审计正确性

**与本地项目对比**：
- torvox 用 `libghostty-vt`（Rust 绑定）+ wgpu —— ghostling 的 C API 演示验证了 libghostty 的**架构灵活性**（任何 UI 框架可集成）
- **唯一参考价值**：libghostty C API 的调用顺序（new→write→snapshot→render 循环）与 torvox 的 `GhosttyTerminal` 封装一致

**依赖**：Raylib（不适用本项目）。

## 2. osmosis（Slint 应用骨架）— 低价值

**定位**：Slint UI 跨端应用骨架（点击计数器 + 客户端-服务端往返）。非终端。

**功能**：桌面/Android/Web/iOS 构建矩阵；运行时宽度切换版式（<600px 紧凑导航）—— Slint 框架特性，与 Android 无关。

**参考价值**：仅文档组织（`CONTEXT.md`、`docs/adr/`、`AGENTS.md` 协同）。无终端功能可对比。

**deep-v2 增量（亲自复核 docs/adr/ + xtask/ + .serena/）**：ADR 体系真实存在 **12 篇**（0001-0012），对 torvox 有 3 篇直接值得吸收：

| ADR | 内容 | torvox 借鉴 |
|-----|------|-----------|
| 0005 wgpu-device-as-shared-base | **wgpu-29 device 作全端统一基座**：mobile=skia-on-wgpu、desktop/web=femtovg-on-wgpu、bevy 3D 同 device 离屏渲染供 Slint 采样 | torvox 渲染是自研 cell pipeline + wgpu；"共享 device 是可选但 wgpu 27→30 升级/纹理共享需注意 device 归属" — 无代码落点，记录 |
| 0004 xtask-owns-compilation-not-toolchains | 编译归 xtask，工具链不强制 | torvox scripts/（nu）同思路，确认不需改 |
| 0002 send-boundary-lives-inside-api-crate | 平台边界封装在 api crate 内 | 与 torvox "bridge 层唯一 JNI 入口"一致 |

- `xtask/src/`（android.rs 10KB、boundaries.rs、shell.rs）——跨端编译编排参考
- `.serena/project.yml`——AI 工具链配置（torvox 无 serena，不吸收）
- `app/desktop 强制 Vulkan 无软渲染兜底`（ADR 0005 连带约束）——**torvox 模拟器用 GL 后端**，与此哲学相反但各自环境合理（torvox 有 lavapipe 软 GPU），记录不采纳

## 3. ply（Rust 终端 + proot）— 中价值

**定位**：Rust 编写的 Android 原生终端（v0.1.0），Linux 环境在 Android 上运行。

**功能**（`src/main.rs`、`src/proot.rs`）：
- 简单 shell 循环（`println!` 提示符 + stdin 读取 + 命令执行）—— 非真实终端，是 REPL 演示
- `setup_environment`：`TERM=xterm-256color`、`SHELL=/system/bin/sh`、`HOME=/data/data/com.ply/ply-home`
- `proot.rs`：proot 启动 Linux 环境（与 torvox 的 bootstrap 不同——ply 用 proot 用户态，torvox 用 termux 风格 native ELF + linker）
- `MainActivity.kt` 直接运行 Rust 二进制

**对比 torvox**：
- ply 的 `setup_environment` 环境变量设置模式（TERM/SHELL/HOME）与 torvox 的 `prefixEnvironment()` 一致
- proot 方案 vs torvox 的 LD_PRELOAD/linker 方案：proot 更慢但兼容性更好；torvox 的 native 方案性能更优（用户要求"不得降性能"，torvox 方案正确）
- ply 是早期原型（v0.1.0），无选择/渲染/IME 系统

**依赖**：无第三方（纯 Rust std + proot）。

## 4. fission（Rust 应用框架）— 低价值

**定位**：跨端（macOS/Windows/Linux/Web/Android/iOS/Terminal）GPU 应用框架，含 Terminal widget。

**功能**（README）：应用模型、widgets、渲染管线、平台 shell、测试工具、打包发布全生命周期。1009 个源文件。

**对比 torvox**：fission 是完整框架（类似 egui 但更重）；torvox 是专用终端（wgpu + Compose 自研）。fission 的 Terminal widget 是可选功能，其渲染管线设计（declarative widgets + reducers）与 torvox 架构无关。**不适用**。

## 5. wgpu-example（Rust/Winit/Egui/Wgpu 三角形）— 中价值

**定位**：跨平台 wgpu 演示（桌面/WebGL/WASM/Android/SteamDeck/OpenXR/光追）。

**功能**：wgpu + winit + egui 最小启动代码；Android 通过 `Cross.toml` 交叉编译 + `justfile` 构建。

**对比 torvox**：
- wgpu 初始化模式（instance → adapter → device → surface → pipeline）与 torvox 的 `context.rs` 一致
- **Android 构建**（`Cross.toml` 交叉编译配置）是参考：torvox 用 `cargo ndk`，wgpu-example 用 cross + docker——torvox 方案更直接
- egui 集成（egui-wgpu 渲染器）——torvox 不用 egui（Compose UI），不适用

**依赖**：winit/egui/egui-wgpu（torvox 不适用——Compose 承担 UI）。

## 6. zed-port（Zdroid，Zed 编辑器移植）— 中价值

**定位**：Zed 编辑器完整移植到 Android（gpui_android 平台后端 + Adreno Vulkan 合成 + 捆绑 Termux 用户态）。1858 个源文件。

**功能**（README + `crates/gpui_android/examples/zed_android/src/lib.rs`）：
- **Termux-derived 用户态捆绑**（`com.zdroid` 包）：apt/bash/git/ssh/node/go/rust-analyzer 在应用私有数据目录内进程内运行——与 torvox 的 bootstrap 思路同源
- **运行时适配器**：`RuntimeProvider::terminal_shell`、`env_for_terminal` 环境覆盖、chroot 适配器（Kali）——多后端 shell 选择
- `crates/terminal/src/`：完整终端（映射 keys/mouse/colors、hyperlinks、pty_info）
- `zd-exec`：执行器（terminal.shell 配置）

**对比 torvox**：
- zed-port 的**终端环境覆盖**（`register_terminal_env_overlay`）模式：不同运行时适配器注入不同 env——torvox 的 `prefixEnvironment()` 可借鉴其分层（bootstrap vs chroot vs termux 现有安装）
- zed 终端与 torvox 同用 Rust 终端（zed 用 alacritty 风格自研，torvox 用 libghostty-vt）——libghostty 更完整
- zed-port 的 gpui 渲染架构（每像素 Adreno Vulkan 合成）与 torvox 的 wgpu 不同，无直接移植价值

**依赖**：gpui（Zed 自研 UI 框架，不适用 torvox）。

## 7. sushi-ssh（SSH 客户端）— 中价值

**定位**：SSH 客户端（Termux 兼容），含终端视图与 Gemini AI 集成。

**功能**（`app/src/main/java/net/hlan/sushi/`）：
- `TerminalBackend.kt`、`TerminalView.kt`、`TerminalActivity.kt`、`TerminalSessionHolder.kt`
- `TerminalLogRepository.kt`/`ConsoleLogRepository.kt`：终端日志持久化
- **测试**：`TerminalViewEscapeTest.kt`、`TerminalViewSelectionTest.kt`（androidTest 选择测试）
- AI 转录（GeminiTranscriptEntry）

**对比 torvox**：sushi-ssh 的终端视图是轻量自研（非 GPU）；`TerminalViewSelectionTest` 的测试模式（androidTest 中验证选择）与 torvox 的 `SelectionEspressoTest` 类似。无突破性差异。

## 8. moke（SSH/mosh/sftp 客户端）— 中价值

**定位**：SSH/mosh 客户端 + Termux 兼容 + sftp 传输。106 个源文件，测试丰富。

**功能**：
- `mosh` 支持（MoshPtyTest、MoshBootstrapTest）——mosh 的 UDP 会话迁移是移动场景优势
- `TmuxAttachTest`、`TmuxTest`：tmux 集成测试
- `SessionTitleTest`：会话标题（OSC 序列）
- sftp 断点续传（TransferResumeTest）
- `MokeHostKeyVerifier`：SSH 主机密钥验证
- 更新检查（UpdateCheckerTest、PrereleasePickTest）

**对比 torvox**：moke 的测试组织（每个功能专项测试）是良好实践；SSH/mosh 功能 torvox 无（不适用——torvox 是本地终端）。tmux 会话管理（tmux 包装）是 torvox 缺失的**用户功能**（Termux 用户常用 tmux 实现会话持久化）。

## 9. redterm（Linux distro 安装器终端）— 低价值

**定位**：Android 终端 + Linux distro 安装（DistroRegistry/DistroInstaller）。

**功能**：`DnsHelper`（DNS 优化）、`AppLock`、`CrashHandler`、`StoragePermission`、WelcomeActivity。

**对比 torvox**：redterm 的 distro 安装（proot chroot）与 torvox bootstrap 不同；CrashHandler 模式（全局崩溃捕获+日志）torvox 已有（BootGuard）。无新意。

## 10. terminator（Compose 终端，Kotlin 自研）— 中价值

**定位**：纯 Kotlin 终端模拟器（Compose UI + 自研 VT 解析）。

**功能**（`terminal-emulator/src/main/java/com/terminator/emulator/`）：
- `TerminalEmulator.kt`：自研状态机解析（NORMAL/ESCAPE/CSI/OSC/CHARSET 五态）+ MouseMode（NONE/X10/NORMAL/BUTTON_EVENT/ANY_EVENT）
- `TerminalBuffer.kt`：单元格缓冲
- `NativePty.kt`：PTY（JNI）
- `TerminalView.kt`、`TerminalSession.kt`
- 回调：onBell/onTitleChanged/onCursorMoved/onContentChanged

**对比 torvox**：
- terminator 的 VT 解析是简化实现（~100 行核心）——torvox 的 libghostty-vt 完整得多（kitty graphics、六元组、OSC 全支持）
- **MouseMode 枚举**（X10/NORMAL/BUTTON_EVENT/ANY_EVENT）与 torvox 的 `MouseModeTracker` 对应
- `onCursorMoved` 回调模式：torvox 用 CellCursor 快照，更优
- 无 GPU 渲染（Canvas 绘制）

**依赖**：无第三方终端依赖。不适用 torvox。

## 11. onecode（AI 终端助手）— 低价值

**定位**：AI 辅助编码终端（模板项目结构，3 个源文件）。仅 MainActivity + 测试壳。无实质内容。

## 12. cpmdroid（终端）— 低价值

**定位**：终端（WIP.md 标记开发中）。12 个源文件。无实质内容。

## 13. neotermux（Termux 重写，Compose）— 中价值

**定位**：Termux 的现代 Compose 重写（终端/文件管理器/git/进程管理多屏）。

**功能**（`app/src/main/java/com/neotermux/app/ui/screens/`）：
- `terminal/TerminalViewModel.kt` + `TerminalScreen.kt`
- `filemanager/`、`git/`、`processmanager/` 分屏导航
- `TerminalViewModel` 状态管理模式

**对比 torvox**：neotermux 的多功能屏幕（文件管理器/git/进程管理）是 torvox 缺失的产品功能（torvox 专注终端）；终端实现为简化自研。torvox 的终端深度远超。参考价值在 UI 组织（BottomNav 多屏）。

## 14. termux-kotlin（Termux Kotlin 重写）— 高价值（结构参考）

**定位**：Termux 官方应用的 Kotlin 重写版（305 个 Kotlin 文件）。

**功能**：
- `termux-shared/`：终端 IO（TerminalExtraKeys、BellHandler）、TermuxTerminalViewClientBase（Termux 的 TerminalView 客户端回调）、TermuxSession（shell 命令 runner）
- `AmSocketServer`：`am` 命令的 socket 服务器（termux-am 机制）
- `ArgumentTokenizer`：shell 参数分词
- `StreamGobbler`：输出流消费
- `ReflectionUtils`、`Logger`、`NotificationUtils`、`KeyboardUtils`、`ViewUtils`

**对比 torvox**：
- `TermuxTerminalViewClientBase`：Termux 的客户端回调抽象（copyModeChanged、onSingleTapUp 等）——torvox 的 `TerminalQueryPort` seam 是等价设计
- **`ArgumentTokenizer`**：shell 参数分词工具——torvox 的 bootstrap/postinst 执行（`sh -c` 拼接）可借鉴（避免注入问题）
- `AmSocketServer`：torvox 的 MCP Unix socket 服务是类似机制（更现代）
- `StreamGobbler`：torvox 的 LogcatFileWriter 同功能
- **`TerminalExtraKeys`**：extra keys 定义（CTRL/ALT/ESC/TAB 等）——torvox 的 `ModifierBar`/`NerdKeyLabels` 同功能

## 15. termx（终端，含 telephony）— 低价值

**定位**：终端 + 传感器/电话信息（SensorProvider/TelephonyInfo）——功能杂糅。

**功能**：`AssetInstaller`（assets 安装）、`FullscreenManager`、`FontManager`、`ShellUtils`、`PreferenceManager`。

**对比 torvox**：`AssetInstaller`（从 APK assets 解压安装）与 torvox bootstrap 解压类似但更简单。`FontManager`（字体管理）torvox 已有（loadFontFile）。无新意。

## 16. reterminal（终端，多模块）— 中价值

**定位**：多模块终端应用（core/main + core/resources + 更新管理）。

**功能**：`UpdateManager`（应用更新检查）、`MainActivityNavHost`（导航）、`Res.kt`（资源）、MainViewModel。

**对比 torvox**：`UpdateManager` 模式（GitHub releases 检查更新）torvox 未实现（用户未要求）；导航/资源组织无特别价值。

---

## 综合结论

| 仓库 | 价值 | 可移植点 |
|------|------|----------|
| ghostling | 中 | libghostty C API 调用链验证 |
| osmosis | 低 | 无 |
| ply | 中 | 环境变量设置模式 |
| fission | 低 | 无（框架过重） |
| wgpu-example | 中 | wgpu 初始化 + Cross.toml |
| zed-port | 中 | 终端环境覆盖分层、Termux 用户态捆绑 |
| sushi-ssh | 中 | TerminalViewSelectionTest 模式 |
| moke | 中 | tmux 会话管理、mosh |
| redterm | 低 | 无 |
| terminator | 中 | MouseMode 枚举、回调模式 |
| onecode/cpmdroid | 低 | 无（空壳） |
| neotermux | 中 | 多屏 UI 组织 |
| termux-kotlin | **高** | ArgumentTokenizer、StreamGobbler、ExtraKeys、客户端回调抽象 |
| termx | 低 | 无 |
| reterminal | 中 | UpdateManager |

**最值得吸收**：termux-kotlin 的 `ArgumentTokenizer`（postinst 命令构建安全）、zed-port 的终端环境分层、moke 的 tmux 会话管理。

**deep-v3 增量（复核第 1 轮：xtask/android.rs 前 60 行）**：`cargo xtask android` 把原 `scripts/build-apk.sh` 搬进 Rust，换来两处正确性：① ABI→target triple 映射编译期穷尽性检查（`Abi` enum + `parse`/`triple` 全匹配）；② platform jar 版本排序用数值比较而非 `ls | sort -V | tail -1`。torvox 用 nu 脚本（scripts/build-android-libs.nu，禁改目录）——**思想对比记录**：若未来重构脚本，可评估 xtask 模式（编译期枚举 + 数值比较），当前不动。

**deep-v3 增量（复核第 1 轮：ProotRunner.kt）**：proot 启动器（Kotlin）——`PROOT_TMP_DIR`/`PROOT_LOADER`（native lib）+ `LD_PRELOAD=libfakeuid.so` + 合成 /proc + resolv.conf/group 写入 + **Android 系统变量透传**（ANDROID_ROOT/ANDROID_DATA/ANDROID_RUNTIME_ROOT/ANDROID_TZDATA_ROOT/ANDROID_ART_ROOT/ANDROID_I18N_ROOT/BOOTCLASSPATH/DEX2OATBOOTCLASSPATH/EXTERNAL_STORAGE 9 个）。torvox pty.rs:883 只设 ANDROID_ROOT=/system——**差异 P3**：bootstrap 场景无需其余变量，完整 proot rootfs 场景才需透传（torvox 当前 bootstrap 方案不动）。

## osmosis deep-v1 增量（2026-08-07：xtask 精读）

### xtask/boundaries.rs（343 行）——依赖边界检查工具
- `FORBIDDEN_IN_CONTRACT`（:22）：契约 crate 禁 IO 依赖
- `api_is_tokio_free_on_wasm`（:103）、`web_ios_free_of_3d`（:146）、`web_free_of_native_audio`（:180）：**cargo tree 输出解析 + depends_on 检查**——编译期验证平台 crate 边界
- `vendored_proto_matches_upstream`（:216）：**vendored 文件与上游 diff 检查**（first_difference:247）——防止 vendored 代码漂移
- **torvox 对照**：torvox 单 crate（native/）无 crate 边界——但**两类思想可借鉴**：① vendored 检查（torvox generated-patches/ 由 bootstrap 脚本生成，patches/ 与上游 diff 靠人维护——可加自动 diff 检查，P3）② feature 门控边界（mcp feature 依赖检查，P3）

### xtask/android.rs（326 行）——cargo-ndk 打包
- FEATURES 环境变量透传 cargo features（:133-134）+ ABIs 解析——与 torvox build-android-libs.nu 等价（确认）

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | vendored 文件与上游自动 diff 检查（boundaries.rs:216）——torvox patches 可借鉴 | P3 |
| 2 | cargo tree 依赖边界断言——torvox feature 门控可借鉴 | P3 |

## redterm/terminator/onecode/cpmdroid deep-v1 增量（2026-08-07 精读确认）

### redterm DistroInstaller.kt（630 行）
- **tar.xz 下载 → verifyChecksum(expectedSha) → 提取 → 进度回调（percent/speed）**
- **torvox 对照**：torvox BootstrapInstaller zip 下载 + staging + symlink 白名单——**SHA 校验（verifyChecksum）torvox 有 sha256 marker 待办注释**（P0 已记录）——redterm 确认该模式标准

### terminator（Compose 终端，29 文件）
- SessionForegroundService/SessionRepository/SettingsRepository——**torvox 已实现全部等价**（TerminalForegroundService/TerminalRuntime/DataStore）

### onecode（3 文件，AI 终端助手）
- 仅 MainActivity + 测试——**低价值确认**（之前 small-repos §3 已覆盖）

### cpmdroid（CP/M 模拟器，非终端）
- EmulatorEngine/TerminalView——**非终端模拟器确认**（之前 small-repos §4 已覆盖）
