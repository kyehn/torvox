# 深度研究补充：ghostty-android-terminal (sylirre) —— UI/设置/主题/native 集成

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/ghostty-android`（depth 1）
> 本文是 `research-ghostty-android.md`（已覆盖 `TerminalView.java` 选择系统）的**补充篇**，覆盖该仓库其余全部源码：
> `app/src/main/java/io/github/sylirre/terminal/ui/` 下全部 Java、`app/src/main/cpp/`（JNI/native 层）、`app/build.gradle`。
> 对比对象：torvox（Kotlin + Compose + Rust libghostty-rs 渲染，`android/app/src/main/java/terminal/emulator/`）。

---

## 1. 文件总览（模块地图）

ghostty-android 的 Android 端分三层（约 30 个 Java 文件 + 4 个 C 文件）：

| 层 | 内容 |
|---|---|
| `term/`（模拟器/会话） | `TerminalNative`（JNI 面）、`TerminalEmulator`（锁）、`TerminalSession`（PTY）、`SessionManager/Service`、`ScreenSnapshot`、`OscSideScanner`、`Userland*`（arm64chroot rootfs）、`RootfsBackup` |
| `ui/`（界面，本文主体） | 主屏 `MainActivity`/`TerminalView`/`TabStripView`/`SearchBarView`/`ExtraKeysView`；次屏 `SettingsActivity`/`ThemeActivity`/`ExtraKeysActivity`/`OnboardingActivity`；设计系统 `Chrome`/`ChromePalette`/`EdgeInsets`/`TopBarView`/`Dialogs`；模型 `AppSettings`/`ThemeStore`/`ExtraKeysConfig`/`TerminalTheme`/`TerminalFontStore`/`BackgroundImageStore` |
| `cpp/`（native） | `pty_jni.c`（PTY/fork/exec）、`terminal_jni.c`（libghostty-vt 绑定，~2300 行）、`kitty_unicode.c`、`png_decode.c`（stb_image）、`chroot_ng_embed.c`、CMakeLists.txt |

**整体架构要点**（CLAUDE.md + docs/architecture.md）：
- 纯 Java 自绘（无 AppCompat/Material/Compose 运行时依赖，主依赖只有 `org.tukaani:xz`），minSdk 29（Zig 构建的 libghostty-vt.a 用 ELF TLS）、targetSdk 36。
- native 回调（write-pty/bell/title/size）**只缓冲不回调 Java**：效果在 `feed()` 后由 Java 轮询（`TerminalEmulator` 全同步锁）。
- 主题色通过 `TerminalEmulator.setColors` 推进 libghostty-vt；主屏 chrome（顶栏/标签/键帽/搜索）由 `ChromePalette` 从主题背景派生。

---

## 2. 逐文件功能说明（文件:行号）

### 2.1 MainActivity（ui/MainActivity.java，80 行起）

单 Activity 宿主：标签条 + TerminalView + extra-keys 工具栏；**会话存在 `SessionManager` 进程单例里，Activity 只绑定/重绑当前会话**（注释 :68-71）。

- `onCreate` :128 —— 绑定视图、根 insets 监听（edge-to-edge，IME 跟踪）、onboarding 判定（:144-147：未完成且无 rootfs → 启动 `OnboardingActivity`）、`exitReceiver`（通知栏 Exit 广播 :119）。
- `applyAllSettings` :321 —— **设置生效的总入口，只在 `onResume` 调用一次**：主题→背景图→字体→边距→窗口行为→输入开关→extra keys。次屏（Settings/Theme/ExtraKeys）改的是进程级 store，返回主屏时统一重放。
- `applyTheme` :354 —— `themeStore.current()` → `toPalette256()` → 对**每个**会话 `emulator.setColors(fg,bg,cursor,palette)` + `setCursorStyle` + `setGraphemeClustering`；随后 `applyChrome`。
- `applyChrome(ChromePalette)` :377 —— 把主屏整套 chrome 从主题背景重着色：root 背景、顶栏、按钮、标签条、extra keys、搜索条、系统栏亮/暗图标（API 30+ `setSystemBarsAppearance`）。
- `applyBackgroundImage` :417 —— `BackgroundImageStore.decode`（按屏幕尺寸降采样 + 模糊）→ `terminal.setBackgroundImage(bmp, alpha)`；解码失败即清掉持久化路径（stale/corrupt 自愈）。
- `applyTerminalFonts` :431 —— 加载 4 个字体槽（regular/bold/italic/bold-italic），解码失败同样自愈清路径。
- `createFirstSession` :464 / `createSession` :469 —— 首个会话由 layout pass 后的 pending listener 触发（避免宽度为 0 时 spawn）；`:491` userland 分支组装 `UserlandOptions`。
- 标签操作：`switchTo` :517、`confirmCloseTab` :579（`confirmSessionClose` 开关）、`closeTab` :591（关最后一个自动重建；:1152 启动失败的自愈：关掉→toast→重开）、`updateTabs` :1025、`tabTitle` :1020（OSC 标题 + 序号）。
- 搜索：`showSearch` :531 / `hideSearch` :537 / `setSearchButtonActive` :550（搜索按钮高亮跟随搜索条开关状态）；`:565 onBackPressed` 优先关搜索条。
- 提示导航（prompt nav）：`applyPromptNav` :936 / `updatePromptNav` :945 —— 滚到底部时显示"跳到最近提示符"上下文 chip（`animateContextualChip` :952）。
- 设置入口 `openSettings` :639 —— `startActivityForResult(REQ_SETTINGS)`；备份/恢复是终端耦合流程，SettingsActivity 以 result 交回 MainActivity 执行（`onActivityResult` :704 的 ACTION_BACKUP/RESTORE/SETUP_USERLAND 分发）。
- `maybePromptBatteryOptimization` :621 —— 只问一次（SharedPreferences 标记）、`forceShell` 测试缝下跳过、无此 UI 的设备静默跳过。
- 会话监听器（`TerminalSession.Listener`）：`onUpdate` :1043、`onTitleChanged` :1048、`onBell` :1053（四模式：off/haptic/screen-flash/sound，带 300ms 节流）、`onClipboardWrite` :1069（OSC 52 写剪贴板 + 1500ms 节流 toast）、`onClipboardQuery` :1084（OSC 52 读 + `sanitizeClipboardSelection` :1106 防二进制垃圾）、`onProgress` :1116（OSC 9;4 进度 → TabStripView 环）、`onExited` :1144。
- `runBackup` :745 / `runRestore` :777 —— rootfs tar.gz 流式备份/恢复（`RootfsBackup` + 进度条 `ProgressHandle` :895，支持取消，AtomicBoolean 协同）。

### 2.2 设计系统：Chrome / ChromePalette / EdgeInsets / TopBarView / Dialogs

**Chrome（ui/Chrome.java，:23）** —— 纯代码视图的"设计令牌"门面：`color` :28、`dp` :33、`dimen` :38 解析 `res/values` 令牌；`rounded` :45（圆角实心矩形）、`ripple` :62（accent 半透明按压涟漪 + 圆角 mask）、`stateful` :92（pressed/normal 状态列表，用于 sticky 修饰键的锁定态）。设计意图（注释 :15-22）：布局与 Java 画同一套颜色，杜绝手写 hex 常量。

**ChromePalette（ui/ChromePalette.java，:34）** —— **主屏 chrome 随主题背景派生**（`from(Context, terminalBg)` :81）：
- 亮度判定 `Color.luminance(bg) < 0.5f`（:84）：深色 → 原样使用 stock 令牌（indigo-on-near-black 品牌身份，默认主题渲染不变）；浅色（如 Solarized Light）→ 浅 chrome：surface 向黑步进（0.045/0.08/0.13/0.18 混合，`mix` :121），ink 翻成近黑中性色，accent 保持品牌 indigo。
- 附带**palette 解析版 drawable 工厂**（Chrome 的孪生）：`rounded` :131、`ripple` :145、`pressRipple` :160、`barSurface` :180（surface1 + 一条发丝线 divider，`edgeAtBottom` 选顶/底边——顶栏/搜索条用底边、extra-keys 工具栏用顶边）。

**EdgeInsets（ui/EdgeInsets.java，:29）** —— 次屏（设置/主题/extra-keys）的 edge-to-edge 方案：**逐表面路由 insets**（`apply` :33）——状态栏 → 顶栏额外 top padding（表面延伸到状态栏下）；导航栏 → 滚动内容底部 padding（`clipToPadding` 关，内容可滚到栏下）；左右/刘海 → root padding。注释 :13-24 明确反对"整个 root 加 padding"（会在顶栏上方/内容下方留下 window-background 色带）。

**TopBarView（ui/TopBarView.java，:31）** —— 次屏共享顶栏（返回箭头 + 标题 + 尾部文字动作槽）：构造 :37（表面 + 发丝底边 + elevation 阴影），`addTextAction(labelRes, prominent)` :87（Done 用 accent 加粗、Reset 用 text_secondary）。一个组件保证 Settings/Theme/ExtraKeys 三个次屏几何与视觉一致。

**Dialogs（ui/Dialogs.java，:32）** —— 共享对话框词汇：`field` :41（URI 输入类型防自动纠正、monospacePath 模式软换行不滚动，PATH/命令可读性）、`prompt` :69/:76（**内联校验**：`validate` 返回错误串则错误显示在字段下方、对话框不关闭、已输入内容不丢；"Use default" 中性按钮；正按钮 listener 在 `show()` 后安装以阻止默认关闭 :99/:113）、`confirmDanger` :132（危险动作确认，正按钮 tint 成 danger 色）。

### 2.3 主题系统：TerminalTheme / ThemePresets / ThemeStore / ThemeActivity / ThemePreviewView / ThemePickerDialog / ColorPickerDialog

**TerminalTheme（ui/TerminalTheme.java，:19）** —— 不可变主题：name + fg/bg/cursor + 16 ANSI 色（构造校验长度 :33）。**`toPalette256` :55** 是关键：0–15 用 ANSI 色，16–231 生成标准 xterm 6×6×6 色立方（levels {0,95,135,175,215,255}），232–255 24 级灰阶——"只有 16 个命名色可主题化，256 色程序仍正确渲染"。`toCsv` :75 / `fromCsv` :95 做紧凑持久化（19 个 hex，逗号分隔），坏串抛异常由调用方跳过。

**ThemePresets（ui/ThemePresets.java，:17）** —— 只读内置主题列表（:22 `ALL`）：Default（经典 xterm 白底黑字）、Solarized Dark/Light、Dracula、Nord、One Dark、Tokyo Night 等；`DEFAULT` :80 = 第一个；`isPreset` :83 防用户覆盖内置名。

**ThemeStore（ui/ThemeStore.java，:27）** —— 主题的"真相源"：presets + 用户主题 + 当前选择，全部持久化在一个命名 SharedPreferences 文件（`themes`，:29）里。用户主题存 JSON 数组 `{name, colors}`（`saveUserTheme` :110，重名替换、preset 名拒绝）；读取时**逐条容错**（:57 坏条目跳过而不是全丢）；`current` :84 按名解析、失败回退 DEFAULT；`nameExists` :102 防重名。

**ThemeActivity（ui/ThemeActivity.java，:54）** —— 全屏主题编辑器。模型（注释 :47-53）：单一可变 working copy（`fg/bg/cursor/ansi[16]`，:71-72），编辑标 `dirty`，**只有显式 Save / Save as… 才持久化颜色**；选主题立即持久化选择（返回主屏 `onResume` 生效）；未保存草稿退出时确认（`confirmIfDirty`）。`CURSOR_STYLES` :92 只暴露 block/underline/bar（引擎支持的 hollow block 故意不暴露）。还承载三个全局设置：背景图（`bgOpacity/bgBlur` SeekBar + 预览位图 :83-86）、光标形状/闪烁、4 个字体槽的文件选择（`REQ_PICK_*_FONT` :61-65）。色板格子 `swatchFill` :639（hairline 描边保证黑色可见）。

**ThemePreviewView（ui/ThemePreviewView.java，:32）** —— 纯自包含的主题预览（不需要模拟器）：画模拟 shell 提示符、彩色文件列表、错误/警告行、光标（形状+闪烁镜像全局设置，`BLINK_MS` 530 :35 由 `blinkTick` :62 驱动）、所有字体样式样本、16 个 ANSI 色条（:37-43 布局常量与 onDraw 严格对应防漂移）。字体大小取用户真实字号（clamp 10–18sp，:81-85）。`setBackgroundImage` :109 让模糊/透明度滑杆有实时反馈。

**ThemePickerDialog（ui/ThemePickerDialog.java，:29）** —— 主题选择器：每行 = 名称 + **MiniPaletteView 迷你色条**（:111，bg/fg/6 个主 ANSI 色小圆角格），分"内置预设/我的主题"两节（`addSection` :58），选中行 accent 底。替换旧的纯名称单选列表。

**ColorPickerDialog（ui/ColorPickerDialog.java，:46）** —— 纯代码构建的视觉取色器（无第三方依赖）：SV 饱和度/明度场（`SvFieldView`）+ 色相条（`HueBarView`）+ 前后对照色块 + #RRGGBB hex 输入（`HexFilter` :366 只收 0-9A-F）+ 主题色快捷 chips。**防回环守卫**（:104 `updating[]` 单布尔）防止字段/条/hex 三个监听器互相触发死循环；`:62` HSV 初始化。

### 2.4 字体：TerminalFontStore（ui/TerminalFontStore.java，:20）

**4 个槽位**：regular/italic/bold/bold-italic（`DEFAULT/ITALIC/BOLD/BOLD_ITALIC` :22-25），每个一个固定文件（`terminal_font_default` 等，:27-30）存在 `getFilesDir()`。
- `importFrom` :53 —— **把 document-provider 选的字体复制进 app 私有存储**（注释 :16-18：content URI 权限不跨进程存活），先写临时文件再原子 rename（:74-78）。
- `load` :82 —— `Typeface.createFromFile`，失败返回 null（调用方据此自愈清路径）。
- 设计点：缺 italic 文件时 TerminalView 回退到合成斜体（ThemeActivity 注释 :100-101）；字体是**全局外观设置**，不属主题 working copy（不标 dirty）。

### 2.5 背景图：BackgroundImageStore（ui/BackgroundImageStore.java，:27）

与字体同思路：`importFrom` :51 把 picked 的 `content://` URI **复制到固定私有文件**（`terminal_background`，:29），持久化的就是这个路径（`AppSettings.backgroundImagePath`）。
- `decode(path, reqW, reqH, blurPercent)` :76 —— 先 `inJustDecodeBounds` 读尺寸 → 2 的幂 `inSampleSize` 降采样（:91，保持 ≥ 请求尺寸的最小采样）→ 可选模糊。
- `blur` :105 —— **自写双趟 box blur**（水平+垂直，`BLUR_PASSES=2` :38 近似高斯），工作位图最长边 ≤ `BLUR_MAX_DIM=1080`（:36，模糊本来就会丢细节，先缩小再模糊省内存），半径 = `blurPercent/100 × 0.05 × 短边`。注释 :99-104 明确选 box blur 而非 RenderScript/RenderEffect 是为了全 API 级别可移植。
- `clear` :65、内存安全：模糊中回收中间位图（:107 `scaleDown` 收缩时回收 src）。

### 2.6 extra keys：ExtraKey / ExtraKeysConfig / ExtraKeysView / ExtraKeysActivity / KeyCaps / KeyCapView / Glyphs

**ExtraKey（ui/ExtraKey.java，:48）** —— 不可变按键描述符。三类（`Kind` :50）：`KEY`（非打印键走 `TerminalView.dispatchKey`）、`TEXT`（字面字符串走 `dispatchText`）、`MODIFIER`（粘性 CTRL/ALT）。KEY/TEXT 可带 `mods` 位掩码 = **单击组合键**（Ctrl-C、Ctrl-→），与粘性修饰键正交（分发时 OR 合并，注释 :20-26）。`id` 是持久化令牌：目录键 `"esc"`、`"lit:<text>"`（自定义文本键）、`"combo:<mods>:<base>"`（修饰组合，mods 取 `"CAS"` 子集）。工厂 `key/text/modifier/comboKey/comboText` :78-95，装饰器 `withWidth` :98 / `withSecondary` :103（swipe-up 副键，含宽 1.0/1.5/2.0 与副键引用）。

**ExtraKeysConfig（ui/ExtraKeysConfig.java，:52）** —— extra keys 的"真相源"：内置目录 + 命名 profile（每个 = 有序行 × 键位）。
- 常量：`DEFAULT_IDS` :75（esc ins del pgup home up end / tab ctrl alt pgdn left down right）、`DEFAULT_ROWS` :103（两行拆分的倒 T 箭头簇）、`MAX_ROWS` :83（=4，行吃纵向空间）、`MAX_PROFILES` :86（=8）、`COMBO_PRESET_IDS` :114（Ctrl-C/d/z/l/r/a/e/u/k/w/\\、Shift-Tab、Ctrl-←/→ 的"添加键"预设面板）。
- 持久化单 JSON 值（key `"order"` :56），**读取器兼容 v2/v1/v0 三种形状**（:35-44：v2 对象含 profiles 数组、v1 数组套数组 = 单 Default profile 多行、v0 扁平数组 = 单行），老安装自动迁移。
- 目录刻意限定在 native `map_keycode` 真能编码的键（注释 :48-50："提供按了没反应的键 = 死按钮"）。
- `reset` :689、测试缝 `seedRawForTest` :696（迁移测试直接注入原始持久化值）。

**ExtraKeysView（ui/ExtraKeysView.java，:54）** —— 软键盘上方的全宽弹性网格"键盘"：行 = 水平 LinearLayout，键 `layout_weight = key.width`（1.0/1.5/2.0），**永不滚出屏幕**（注释 :36-40）。
- 手势（:42-52）：tap 主键；**swipe up 发副键**（`swipeThresholdPx` :84 = 2×touchSlop 与 20dp 取大）；长按 KEY/TEXT 自动重复（`REPEAT_INTERVAL_MS=80` :81）、长按 MODIFIER 锁定。
- 粘性修饰：`TerminalView.StickyModifiers` 单例（:57，`attachTerminal` :122 注入），`updateToggles` 实时重着色已按下的 ModButton（:90）。
- 视图开关与配置：`setRowEnabled` :137（整条隐藏但不动配置）、`setHideWhenKeyboardHidden` :146（IME 收起时工具栏隐藏，`setKeyboardVisible` :177 由 Activity 喂 IME 状态）、`setShowSwitch` :155（多 profile 时前导切换列，`buildSwitchColumn` :227：点按循环切换、长按列表选择）、`setKeyVerticalPaddingDp` :166（行高旋钮）。
- `reload` :184 全量重建；`applyPalette` :115 换色即重建。
- `keyBgLocked` :444 —— 锁定态用 accent 底（与普通按压涟漪区分）。

**KeyCaps（ui/KeyCaps.java，:22）** —— **键帽样式的唯一出处**：`make` :32（等宽粗体、令牌 padding、pressed-fill+涟漪背景）；`uniformize` :60 —— **行内统一字号**：先按最窄键可容纳的尺寸算全行统一 sp（8–15sp 区间），再逐个 `setUniformTextSizeSp`，防止"每键各自 autosize"导致同行字大小混杂。注释 :16-21 提到曾发生编辑器与工具栏 3dp/6dp 漂移，故收敛到单一工厂保证 WYSIWYG。

**KeyCapView（ui/KeyCapView.java，:26）** —— 键帽视图，**故意用 TextView 子类**（注释 :18-24）：主标签保持真文本（`Glyphs.applyTo` 可加 span、Espresso `withText("ESC")` 能匹配），`onDraw` :90 在右上角叠画副键提示（`hint` StaticLayout 预排 :50-83）。构造 :38 开 `setAutoSizeTextTypeUniformWithConfiguration`（8–15sp 自动收缩长标签），`setUniformTextSizeSp` :50 用固定尺寸替换 per-cap autosize。

**Glyphs（ui/Glyphs.java，:33）** —— **符号字形矢量图标化**：`MAP` :47 把 ▲↑▼↓←→↵⌫─✕×☰🔍⚙ 等码点映射到 bundle 的 VectorDrawable，`apply` :72 扫描标签逐码点替换成 tinted `ImageSpan`（复合标签如 `"CTRL ◀"` 仍正常：修饰符是文本、箭头是图标）；未映射码点原样保留（自回退）。**`CenteredIconSpan` :112 是关键修复**：`getSize` 取字体行盒与居中图标盒的并集（:117-131），避免纯图标键比文本键矮（stock `ALIGN_CENTER` 会用 drawable 高度覆写行高）。注释 :30-31：minSdk 29 保证框架 VectorDrawable 可用，无需 PNG 兜底。

### 2.7 搜索：SearchBarView（ui/SearchBarView.java，:45）

Find bar：圆角查询框（前导搜索图标）+ 匹配计数 + 大小写开关 + 上一个/下一个 + 关闭。
- **覆盖式布局**（注释 :35-38）：是主布局 FrameLayout 的子层（顶栏下方），**打开不改变 terminal 尺寸 → 不触发 SIGWINCH**；200ms 滑入/淡出。
- `Listener` :47 接口把查询委托回 MainActivity → TerminalView 搜索方法；实现 `TerminalView.SearchListener` :45 接收 native 推回的实时计数。
- 输入防抖 `DEBOUNCE_MS=150` :54（`postDelayed` :110-114）；IME 回车：待发查询先 flush、否则当"下一个"（:116-120）。
- 输入类型 `TYPE_TEXT_VARIATION_FILTER`（:101-103 无纠正无建议）+ `IME_FLAG_NO_EXTRACT_UI`。
- 计数显示 `lastCountNoResults` :65 状态用于"无结果"样式；`applyPalette` 随主题 chrome 重着色；尺寸与顶栏对齐（:77-80 注释：与顶栏等高同列）。

### 2.8 标签页：TabStripView（ui/TabStripView.java，:49)

会话标签条：横向滚动圆角 pill 行（每 pill 带 ✕，活动项 accent 边框环）+ **钉死的 + 按钮**（永不滚出视口）。
- `Listener` :51：onTabSelected/Closed/NewTab/**onNewTabLongPress**（长按 + 开非默认会话类型，MainActivity :241 接 rootfs 未装则进 onboarding setup-only）。
- **OSC 9;4 进度**：`TabProgress` :60（`PROGRESS_*` 状态 + 0..100），`TabRing` :288 自定义 Drawable 在 ✕ 周围画进度环（indeterminate 自旋动画 :318），错误红/暂停黄（:107-108）——与 ✕ 几何分离，两个信号不打架（注释 :39-41）。
- **`update` :158 原地调和而非重建**：标题变化只改 label（:174）、活动态只改样式（:177），`paletteGen` :79 代际计数让换主题时统一 restyle；`LayoutTransition` 动画开合（:185 `transitionsArmed` 首次布局后才武装，避免初始动画）。
- 边缘渐隐提示还有标签滚出视口（:114-115）。

### 2.9 设置：AppSettings / SettingsSection / Setting / SettingsActivity / StoragePermission

**AppSettings（ui/AppSettings.java，:21）** —— 全部用户设置的类型化门面，持久化在命名 SharedPreferences 文件（`settings`，:23）。~40 个键（:24-69）：屏常亮、沉浸模式、rich keyboard、extra keys 启用/开关列/行高、scrollback（默认 10_000 :72）、背景图路径/不透明度（默认 35% :75）/模糊、光标样式/闪烁、触屏键盘、左右边距、grapheme clustering、平滑滚动、鼠标跟踪、点按开链接、提示导航、剪贴板读写、OSC 9;4 进度、存储绑定、退出终止进程、关会话确认、铃声（BELL_OFF/HAPTIC/SCREEN_FLASH/SOUND :87-90）、userland 全套（shell/identity/home/cwd/locale/PATH/JIT 4–128MiB 步进 4 :81-85）、onboarding 完成、4 个字体路径。注释 :17-19 的扩展契约："加一个设置 = 一对类型化 getter/setter + 在 SettingsActivity.buildSections 声明一行"。

**SettingsSection（ui/SettingsSection.java，:12）** —— 一个标题 + 一组 Setting（渲染为一张卡片）。

**Setting（ui/Setting.java，:40）** —— **声明式设置行抽象**（torvox 已吸收此模式，见 §4.7）：
- 基类：title + summary + 可选 `enabledWhen` 门控（:69，返回 false 整行置灰；**任一设置变更即重估全部门控**，如"隐藏 extra-keys 工具栏 → 编辑器行置灰"，:48-51）+ `navigates`（:75 行尾加 chevron，点击开新屏）+ `notifyChanged` :91。
- 子类：`Toggle` :109（Switch，点行也可切换）、`Choice` :130（固定列表取值，AlertDialog 单选）、`Slider` :340（SeekBar + 只读数值，条独占交互——`:397` 行背景置 null 去掉死涟漪）、`Action`（点击执行，用于打开主题编辑器/备份等）。
- 抽象 `createControl` :99 / `onRowClick` :106 —— 屏幕层对类型无感知（注释 :28-38：新增类型 = 加兄弟子类，无需改宿主）。

**SettingsActivity（ui/SettingsActivity.java，:52）** —— 设置屏。`buildSections` :95 声明式组装 4 组（Appearance/Bell & input/Extra keys/Userland），`render` :353 统一渲染（SectionHeader + 卡片 + 行 + divider + `refresh` 门控重估 :356）。分组示例：外观组含"Theme"（Action → ThemeActivity，:99-104）、铃声模式 Choice、左右边距 Choice；userland 组含引擎选择（`:309 hasChrootNg` 才显示 chroot-ng 开关，JIT 行在 chroot-ng 选中时置灰——`enabledWhen` 链式门控 :317-333）、备份/恢复（`delegateAndFinish` :722 以 activity result 交回 MainActivity）。存储绑定权限流程在 `onResume` 补完（:90 `completePendingStorageBindingIfGranted`）。**只写进程级 store、不直接改活会话**（注释 :45-50）——返回主屏统一 `applyAllSettings`。

**StoragePermission（ui/StoragePermission.java，:14）** —— `granted` :18：API 30+ 用 `Environment.isExternalStorageManager()`（MANAGE_EXTERNAL_STORAGE），以下用 WRITE_EXTERNAL_STORAGE 检查；MainActivity 与 SettingsActivity 共用。

### 2.10 引导：OnboardingActivity（ui/OnboardingActivity.java，:49）

首次运行向导，三步：欢迎 hero（功能亮点）→ 发行版选择（`UserlandDistro.bundled` 枚举 APK 内 rootfs assets；含"仅 Android shell"卡）→ 安装（tar.xz 解压，**按压缩资产已知大小显示确定进度**）。
- `EXTRA_SETUP_ONLY` :52 —— 跳过欢迎步（新标签长按 + / 设置里"Install Linux"入口复用）。
- 自愈：已装 rootfs 时直接 `RESULT_OK` 结束并补写 onboarding 完成标记（:102-107）；取消返回不标记完成 → 下次启动再展示（注释 :39-42）。
- 安装完成写回 shell/home 等 userland 设置（:601-603）。
- insets 处理（:117-120）注释值得抄："pad content 而不是 root，让装饰性光晕保持全出血"。

### 2.11 native 层与构建（app/src/main/cpp/、app/build.gradle）

**app/build.gradle** —— 无 AppCompat/Material 依赖；`minSdk 29`（:19，注释：Zig 构建的 libghostty-vt.a 用 ELF TLS，bionic 自 29 才支持）、`targetSdk 36`、NDK 28.2、ABI 仅 `arm64-v8a, x86_64`（:25-27，与预编译 libghostty-vt.a 对应）；`externalNativeBuild` CMake（:30-34）；rootfs assets 目录存在才注入 sourceSets（:41-42 注释）；运行时依赖只有 `org.tukaani:xz:1.10`（:65，纯 Java XZ 解码，tar 解析自写——rootfs 只需文件/目录/链接）。

**CMakeLists.txt（app/src/main/cpp/CMakeLists.txt）** —— 三层静态库 + 一个 JNI so：
- `ghostty-vt` 静态导入 :8-10（`prebuilt/${ANDROID_ABI}/libghostty-vt.a`）。
- `arm64chroot` :12-45 —— AArch64 用户态模拟器（qemu-user 风格 ISA 模拟 + proot 式 rootfs 容器），**源码清单从子模块自己的 Makefile 正则提取**（:30-37，防两份清单漂移），`-w` 编译、`ANDROID_JNI` 宏（main 改名）。
- `chroot-ng` :69-93 —— 原生 AArch64 引擎（seccomp/SIGSYS 路径翻译 + 用户态 ELF 加载器，guest 代码直接跑在 CPU 上），仅 arm64-v8a；freestanding 编译，`memset=cng_embed_memset` 等预处理器改名（:80-89）防止与 Bionic 符号冲突；`--exclude-libs` :137 隐藏全部符号。
- `pngdec` :99（stb_image 包装，Kitty 图形用）。
- `libterm.so` :105 —— `pty_jni.c + terminal_jni.c + kitty_unicode.c`，`-Wall -Werror`；`:119-127` 链接：`-Wl,--whole-archive` 强拉 arm64chroot（弱引用钩子 exec_fpsimd/sysreg_exec/sysreg_init/smccc_conduit 若不强拉会被链接器丢弃 → 所有 guest FP/SIMD 与 MRS/MSR 变成 SIGILL，注释 :112-118 详细解释）。

**pty_jni.c** —— Java 持有 master fd（ParcelFileDescriptor），此文件只做 Java 做不了的：openpt/fork/exec、TIOCSWINSZ、waitpid/kill。
- `spawn_on_pty` :67 —— open `/dev/ptmx` → grantpt/unlockpt/ptsname_r → **初始 winsize 就带像素字段**（:84-87：ws_xpixel/ws_ypixel = cols×cell_w，注释 :80-83：Kitty icat 之类程序用 TIOCGWINSZ 像素值给图像定尺寸，且会话固定网格尺寸不再 resize，必须初始就正确）→ fork 子进程：setsid、slave 变成控制 tty（:101-108）、**清空信号掩码与 disposition**（:110-118，注释：fork 继承 ART 线程的信号处理，execve 会重置但进程内引擎不会——chroot-ng 的 guest 原生 fault 若落到 ART 的 SIGSEGV handler 会在无 ART 的进程里造 Java 崩溃报告）。
- 两种 spawn：`execve()`（Android shell）或进程内 `arm64chroot_main()`/`chroot_ng_main()`（userland，按 argv[0] 选择；W^X 政策禁止 exec app data 下任何东西，引擎从不 exec guest 二进制，:12-16）。
- `hasChrootNg` :236 —— JNI 编译期能力探测，Java 侧据此隐藏引擎开关。

**terminal_jni.c**（~2300 行，libghostty-vt 绑定）—— 结构：
- `TermCtx` :30-98：terminal/render_state 句柄、reused 的 mouse/key event、grid 尺寸、**write-pty 回写缓冲**（out/out_len :70-73）、搜索状态（`SearchMatch` 环 :75-90，`search_dirty` 每次 feed 置位 :339）。
- 回调全部**通过 typedef 赋值**（:141-145，注释 :139-140：签名漂移编译期失败而不是运行时栈损坏）。
- `terminalNew` :147 —— **scrollback 字节预算换算**（注释 :154-166，关键洞见）：ghostty 的 max_scrollback 是**字节**预算不是行数，每行裸网格 8×(cols+1) 字节，但每页还有固定元数据（styles/graphemes/hyperlinks/strings），实测每保留行 ~1.6× 裸网格成本 → **按 2× 上浮**（:167-169）。
- `terminalSetColors` :263 —— `GHOSTTY_TERMINAL_OPT_COLOR_*` 设 fg/bg/cursor + 256 色 palette；注释 :255-262：这些是**默认值**，程序 OSC 4/10/11/12 覆盖仍生效、palette set 保留逐索引覆盖。
- `terminalFeed` 前 NUL 剥离（:317-335，注释：mpv `--vo=kitty` 每帧 APC 块尾带 NUL，VT 引擎原样收集 → base64 损坏整帧黑屏）。
- **搜索实现在 native**：`run_search` :1729 起 —— UTF-8 码点解码 → `fold_cp` :1760（ASCII 内联 + 生成式 `CASE_FOLD` 表做 simple case fold，**不支持多字符折叠**如 ß，注释 :1752-1759）→ 按逻辑行（软换行合并）逐行扫（`search_line` :1836，尾部空白格裁掉、内部空单元当空格）→ `match_store` :1826 环形缓冲只留最近 `match_cap` 个可导航命中、`match_total` 诚实计数 → `match_rotate_left` :1814 三反转旋转让当前命中变首项。`terminalSearchSet` :2021 / `terminalSearchNext/Prev` :2057 / `terminalSearchClear` :2082 —— **高亮通过 selection 机制安装**（`show_match` :1956 设 `GHOSTTY_TERMINAL_OPT_SELECTION`，清除时置 NULL :2097），复用引擎自带的选择渲染，视图零改动。

**TerminalNative.java（term/TerminalNative.java）** —— JNI 常量 + 声明：事件位（EVENT_BELL/TITLE :22-23）、单元格属性位（ATTR_BOLD/ITALIC/UNDERLINE/STRIKE/WIDE/BLINK/HYPERLINK :26-38，下划线形状 3-bit 域 :45-52）、修饰位 MOD_* :55-57、光标样式 :60-63、选择标志 :66-68、输入模式 :71-74、鼠标 :77-92、Kitty 图形放置记录 GFX_STRIDE=14 :100-114。注释要求：与 `terminal_jni.c` 的 `meta[]` 布局**必须同步**。

---

## 3. 与 torvox 的功能对比

| 功能 | ghostty-android | torvox | 结论 |
|---|---|---|---|
| 主题模型 | `TerminalTheme`（19 色，不可变）+ `toPalette256` 生成 256 色 | `TerminalTheme` data class（16 ANSI + bg/fg/cursor/**selectionBg**） | 都有；torvox 多了 selectionBg，缺 256 色立方生成 |
| 内置主题 | 9 个（ThemePresets：Default/Solarized×2/Dracula/Nord/Gruvbox/Catppuccin Mocha/Tokyo Night/One Dark） | 16 个（BuiltInThemes）+ Material You 动态色 | 都有，torvox 更多 |
| 用户自定义主题 | **有**（ThemeStore JSON + ThemeActivity 编辑器 + CSV 持久化） | **无**（只有固定/日/夜模式选择） | ghostty-android 独有 |
| 取色器 | ColorPickerDialog（HSV 场 + hex + chips） | 无 | ghostty-android 独有 |
| 主题预览 | ThemePreviewView（自绘，含光标闪烁/背景图） | 无 | ghostty-android 独有 |
| chrome 随主题派生 | ChromePalette（亮度 <0.5 判定深/浅 chrome） | 无（Material 主题与终端主题分离） | ghostty-android 独有 |
| 自定义字体文件 | **有**（TerminalFontStore 4 槽导入私有存储） | **无**（只选 fontFamily 族名 + 字号） | ghostty-android 独有 |
| 背景图 | 有：复制私有存储 + 降采样 + **Java 自写 box blur** | 有：存 URI + Coil 解码（EXIF/1920×1080）→ RGBA 送 native 模糊 | 都有，实现不同（见 §3.5） |
| extra keys 布局 | 弹性网格行（权重 1/1.5/2）、**多 profile**、宽度/副键 | 固定两行等宽 + JSON 布局（含自定义文本键） | 都有；ghostty-android 功能多得多 |
| extra keys 编辑器 | **有**（ExtraKeysActivity 全屏 WYSIWYG + 拖放） | **无**（无编辑 UI） | ghostty-android 独有 |
| 符号字形 | Glyphs：矢量图标替换（ImageSpan） | NerdKeyLabels（Nerd Font 字形开关） | 思路不同，各自成立 |
| 搜索 UI | SearchBarView（覆盖层、150ms 防抖、计数、大小写） | TextSearchBar（Compose，fuzzy 开关） | 都有 |
| 搜索实现 | **native C**（case-fold 表 + 逻辑行 + 环缓冲） | **native Rust**（searchAllInScrollback + setSearchHighlights） | 都有，torvox 多了 fuzzy |
| 标签页 | TabStripView（pill + OSC 9;4 进度环 + 动画） | SessionDrawer（侧滑抽屉） | 交互范式不同；**OSC 9;4 进度 torvox 无** |
| 顶部栏 | TopBarView（次屏）+ 主屏自绘顶栏 | 无（Compose 全屏） | torvox 无此概念 |
| 设置存储 | AppSettings（命名 SharedPreferences，同步 API） | SettingsRepository（DataStore Flow，响应式） | 都有，torvox 架构更现代 |
| 设置 UI | 声明式 Setting（Toggle/Choice/Slider/Action + enabledWhen） | SettingsComponents（**已声明 modeled on ghostty-android**）+ SettingsScreen | **torvox 已吸收** |
| JNI/libghostty | C JNI + 静态链接 libghostty-vt.a + arm64chroot 模拟器 | Rust libghostty-rs + NativeBridge external funs | 架构不同，见 §3.9 |
| minSdk / 目标 | 29 / 36 | 33 / 34 | torvox 更高（33 起无兼容包袱） |

### 3.1 主题系统对比（重点）

**torvox 现状**（`ui/theme/TerminalTheme.kt`）：`TerminalTheme` data class（:13-24，含 selectionBg）+ `BuiltInThemes` 16 个（:26+，Dracula Plus/Catppuccin×2/Nord/…）+ `dynamicTerminalTheme` :561（Material You 动态色映射到终端主题）。模式：固定/日/夜（SettingsRepository `themeName/dayThemeName/nightThemeName/themeMode` :23-26）。应用路径：`TerminalRuntime.kt:505` `BuiltInThemes.byName` → `bridge.setTheme`（Bridge.kt:529 打包 54 字节 → NativeBridge.setTheme → Rust）。**主题与 chrome 解耦**：Material 主题独立于终端主题。

**ghostty-android 的增量**：
1. **用户自定义主题全链路**：ThemeStore（JSON 持久化 + 容错 + 重名替换）+ ThemeActivity（working copy + dirty + Save/Save as + 删除）+ ColorPickerDialog + ThemePreviewView。torvox 完全没有这一档。
2. **`toPalette256` 的 6×6×6 立方生成**：ghostty 引擎只主题化 16 色，其余由标准 xterm 生成规则补齐。torvox 的 setTheme 打包 54 字节 ≈ 19 色 ×3 字节，同样只传 16 色 + fg/bg/cursor——若 torvox 想主题化 256 色程序输出，需要 Rust 侧等价生成（或在 Kotlin 侧生成后全量传 256 色，768 字节）。
3. **ChromePalette 亮度派生**：浅色终端主题 → 浅 chrome。torvox 的 Compose UI 可等价实现：从 `terminalBackground` 计算 luminance，动态切 `lightColorScheme/darkColorScheme`。

### 3.2 字体对比

ghostty-android：4 槽位文件导入（regular/italic/bold/bold-italic），`Typeface.createFromFile`，缺文件回退（italic → 合成斜体）。**渲染在 Java 侧**（TerminalView 用 Android Typeface 画）。

torvox：`fontFamily`（monospace/sans-serif/serif 族名，`FontUtils.resolveEffectiveFontFamily` :5）+ `fontSize`（默认 10sp）；**渲染在 Rust 侧**（native 字体栅格化，SettingsScreen :246-247 显示"Active: $defaultFontName (CJK fallback…)"）。**结论**：torvox 无法直接复用 TerminalFontStore（渲染管线在 Rust，Typeface 对象送不过去），但"复制到私有存储"的**权限寿命模式**适用（若未来支持自定义字体文件，应复制文件而非存 URI）。

### 3.3 背景图对比

| 维度 | ghostty-android | torvox |
|---|---|---|
| 持久化 | 复制到私有文件，存路径 | 存 content URI 字符串（DataStore） |
| 解码 | BitmapFactory + 2 幂采样 | Coil（EXIF 方向、1920×1080 上限、HARDWARE→软件位图防御 :1156-1160） |
| 模糊 | **Java 自写双趟 box blur**（≤1080 工作位图） | **Rust 侧模糊**（`bridge.setBackgroundParams(radius, alpha×10)`） |
| 透明度 | alpha 2.55 缩放 :426 | alpha×10 送 native |
| 失效自愈 | 解码失败清路径 :424 | runCatching 吞异常 |
| 冷启动竞态 | 无（同步路径，applyAllSettings） | 有专门处理：等 bridge 最多 15s（:1138-1143，注释 round-203 实测） |

**点评**：torvox 的模糊在 Rust（可能质量更好/更省内存），URI 方案有**权限失效风险**（用户撤销/系统清理后 URI 不可读，Coil 静默失败 = 背景图悄悄消失）；ghostty-android 的"import 时复制到私有存储"方案更稳。吸收建议：torvox 可在选图时复制文件、存文件路径（或至少存 URI 时在桥接失败后自愈清路径——ghostty-android :424 的 stale/corrupt 自愈模式）。

### 3.4 extra keys 对比

torvox：`ModifierBar.kt`（固定两行等宽、`ModifierKey` :52 含 ctrl/alt/isToggle/isSessionButton、`defaultModifierKeys` :63）+ `ToolbarPreferences.kt`（SharedPreferences JSON 布局 :72-144，ToolbarKey 枚举 30 个内置键含符号覆盖/长按重复/repeatable、Custom 文本键）+ `ModifierState`（CTRL/ALT 粘性）。无宽度、无副键、无多 profile、无编辑器 UI、无自动重复（除了 repeatable 标记——需确认实现）。

ghostty-android 增量（torvox 缺口）：
1. **WYSIWYG 编辑器**（ExtraKeysActivity）：profile 条 + 活动 grid 即改即存（无 Save 步骤，注释 :50-53）、点按编辑（换键/宽度/副键/删除）、长按拖放（平台 DnD + 间隙视图 `gapView` + 底部危险删除条）、行高滑杆（2–28dp）、"Add keys" 预设面板（COMBO_PRESET_IDS）。
2. **弹性宽度**（weight 1/1.5/2）——torvox 等宽。
3. **swipe-up 副键**（含右上角提示）。
4. **粘性修饰键锁定态视觉**（accent 底）。
5. **行内统一字号**（KeyCaps.uniformize）。

### 3.5 搜索对比

ghostty-android：**搜索逻辑全在 C**（terminal_jni.c，逻辑行合并软换行 + 生成式 case-fold + 环形缓冲），UI 是覆盖层不触发 SIGWINCH，防抖 150ms，**高亮复用 selection 机制**（引擎原生渲染）。UI 细节：IME 回车 = 待发查询 flush 或下一个、`IME_FLAG_NO_EXTRACT_UI`、计数无结果态。

torvox：**搜索逻辑在 Rust**（`searchAllInScrollback(query, caseSensitive, fuzzyMatch)` NativeBridge.kt:189，**多 fuzzy 匹配**）+ 高亮 `setSearchHighlights`（字节下发）；UI TextSearchBar（Compose，OutlinedTextField + 大小写/fuzzy 开关 + 上/下/关闭）。`findMatches`（TextSearchBar.kt:79）是 Kotlin 侧备用实现（含宽字符列计算 `isWideChar` :49——这是 torvox 特有的细节，ghostty-android 无此问题因搜索在 native）。

**对比结论**：两者搜索都达标；torvox 的 fuzzy 是超集；ghostty-android 的"防抖 + 回车 flush + 计数推送 + 无 SIGWINCH"UX 细节可借鉴（torvox 是否覆盖层需查 TerminalScreen——其搜索 UI 在 Column 中，可能挤压 terminal 触发 resize）。

### 3.6 标签页对比

torvox：SessionDrawer（侧滑抽屉：会话列表/切换/关闭/新建），**无标签条**。ghostty-android：顶部 TabStripView。

- torvox 若加标签条，可吸收：`update` 原地调和（:158，不重建 view 避免打断涟漪/闪烁）、LayoutTransition 动画、`paletteGen` 代际 restyle、钉死 + 按钮。
- **OSC 9;4 进度**：torvox 完全没有（grep 无结果）。ghostty-android 的实现链路：`OscSideScanner`（term/）解析 OSC 9;4 → `TerminalSession.onProgress` → MainActivity.onProgress :1116 → TabStripView.setProgress :207 → TabRing。torvox 若渲染在 Rust，应在 Rust 侧解析 OSC 9;4（libghostty 不暴露）再经 PollEvent 上报——OscSideScanner 的"跨 read 边界携带状态 + 1MiB 上限"设计（reference-projects.md §9 已记录）值得抄。

### 3.7 顶部栏对比

torvox 无顶部栏（Compose 沉浸全屏，会话标题在抽屉里）。ghostty-android 主屏顶栏承载：prompt-nav 上一/下一按钮、搜索按钮（状态高亮跟随搜索条）、设置齿轮、OSC 标题；全部经 `ChromePalette` 重着色（`styleTopBarButton` :404 连 `Glyphs.applyTo` 重 tint 矢量图标）。次屏用共享 TopBarView。**对 torvox 的价值**：如果未来加顶栏（如搜索入口/设置入口常驻），TopBarView 的"共享组件 + EdgeInsets 逐表面路由"和 ChromePalette 的重着色模式是现成模板；但 Compose 里 Material TopAppBar + colorScheme 更自然，价值主要在 EdgeInsets 思路。

### 3.8 设置对比

torvox 已明确吸收 ghostty-android 的声明式 Setting 模式（SettingsComponents.kt:5-7 注释原文："Modeled on ghostty-android's declarative Setting pattern"）：SettingsRow/SettingsSliderRow/SettingsSwitchRow/SettingsSelectorRow。**尚缺**：`enabledWhen` 门控（torvox 用 `if` 条件渲染代替）、`navigates`/Action 行（torvox 的"打开子屏"是独立 Composable）、Slider 的"条独占交互去死涟漪"细节。AppSettings 的同步 SharedPreferences vs torvox DataStore Flow：torvox 更优，无需吸收。

### 3.9 JNI/libghostty 集成对比

| 维度 | ghostty-android | torvox |
|---|---|---|
| 绑定方式 | C JNI 手工绑定 libghostty-vt（C API） | Rust libghostty-rs + NDK JNI（external funs → Rust crate） |
| 渲染 | Java Canvas（快照 int 数组） | Rust wgpu（glyphon 文本、单元着色） |
| 线程模型 | libghostty-vt 非线程安全 → TerminalEmulator 全同步锁 | Rust 侧管理（RenderWatchDog 等） |
| 事件 | native 缓冲 → Java 轮询（无 upcall） | PollEvent 轮询（NativeQueryPort） |
| 搜索 | C 侧自写（terminal_jni.c） | Rust 侧（searchAllInScrollback） |
| 选择 | C 侧 selection API + 快照镜像 | Rust SelectionState + setSelection |

**可借鉴的 C 层细节**（与语言无关的设计知识）：
1. `terminalNew` 的 **scrollback 字节预算 2× 系数**（:167-169）——torvox 的 Rust 侧如果也走 `max_scrollback` 字节预算，同样适用（实测每保留行 ~1.6× 裸网格成本）。
2. 回调必须走 typedef 赋值（编译期签名检查）——torvox Rust 侧 bindgen 自动保证，无需动作。
3. `terminalFeed` 的 **NUL 剥离**（:317-335）——mpv --vo=kitty 的 APC 块尾 NUL 会毁掉整帧 Kitty 图形；torvox 若支持 Kitty 图形（GFX 快照说明支持），Rust 侧 feed 前应有同样防御（可在 `feed_bytes`/process_bytes 入口做 memchr 检查，无 NUL 走零拷贝快路径）。
4. winsize 初始就带像素字段（pty_jni.c :84-87）——torvox 的 Rust PTY spawn 同样应设 ws_xpixel/ws_ypixel，否则 Kitty icat 等程序报"screen sizes in pixels"错误。
5. fork 子进程清信号掩码/disposition（pty_jni.c :110-118）——torvox 用 Rust Command 则 fork/exec 由 libc 处理，execve 会重置 disposition，此问题主要在进程内引擎场景，torvox 无此需求。
6. **搜索高亮复用 selection 机制**（terminalSearchClear :2097）——torvox 已有独立 setSearchHighlights 通道，无需吸收。

---

## 4. 依赖分析：能否用于 torvox？

### 4.1 可移植性总评

ghostty-android 是**纯 Java + 自绘 View + 无 androidx 运行时依赖**的代码库；torvox 是 **Kotlin + Compose + Rust 渲染**。因此：

- **不能直接搬**：所有 View/Activity 类（Chrome 的 GradientDrawable/RippleDrawable 工厂、KeyCapView/TextView 技巧、TabStripView 的 View 调和、Dialogs 的 AlertDialog 流程、ColorPickerDialog 的 Canvas 绘制）。
- **可直接移植（算法/模型，无依赖）**：TerminalTheme 的 256 色立方生成、ThemeStore 的 JSON 持久化模式、BackgroundImageStore 的 box blur 算法、Glyphs 的码点→图标映射思路（Compose 用 ImageVector + AnnotatedString）、KeyCaps.uniformize 的行内字号算法、ExtraKeysConfig 的 v0/v1/v2 迁移读取器、Setting 的 enabledWhen/navigates 模式（torvox 已部分吸收）、ChromePalette 的亮度派生。
- **架构级借鉴（跨语言设计）**：terminal_jni.c 的 scrollback 字节预算、NUL 剥离、回调 typedef；pty_jni.c 的 winsize 像素字段；"效果轮询不回调"；"设置只写 store、onResume 统一重放"（torvox 用 Flow 天然实现）。
- **不适用**：arm64chroot/chroot-ng 模拟器（torvox 不跑 Linux userland，走 /system/bin/sh + Termux 式 bootstrap）、OscSideScanner（torvox 的 PTY 在 Rust 侧）、OnboardingActivity 的发行版安装向导（torvox 是 BootstrapInstaller，功能等价已存在）。

### 4.2 具体依赖清单（无新依赖即可吸收的）

| 吸收项 | 依赖 | 说明 |
|---|---|---|
| 256 色立方生成 | 无 | 纯函数，Kotlin 直译 |
| ThemeStore JSON | org.json（Android 内置）| torvox 用 kotlinx.serialization 等价 |
| box blur | 无 | 纯像素算法，Kotlin 直译 |
| 复制到私有存储 | 无 | 纯文件 IO |
| Glyphs 码点映射 | 无 | Compose ImageVector + span 或 Text composable |
| enabledWhen 门控 | 无 | 已有 SettingsComponents 上加参数即可 |

---

## 5. 可吸收到 torvox 的具体内容（含代码注释建议）

按优先级排序：

### P0（低成本高价值，纯 Kotlin/模型层）

**1. 用户自定义主题（ThemeStore + 256 色生成）**
torvox 已有 `TerminalTheme` data class 与 `BuiltInThemes`。吸收：
```kotlin
// ghostty-android TerminalTheme.java:55 toPalette256 直译
// 注意：只主题化 16 ANSI 色，16-231 用标准 xterm 6×6×6 立方
// （levels = {0, 95, 135, 175, 215, 255}），232-255 为 24 级灰阶。
fun TerminalTheme.toPalette256(): List<Color>
```
用户主题存储（`ThemeStore.java:110 saveUserTheme` 模式）：
```kotlin
// 参照 ghostty-android ThemeStore：单 JSON 数组 {name, colors}，
// 读取逐条容错（坏条目跳过而非全丢），preset 名不可覆盖，
// 重名替换。SharedPreferences 换 DataStore 即可。
```

**2. 设置 UI 的 enabledWhen 门控补齐**
`SettingsComponents.kt` 已声明吸收 ghostty-android 模式，但缺 `Setting.enabledWhen`（Setting.java:69）。注释建议：
```kotlin
// ghostty-android Setting.java:48-51：门控在"任一设置变更"时重估，
// 一个开关能实时置灰另一行（如"隐藏 extra-keys 工具栏 → 编辑器行置灰"）。
// torvox 当前用 if 条件渲染，等价但缺"变更时联动重估"的单一通道。
```

### P1（中等成本，有明确收益）

**3. 背景图"复制到私有存储 + 自愈"**
`BackgroundImageStore.java:51 importFrom` 与 `MainActivity.java:424` 的 stale/corrupt 自愈。注释建议：
```kotlin
// ghostty-android BackgroundImageStore：content:// URI 权限不跨进程存活，
// 选图时复制进 filesDir 固定文件，持久化的是文件路径；
// 解码失败（路径失效/损坏）时清掉持久化路径而不是静默丢图。
// torvox 当前存 URI 字符串 + runCatching 吞异常 = 背景图可能悄悄消失。
```

**4. 搜索 UX 细节**
`SearchBarView.java`：150ms 防抖（:54）、IME 回车 flush 待发查询或当"下一个"（:116-120）、`IME_FLAG_NO_EXTRACT_UI`（:105）、覆盖层不触发 SIGWINCH（:35-38）。注释建议：
```kotlin
// ghostty-android SearchBarView：搜索条是覆盖层而非布局列成员，
// 打开/关闭不改变终端尺寸 → 不产生 SIGWINCH；
// 输入防抖 150ms + IME 回车先 flush 待发查询、否则前进到下一匹配。
// 需核查 torvox 搜索条是否挤压终端（若在 Column 中则会触发 resize）。
```

**5. OSC 9;4 进度环**
torvox 无此功能。ghostty-android 链路：`OscSideScanner`（term/，跨 read 边界状态机 + 1MiB 上限）→ `TabStripView.TabRing`（:288，✕ 旁进度环，indeterminate 自旋）。torvox 渲染在 Rust：**应在 Rust 侧解析 OSC 9;4**（libghostty 不暴露该序列，与 OscSideScanner 同理），经 PollEvent 上抛。注释建议：
```kotlin
// ghostty-android OscSideScanner：libghostty 不暴露的序列在 Java 侧
// 用跨 read 边界携带状态 + 1MiB 上限的状态机补全（OSC 52 剪贴板、
// OSC 9;4 进度）。torvox 的 PTY 读循环在 Rust，等价逻辑应在 Rust 侧。
```

### P2（按需吸收）

**6. 标签条（若未来做）**：`TabStripView.update:158` 原地调和 + `paletteGen` 代际 restyle + LayoutTransition 武装时机（:185）。

**7. 行内统一字号**（`KeyCaps.uniformize:60`）：Compose 的 ModifierBar 若支持自定义宽度后，同行键的字号统一算法直接可用。

**8. Glyphs 图标化**：`Glyphs.java` 的码点→矢量图标映射（:47-65 MAP）。torvox 已有 NerdKeyLabels（Nerd Font 字形），二选一即可；若走矢量图标路线，`CenteredIconSpan.getSize:117` 的"字体行盒∪图标盒"修复对应 Compose 的 `Placeable` 尺寸处理。

**9. winsize 像素字段**：`pty_jni.c:84-87`。Rust PTY spawn 处补 `ws_xpixel/ws_ypixel`，让 `icat` 等程序能正确按像素定尺寸。注释建议：
```rust
// ghostty-android pty_jni.c:80-87：初始 winsize 必须带像素字段，
// Kitty icat 经 TIOCGWINSZ 读取；会话以最终网格尺寸 spawn 后不再 resize。
```

**10. NUL 剥离**：`terminal_jni.c:317-335`。Rust feed 入口（若支持 Kitty 图形）：
```rust
// ghostty-android terminal_jni.c:317：mpv --vo=kitty 每帧 APC 块尾带 NUL，
// VT 引擎原样收集会毁掉 base64 图像数据致整帧黑屏；feed 前剥离，
// 常见无 NUL 情况走零拷贝快路径（memchr 检查）。
```

**11. scrollback 字节预算 2× 系数**：`terminal_jni.c:154-169`。torvox Rust 侧若经 `max_scrollback` 传行数需求，同算法适用。

**12. 主题编辑器**（P2 大工程，需评估）：ThemeActivity 的 working-copy + dirty + confirmIfDirty 模型 + ColorPickerDialog + ThemePreviewView。若 torvox 要做"自定义主题"，这是唯一现成完整参考；ColorPickerDialog 的 `updating[]` 防回环守卫（:104）是移植时的必抄细节。

---

## 6. 项目文档吸收价值

1. **docs/architecture.md + CLAUDE.md 的写作模式**：ghostty-android 用"分层图 + 数据流 + **不可变不变量列表**"（"libghostty-vt 不是线程安全的…"、"效果是轮询的不是推的…"、"Java 是哑渲染器…"）组织架构文档，比罗列类更易维护。torvox 的 docs 可对照补"不变量"一节。
2. **`meta[]`/常量双端同步契约**（TerminalNative.java:66-68 注释 + CLAUDE.md:110-113）：ghostty-android 把"JNI 常量必须与 C 侧同步"写成显式契约文档；torvox 的 NativeBridge/Bridge 同样有此类约束（GFX 布局、PollEvent 协议），可显式化。
3. **测试缝模式**：`EXTRA_FORCE_SHELL`（MainActivity.java:86，"测试缝：强制普通 shell 使 UI 测试确定"）、`seedRawForTest`（ExtraKeysConfig.java:696，迁移测试直接注入原始持久化值）——torvox 的 migration 测试可借鉴后者。
4. **"WYSIWYG 单一工厂防漂移"的教训记录**（KeyCaps.java:16-21 注释：曾发生 3dp/6dp 漂移）：torvox 的 ModifierBar 与未来编辑器共用工厂时，这条注释值得保留。
5. **即改即存 vs Save 按钮**：ExtraKeysActivity 的"无 Save/dirty 步骤，每次编辑即持久化"（注释 :50-53）与 ThemeActivity 的"working copy + dirty + 显式 Save"是**两种相反模型**，ghostty-android 按"编辑频率/误操作代价"选择了不同方案（键布局高频低危→即改即存；主题颜色低频高危→显式保存）。torvox 文档可在 UX 决策记录里引用这个权衡框架。

---

## 7. 结论

- ghostty-android 的 UI 层是**"零依赖自绘"的极端范本**：Chrome 令牌系统、ChromePalette 亮度派生、Glyphs 图标化、TabStripView 原地调和、Setting 声明式模式，每个都有值得抄的注释级设计理由。
- **最值得 torvox 吸收的三件事**：① 用户自定义主题链路（模型层可直译）；② 背景图"复制私有存储 + 失效自愈"（修 torvox 的 URI 权限隐患）；③ native 层的三个通用知识（scrollback 字节预算、NUL 剥离、winsize 像素字段）。
- **torvox 已吸收**：Setting 声明式模式（SettingsComponents.kt 自述）。**torvox 已超越**：fuzzy 搜索、Material You 动态主题、DataStore 响应式设置、Rust 侧渲染（模糊/文本）。
- 已确认的 torvox 缺口（本次研究新增）：自定义主题编辑器、字体文件导入、extra keys 宽度/副键/编辑器、OSC 9;4 进度、标签条交互、顶部栏概念、搜索防抖/覆盖层细节。

## deep-v6 增量（复核第 3 轮：ExtraKeysConfig + KeyCapView + TerminalFontStore）

### ExtraKeysConfig.java（699 行）

完整额外按键配置系统：
- `DEFAULT_IDS`（:75-88）：esc/ins/del/pgup/home/up/end/tab/ctrl/alt/pgdn/left/down/right——**与 torvox ModifierBar 同款双行布局**（:103-105 DEFAULT_ROWS）
- `KeySpec`（:133-163）：id + flex 宽度 + 可选 secondaryId（长按第二功能）
- **`CUSTOM_PREFIX = "lit:"`**（:59）：自定义字符键；**`COMBO_PREFIX = "combo:"`**（:67）
- **`COMBO_PRESET_IDS`**（:114-117）：14 个组合键预设（combo:C:c/C:d/C:z/C:l/C:r/C:a/C:e/C:u/C:k/C:w/C:\\ /S:tab/C:left/C:right）
- `Profile`（:166-177）：rows 布局持久化（SharedPreferences "extrakeys"）

**torvox 对照**：ModifierBar.kt 按钮**硬编码**（ESC/SCROLL/HOME/END/PGUP/PGDN/TAB/CTRL/ALT/...），无自定义/combo/Profile。**功能差异 P2**：辅助按键栏不可配置（用户未要求，记录不实现）；若未来做可配置按键栏，ExtraKeysConfig 是完整蓝本（KeySpec width flex + secondaryId 长按 + combo 预设）。

### TerminalFontStore.java（100 行）

字体存储（文件复制 + 名称注册）——torvox loadFontFile 直传路径，无存储层；与 haven/moke 字体安装器同类，P2 已记录。

## deep-v1 增量（2026-08-07 全文件精读轮 #2：term/ 模块）

### 本次精读文件
- OscSideScanner.java（term/OscSideScanner.java 完整 200 行）
- ScreenSnapshot.java（前 60 行 + meta 布局）
- SessionCommand.java（完整）、StorageBindings.java（前 80 行）
- UserlandDistro.java / UserlandRootfs.java / UserlandOptions.java（结构扫描）
- RootfsBackup.java / SessionService.java / TerminalNative.java（结构确认）

### OscSideScanner（libghostty-vt 不回调的 OSC 旁路扫描）
- 4 态状态机（GROUND/ESC/OSC/OSC_ESC）+ **MAX_PAYLOAD=1MB 上限**（超限丢弃并在终止符处重同步）+ NUL 忽略（匹配引擎 NUL strip）+ 跨 PTY read 边界保持状态
- OSC 52 剪贴板：`52;<sel>;<base64|?>`——写/查询两通道；sel 空默认 "c"；Base64 解码失败静默丢弃
- OSC 9;4 进度（ConEmu）：`9;4;<state>[;<value>]`——state 0-4 校验、value 0-100 clamp
- **设计要点**：与 VT 引擎并行旁路（"never alters what the engine is fed"）——ghostty 引擎不回调的 OSC 由旁路补
- **torvox 对照**：torvox OSC 52 由 ghostty 引擎 + event.rs Clipboard 处理（已有）；**OSC 9;4 进度 torvox 无（P3）**——libghostty-vt 同样不回调，torvox 若需 ConEmu 进度条可参考此旁路模式（但 torvox 有 poll_notification OSC 9 通道，9;4 可扩展）

### ScreenSnapshot meta[16] 布局
- meta[0-5]：cursorInViewport/cursorX/cursorY/cursorStyle/cursorVisible/cursorBlinking；meta[7-8]：defaultBg/defaultFg；**meta[15]：cursorColor（OSC 12 覆盖）**
- graphemes 溢出缓冲：slot0=记录数 + [cellIndex, count, cp0...]——**与 torvox CellData.grapheme_extra 同构**（确认 torvox 设计）
- **torvox 对照**：torvox CellCursor 含 cursorStyle/cursorVisible，cursorBlinking 由 Kotlin 侧设置——**meta[15] cursorColor（OSC 12）torvox 未处理**（P3 记录）

### SessionCommand.androidShell 环境变量（7 个）
PATH=/system/bin、HOME、TMPDIR、TERM=xterm-256color、LANG=en_US.UTF-8、ANDROID_ROOT、ANDROID_DATA——**torvox base_env 已超越**（参考 termux-kotlin 透传 14 系统变量，P1 已修）

### UserlandRootfs（tar 解包安全——与 torvox BootstrapInstaller 同理念）
- 手写 tar 解析（readBlock/isZeroBlock/octal/parsePax 扩展头）+ **withinRoot 防路径逃逸** + deleteIfSymlink + chmod 恢复 + staging→publish 原子
- **torvox 对照**：torvox BootstrapInstaller zip-slip 防护（MAX_SYMLINKS_BYTES + symlink-path escape guard + staging 原子安装）——**同理念不同格式（tar vs zip），torvox 已实现等价防护**（确认）

### StorageBindings（arm64chroot bind mounts）
proot/chroot 方案的标准 mount 集（Documents/Pictures/DCIM/Music/Downloads/Movies → /mnt/*）——torvox 用 linker64 方案（不同架构，不适用）

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | OSC 9;4 进度（ConEmu）torvox 无 | P3 |
| 2 | OSC 12 cursorColor（meta[15]）torvox 未处理 | P3 |
| 3 | ScreenSnapshot graphemes 溢出与 torvox grapheme_extra 同构 | 确认 |
| 4 | UserlandRootfs tar 安全与 torvox zip-slip 等价 | 确认 |
| 5 | androidShell 7 env——torvox 已超越（14 变量） | 确认 |
