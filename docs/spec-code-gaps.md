# 代码与规范差异审计

审计日期：2026-10-31（第 1–5 轮）、2026-11-01（第 6–8 轮）。范围：`main`（含本轮从 `wb` /
`wb2` 合入的提交）与工作区改动。
依据：`docs/specification/` 全部文档；`openspec/specs` 作为次级规范一并核对。代码位置用
文件与符号表示，不写行号。

## 结论

- 配置面：`termux.properties` 实解析 20 个键（16 个标量键 + 4 个 `shortcut.*`），
  `docs/specification/config.md` 与 `PROHIBITED.md` 现已全部收录；
  代码与规范对齐，不再是「规范未收录」。
- `colors.properties` 解析已按用户指令移除，实现与 `config.md`、`PROHIBITED.md` 一致。
- 修饰键栏默认布局：**与 Termux 上游一致**（上游 v0.119.0-beta.3 的
  `DEFAULT_IVALUE_EXTRA_KEYS` 首行即 `ESC DRAWER SCROLL HOME UP END PGUP`，并非
  `ESC / - …`）。此前文档与代码注释把默认值说成「上游把 `/` 与 `-` 换成 DRAWER 与
  SCROLL」是**错的**，本轮已按上游源码更正。
- 上游键名别名表此前未实现：`ESCAPE`/`CONTROL`/`PAGEUP` 等写法会被当成未知键名丢掉。
  本轮已实现（`TerminalKeyCatalog.normalizeKeyName`）。
- `extra-keys` 的 `{macro: ...}` 此前只解析不渲染；本轮已按上游语义实现播放
  （`MacroParser` + `TerminalViewModel.sendMacro`），含无键码记号的字面量文本。
  `popup` 此前只把字符串当纯文本发出、对象形式整个丢弃；本轮按上游语义实现为**完整的次级按键**（见下节）。
  `FN`/`SHIFT` 键仍不支持，已写入 `PROHIBITED.md`。
- `volume-keys` 此前无条件拦截音量减；本轮补上上游口径「外接字母键盘在场时不抢它的
  音量键」。
- `shortcut.*` 会话快捷键此前未实现；本轮按 `termux.properties` 解析 `Ctrl+<字符>` 并在
  实体键盘事件进入终端前匹配，`rename-session` 明确忽略。
- 边界差异：配置重读时机与实现一致；超大文件口径已按实现写明（沿用上一次成功的配置，
  首次加载即默认值）；`beam` 取值在 `DESIGN.md` 与 `config.md` 现已一致，且已注明上游无此取值。
- 字体：`~/.config/termux` 的默认字体与字体目录现已纳入扫描/注册，主路径优先。
- Bootstrap 安装前清空而非拒绝已存在的 `usr-staging`：本轮定案保留清空策略，`DESIGN.md`
  已按现状更正（见下节）。
- Bootstrap 的内置 SHA-256 校验不是 `PROHIBITED.md` 禁止的「随 zip 分发的独立校验文件」：
  它不读取外部校验文件，只对钉住的预设 URL 生效，复杂度与性能开销低，保留合理。
- 本轮清掉了 `openspec/specs` 里已删除功能的残留条款（响铃、光标闪烁、会话切换提示），
  使次级规范重新与实现对齐；其余差异按现状登记，未擅自删除功能。

## 配置键面：20 个键（与 config.md 对齐）

`docs/specification/config.md` 与 `PROHIBITED.md` 现已收录全部 20 个键；代码解析与文档一致，
不再是「三键限制」下的违规。下表仅作现状登记，键面本身已无规范差异。

`TermuxPropertiesParser.parse` 解析下表 20 个键（16 个标量键，加上 `SessionShortcut.entries`
的 4 个 `shortcut.*` 键），`TermuxProperties` 为每个键保留字段，并各有消费方。

| 键 | 实际作用 | 与规范的关系 |
| --- | --- | --- |
| `default-working-directory` | 会话工作目录 | 允许 |
| `terminal-transcript-rows` | 回滚行数 | 允许 |
| `terminal-cursor-style` | 光标默认样式 | 允许 |
| `use-fullscreen-workaround` | 全屏时系统栏改为滑动临时唤出 | config.md 已收录 |
| `fullscreen` | 隐藏系统栏 | config.md 已收录 |
| `ctrl-space-workaround` | 补发 Ctrl+空格 | config.md 已收录 |
| `hide-soft-keyboard-on-startup` | 启动时是否请求输入法 | config.md 已收录 |
| `extra-keys-text-all-caps` | 键帽大小写 | config.md 已收录 |
| `terminal-margin-horizontal` | 终端左右边距 | config.md 已收录 |
| `terminal-margin-vertical` | 终端上下边距 | config.md 已收录 |
| `terminal-toolbar-height` | 修饰键栏高度倍率 | config.md 已收录 |
| `extra-keys-style` | 键帽字形档位 | config.md 已收录 |
| `extra-keys` | 键栏布局（行、键位、字形、`display` 覆盖、`popup` 次级键、宏播放、键名别名） | config.md 已收录；默认布局与 Termux 上游一致（见下节） |
| `shortcut.create-session` | 实体键盘 Ctrl 组合键新建会话 | 允许（config.md 已收录） |
| `shortcut.next-session` | 实体键盘 Ctrl 组合键切到下一会话 | 允许（config.md 已收录） |
| `shortcut.previous-session` | 实体键盘 Ctrl 组合键切到上一会话 | 允许（config.md 已收录） |
| `shortcut.rename-session` | 解析时明确忽略并记警告（本仓无重命名 UI） | config.md 已收录（不实施） |
| `back-key` | 返回键改发 ESC | config.md 已收录 |
| `volume-keys` | 音量减改为虚拟 Ctrl | config.md 已收录 |
| `soft-keyboard-toggle-behaviour` | 键盘键改为启用/禁用输入法 | config.md 已收录 |

证据：`TermuxPropertiesParser.parse` 的 `boolean`、`integer`、`float`、`choice`
与 `extraKeys`、`sessionShortcuts` 分支；`TermuxProperties` 的字段与默认值；消费方分别是
`MainActivity`（`ctrlSpaceWorkaround`、`backKey`、`volumeKeys`、`sessionShortcuts`）、
`TerminalScreen`（`useFullscreenWorkaround`、`fullscreen`、`hideSoftKeyboardOnStartup`、
`extraKeys*`、`terminalMargin*Dp`、`softKeyboardToggleBehaviour`）、`SystemBars`
（`fullscreen`）、`TerminalRuntime`（`terminalToolbarHeightScale`、`extraKeys`）。

其中默认值本身改变行为：

- `extra-keys` 默认布局与上游一致（首行 `ESC DRAWER SCROLL HOME ↑ END PGUP`，次行
  `TAB CTRL ALT ← ↓ → PGDN`），DRAWER 的 `popup: 'PASTE'` 按上游语义写成次级按键，
  长按即粘贴。
- `volume-keys` 默认 `virtual`，`MainActivity` 默认把音量减拦截为虚拟 Ctrl（按住期间持续生效）；
  `DESIGN.md` 未声明该行为。
- `hide-soft-keyboard-on-startup` 默认 `true`，与上游的 `false` 相反（本应用启动时从不
  主动请求输入法，默认值必须反映既有外观），已在 `config.md` 与 `openspec` 注明。
- `terminal-margin-horizontal` 默认 3dp、`terminal-margin-vertical` 默认 0dp、
  `terminal-toolbar-height` 默认 1 倍：默认视觉与 Termux 接近。

其余键默认值不改变现有行为，非默认值会启用未声明能力。

## colors.properties（已移除）

`TermuxPropertiesRepository` 不再读取第二个文件，`ColorsPropertiesParser`、
`RenderContract.colors`、`Bridge.setPaletteOverride` 与原生覆盖通道均已删除。
终端配色只来自内置主题，`colors.properties` 存在也不生效。

此前该能力与 `config.md`、`PROHIBITED.md` 的禁令冲突；移除后实现与规范一致。

## Bootstrap：清空而非拒绝已存在的 staging（已定案）

`DESIGN.md` 此前要求安装前 `usr-staging` 目录必须不存在，存在则拒绝执行；实现里
`BootstrapInstaller.install` 先调 `clearStaging(stagingDir)`：目录能清空就继续，只有清不净
才抛错中止。

定案：保留清空策略，`DESIGN.md` 已按现状更正。理由是原子性意图（staging 里不能混进旧树）
由「清不净即中止」保证，而清空能让上一次崩溃留下的残留自愈——按字面「存在即拒绝」会把
用户永久卡在「必须手动删目录才能重装」。并发安装由单一安装入口串行，不另设锁。

## 修饰键栏默认布局

`TermuxProperties.DEFAULT_EXTRA_KEYS` 第一行为 `ESC DRAWER SCROLL HOME UP END PGUP`，
第二行为 `TAB CTRL ALT LEFT DOWN RIGHT PGDN`。这与 termux v0.119.0-beta.3 的
`DEFAULT_IVALUE_EXTRA_KEYS` 一致（上游同样含 DRAWER 与 SCROLL）：上游把 DRAWER 写成
`{key: 'DRAWER', popup: 'PASTE'}`，本应用按同一写法表达，长按即粘贴。

`DESIGN.md` 已按此更正记录。抽屉仍有 `ModalNavigationDrawer` 的左缘滑动入口；滚动锁定
是本应用相对 Termux 的补充功能，但入口本身在上游默认键栏里。

## 上游键名别名（此前缺失）

上游 `ExtraKeysConstants.CONTROL_CHARS_ALIASES` 在解析每个键名时先做一次别名替换，
故 `ESCAPE`、`CONTROL`、`SHFT`、`RETURN`、`FUNCTION`、`LT`/`RT`/`DN`、
`PAGEUP`/`PAGE UP`/`PAGE-UP`、`PAGEDOWN`/`PAGE DOWN`/`PAGE-DOWN`、`DELETE`、
`BACKSPACE`、`BACKSLASH`、`QUOTE`、`APOSTROPHE` 都与原名等价。

本轮已实现（`TerminalKeyCatalog.normalizeKeyName`），并同时用于键码/特殊键查表、字形
查表与宏记号解析。此前这些写法会被当成未知键名丢掉。

## 宏播放（此前只解析）

`{macro: "CTRL f d"}` 此前被解析后丢弃（键栏不渲染）。本轮按上游
`TerminalExtraKeys.onExtraKeyButtonClick` 的语义实现：宏串按空白切分，`CTRL`/`ALT`/
`SHIFT` 作用于其后第一个普通记号，普通记号经原生键编码器发出（连同平台文本，否则
可打印键不产生字节）；没有键码的记号按上游 `TerminalView.inputCodePoint` 的语义直接
写出（Ctrl 逐码点折叠、Alt 逐码点加 ESC、Shift 对文本无作用）。

不可用的宏（含 `FN` 或未知记号）只丢自己那一项，不作废整行。`FN`/`SHIFT` 键本身与
`volume-keys` 的虚拟 FN 映射不支持，已写入 `PROHIBITED.md`。

## extra-keys 的 popup（此前只当纯文本）

上游的 `popup` 是**完整的按键**（`ExtraKeysInfo.normalizeKeyConfig` 把字符串包成
`{key: 'X'}`，对象原样成为次级按键）。本应用此前只把字符串 `popup` 当纯文本发出，
对象形式整个丢弃并记警告：同一份 `extra-keys` 在两个应用里长按的结果不同。

本轮已实现：

- `ExtraKey.popup` 改为 `ExtraKey?`，popup 与主键同构：字符串是键名或字面量，对象
  形式可带 `macro`。
- 键栏引入 `KeyAction`（编码器键码 / 特殊动作 / 字面量文本 / 宏），主键与 popup 共用
  同一条派发路径：键名走编码器，特殊键走键栏回调，宏按顺序播放，字面量直接写出。
- 长按执行次级按键且不连带触发主键；次级按键不可用时只丢长按动作并记警告。

与上游的一处差异：上游由「上滑」触发并弹出浮层，本应用没有浮层，长按直接执行次级键
（语义一致，少了中间那层浮层）。`PROHIBITED.md` 中「对象形式 `popup` 不支持」一条已
删除。

## 会话快捷键（此前未实现）

`shortcut.create-session` / `shortcut.next-session` / `shortcut.previous-session` 此前未解析；
本轮按 `termux.properties` 的 `Ctrl+<字符>` 语义实现：在实体键盘事件进入终端前匹配，
命中时消费下按与抬起，分别新建会话、切到下一个/上一个会话。`Ctrl+Shift` 与配置的小写
等价，Ctrl+Alt 不参与。`shortcut.rename-session` 没有对应 UI，解析时明确忽略并记警告。

## Ctrl+空格的抬起事件（此前漏回输入法）

`ctrl-space-workaround` 此前只在下按补发 NUL，抬起事件仍漏回输入法/编码器，同一次
组合键有两条去路。本轮改为上下两路都消费，只在下按发送一次；原生侧补一条回归锁住
「Ctrl+空格在没有平台文本时仍是 NUL」（补偿走的正是这条只下键码的通道）。

## 边界差异

- 配置重读时机：`config.md` 写「启动首帧前与会话配置重下发时重读」，实现一致
  （`TerminalRuntime.start` 与 `TerminalRuntime.applySettings`），比上游的「下次启动
  会话时生效」更早。
- 超大文件口径：实现把超限当作读失败（`TermuxPropertiesRepository.readFile`），保留上一次
  成功的结果；首次加载时「上一次成功的结果」就是默认值，重载时沿用旧配置。规范已按
  现状写明（`config.md` 与 `openspec` 的「配置过大」场景），差异消解。
- `terminal-cursor-style` 的 `beam`：`config.md` 与 `DESIGN.md` 均允许 `block`/`bar`/`beam`/`underline`，
  实现接受 `beam`，落成竖线（与 `bar` 同形）。termux 上游只有 `block`/`bar`/`underline`，
  `beam` 是本应用额外接受的别名，已在文档中注明。

## 字体：`.config/termux` 路径已纳入（已修复）

`DESIGN.md` 字体节允许两组路径：`~/.termux` 与 `~/.config/termux`。

实现现已同时覆盖两组路径：

- `FontUtils.termuxDefaultFontFile` 先查 `~/.termux/font.*`，其次 `~/.config/termux/font.*`，
  `Bridge.probeDefaultFontFile` 据此决定主字体覆盖。
- `FontUtils.termuxFontDirs` 返回两组 `fonts` 目录；`TerminalViewModel.applyStoredFontSettings`、
  `refreshFontList` 与 `TerminalRuntime.applyRenderSettings` 都把所有存在的目录注册到 bridge。

主路径优先，不复制/移动文件，渲染逻辑不变。

## Bootstrap 内置 SHA-256 与禁止条目

`PROHIBITED.md` 禁止的是「Bootstrap zip 的 sha256 独立校验文件」，即随 zip 一起分发的
外部校验文件。实现不是这种独立校验文件：`BootstrapDownloader` 只在 URL 等于
`TermuxBootstrap.downloadUrl(architecture)` 时，使用 `TermuxBootstrap.sha256(architecture)`
内置的钉住摘要校验；自定义 URL 无法预知内容，会明确记录日志并跳过，不会静默降级。
摘要在下载落盘途中顺带计算，不为校验再读一遍归档。

评估：这是有意义的安全校验，防止引导 URL 被用户改成不可信来源后执行其中的 `postinst`；
复杂度低、性能开销低，与禁止条目字面不冲突。保留合理。

## 规范文档自身的不一致

- `docs/specification/DESIGN.md` 与 `docs/specification/config.md` 对
  `terminal-cursor-style` 的取值集合现已一致（均含 `block`/`bar`/`beam`/`underline`）。
- `openspec/specs/termux-configuration` 中 `colors.properties` 覆盖与 `extra-keys`
  布局的 MUST 要求与 `docs/specification` 的禁令相反。`colors.properties` 要求已随
  功能移除；`extra-keys` 的布局要求现已与 `config.md` 对齐，逐键条款（`fullscreen`、
  `use-fullscreen-workaround`、`ctrl-space-workaround`、`extra-keys-text-all-caps`、
  `terminal-margin-*`、`terminal-toolbar-height`、`back-key`、`volume-keys`）也已补齐。
- `openspec/specs/termux-configuration` 的「键栏中不可渲染的项」单条 requirement 超过
  500 字符，`openspec validate --all --strict` 不过（本轮拆分该条并把 `popup` 语义
  单独成条后，41 项全部通过）。

## 规范文档与实现的漂移

以下条款描述的功能已在实现中删除，`openspec/specs` 仍写着它们，本轮已清掉：

- `termux-configuration` 的 `bell-character` 响铃方式（实现已无 BEL 事件链路，
  `DESIGN.md` 要求忽略响铃）。
- `termux-configuration` 的 `terminal-cursor-blink-rate` 区间与相位（实现已无补帧，
  `PROHIBITED.md` 禁止光标闪烁开关与速度）。
- `termux-configuration` 的会话切换提示与 `disable-terminal-session-change-toast`
  （实现已无该提示）。
- `upstream-alignment` 的事件集描述里残留的 bell（响铃不在事件集内）。

## 分支合入结果

`wb` 与 `wb2` 的提交逐个过筛后合入 `main`：引导归档校验和与其规范、内置摘要的十六进制
工具、installer 用例前提确认、`colors.properties` 读失败保留上次覆盖、键栏坏项只丢自己
一项与 `display` 覆盖键帽、创建与缩放共用尺寸判据与 OSC 52 按字符边界截断、输入三态
循环清理、PTY 等待可写超时的换算、`extra-keys` 空布局与坏项口径、以及对应的
`openspec` 条款。

未合入的部分及原因：

- 两个分支重复的提交（滚动门控超时、归档校验和的早期版本、规范小节名引用等）在
  `main` 已有等价实现，不重复合入。
- 响铃链路与光标闪烁补帧在两个分支上是旧实现，`main` 已按规范删除，不回退。
- `night-mode` 覆盖、会话切换提示在分支上仍是旧实现，`main` 已删除，不回退。
- `wb` 的 spotless 排版提交与分支自带的审计文档不并入：前者只改格式，后者是分支当时的
  阶段性结论，本文件取而代之。

## 第 6 轮发现的三处漂移（已订正）

第 6 轮（2026-11-01）全量通读 Rust 与 Kotlin 源码后，逐项复核「规范—文档—实现」三方对应，
发现三处漂移，均已订正；无代码改动。

1. **次级规范引用了不存在的目录**：`openspec/specs/upstream-alignment/spec.md` 三处仍称
   有 6 份综合研究报告「现役在 `docs/research/`」，但该目录已随一次仓库清理提交（移除 `docs/research` 并收敛 STATUS
   与 semgrep 覆盖配置）整体删除。
   同一规范另有 `termux-app.md` 索引行，把上游 `termux.properties` 归入「不适用」的理由写作
   「`PROHIBITED.md` 明禁解析该文件」——现行 `PROHIBITED.md` 只禁**未列入 `config.md` 的键**，
   且本仓已按 `termux-configuration` 解析了允许的 20 个键。
   已由 openspec 变更 `2026-11-01-fix-research-index-retirement-drift` 订正：
   研究报告口径改为「全部 111 份已整体退役，索引表是唯一常驻载体」，
   `termux-app.md` 索引行的判定理由按现行禁令与现行实现更正。
2. **本文件的键面计数漏计**：「配置键面」小节写「16 个键」，漏计 4 个 `shortcut.*` 键，
   实为 20 个；而同一文件的结论节已经描述了 `shortcut.*` 的实现——文件内部已不自洽。
   已更正小节标题、表内补四行，并同步更正结论节口径。
3. **本文件的审计轮次记录顺序错乱**：第 1 轮条目被排在第 2–5 轮之后且孤立成段，
   轮次是时间线，倒序不可读。已按轮次升序重排。

## 第 7 轮发现并修复的四处实现缺陷（有代码改动）

复核方式：逐文件通读 `native/src` 与 `android/app/src/main` 的生产代码（测试代码单列），
再用 `cargo clippy -- -W clippy::pedantic` 的截断类告警逐个核对是否存在越界路径。四处
缺陷都不违反规范条文——它们违反的是规范**要求达到的效果**，故按「实现缺陷」而非「规范
漂移」记录。

1. **同族常规重复面冒充斜面**（`render/font/pipeline.rs::resolve_style_face`）。`fontdb`
   的 `query` 是「最佳匹配」而非严格过滤：同一字体文件被装进库两次（系统扫描一遍、
   `load_font_file` 或用户字体目录再装一遍）时，族内会出现两个同族同字形、样式都是
   `Normal` 的面，查询会把**另一个常规面**当作斜体请求的最佳匹配返回。原判据只比较
   「命中面是否等于基底面」，于是这一情形被判为「有真实斜面」，斜体整段直立显示且无
   任何日志。现判据改为「命中面自身必须带有所请求的样式」，否则合成；基底面自己就是
   所需样式时仍返回基底面以跳过二次合成。回归用例在
   `render/font/tests/loading.rs::duplicate_regular_face_is_not_taken_as_the_italic_face`，
   已反向验证（撤掉修复即红）。
2. **剪贴板队列溢出告警未限频**（`terminal/session/output.rs::drain_callback_events`）。
   溢出逐条 `warn`：病态程序每次写入都能触发一条，logcat 被同一句话刷满。现改为
   500ms 窗口限频，行内带窗口内累计丢弃条数（条数不因限频而丢失），与渲染洪泛、事件
   队列溢出的限频同口径；`slow_log::CountedLogThrottle` 因此补了成批计数路径。
3. **会话创建内联重复的网格尺寸判据**（`terminal/session/spawn.rs`）。`spawn` 自己写了
   一遍 0/`u16` 上界校验，与 `validated_grid` 同义。两份判据迟早分叉（`as u16` 会静默
   截断越界尺寸），改为复用同一函数。
4. **字体族基面取到粗/斜面，空族名匹配任意面**（`render/font/pipeline/families.rs`）。
   `load_font_file` 取文件中**首个有族名**的面作基面：文件里 Bold 排在 Regular 之前时，
   基面就是粗面，而基面是全部无样式文本的出字入口——用户只选了字体文件，整屏正文却
   变成粗体。现优先取族内常规面（字重 < 700 且非倾斜），无常规面时退回首个有族名的面。
   同文件的族名模糊匹配还漏了归一化后为空的闸门：族名只有 `-`/`_`/空白时 `cleaned`
   为空串，而 `contains("")` 恒真，`set_font_family("-")` 会匹配到库内任意面并返回成功。
   现归一为空即判未找到。两条判据对应 openspec `font-selection` 的两个新 requirement，
   回归用例在 `families.rs` 的测试模块（含判别力反向验证）。

验证：`cargo clippy --all-targets -- --deny warnings` 通过；`cargo test --lib` 782 用例
通过；`openspec validate --all --strict` 41/41 通过。

## 第 8 轮发现并修复的缺陷（Kotlin 字号区间与原生文本读口）

本轮按 `kotlin-footguns` 的 `footgun-scan` 对 `android/app/src/main/java` 全量扫描：
10 处命中全部落在 `look`（需人工判断）级别，逐条核对后 8 处判为不适用（两处
`runBlocking` 都在专用线程上桥接同步原生调用；三处「同名 Job 重启」都带
`isActive` 单飞闸门；两处 `pointerInput(Unit)` 分别经 `rememberUpdatedState` 读最新
回调、且只消费空点击；`System.loadLibrary` 之后有装载线程与结果日志）。另两处命中
引出一处真实缺陷：

1. **字号区间收紧后调节条停在旧值**（`ui/SettingsScreen.kt` 的
   `AppearanceSectionContent`）。`rememberSaveable` 的初值只在首次组合时按区间钳位，
   而 `AndroidManifest.xml` 里 `MainActivity` 的 `android:configChanges` 自行处理
   `orientation|screenSize|fontScale`，组合不重建：
   宽屏存下的 96sp 旋转到 360dp 竖屏后，上界只剩 30sp，滑块状态仍是 96。Slider 对越界
   值只钳拇指、不钳读数——拇指停在轨末、读数打印 96sp、原生按 30sp 渲染，三者互不
   相同；用户按下拇指的那一刻，指针位置换算出的上界被写回，等于「看了一眼设置就被改
   了值」。现新增 `SettingsRepository.clampFontSize` 作为唯一钳位式，区间一变即就地
   收敛；`TerminalRuntime.clampFontSizeSp` 改为委托（此前仓库里有两份
   `coerceIn(MIN, fontSizeMaxSp(...))`）；原生尚未上报度量时的字体信息占位行也改用
   调节条同值，不再报出一个并未渲染的字号。存量值只收敛显示与生效值，不回写
   DataStore——回到宽屏时原值仍有效。
2. **可见文本吞掉空行**（`terminal/ghostty_terminal/internal/dispatch.rs` 的
   `Query::ReadVisibleText`）。`read_line_text_impl` 对空行返回 `None`，原实现连
   换行一起跳过：内容行之后每一行都上移一行，调用方按行号取到的是错位内容；同一份
   终端在 `ReadAllText` 里是保行数的，两条读口对同一屏给出不同行距。现空行也产出
   换行，行号与网格一一对应。

验证：`cargo clippy --all-targets -- --deny warnings` 通过；`cargo test --lib` 783
用例通过（新增 `read_visible_text_keeps_blank_rows`）；`openspec validate --all
--strict` 41/41 通过；`footgun-scan --base origin/main` 对本轮新增行 0 命中。Kotlin
侧仍无 Android SDK 可执行 `testDebugUnitTest`，字号钳位的新用例
（`FontSizeRangeTest.clampPullsStoredValuesIntoTheCurrentRange`）只做静态复核。

## 审计轮次记录

轮次按推进顺序排列；每轮只记当轮新增差异，已订正的旧结论在对应条目里注明更正。

- 第 1 轮（2026-10-31）：全量复核代码与规范文档，未发现新增差异。验证：`cargo fmt --check` 通过、`openspec validate --all --strict` 41/41 通过、`testDebugUnitTest` 762 用例通过（含 native 产物）。无代码修改。
- 第 2 轮（2026-10-31）：复核 TermuxProperties 键面与解析器；`cargo fmt --check` 通过、openspec 41/41 通过。无代码修改。（第 6 轮更正：本轮原文写「16 个键与规范一致」，但 `shortcut.*` 四个键当时已落地且已收录于 `config.md`，键面实为 20 个。）
- 第 3 轮（2026-10-31）：复核禁止功能项（MCP、Termux API 扩展、termux-exec、proot、X11、Shizuku 等）在代码中无实现；仅注释提及 termux-exec 的 login 探针。无代码修改。
- 第 4 轮（2026-10-31）：复核字体系统与 fonts.xml 解析链路、AndroidManifest applicationId 与 SDK 版本，与规范一致。无代码修改。
- 第 5 轮（2026-10-31）：复核环境变量白名单、Nix 存储路径引用、Cargo/Gradle 版本与 ABI 过滤，与规范一致。无代码修改。
- 第 6 轮（2026-11-01）：全量通读 Rust 与 Kotlin 源码后复核「规范—文档—实现」三方对应，发现并订正上列三处漂移。无代码修改。
- 第 7 轮（2026-11-01）：逐文件通读生产代码并用 pedantic 级 clippy 核对截断类告警，发现并修复上列四处实现缺陷（字体斜/粗面判定、剪贴板告警限频、网格尺寸判据去重、字体族基面与空族名匹配）。每处均带回归用例；`cargo test --lib` 782 用例通过、openspec 41/41 通过。
- 第 8 轮（2026-11-01）：用 `kotlin-footguns` 的扫描器全量扫描 Kotlin 生产代码，10 处命中逐条判定（8 处不适用、2 处引出字号区间缺陷），并修复原生可见文本吞空行的问题；`cargo test --lib` 783 用例通过、openspec 41/41 通过。
