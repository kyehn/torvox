# 代码与规范差异审计

审计日期：2026-10-31。范围：`main`（含本轮从 `wb` / `wb2` 合入的提交）与工作区改动。
依据：`docs/specification/` 全部文档；`openspec/specs` 作为次级规范一并核对。代码位置用
文件与符号表示，不写行号。

## 结论

- 配置面：`termux.properties` 实解析 16 个键，`docs/specification/config.md` 与 `PROHIBITED.md` 现已全部收录；
  代码与规范对齐，不再是「规范未收录」。剩余差异集中在修饰键栏默认布局、字体扫描路径、Bootstrap staging 与边界口径。
- `colors.properties` 解析已按用户指令移除，实现与 `config.md`、`PROHIBITED.md` 一致。
- 修饰键栏默认布局：`DESIGN.md` 现已明确记录在 Termux 上游基础上将 `/` 与 `-` 替换为
  `DRAWER` 与 `SCROLL`，键面与规范对齐；剩余为产品取舍，不再是文档漂移。
- 边界差异：配置重读时机与实现一致；超大文件口径首次加载与重载略有分叉（实现保留上次成功结果）；
  `beam` 取值在 `DESIGN.md` 与 `config.md` 现已一致。
- 字体：`~/.config/termux` 的默认字体与字体目录现已纳入扫描/注册，主路径优先。
- 新增差异：Bootstrap 安装前清空而非拒绝已存在的 `usr-staging`，与 `DESIGN.md` 字面要求
  不一致；实现更利于恢复失败安装，但并发/失败语义需确认。
- Bootstrap 的内置 SHA-256 校验不是 `PROHIBITED.md` 禁止的 sidecar 校验：它不读取外部
  校验文件，只对钉住的预设 URL 生效，复杂度与性能开销低，保留合理。
- 本轮清掉了 `openspec/specs` 里已删除功能的残留条款（响铃、光标闪烁、会话切换提示），
  使次级规范重新与实现对齐；其余差异按现状登记，未擅自删除功能。

## 配置键面：16 个键（与 config.md 对齐）

`docs/specification/config.md` 与 `PROHIBITED.md` 现已收录全部 16 个键；代码解析与文档一致，
不再是「三键限制」下的违规。下表仅作现状登记，键面本身已无规范差异。

`TermuxPropertiesParser.parse` 解析下表 16 个键，`TermuxProperties` 为每个键保留字段，
并各有消费方。

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
| `extra-keys` | 键栏布局（行、键位、字形、`display` 覆盖） | config.md 已收录；`DESIGN.md` 默认布局在 Termux 上游基础上将 `/` 与 `-` 替换为 DRAWER 与 SCROLL（见下节） |
| `back-key` | 返回键改发 ESC | config.md 已收录 |
| `volume-keys` | 音量减改为虚拟 Ctrl | config.md 已收录 |
| `soft-keyboard-toggle-behaviour` | 键盘键改为启用/禁用输入法 | config.md 已收录 |

证据：`TermuxPropertiesParser.parse` 的 `boolean`、`integer`、`float`、`choice`
与 `extraKeys` 分支；`TermuxProperties` 的字段与默认值；消费方分别是 `MainActivity`
（`ctrlSpaceWorkaround`、`backKey`、`volumeKeys`）、
`TerminalScreen`（`useFullscreenWorkaround`、`fullscreen`、`hideSoftKeyboardOnStartup`、
`extraKeys*`、`terminalMargin*Dp`、`softKeyboardToggleBehaviour`）、`SystemBars`
（`fullscreen`）、`TerminalRuntime`（`terminalToolbarHeightScale`、`extraKeys`）。

其中默认值本身改变行为：

- `extra-keys` 默认把第一行的 `/` 与 `-` 换成 `DRAWER` 与 `SCROLL`：`DESIGN.md` 现已明确记录这一替换，
  作为本应用相对 Termux 的有意偏离（抽屉与滚动锁定需要固定入口）。
- `volume-keys` 默认 `virtual`，`MainActivity` 默认把音量减拦截为一次性 Ctrl；
  `DESIGN.md` 未声明该行为。
- `terminal-margin-horizontal` 默认 3dp、`terminal-margin-vertical` 默认 0dp、
  `terminal-toolbar-height` 默认 1 倍：默认视觉与 Termux 接近，但键面本身违规。

其余键默认值不改变现有行为，非默认值会启用未声明能力。

## colors.properties（已移除）

`TermuxPropertiesRepository` 不再读取第二个文件，`ColorsPropertiesParser`、
`RenderContract.colors`、`Bridge.setPaletteOverride` 与原生覆盖通道均已删除。
终端配色只来自内置主题，`colors.properties` 存在也不生效。

此前该能力与 `config.md`、`PROHIBITED.md` 的禁令冲突；移除后实现与规范一致。

## Bootstrap：清空而非拒绝已存在的 staging

`DESIGN.md` 要求安装前 `usr-staging` 目录必须不存在，存在则拒绝执行。实现里
`BootstrapInstaller.install` 先调 `clearStaging(stagingDir)`：目录能清空就继续，只有清不净
才抛错中止。这不是「存在即拒绝」，而是「存在就清掉」。

评估：清空残留 staging 能恢复上一次失败安装，避免用户手动删目录；但与规范字面
不同，且把「上一次安装失败留下的 staging」与「并发安装」混在一起处理。复杂度低，
风险主要在并发/失败恢复语义，建议明确是保留清空策略还是按规范改成拒绝。

## 修饰键栏默认布局

`TermuxProperties.DEFAULT_EXTRA_KEYS` 第一行为 `ESC DRAWER SCROLL HOME UP END PGUP`，
第二行为 `TAB CTRL ALT LEFT DOWN RIGHT PGDN`。上游 termux 的默认首行是
`ESC / - HOME ↑ END PGUP`：本应用把 `/` 与 `-` 换成了抽屉与滚动锁定两个入口。

`DESIGN.md` 现已明确记录：固定 2 行 7 列，默认布局在 Termux 上游基础上将 `/` 与 `-`
替换为 DRAWER 与 SCROLL 入口。抽屉仍有 `ModalNavigationDrawer` 的左缘滑动入口；滚动锁定是
本应用相对 Termux 的有意补充。键面本身已无规范差异，剩余的是产品取舍，不是文档漂移。

## 边界差异

- 配置重读时机：`config.md` 写「启动首帧前与会话配置重下发时重读」，实现一致
  （`TerminalRuntime.start` 与 `TerminalRuntime.applySettings`），比上游的「下次启动
  会话时生效」更早。
- 超大文件口径：`config.md` 写「单文件超过 64 KiB 视为无效并跳过」；实现把超限当作
  读失败（`TermuxPropertiesRepository.readFile`），保留上一次成功的结果。首次加载时
  「上一次成功的结果」就是默认值，两者一致；重载时才会分叉。
- `terminal-cursor-style` 的 `beam`：`config.md` 与 `DESIGN.md` 均允许 `block`/`bar`/`beam`/`underline`，
  实现接受 `beam`，落成竖线（与 `bar` 同形）。

## 字体：`.config/termux` 路径已纳入（已修复）

`DESIGN.md` 字体节允许两组路径：`~/.termux` 与 `~/.config/termux`。

实现现已同时覆盖两组路径：

- `FontUtils.termuxDefaultFontFile` 先查 `~/.termux/font.*`，其次 `~/.config/termux/font.*`，
  `Bridge.probeDefaultFontFile` 据此决定主字体覆盖。
- `FontUtils.termuxFontDirs` 返回两组 `fonts` 目录；`TerminalViewModel.applyStoredFontSettings`、
  `refreshFontList` 与 `TerminalRuntime.applyRenderSettings` 都把所有存在的目录注册到 bridge。

主路径优先，不复制/移动文件，渲染逻辑不变。

## Bootstrap 内置 SHA-256 与禁止条目

`PROHIBITED.md` 禁止的是「Bootstrap zip 的 sha256 sidecar 校验」，即随 zip 一起分发的
外部校验文件。实现不是 sidecar：`BootstrapDownloader` 只在 URL 等于
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
  功能移除；`extra-keys` 的布局要求现已与 `config.md` 对齐，`openspec` 仍缺
  `fullscreen`、`use-fullscreen-workaround`、`ctrl-space-workaround`、`extra-keys-text-all-caps`、
  `terminal-margin-*`、`terminal-toolbar-height`、`back-key`、`volume-keys` 的逐键条款，
  仅作能力登记，不阻断实现。

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