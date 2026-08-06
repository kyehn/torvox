# torvox 参考仓库研究索引

> 本文档汇总 `docs/reference/` 下全部 **26 个参考仓库** 的深度研究报告，是研究结果的**总索引**：总览表 → 各仓库结论摘要 → torvox 功能缺口 → 可吸收内容（按优先级）→ 使用指南。
> 生成时间：2026-08-06 ｜ 对比基准：torvox（Android 终端，Kotlin Compose + Rust native + wgpu 30 + libghostty-vt + MCP）

---

## 1. 研究总览表

| # | 仓库（链接） | 研究文档 | 核心价值 | 吸收优先级 |
|---|---|---|---|---|
| 1 | [zelland](https://github.com/njreid/zelland) | `research_zelland_wgpu.md`、`research-all-projects.md` §1、`02-comprehensive-tridirectional-comparison.md` | 与 torvox 同栈（wgpu + libghostty-vt）的 SSH 终端：行级脏缓存、捏合缩放、SSH 会话管理 | **P0**（行级脏缓存/捏合缩放） |
| 2 | [wgpu-in-app](https://github.com/jinleili/wgpu-in-app) | `research-wgpu-in-app.md`、`research_zelland_wgpu.md`、`research-all-projects.md` §2 | 不用 winit 把 wgpu 嵌进原生 App 的权威样板（app-surface crate），与 torvox 同代 wgpu 30 API | **P0**（jni_fn 宏、日志降噪） |
| 3 | [ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal) | `research-ghostty-android.md`、`research-ghostty-android-extra.md`、`research-all-projects.md` §3 | **选择系统 UX 的最佳参考**（多击选择/边缘滚动/菜单锚定）+ 主题/背景图/extra keys/标签条全套 UI 模型 | **P0**（选择系统 + 主题链路） |
| 4 | [termux-app](https://github.com/termux/termux-app) | `research-termux-app.md`、`research-termux-app-extra.md`、`research-all-projects.md` §4 | Android 最成熟终端的文本选择系统（onGetContentRect 锚定、手柄拖动、宽字符吸附）与选择文本提取语义 | **P0**（wrap 拼接 + 宽字符换算） |
| 5 | [connectbot/termlib](https://github.com/connectbot/termlib) | `research-termlib.md`、`research-supplement-4.md` §1、`research-all-projects.md` §5 | 与 torvox 架构最接近的 Compose 终端库：OSC 133 语义段、Compose 键序列模式、放大镜、无障碍 | **P0**（OSC 133 / Compose 模式 / 放大镜） |
| 6 | [warp-mobile-android](https://github.com/ImL1s/warp-mobile-android) | `research-warp.md`、`research-warp-extra.md`、`research-all-projects.md` §6 | PTY AS-safe 实现、bootstrap 原子安装、IME composing diff、emoji 分类边界、AI agent 架构模板 | **P0**（composing diff / sha256 sidecar / winsize） |
| 7 | [GlassHaven/Haven](https://github.com/GlassHaven/Haven) | `research-haven.md`、`research-haven-extra.md` | 智能复制（smartCopy）、跨行换行 URL 选择、DECCKM 修复、MCP 同意门控、mosh 协议知识库 | **P1**（smartCopy / 跨行 URL / 同意门控） |
| 8 | [moke](https://github.com/briqt/moke) | `research-mid-repos-b.md` §2、`research-other-repos.md` §8 | 代码质量最高的 SSH/mosh/SFTP 客户端：传输抽象层、TOFU、tmux 集成 | **P1**（SSH/TOFU 全栈，若做 SSH 则 P0） |
| 9 | [ReTerminal](https://github.com/RohitKushvaha01/ReTerminal) | `research-mid-repos-b.md` §4 | 多模块终端：快捷键录制、虚拟键盘全套、PRoot 启动链 | **P2**（快捷键录制） |
| 10 | [termux-kotlin-app](https://github.com/reapercanuk39/termux-kotlin-app) | `research-termux-kotlin.md`、`research-other-repos.md` | termux-app 全量 Kotlin 重写：shell 命令工程范式（ArgumentTokenizer、AmSocketServer、StreamGobbler、Logger） | **P0**（ArgumentTokenizer / SO_PEERCRED） |
| 11 | [ghostling](https://github.com/ghostty-org/ghostling) | `research-small-repos.md` §1、`research-other-repos.md` §1 | libghostty C API 官方最小演示：effects 回调完整性、kitty graphics 管线、输入编码 | **P0**（effects 完整性对照） |
| 12 | [GNOME Console (kgx)](https://gitlab.gnome.org/GNOME/console) | `research-gnome-console.md`、`research-supplement-4.md` §2 | GTK4+VTE 终端的搜索 UX：收窄搜索保持当前匹配、Copy 动态禁用、粘贴确认 | **P1**（搜索收窄 UX） |
| 13 | [shashlik-map](https://github.com/ShashlikMap/shashlik-map) | `research-shashlik.md`、`research-supplement-4.md` §3 | Slint 渲染框架：wgpu Android is_emulator 双后端（模拟器 GL / 真机 Vulkan）+ GL 兜底重试 | **P1**（wgpu 双后端） |
| 14 | [fission](https://github.com/fission-ui/fission) | `research-fission.md`、`research-other-repos.md` §4 | 生产级 Rust 跨端 UI 框架：LiveTest 协议、无 TTY 确定性帧 + PNG 截图、平台能力 trait | **P1**（测试方法论，非框架本身） |
| 15 | [osmosis](https://github.com/yebei199/osmosis) | `research-mid-repos-a.md` §1 | Slint+Bevy 应用骨架：wgpu 共享 device + framebuffer 读回（MCP 截图）、依赖边界检查 | **P2**（MCP screenshot 模式） |
| 16 | [RedTerm](https://github.com/GlobalTechInfo/RedTerm) | `research-mid-repos-a.md` §2、`research-other-repos.md` §9 | proot 发行版终端：init.sh 脚本、输出导出、指纹锁 | **P2**（init.sh / 输出导出） |
| 17 | [terminator](https://github.com/8dmusichannels-star/terminator) | `research-mid-repos-a.md` §3 | 纯 Kotlin+Compose 自研终端（CPU 渲染）：会话元数据持久化、Fn 第二层虚拟键 | **P1**（会话持久化 / Fn 层） |
| 18 | [TermX](https://github.com/mwmQi/TermX) | `research-mid-repos-a.md` §4、`research-other-repos.md` §14 | 功能密度极高的 Termux 风格终端：附加键栏自定义布局、scrollback 行数设置、BellHandler | **P1**（附加键栏布局） |
| 19 | [sushi-ssh-client](https://github.com/hlan-net/sushi-ssh-client) | `research-mid-repos-b.md` §1、`research-other-repos.md` §7 | SSH 客户端 + Gemini AI：命令安全分级、AI 对话→命令→确认闭环 | **P2**（命令安全分级，MCP 增强） |
| 20 | [NeoTermux](https://github.com/developer-mahabbat/NeoTermux) | `research-mid-repos-b.md` §3、`research-other-repos.md` §15 | Termux 的 Compose 重写（原型/占位）：工具型屏幕 UI 模式 | **不吸收**（原型，仅 UI 组织参考） |
| 21 | [OnecodeTerminal](https://github.com/hishow1996/OnecodeTerminal) | `research-small-repos.md` §3、`research-other-repos.md` §12 | Ubuntu on Android（proot）：隐私黑屏覆盖、AIDL 服务化、会话状态机 | **P1**（隐私黑屏） |
| 22 | [Ply](https://github.com/shafinthedev/Ply) | `research-small-repos.md` §2、`research-other-repos.md` §3 | Rust 终端 REPL 原型（非真实终端）+ proot | **不吸收**（反模式记录） |
| 23 | [cpmdroid](https://github.com/avwohl/cpmdroid) | `research-small-repos.md` §4、`research-other-repos.md` §13 | CP/M 模拟器：键盘遮挡"屏内滚动不缩字号"、节流持久化、WIP 过程文档 | **P2**（屏内滚动 / 节流持久化） |
| 24 | [zed-android-port](https://github.com/GeneralKaos666/zed-android-port) | `research-zed-port.md`、`research-other-repos.md` §6 | Zed 编辑器 Android 移植：终端环境分层（EnvOp）、URL 超链接检测、前台进程组跟踪、按键映射表 | **P0**（环境分层 / 超链接 / 组杀） |
| 25 | [wgpu-example](https://github.com/matthewjberger/wgpu-example) | `research-wgpu-example.md`、`research-other-repos.md` §5 | wgpu 极限演示：无限 LOD 网格着色器、程序化几何生成器、CI sccache 配置 | **P2**（sccache / 条件性网格） |
| 26 | [rin](https://github.com/pavelc4/Rin) | `research-rin.md`、`research-supplement-4.md` §4 | 简洁 Rust 终端引擎（vte + 自研 grid）：TerminalEngine 组合模式、Renderer trait | **不吸收**（小型化架构参考） |

> 注：GNOME Console 官方仓库托管于 GitLab（无 GitHub 官方镜像），故链接使用 `gitlab.gnome.org`，其余 25 个均为 GitHub 完整 URL。
> 附加对比文档：`00-TORVOX-BASELINE.md`（torvox 功能基线清单，所有研究的三向对比基准）、`02-comprehensive-tridirectional-comparison.md`（渲染/选择/JNI/终端/Bootstrap/MCP 六个维度的横向对比 + 依赖建议 + 代码注释索引）。

---

## 2. 各仓库核心结论摘要

**1. zelland** — 与 torvox 技术栈最接近（wgpu + libghostty-vt + SSH）。torvox 单 pass 渲染优于其 3-pass；可吸收：行级脏缓存（`src-tauri/src/renderer/mod.rs`）、捏合缩放、SSH 连接管理（russh/ssh2）、鼠标编码标准实现、`get_cell_size()` 动态更新模式。`research_zelland_wgpu.md` §5、`02-comprehensive-tridirectional-comparison.md` §1.1。

**2. wgpu-in-app** — "不用 winit 把 wgpu 嵌进原生 App"的最权威样板，与 torvox 同用 wgpu 30，依赖零冲突。torvox 的 surface 生命周期（ADR-0007 惰性 attach）整体优于其"整树销毁重建"；可吸收：`jni_fn` 宏（ffi/android.rs:10-19）、wgpu_hal/naga 日志分级过滤、view_formats/acquire 差异对照注释。`research-wgpu-in-app.md` §8。

**3. ghostty-android** — 选择系统 UX 最佳参考：选择状态由模拟器拥有（`TerminalView.java:298-314`）、tapCount 多击选择（:1051-1083）、Callback2 + onGetContentRect 菜单锚定（:1469）、selectionGeometryKey 工具栏重定位（:1157）、边缘滚动（:1190）。UI 层"零依赖自绘"范本：ThemeStore/256 色生成、BackgroundImageStore、TabStripView 原地调和。最值得吸收：用户自定义主题链路、背景图"复制私有存储+自愈"、native 层三知识（scrollback 字节预算、NUL 剥离、winsize 像素字段）。`research-ghostty-android.md` §7、`research-ghostty-android-extra.md` §7。

**4. termux-app** — 文本选择系统的最佳参考：`Callback2.onGetContentRect` 锚定算法（`TextSelectionCursorController.java:194-215`）、手柄拖动状态机（:218-306，顺序保持+边缘滚动+宽字符吸附）。torvox 选择系统功能上已超越（GPU 反色、URL 智能扩展），但**菜单锚定与宽字符吸附是两个明确移植缺口**；选择文本提取的 wrap 感知拼接与列→char 换算（`TerminalBuffer.java:52-106`、`TerminalRow.java:92-120`）是 CJK 场景复制正确性的关键。`research-termux-app.md` §7、`research-termux-app-extra.md` §9。

**5. termlib** — 唯一用 Compose 状态驱动选择的参考项目，与 torvox UI 层重叠度最高。torvox 空白项：OSC 133 语义段（`TerminalEmulator.kt:830-930`）、Compose 键序列模式（`ComposeMode.kt:30`）、自绘放大镜（`Terminal.kt` MagnifyingGlass）、无障碍朗读（AccessibilityOverlay）、URL 滚动位置惰性缓存、CellRun 游程编码。终端核心（libvterm）不适用。`research-supplement-4.md` §1。

**6. warp** — PTY 的 async-signal-safe 实现（`pty.rs:106-160`，fork 前预构建 CStrings）与 bootstrap 原子安装（`bootstrap.rs:1-48`，sha256 sidecar）黄金参考。可直接吸收：IME composing diff（`WarpInputView.kt:587-615`）、emoji 分类边界测试（`lib.rs:2192-2307`）、PTY 初始 winsize（`WarpTerminalService.kt:797-808`）、fast-death 恢复（:906-915）、ASystemFontIterator 字体发现。ash 直接 Vulkan 与 Block 模型不建议吸收。`research-warp.md` §10、`research-warp-extra.md` §11。

**7. Haven** — 智能复制的唯一参考：`expandAcrossUrlWrap` 跨行 URL 选择（`SelectionToolbar.kt:120-214`）、smartCopy TUI 边框剥离（:357-405）、SmartTerminalClipboard 剪贴板代理（:407-430）。协议层知识：DECCKM 预推修复（`MoshSession.kt:98-103`）、MOSH.md 完整协议笔记、MCP 同意门控（`AgentConsentManager.kt:351-364`）。除 SelectionToolbar 与 DECCKM/同意门控外，代码级吸收点有限，价值主要在文档与模式。`research-haven.md` §7、`research-haven-extra.md` §19。

**8. moke** — 四仓库中代码质量最高。最值得吸收：SSH + TOFU 全栈（SshConnector/SshTransport/MokeHostKeyVerifier/KnownHosts）——若 torvox 走 Kotlin 侧 sshj 直接搬，Rust 侧 russh 对应实现；SFTP 传输四件套、tmux 集成思路（SSH exec + 解析输出）、字体合成回退（CustomFallbackBuilder 缺字）。`research-mid-repos-b.md` §2.5。

**9. ReTerminal** — 快捷键录制三件套（硬件键盘）与虚拟键盘增强是 P1 吸收点；PRoot rootfs 全量（GPL）仅评估不吸收。`research-mid-repos-b.md` §5。

**10. termux-kotlin** — 终端技术栈全面落后于 torvox，价值集中在 shell 命令工程：ArgumentTokenizer（字符串→argv 安全拆分，torvox 真实缺口，P0）、AmSocketServer 的 SO_PEERCRED 校验（→ mcp.rs 纵深防御，P0）、Logger 分块（logcat 4068B 上限，P1）、TerminalExtraKeys 宏语义（→ ModifierBar，P1）、ExecutionCommand exitCode/errCode 双轨（P1）。`research-termux-kotlin.md` §9。

**11. ghostling** — libghostty C API 官方最小演示，验证 libghostty 架构灵活性。立即行动项：effects 完整性对照（write_pty/size/device_attributes/xtversion/title_changed/color_scheme，`main.c:1104-1189`）、EIO→EOF 处理、Kitty graphics 管线核对、Zig 路径缓存坑。`research-small-repos.md` §1.5、§6。

**12. GNOME Console** — 两个 UX 细节：搜索收窄保持当前匹配（`kgx-tab.c:191-250`，narrowing_down 判定 + 先设 regex 再 find_previous）、Copy 动态禁用（`kgx-terminal.c:705-708`）。粘贴确认对话框是安全 UX 参考。`research-gnome-console.md` §7。

**13. shashlik** — 最有价值单点：wgpu Android is_emulator 双后端 + GL 兜底重试（`app-surface/src/android.rs:25-37`），应吸收进 torvox `wgpu_backend.rs` 后端选择逻辑；NativeWindow RAII 封装与 torvox attachWindow 思路一致。`research-shashlik.md` §3。

**14. fission** — "为确定性可测性而生的通用 UI 框架"，价值不在 widget/渲染技术，而在工程方法论。吸收窗口四点：LiveTest 协议（TCP + 语义选择器 + 注入器，`fission-test-driver/src/lib.rs:37-609`）、无 TTY 确定性帧 + PNG 截图（`fission-shell-terminal/src/frame.rs:80-173`）、失效分类/增量渲染缓存思路、平台能力 trait（host trait + Memory/Unsupported）。全家桶（widget 树/IR/布局引擎）不建议引入。`research-fission.md` §6、§7。

**15. osmosis** — 依赖先进激进（Slint fork + Bevy）但底层与 torvox 同代；不值得引入依赖，值得抄架构模式：wgpu 共享 device + framebuffer 读回（MCP screenshot 工具）、依赖边界检查脚本（cargo tree 断言）、Tee 双路消费、12 个高质量 ADR 写作参照。`research-mid-repos-a.md` §1.6-1.7、§5.2。

**16. RedTerm** — proot 发行版支持建议先不做（与 Termux bootstrap 定位冲突）；若未来做，复用 DistroRegistry→DistroInstaller→ProotRunner→init.sh 骨架，`assets/init.sh` 可直接抄。输出导出、指纹锁（AppLock）为条件性小项。`research-mid-repos-a.md` §2.6、§5.2。

**17. terminator** — 手写 VT 解析器（~500 行）与 CPU 渲染是"最小可行终端"范本，torvox 不退回；可吸收：会话元数据持久化 + 重启恢复（SessionRepository + SessionEntry）、Fn 第二层虚拟键（VirtualKeyBar sticky 实现）、VT 解析测试对照基线。`research-mid-repos-a.md` §3.6、§5.2。

**18. TermX** — 1MB+ 自研代码（X11/SSH/SFTP 服务器）是"重复造轮子"反面教材，验证 torvox "vendored ghostty + MCP"路线正确。可吸收：附加键栏自定义布局（ExtraKeysView 的 extra-keys 属性解析）、scrollback 行数设置项、PTY termios 配置对照清单、BellHandler 四模式。`research-mid-repos-a.md` §4.6、§5.2。

**19. sushi-ssh** — 终端视图轻量自研，无突破性差异；可吸收：命令安全分级三档 gate + 确认卡片 UI（AI/脚本自动化，对应 torvox MCP 增强）、TerminalViewSelectionTest 测试模式。`research-mid-repos-b.md` §1、`research-other-repos.md` §7。

**20. NeoTermux** — 完成度极低（原型/占位），无终端核心价值；仅工具型屏幕 UI 组织（文件/包/进程/Git/SSH/编辑器）可作未来功能入口布局参考。**不吸收**。`research-mid-repos-b.md` §3。

**21. onecode** — 自研 ANSI 解析器是产品妥协（倒退），价值在外围产品层：隐私黑屏覆盖（`MainActivity.kt:190-213`，torvox 隐私敏感场景直接可做）、缩放手势结束一次性 resize（避免 TUI 错位，需核对）、单例 Manager + StateFlow + provider 抽象。`research-small-repos.md` §3.5。

**22. Ply** — 非真实终端（REPL 演示），无功能性吸收；作为反模式记录：`curl | sh` 无校验安装（main.rs:99-103）、rootfs 解压目标错误、manifest 声明不存在的 Service。`research-small-repos.md` §2.5。

**23. cpmdroid** — Z80 模拟器，与终端无关；可吸收嵌入式风格实践：键盘弹出时"屏内滚动不缩字号"（TerminalView.kt:374-434）、新输出复位滚动（:530-540）、NVRAM 节流持久化（MainActivity.kt:973-1024）、WIP.md 过程文档样板。`research-small-repos.md` §4.5。

**24. zed-port** — 最有价值单一概念是**终端环境分层**（`util::env::EnvOp` Set/Remove + `RuntimeProvider::env_for_terminal`，env.rs:19/52）：解决 PTY 子进程 env 无法继承进程 env 等三个问题，torvox 改造成本极低。其次：URL_REGEX 20 协议 + 尾标点清理 + 括号平衡（terminal_hyperlinks.rs:20/177）、前台进程组跟踪（pty_info.rs:29-53）、按键映射补全（mappings/keys.rs caret 记法 + APP_CURSOR + CSI 27）。VT 引擎与 gpui 平台层不吸收。`research-zed-port.md` §11。

**25. wgpu-example** — "一套代码多平台"样板，torvox 在 Android 专用路径上已领先。可吸收：grid.wgsl 无限 LOD 网格（需先加深度纹理，条件性）、程序化几何生成器（raytracing.rs:214-429，供渲染测试）、CI sccache 配置（rust.yml:10-21）。winit/egui/OpenXR 全家桶不吸收。`research-wgpu-example.md` §8。

**26. rin** — 最小化参考：证明自研终端引擎 + JNI 的可行结构，libghostty-vt 在功能完整性上全面超越；参考价值限于 TerminalEngine 组合模式与 Renderer trait 设计（可选）。**不吸收**。`research-rin.md` §7。

---

## 3. torvox 功能缺口清单（汇总去重）

> 汇总所有研究文档中标记为 **torvox 缺失/没有** 的功能，按类别分组去重。优先级与来源文档见各条目。

### 3.1 选择系统（P0 密集区）
| 缺口 | 来源文档 |
|---|---|
| 多击选择（双击选词/三击选行，tapCount 自计数不依赖 GestureDetector） | `research-ghostty-android.md` §2.2、`research-all-projects.md` §P0、`02-comprehensive-tridirectional-comparison.md` §2.2 |
| 手柄拖拽边缘滚动（拖到屏幕边缘自动滚动 scrollback） | `research-ghostty-android.md` §2.5、`research-termux-app.md` §2.4、`02-...comparison.md` §2.3 |
| 菜单锚定精确到列：Callback2 + onGetContentRect（torvox 现为 Callback 锚定整个 view） | `research-termux-app.md` §2.2、`research-ghostty-android.md` §2.3、`02-...comparison.md` §2.2 |
| selectionGeometryKey 48 位几何键优化工具栏重定位（避免不必要重定位） | `research-ghostty-android.md` §2.4、`research-all-projects.md` §P0 |
| 宽字符列→char 索引换算（CJK 行选择复制切错边界） | `research-termux-app-extra.md` §7-2 |
| wrap 感知的选择文本拼接（软换行行不插 `\n`、保留尾空格） | `research-termux-app-extra.md` §7-1 |
| 智能复制 smartCopy（TUI 边框剥离 + soft-wrap 解包） | `research-haven.md` §2.3、§7 |
| 跨行换行 URL 选择 expandAcrossUrlWrap（URL-safe 字符集 + 缩进散文区分 + looksLikeFullUrl 验证） | `research-haven.md` §2.2、§7 |
| 锚点移动按钮（d-pad 逐字符导航选择） | `research-haven.md` §2.3、`02-...comparison.md` §2.4 |
| 剪贴板代理拦截模式 SmartTerminalClipboard | `research-haven.md` §7 |

### 3.2 搜索 / URL / 无障碍
| 缺口 | 来源文档 |
|---|---|
| 搜索收窄时保持当前匹配（narrowing_down 判定） | `research-gnome-console.md` §2、§7 |
| 搜索条覆盖层不触发 SIGWINCH + 150ms 防抖 + IME 回车 flush | `research-ghostty-android-extra.md` §5-4 |
| URL 标点修剪/精确检测（尾标点清理 + 括号平衡） | `research-all-projects.md` §P1、`research-zed-port.md` §9.3-C |
| URL 缓存按滚动位置惰性重建（当前每次全量扫描） | `research-supplement-4.md` §1.3 |
| OSC 133 语义段模型（prompt/command/output 分段 + getLastCommandOutput） | `research-supplement-4.md` §1.2 |
| 无障碍朗读/按语义段导航（AccessibilityOverlay） | `research-supplement-4.md` §1.11、`research-all-projects.md` §P2 |
| Copy 动态禁用（clipboardHasText 元数据检查，避免剪贴板 toast） | `research-gnome-console.md` §6、`research-ghostty-android.md` §6 |
| 粘贴确认对话框（多行内容粘贴前确认） | `research-gnome-console.md` §4 |

### 3.3 输入 / IME / 键盘
| 缺口 | 来源文档 |
|---|---|
| IME composing diff（setComposingText 与 commitText 交替时的 PTY 侧同步） | `research-warp-extra.md` §9.1 |
| Compose 键序列模式（Compose + o + o → °，硬件键盘用户空白） | `research-supplement-4.md` §1.7 |
| Fn 第二层虚拟键（F1-F12 + sticky 语义） | `research-mid-repos-a.md` §3.6 |
| 附加键栏自定义布局（extra-keys 属性字符串解析） | `research-mid-repos-a.md` §4.6 |
| 键序列宏（空格分隔键序列，TerminalExtraKeys isMacro） | `research-termux-kotlin.md` §7.4 |
| 按键映射补全（Ctrl+字母 caret 全表、APP_CURSOR 双套、CSI 27 修饰码） | `research-zed-port.md` §9.3-D |
| 硬件修饰键瞬态/锁定状态判定细化 | `research-supplement-4.md` §1.6 |
| 鼠标报告支持（passTouchToRust，SGR 鼠标编码） | `02-comprehensive-tridirectional-comparison.md` §3.1 |

### 3.4 渲染 / GPU / 字体
| 缺口 | 来源文档 |
|---|---|
| 行级脏缓存（只重绘变化行，torvox 当前全帧重绘） | `research_zelland_wgpu.md`、`research-all-projects.md` §P1 |
| 捏合缩放 pinch-to-zoom | `research_zelland_wgpu.md` §2.1、`research-all-projects.md` §1 |
| wgpu is_emulator 双后端（模拟器 GL / 真机 Vulkan）+ GL 兜底重试 | `research-shashlik.md` §3 |
| CellRun 游程编码（减少 GPU 上传与 drawText 调用） | `research-supplement-4.md` §1.3 |
| 自绘放大镜（系统 Magnifier 在部分 ROM 失效） | `research-supplement-4.md` §1.1 |
| findOptimalFontSize 精确二分（当前按 dp 粗调） | `research-supplement-4.md` §1.1 |
| 字体合成回退（CustomFallbackBuilder 缺字，中文/emoji） | `research-mid-repos-b.md` §2.4 |
| emoji 分类边界测试（防区间过度扩张 → tofu） | `research-warp-extra.md` §9.2 |
| ASystemFontIterator 字体发现（NDK 29+，替代 /system/fonts 扫描） | `research-warp.md` §5 |
| 无限 LOD 网格着色器（3D 背景/调试，需先加深度纹理） | `research-wgpu-example.md` §6.1 |
| 无 TTY 确定性帧 + PNG 截图（CPU 侧网格帧快照） | `research-fission.md` §6.2 |

### 3.5 会话 / 进程 / Bootstrap
| 缺口 | 来源文档 |
|---|---|
| 前台进程组跟踪（tcgetpgrp + 组杀，当前仅单进程 kill 会残留 vim/top） | `research-zed-port.md` §9.3-B |
| 终端环境分层（EnvOp Set/Remove overlay，当前编译期常量写死） | `research-zed-port.md` §9.3-A |
| bootstrap sha256 sidecar 校验（损坏 zip 不再"看起来已安装"） | `research-warp.md` §3、`research-warp-extra.md` §9.3 |
| PTY 初始 winsize 先于首读（spawn 与首帧 resize 竞态 → 折行错位） | `research-warp-extra.md` §9.4 |
| fast-death 恢复（shell 启动即崩检测 + 指数退避重试） | `research-warp-extra.md` §9.5 |
| 会话元数据持久化/重启恢复 | `research-mid-repos-a.md` §3.6 |
| scrollback 行数设置项 | `research-mid-repos-a.md` §4.6、§5.2 |
| 输出导出到文件 | `research-mid-repos-a.md` §2.6、§5.2 |
| 符号链接重放（bootstrap symlink 结构在 zip 解压下丢失） | `research-zed-port.md` §9.3-E |
| 多用户/主用户检查（TermuxInstaller.java:80-90） | `research-termux-app-extra.md` §5.7 |
| 内嵌 bootstrap zip 离线安装（可选） | `research-termux-app-extra.md` §5.7 |

### 3.6 安全 / MCP / Agent
| 缺口 | 来源文档 |
|---|---|
| ArgumentTokenizer（字符串→argv 安全拆分，MCP cmd 工具字符串命令） | `research-termux-kotlin.md` §7.1 |
| MCP socket SO_PEERCRED 校验（防同 uid 越权进程） | `research-termux-kotlin.md` §7.2 |
| MCP 同意门控（前台弹出 + 后台超时 DENY + ONCE_PER_SESSION） | `research-haven-extra.md` §19-2 |
| MCP screenshot 工具（framebuffer 读回，给 agent OCR/验证） | `research-mid-repos-a.md` §1.6、§5.2 |
| 命令安全分级/确认卡片（AI 工具调用 gate） | `research-mid-repos-b.md` §5.1、`research-other-repos.md` §7 |
| 能力开关环境变量导出（TORVOX__MCP_SERVER_ENABLED） | `research-termux-kotlin.md` §7.2 |
| 指纹锁（隐私模式） | `research-mid-repos-a.md` §2.6、§5.2（条件性） |

### 3.7 UX / 外围（P2 / 远期）
| 缺口 | 来源文档 |
|---|---|
| 隐私黑屏覆盖（切后台盖黑层防截屏） | `research-small-repos.md` §3.5 |
| OSC 9;4 进度环（Rust 侧解析，libghostty 不暴露） | `research-ghostty-android-extra.md` §5-5 |
| 自定义主题编辑器（working-copy + dirty + ColorPicker） | `research-ghostty-android-extra.md` §5-P2 |
| 背景图"复制私有存储 + 失效自愈"（修 URI 权限隐患） | `research-ghostty-android-extra.md` §5-3 |
| 标签条 / 顶部栏概念 | `research-ghostty-android-extra.md` §3.6-3.7 |
| 键盘弹出时"屏内滚动不缩字号" | `research-small-repos.md` §4.5 |
| 新输出复位滚动位置 | `research-small-repos.md` §4.5 |
| 节流持久化（NVRAM 5s + 退出保存 + 失败重试） | `research-small-repos.md` §4.5 |
| 快捷键录制（硬件键盘） | `research-mid-repos-b.md` §5.1 |
| tmux 集成 / SSH 客户端 / SFTP 断点续传 / mosh | `research-mid-repos-b.md` §5.1（SSH 系列，做 SSH 时 P0） |
| Split Panes 分屏 / Block 模型 / AI 集成 | `research-all-projects.md` §P2、`research-warp-extra.md` §11 |
| 悬浮窗终端 / 开机脚本 / proot 发行版 | `research-mid-repos-a.md` §5.2（远期/条件性，proot 不建议） |
| 设置 UI enabledWhen 门控、Slider 独占交互 | `research-ghostty-android-extra.md` §3.8、§5-2 |
| 响铃防抖（MIN_PAUSE + postDelayed 合并 + Vibrator NPE 防护） | `research-termux-kotlin.md` §7.5 |
| Logger 分块（logcat 4068B 上限）+ logPrivate | `research-termux-kotlin.md` §7.3 |

---

## 4. torvox 可吸收内容清单（按优先级排序）

> 标注来源文档与文件:行号（行号以研究时仓库快照为准）。P0=立即有价值，P1=近期，P2=远期/按需，另有"不吸收"汇总。

### P0（立即吸收，低成本高价值）

| # | 吸收项 | 来源文档 | 参考位置（仓库文件:行号） | torvox 落点建议 |
|---|---|---|---|---|
| 1 | 多击选择 tapCount 自计数（双击/三击） | `research-ghostty-android.md` §2.2 | `TerminalView.java:1051-1083` | `TerminalSurface.kt` 手势层 |
| 2 | Callback2 + onGetContentRect 菜单锚定 | `research-termux-app.md` §2.2 | `TextSelectionCursorController.java:194-215` | `SelectionActionCallback` 迁移 |
| 3 | 手柄拖动边缘滚动 + 顺序保持 + 宽字符吸附 | `research-termux-app.md` §2.4 | `TextSelectionCursorController.java:218-306,307-337` | `TerminalSurface.kt` HandleDrag |
| 4 | 选择文本：wrap 感知拼接 + 列→char 换算 | `research-termux-app-extra.md` §7-1/7-2 | `TerminalBuffer.java:52-106`、`TerminalRow.java:92-120` | `TerminalViewModel.extractSelectedText` |
| 5 | IME composing diff（最长公共前缀 + DEL 按 code point） | `research-warp-extra.md` §9.1 | `WarpInputView.kt:587-615` | `InputCoalescer` |
| 6 | bootstrap sha256 sidecar 校验 | `research-warp.md` §3、`research-warp-extra.md` §9.3 | `bootstrap.rs:1-48` | `BootstrapInstaller.kt` staging 安装 |
| 7 | PTY 初始 winsize 先于首读 | `research-warp-extra.md` §9.4 | `WarpTerminalService.kt:797-808` | `TerminalRuntime` spawn 路径 |
| 8 | ArgumentTokenizer 字符串→argv 安全拆分 | `research-termux-kotlin.md` §7.1 | `termux-shared/.../shell/ArgumentTokenizer.kt`（BSD） | MCP `cmd` 工具 + installer |
| 9 | MCP SO_PEERCRED 校验 | `research-termux-kotlin.md` §7.2 | `LocalSocketManager.getPeerCredNative`（termux-shared:316-323） | `mcp.rs` UnixStream accept |
| 10 | 终端环境分层 EnvOp overlay | `research-zed-port.md` §9.3-A | `crates/util/src/env.rs:19,52` | `shell_env.rs` 增加 overlay 字段 |
| 11 | URL 超链接正则 + 尾标点清理 + 括号平衡 | `research-zed-port.md` §9.3-C | `terminal_hyperlinks.rs:20,177` | 新 `hyperlinks.rs` 模块 |
| 12 | 前台进程组跟踪 + 组杀 | `research-zed-port.md` §9.3-B | `pty_info.rs:29-53` | `pty.rs` / `session.rs` |
| 13 | OSC 133 语义段模型 | `research-supplement-4.md` §1.2 | `TerminalEmulator.kt:830-930,1414-1452` | Rust 侧 ghostty OSC 133 处理 |
| 14 | Compose 键序列模式 | `research-supplement-4.md` §1.7 | `ComposeMode.kt:30`（~200 条组合表） | `ModifierBar` / `TerminalInputEncoder` |
| 15 | 搜索收窄保持当前匹配 | `research-gnome-console.md` §2 | `kgx-tab.c:191-250` | `TextSearchBar.kt` |
| 16 | 智能复制 smartCopy + 跨行 URL 选择 | `research-haven.md` §2.2/2.3 | `SelectionToolbar.kt:120-214,357-405` | `SelectionExpander.kt` |
| 17 | DECCKM 预推修复（非 PTY 直连会话前推 `ESC [?1h`） | `research-haven-extra.md` §19-1 | `MoshSession.kt:98-103` | 会话启动路径 |
| 18 | MCP 同意门控 | `research-haven-extra.md` §19-2 | `AgentConsentManager.kt:351-364` | mcp.rs 工具调用层 |
| 19 | wgpu is_emulator 双后端 + GL 兜底 | `research-shashlik.md` §3 | `app-surface/src/android.rs:25-37` | `wgpu_backend.rs` |
| 20 | effects 完整性对照（write_pty/DA/xtversion） | `research-small-repos.md` §1.5 | `ghostling main.c:1104-1189` | Rust 绑定核对 |
| 21 | 隐私黑屏覆盖 | `research-small-repos.md` §3.5 | `MainActivity.kt:190-213` | `MainActivity` onPause |
| 22 | 行级脏缓存 | `research-all-projects.md` §P1 | zelland `renderer/mod.rs` row_cache | `pass.rs` / `cell_builder.rs` |
| 23 | 捏合缩放 | `research-all-projects.md` §1 | zelland `renderer/android.rs` | 手势层 |
| 24 | 自定义主题链路（256 色立方 + ThemeStore 持久化） | `research-ghostty-android-extra.md` §5-1 | `TerminalTheme.java:55`、`ThemeStore.java:110` | `TerminalTheme` / `SettingsRepository` |

### P1（近期，中等成本有明确收益）

| # | 吸收项 | 来源文档 | 参考位置 | torvox 落点建议 |
|---|---|---|---|---|
| 1 | 背景图"复制私有存储 + 失效自愈" | `research-ghostty-android-extra.md` §5-3 | `BackgroundImageStore.java:51`、`MainActivity.java:424` | 背景图设置路径 |
| 2 | OSC 9;4 进度（Rust 侧解析 + 上报） | `research-ghostty-android-extra.md` §5-5 | `OscSideScanner` + `TabRing:288` | Rust PTY 读循环 |
| 3 | 搜索覆盖层 + 防抖 + 不触发 SIGWINCH | `research-ghostty-android-extra.md` §5-4 | `SearchBarView.java:54,105,116-120` | 搜索 UI 布局 |
| 4 | 用户主题 enabledWhen 门控补齐 | `research-ghostty-android-extra.md` §5-2 | `Setting.java:48-51,69` | `SettingsComponents.kt` |
| 5 | 会话元数据持久化/重启恢复 | `research-mid-repos-a.md` §3.6 | `SessionRepository.kt` + `SessionEntry` | DataStore + JNI 重 attach |
| 6 | Fn 第二层虚拟键 | `research-mid-repos-a.md` §3.6 | `VirtualKeyBar` sticky | `ModifierBar.kt` |
| 7 | 附加键栏自定义布局 | `research-mid-repos-a.md` §4.6 | `ExtraKeysView.kt` extra-keys 解析 | `ModifierBar.kt` + Settings |
| 8 | scrollback 行数设置项 | `research-mid-repos-a.md` §4.6、§5.2 | termx/terminator | SettingsRepository + JNI 参数 |
| 9 | 快死恢复 fast-death（秒退检测） | `research-warp-extra.md` §9.5 | `WarpTerminalService.kt:906-915` | BootGuard 体系 |
| 10 | Logger 分块 + logPrivate | `research-termux-kotlin.md` §7.3 | `Logger.kt:49-75,93-110` | LogUtil |
| 11 | BellHandler 防抖 | `research-termux-kotlin.md` §7.4 | `BellHandler.kt:34-63` | TerminalRuntime bell 事件 |
| 12 | emoji 分类边界负例测试 | `research-warp-extra.md` §9.2 | `lib.rs:2192-2307,2206` | 字形分段函数 |
| 13 | ASystemFontIterator 字体发现 | `research-warp.md` §5 | `font_render.rs` | `font_db.rs` |
| 14 | jni_fn 宏消除手写导出名风险 | `research-wgpu-in-app.md` §6-2 | `ffi/android.rs:10-19` | `android/ffi.rs` |
| 15 | wgpu_hal/naga 日志分级过滤 | `research-wgpu-in-app.md` §6-3 | `lib.rs:32-35` | `logging.rs` |
| 16 | LiveTest 协议（in-process 测试驱动） | `research-fission.md` §6.1 | `fission-test-driver/src/lib.rs:37-609` | 新 `live_test.rs` |
| 17 | 无 TTY 确定性帧 + PNG 截图 | `research-fission.md` §6.2 | `fission-shell-terminal/src/frame.rs:80-173` | `render/` 新增 frame_snapshot |
| 18 | 平台能力 trait + Memory/Unsupported | `research-fission.md` §4.3/§6.4 | `android_capabilities.rs` | JNI 能力层 |
| 19 | 快捷键录制（硬件键盘） | `research-mid-repos-b.md` §5.1 | reterminal 三件套 | 硬件键盘层 |
| 20 | 命令安全分级（MCP 增强） | `research-mid-repos-b.md` §5.1 | sushi-ssh CommandSafety | MCP 工具 gate |
| 21 | SSH + TOFU 全栈（若立项） | `research-mid-repos-b.md` §2.5、§5.3 | `SshConnector`/`SshTransport`/`MokeHostKeyVerifier`/`KnownHosts` | russh / sshj |
| 22 | 自绘放大镜 | `research-supplement-4.md` §1.1 | `MagnifyingGlass`（Terminal.kt） | `TerminalScreen.kt` |
| 23 | URL 缓存按滚动位置惰性重建 | `research-supplement-4.md` §1.3 | `TerminalScreenState.kt:155-213` | `TerminalViewModel` |
| 24 | CellRun 游程编码 | `research-supplement-4.md` §1.3 | `CellRun.kt:25-74` | `cell_builder.rs` 打包层 |
| 25 | 输出导出到文件 | `research-mid-repos-a.md` §2.6 | `TerminalActivity.setupExport()` | 会话菜单 |
| 26 | 键盘遮挡"屏内滚动不缩字号" | `research-small-repos.md` §4.5 | `TerminalView.kt:374-434` | `TerminalSurface` IME 弹出 |
| 27 | 新输出复位滚动 + 节流持久化 | `research-small-repos.md` §4.5 | `TerminalView.kt:530-540`、`MainActivity.kt:973-1024` | 滚动/状态持久化 |
| 28 | 粘贴确认对话框 | `research-gnome-console.md` §4 | kgx-paste-dialog | 粘贴路径 |
| 29 | MCP screenshot（framebuffer 读回） | `research-mid-repos-a.md` §1.6 | osmosis `render3d extract_texture` | MCP 工具 |
| 30 | 依赖边界检查脚本 | `research-mid-repos-a.md` §1.6 | osmosis `xtask/boundaries.rs` | CI cargo tree 断言 |

### P2（远期/条件性吸收）

- 无障碍朗读/语义导航（`research-supplement-4.md` §1.11；AccessibilityOverlay）
- 标签条 / 顶部栏（`research-ghostty-android-extra.md` §5-6/3.7）
- 自定义主题编辑器（大工程，`research-ghostty-android-extra.md` §5-P2；ColorPickerDialog `updating[]` 防回环必抄）
- 指纹锁（`research-mid-repos-a.md` §2.6；AppLock ~150 行）
- tmux 集成（`research-mid-repos-b.md` §2.5；Tmux.kt SSH exec + 解析）
- SFTP 断点续传（`research-mid-repos-b.md` §2.5；TransferManager 串行队列 + 任务持久化）
- 字体合成回退（`research-mid-repos-b.md` §2.4；CustomFallbackBuilder）
- 无限 LOD 网格（`research-wgpu-example.md` §6.1；需先加深度纹理）
- CI sccache（`research-wgpu-example.md` §6.3；rust.yml:10-21）
- 程序化几何生成器（`research-wgpu-example.md` §6.2；raytracing.rs:214-429）
- 指纹/隐私、悬浮窗终端、开机脚本（`research-mid-repos-a.md` §5.2）
- proot 发行版（`research-mid-repos-a.md` §2.6、§5.2；**当前不建议**，与 Termux bootstrap 定位冲突；若做则复用 DistroRegistry→init.sh 骨架）
- 主题编辑器、extra keys 宽度/副键/编辑器、字体文件导入（`research-ghostty-android-extra.md` §7）
- 多用户检查、内嵌 bootstrap zip 离线安装（`research-termux-app-extra.md` §5.7）
- AI 集成（warp_ai_mobile 为架构基准，`research-warp-extra.md` §9.7、§11）
- Split Panes / Block 模型（`research-all-projects.md` §P2、`research-warp-extra.md` §11；Block 与 libghostty-vt 冲突，不建议）

### 明确不吸收（供评审对照）

- termux Java 终端模拟器、termux-kotlin 的 Kotlin VT 状态机（libghostty-vt 已超越）—— `research-termux-app-extra.md` §6、`research-termux-kotlin.md` §9
- 手写 VT 解析器（terminator/termx/onecode）—— `research-mid-repos-a.md` §5.2
- ash 直接 Vulkan（wgpu 30 已封装）—— `research-warp.md` §7、`research-warp-extra.md` §11
- TermX 的 X11/VNC/SSH/SFTP 服务器、Cron、HTTP 服务器（反面教材：重复造轮子）—— `research-mid-repos-a.md` §4.7、§5.2
- fission 全家桶（widget 树/IR/布局引擎）、Bevy/Slint、winit/egui/OpenXR —— `research-fission.md` §5.2、`research-mid-repos-a.md` §1.5、`research-wgpu-example.md` §8
- proot 用户态方案（torvox native ELF + linker 性能更优）—— `research-other-repos.md` §3、`research-small-repos.md` §2.3
- zed 的 gpui 平台层、alacritty_terminal 引擎、Zed settings 体系 —— `research-zed-port.md` §9.4
- ply 的 `curl | sh` 无校验安装（反模式）—— `research-small-repos.md` §2.5
- 跨仓库 path 依赖结构（torvox 单仓库 + generated-patches 更优）—— `research-warp-extra.md` §11

---

## 5. 文档使用指南

### 5.1 文档地图

| 文档 | 覆盖范围 | 何时查阅 |
|---|---|---|
| `00-TORVOX-BASELINE.md` | torvox 功能基线（A 终端引擎 / B 渲染 / C JNI / D Kotlin UI / E 已有功能 / F 已确认无） | **任何研究/评审前必读**：判断"torvox 有没有"的唯一基准 |
| `02-comprehensive-tridirectional-comparison.md` | 渲染/选择/JNI/终端核心/Bootstrap/MCP 六维横向对比 + 依赖评估 + 代码注释索引 | 跨仓库横向决策（如"选择系统抄哪家"） |
| `research-all-projects.md` | 26 仓库速览 + P0/P1/P2 汇总 + 依赖推荐 + 缺失功能清单 | 全局优先级盘点（本 README §3/§4 的主要来源） |
| `research-{repo}.md` | 单仓库/单主题深度研究（函数/行号级） | 落地某个吸收项时精读对应章节 |
| `research-{repo}-extra.md` | 补充篇（主文档未覆盖的模块） | 主文档 + 补充篇合起来才是完整研究 |

### 5.2 推荐的开发流程

1. **立项/评估新功能**：先查本 README §3（缺口）→ 确认需求存在 → 查 §4（吸收项）看是否有现成参考 → 精读来源文档对应章节 → 按"代码注释建议"在 torvox 侧落地（所有研究文档的"可吸收内容"均附注释建议，直接复制到代码中并保留来源引用）。
2. **实现时对照**：使用 `00-TORVOX-BASELINE.md` 确认现状边界；使用 `02-comprehensive-tridirectional-comparison.md` §8 的代码注释索引，把参考来源写进 torvox 代码注释（`// 参考 xxx repo:line`），保持可追溯。
3. **评审时核对**：`research-*-extra.md` 中"torvox 已吸收/已超越"的条目，确认对应实现是否真的落地（如 SettingsComponents 的 ghostty-android Setting 模式）。
4. **架构决策**：涉及渲染/选择/会话/安全等关键路径时，交叉阅读多个仓库的同一主题章节（如选择系统看 ghostty-android + termux-app + termlib + haven 四家），再结合 ADR 定案。
5. **验收**：每个吸收项落地后，在 `docs/acceptance.md` 或对应测试中补回归用例（参考 `research-zed-port.md` §9.3-F 的"回归失败模式"纪律）。

### 5.3 优先级速查（一句话版）

- **P0 全在四类**：选择系统（ghostty-android/termux-app/haven）、输入与 IME（warp/termlib/termux-kotlin）、bootstrap 与进程（warp/zed-port/termux-kotlin）、渲染细节（zelland/shashlik/wgpu-in-app）。
- **P1 集中在**：UX 补强（搜索/背景图/设置）、工程基建（测试/日志/CI）、SSH 系列（若立项）。
- **P2/不吸收**：大产品线扩展（AI/proot/服务器类）与"已被 torvox 超越"的技术栈（Java/Kotlin VT 引擎、CPU 渲染、手写解析器）。

---

*本文档由 `docs/reference/` 下全部研究文档汇总生成；各条目详细论证、代码级注释建议与行号引用请回查来源文档。*
