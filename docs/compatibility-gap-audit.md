# Termux / Ghostty Android Terminal 生态兼容缺口深度审计

> 审计对象：torvox 与 `termux-app`、`ghostty-android-terminal`（GAT）、`termux-tools`、`termux-exec`、`termux-api`、`termux-boot`、`termux-styling`、`termux-tasker`、`termux-widget`、`termux-float`、`termux-packages`。
> 约束来源：GitHub `f` 分支 `docs/specification/config.md`、`docs/specification/DESIGN.md`、`docs/specification/PROHIBITED.md`、`AGENTS.md`。本地旧文档不作为依据。
> 目的：列出“容易兼容但尚未实现”的功能缺口，并评估修改后的稳定性、性能与复杂度。不考虑修改成本，但遵守禁止项与配置契约。

## 1. 审计口径

1. 只列与 torvox 现有架构容易兼容、且不违反 `PROHIBITED.md` 的缺口。
2. `config.md` 仅允许解析 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style` 三个 `termux.properties` 键；其余键一律不解析，只可作为应用内设置提供。
3. 以下能力按要求排除：
   - 诊断 / About 信息；
   - 密码输入提示；
   - XTVERSION；
   - 帮助页；
   - 外部应用文件接收；
   - 导出 / 分享转录；
   - 语义提示（OSC 133）；
   - Kitty 键盘协议中较复杂的部分。
4. 设计上明确不做或禁止的能力不列入，例如：bell 响应、光标闪烁、自定义主题、布局编辑器、会话持久化 / 恢复、背景图 / 透明 / 模糊、内嵌发行版、X11、MCP、termux-api、Shizuku、无障碍朗读等。

## 2. torvox 已实现的主要能力

- PTY 生命周期、前台进程组终止、带像素尺寸的 resize：`native/src/terminal/pty.rs`、`native/src/terminal/session/spawn.rs`
- Ghostty VT 解析、搜索、选择、OSC 8 超链接、OSC 52 剪贴板：`native/src/terminal/output_processor.rs`、`native/src/terminal/ghostty_terminal/*`
- 粘滞修饰键、修饰键栏自动重复、手势取消：`android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`
- IME 组字、替换、小提交直发、IME 动画跟随：`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- 滚动物理、滚动收尾、外接滚轮：`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- 字体选择、CJK 回退、固定主题目录：`android/app/src/main/java/terminal/emulator/settings/SystemFonts.kt`
- Bootstrap staging / 原子换入：`android/app/src/main/java/terminal/emulator/installer/*`
- 文档提供者、外部存储权限声明：`android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt`
- 前台服务、通知、唤醒锁续期、Process completed 提示：`android/app/src/main/java/terminal/emulator/service/TerminalForegroundService.kt`
- 鼠标上报（触摸路径与滚轮）：`native/src/terminal/ghostty_terminal/internal/mouse_input.rs`
- Kitty 图像协议与 Unicode 占位符：`native/src/terminal/ghostty_terminal/internal/kitty.rs`

## 3. P0：低成本、低风险

| # | 功能 | 现状 | 稳定性 | 性能 | 复杂度 | 说明 |
|---|---|---|---|---|---|---|
| 1 | Wi-Fi 锁与唤醒锁成对持有 | 仅 `PARTIAL_WAKE_LOCK` | 中：后台网络 TUI 更稳 | 低 | 低 | Termux `TermuxService` 做法；加 `WIFI_MODE_FULL_HIGH_PERF`，与现有续期逻辑成对 |
| 2 | 终端边距调节 toggle | 无 | 低 | 可忽略 | 低 | 应用设置 + Compose padding，不解析 `termux.properties` |
| 3 | 软键盘自动显示策略 | 固定 toggle | 低 | 可忽略 | 低 | `enabled` / `only-if-no-hardware` 两种策略 |
| 4 | 默认硬件键盘快捷键 | 无拦截 | 低 | 可忽略 | 低-中 | Ctrl+Shift+C/V/F、Ctrl+±/0、Tab 切换等 |
| 5 | Virtual Kitty placements 上限 | 无显式上限 | 低 | 中：内存 | 低 | 动态扩容或提高上限，避免大图洪流 |
| 6 | 键码表查漏 | 已覆盖大部分 | 低 | 可忽略 | 低 | NumPad、Caps/Scroll Lock、Break、SysRq 等已映射，仅需持续对照 Android 新键码 |

## 4. P1：输入 / 协议 / 渲染

| # | 功能 | 现状 | 稳定性 | 性能 | 复杂度 | 说明 |
|---|---|---|---|---|---|---|
| 7 | Kitty 键盘 release（仅不费力部分） | 仅 press | 中 | 可忽略 | 中 | 仅协议要求时发送 release；repeat 延后 |
| 8 | 颜色方案上报（CSI ?996n / mode 2031） | 未实现 | 低 | 可忽略 | 中 | 主题亮度变化推送，防抖 |
| 9 | 工作目录跟踪（OSC 7/9/1337） | 未 surfaced | 低 | 低 | 中 | 会话标题 / 新会话 cwd，不新增语义提示 |
| 10 | 更丰富的选区手势 | 仅词选 / 全选 | 低 | 可忽略 | 中 | 选行、词间拖动、矩形选择 |
| 11 | 物理鼠标点击 / 悬停上报 | 仅滚轮 | 中：DeX / Chromebook | 低 | 中 | 按钮 / 悬停 / 真实修饰键 |
| 12 | OSC 9;4 进度上报 side-scanner | 未实现 | 低 | 低 | 中 | GAT 有 side-scanner；torvox 可选实现 |
| 13 | 下划线颜色与上划线渲染复核 | 原生已捕获并渲染 | 低 | 可忽略 | 低 | 已有 deco 通道；仅需确认主题对比度与 DPI |

## 5. P2：可复刻为应用设置的 Termux 属性语义

不解析 `termux.properties`，只作为应用内设置提供：

| # | 功能 | 现状 | 稳定性 | 性能 | 复杂度 | 说明 |
|---|---|---|---|---|---|---|
| 14 | Ctrl+Space workaround toggle | 未实现 | 低 | 可忽略 | 低 | 应用设置 |
| 15 | Char-based input toggle | 未实现 | 低 | 可忽略 | 中 | 应用设置 |
| 16 | Fullscreen workaround toggle | 未实现 | 低 | 可忽略 | 中 | 应用设置 |
| 17 | 点击打开 URL toggle | 未实现 | 低 | 可忽略 | 中 | 需先实现 URL 检测 |

## 6. 明确排除

- 诊断 / About：不需要。
- 密码输入提示、XTVERSION、帮助页、外部文件接收、导出 / 分享转录：绝对禁止。
- OSC 133：不做。
- Kitty 键盘：只做不费力部分。
- 新增 `termux.properties` 键：不解析。
- Bell 震动 / 蜂鸣：设计明确忽略 bell。
- 光标闪烁：设计明确不做。

## 7. 推荐顺序

1. P0 全部。
2. P1 中 7、8、9、10、11、12。
3. P2 作为可选应用设置。

## 8. 风险摘要

- **低风险**：Wi-Fi 锁、边距、软键盘策略、默认快捷键、键码表、Virtual placements 上限。
- **中风险**：Kitty 键盘 release、颜色方案上报、工作目录跟踪、选区手势、物理鼠标、OSC 9;4 side-scanner。主要风险是输入 / 渲染分支增多，需要 instrumented 测试。
- **高风险 / 高复杂度**：语义提示、导出转录、文件接收器、用户态 rootfs、X11、MCP 等，已排除。
- **性能**：大多数候选影响可忽略；Virtual placements 扩容与 OSC 9;4 扫描需特别关注内存与后台线程。
- **复杂度**：P0 低；P1 中；P2 低-中。

## 9. 验证建议

- Wi-Fi 锁：后台网络 TUI（如 `wget`、`ssh`）灭屏 30 分钟后仍活跃。
- 终端边距：设置变更后 Compose padding 立即生效，无黑边。
- 软键盘策略：硬件键盘连接时按策略隐藏/显示。
- 默认快捷键：Ctrl+Shift+C/V/F、Ctrl+±/0、Tab 切换均生效。
- Virtual placements：大图输出不 OOM，缓存按 generation 清理。
- Kitty release：仅在启用 kitty 键盘协议时发送 release。
- 颜色方案上报：主题切换后 CSI ?996n 响应正确。
- 工作目录跟踪：OSC 7/9/1337 更新会话标题或新会话 cwd。
- 选区手势：选行、词间拖动、矩形选择不卡顿。
- 物理鼠标：DeX/Chromebook 上按钮、悬停、滚轮均上报。
- OSC 9;4：进度上报 side-scanner 不阻塞 PTY 主路径。
