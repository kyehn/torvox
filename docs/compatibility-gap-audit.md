# Termux / Ghostty Android Terminal 生态兼容缺口审计

> 审计对象：torvox 与 `termux-app`、`ghostty-android-terminal`（GAT）、`termux-tools`、`termux-exec`、`termux-api`、`termux-boot`、`termux-styling`、`termux-tasker`、`termux-widget`、`termux-float`、`termux-packages`。  
> 约束来源：`docs/specification/config.md`、`docs/specification/DESIGN.md`、`docs/specification/PROHIBITED.md`、`AGENTS.md`。  
> 目的：列出“容易兼容但尚未实现”的功能缺口，并评估修改后的稳定性、性能与复杂度。不考虑修改成本，但遵守禁止项与配置契约。

## 1. 审计口径

1. 只列与 torvox 现有架构容易兼容、且不违反 `PROHIBITED.md` 的缺口。
2. 不新增 `termux.properties` 键：`config.md` 仅允许 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style`。
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

- PTY 生命周期、前台进程组终止、带像素尺寸的 resize。
- Ghostty VT 解析、搜索、选择、OSC 8 超链接、OSC 52 剪贴板。
- 粘滞修饰键、修饰键栏自动重复、手势取消。
- IME 组字、替换、小提交直发、IME 动画跟随。
- 滚动物理、滚动收尾、外接滚轮。
- 字体选择、CJK 回退、固定主题目录。
- Bootstrap staging / 原子换入。
- 文档提供者、外部存储权限声明。
- 前台服务、通知、唤醒锁续期、Process completed 提示。
- 鼠标上报（触摸路径）。

## 3. P0：低成本、低风险

| # | 功能 | 现状 | 稳定性 | 性能 | 复杂度 | 说明 |
|---|---|---|---|---|---|---|
| 1 | Wi-Fi 锁与唤醒锁成对持有 | 仅 partial wake lock | 中：后台网络 TUI 更稳 | 低 | 低 | Termux `TermuxService` 做法；加 `WIFI_MODE_FULL_HIGH_PERF` |
| 2 | 终端边距调节 toggle | 无 | 低 | 可忽略 | 低 | 应用设置 + Compose padding |
| 3 | 软键盘自动显示策略 | 固定 toggle | 低 | 可忽略 | 低 | `enabled` / `only-if-no-hardware` |
| 4 | 默认硬件键盘快捷键 | 无拦截 | 低 | 可忽略 | 低-中 | Ctrl+Shift+C/V/F、Ctrl+±/0、Tab 切换 |
| 5 | Virtual Kitty placements 上限 | 可能受限 | 低 | 中：内存 | 低 | 动态扩容或提高上限 |
| 6 | 键码表补齐 | 部分缺口 | 中：小键盘 / 特殊键 | 可忽略 | 低 | NumPad、Caps/Scroll Lock、Break、SysRq |

## 4. P1：输入 / 协议 / 渲染

| # | 功能 | 现状 | 稳定性 | 性能 | 复杂度 | 说明 |
|---|---|---|---|---|---|---|
| 7 | Kitty 键盘 release（仅不费力部分） | 仅 press | 中 | 可忽略 | 中 | 仅协议要求时发送 release；repeat 延后 |
| 8 | 颜色方案上报（CSI ?996n / mode 2031） | 未实现 | 低 | 可忽略 | 中 | 主题亮度变化推送，防抖 |
| 9 | 工作目录跟踪（OSC 7/9/1337） | 未 surfaced | 低 | 低 | 中 | 会话标题 / 新 Tab cwd |
| 10 | 下划线颜色与上划线渲染 | native 已捕获，UI 未用 | 低 | 可忽略 | 中 | 需渲染颜色通道 |
| 11 | 更丰富的选区手势 | 仅词选 / 全选 | 低 | 可忽略 | 中 | 选行、词间拖动、矩形选择 |
| 12 | 物理鼠标点击 / 悬停上报 | 仅滚轮 | 中：DeX / Chromebook | 低 | 中 | 按钮 / 悬停 / 真实修饰键 |
| 13 | OSC 9;4 进度上报 side-scanner | 未实现 | 低 | 低 | 中 | GAT 有 side-scanner；torvox 可选实现 |

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
2. P1 中 6、8、9、10、11、12、13。
3. P2 作为可选应用设置。

## 8. 风险摘要

- **低风险**：Wi-Fi 锁、边距、软键盘策略、默认快捷键、键码表、Virtual placements 上限。
- **中风险**：Kitty 键盘 release、颜色方案上报、工作目录跟踪、下划线颜色 / 上划线、选区手势、物理鼠标、OSC 9;4 side-scanner。主要风险是输入 / 渲染分支增多，需要 instrumented 测试。
- **高风险 / 高复杂度**：语义提示、导出转录、文件接收器、用户态 rootfs、X11、MCP 等，已排除。
- **性能**：大多数候选影响可忽略；导出转录与 Virtual placements 扩容需特别关注内存与后台线程。
- **复杂度**：P0 低；P1 中；P2 低-中。
