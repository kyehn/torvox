# 代码与规范差异审计

审计日期：2026-10-31。范围：`main`（含本轮从 `wb` / `wb2` 合入的提交）与工作区改动。
依据：`docs/specification/` 全部文档；`openspec/specs` 作为次级规范一并核对。代码位置用
文件与符号表示，不写行号。

## 结论

- 差异集中在配置面：`termux.properties` 实解析 16 个键，规范允许 `config.md` 列出的键；既然这些键
  已有消费方，就不是死代码，而是规范未收录的能力。
- `colors.properties` 解析已按用户指令移除，实现与 `config.md`、`PROHIBITED.md` 一致。
- 修饰键栏默认布局与 Termux 上游不一致：规范要求「布局/按键和 Termux 完全相同」，
  代码把首行的 `/` 与 `-` 换成了 `DRAWER` 与 `SCROLL`。
- 三处边界差异：配置重读时机、超大文件的口径、`beam` 取值在两份规范里的说法不一致。
- 新增差异：字体用户目录只扫描 `~/.termux`，遗漏 `~/.config/termux` 的默认字体与字体目录；
  影响 Termux 用户已有字体配置的兼容性，修复成本低。
- 新增差异：Bootstrap 安装前清空而非拒绝已存在的 `usr-staging`，与 `DESIGN.md` 字面要求
  不一致；实现更利于恢复失败安装，但并发/失败语义需确认。
- Bootstrap 的内置 SHA-256 校验不是 `PROHIBITED.md` 禁止的 sidecar 校验：它不读取外部
  校验文件，只对钉住的预设 URL 生效，复杂度与性能开销低，保留合理。
- 本轮清掉了 `openspec/specs` 里已删除功能的残留条款（响铃、光标闪烁、会话切换提示），
  使次级规范重新与实现对齐；其余差异按现状登记，未擅自删除功能。

## 配置键面：16 个键（已移除实体键盘快捷键设置）

`config.md` 规定只读取 `default-working-directory`、`terminal-transcript-rows`、
`terminal-cursor-style`，其余键与 `colors.properties` 一律不解析；`PROHIBITED.md` 重申。
`TermuxPropertiesParser.parse` 实际解析下表 16 个键，`TermuxProperties` 为每个键保留字段，
并各有消费方。

| 键 | 实际作用 | 与规范的关系 |
| --- | --- | --- |
| `default-working-directory` | 会话工作目录 | 允许 |
| `terminal-transcript-rows` | 回滚行数 | 允许 |
| `terminal-cursor-style` | 光标默认样式 | 允许 |
| `use-fullscreen-workaround` | 全屏时系统栏改为滑动临时唤出 | 三键限制 |
| `fullscreen` | 隐藏系统栏 | 三键限制 |
| `ctrl-space-workaround` | 补发 Ctrl+空格 | 三键限制 |
| `hide-soft-keyboard-on-startup` | 启动时是否请求输入法 | 三键限制 |
| `extra-keys-text-all-caps` | 键帽大小写 | 三键限制 |
| `terminal-margin-horizontal` | 终端左右边距 | 三键限制 |
| `terminal-margin-vertical` | 终端上下边距 | 三键限制 |
| `terminal-toolbar-height` | 修饰键栏高度倍率 | 三键限制 |
| `extra-keys-style` | 键帽字形档位 | 三键限制 |
| `extra-keys` | 键栏布局（行、键位、字形、`display` 覆盖） | 三键限制；`DESIGN.md` 要求固定 2 行 7 列；`PROHIBITED.md` 禁止修改键栏布局 |
| `back-key` | 返回键改发 ESC | 三键限制 |
| `volume-keys` | 音量减改为虚拟 Ctrl | 三键限制 |
| `soft-keyboard-toggle-behaviour` | 键盘键改为启用/禁用输入法 | 三键限制 |

证据：`TermuxPropertiesParser.parse` 的 `boolean`、`integer`、`float`、`choice`
与 `extraKeys` 分支；`TermuxProperties` 的字段与默认值；消费方分别是 `MainActivity`
（`ctrlSpaceWorkaround`、`backKey`、`volumeKeys`）、
`TerminalScreen`（`useFullscreenWorkaround`、`fullscreen`、`hideSoftKeyboardOnStartup`、
`extraKeys*`、`terminalMargin*Dp`、`softKeyboardToggleBehaviour`）、`SystemBars`
（`fullscreen`）、`TerminalRuntime`（`terminalToolbarHeightScale`、`extraKeys`）。

其中默认值本身改变行为：

- `extra-keys` 默认把第一行的 `/` 与 `-` 换成 `DRAWER` 与 `SCROLL`，与 `DESIGN.md`
  「布局/按键和 Termux 完全相同」冲突（详见下节）。
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

冲突：`DESIGN.md` 要求固定 2 行 7 列、布局与按键和 Termux 完全相同。抽屉已有
`ModalNavigationDrawer` 的左缘滑动入口；滚动锁定不在 `DESIGN.md` 中声明。代码注释已
自认这一偏离，冲突仍在。

## 边界差异

- 配置重读时机：`config.md` 写「启动首帧前与会话配置重下发时重读」，实现一致
  （`TerminalRuntime.start` 与 `TerminalRuntime.applySettings`），比上游的「下次启动
  会话时生效」更早。
- 超大文件口径：`config.md` 写「单文件超过 64 KiB 视为无效并跳过」；实现把超限当作
  读失败（`TermuxPropertiesRepository.readFile`），保留上一次成功的结果。首次加载时
  「上一次成功的结果」就是默认值，两者一致；重载时才会分叉。
- `terminal-cursor-style` 的 `beam`：`config.md` 允许 `block`/`bar`/`beam`/`underline`，
  `DESIGN.md` 只列 `block`/`bar`/`underline`。实现接受 `beam`，落成竖线（与 `bar` 同形）。

## 字体：仅扫描 `.termux`，遗漏 `.config/termux`

`DESIGN.md` 字体节明确允许两组路径：

- 默认字体覆盖：`/data/data/com.termux/files/home/.termux/font.ttf` 或
  `/data/data/com.termux/files/home/.config/termux/font.ttf`（也支持 ttc、otf）。
- 用户字体目录：`/data/data/com.termux/files/home/.termux/fonts` 或
  `/data/data/com.termux/files/home/.config/termux/fonts` 存在时加入扫描路径。

实现只认前一组/前一个目录：

- `FontUtils.termuxDefaultFontFile` 只在 `~/.termux` 下查找 `font.ttf`/`font.ttc`/`font.otf`，
  不查找 `~/.config/termux`。`Bridge.probeDefaultFontFile` 用它决定主字体覆盖。
- `FontUtils.termuxFontDir` 只返回 `~/.termux/fonts`；`TerminalViewModel.applyStoredFontSettings`
  与 `refreshFontList` 只注册这一目录，`TerminalRuntime` 的额外字体路径 likewise。

影响：Termux 用户若把字体放在 `~/.config/termux`（规范明确支持的路径），应用会忽略它们，
主字体覆盖不生效、字体列表里也看不到。复杂度低、稳定性影响低；修复只需把两个路径都纳入
查找/注册，不改变渲染逻辑。

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
  `terminal-cursor-style` 的取值集合不一致（见上节）。
- `openspec/specs/termux-configuration` 中 `colors.properties` 覆盖与 `extra-keys`
  布局的 MUST 要求与 `docs/specification` 的禁令相反。`colors.properties` 要求已随
  功能移除；`extra-keys` 仍是两套规范相左，本审计按 `docs/specification` 记为违规。

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