# 代码与规范差异审计

审计日期：2026-10-30。范围：`main` 最近三天的提交、`origin/wb` 与 `origin/wb2` 的未合并提交。依据：`docs/specification/` 全部文档。代码位置用文件与符号表示，不写行号。

## 结论

- 一处系统性违规：`termux.properties` 实际解析 24 个键，`config.md` 只允许 3 个；另外解析 `colors.properties`。
- 由此派生多项未声明能力。其中光标闪烁、响铃响应、`colors.properties`、修饰键栏自定义与默认布局偏离有规范直接冲突。
- 两个边界差异：配置重读时机、超大文件保留旧值。
- `wb` 的 5 个正确提交已合入；7 个提交因触碰禁止项或测试跳过未合入；`wb2` 只含这 7 个提交，没有新增正确修改。

## 一、明确违反

### 配置键面：24 对 3

`config.md` 规定只读取 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style`，其余键与 `colors.properties` 一律不解析；`PROHIBITED.md` 重申。`TermuxPropertiesParser.parse` 实际解析以下 24 个键，`TermuxProperties` 为每个键保留字段。

| 键 | 实际作用 | 冲突 |
| --- | --- | --- |
| `default-working-directory` | 会话工作目录 | 允许 |
| `terminal-transcript-rows` | 回滚行数 | 允许 |
| `terminal-cursor-style` | 光标样式 | 允许 |
| `terminal-cursor-blink-rate` | 光标闪烁补帧 | 三键限制；`PROHIBITED.md` 禁止闪烁开关与速度 |
| `bell-character` | 响铃时震动或出声 | 三键限制；`DESIGN.md` 要求忽略 bell |
| `extra-keys` | 修饰键栏布局 | 三键限制；`DESIGN.md` 要求固定布局；`PROHIBITED.md` 禁止修改布局 |
| `extra-keys-style` | 修饰键栏字形集合 | 三键限制 |
| `extra-keys-text-all-caps` | 修饰键栏大小写 | 三键限制 |
| `terminal-margin-horizontal` | 终端左右边距 | 三键限制 |
| `terminal-margin-vertical` | 终端上下边距 | 三键限制 |
| `terminal-toolbar-height` | 修饰键栏高度倍率 | 三键限制 |
| `night-mode` | 覆盖应用日夜判定 | 三键限制 |
| `soft-keyboard-toggle-behaviour` | 键盘键改为启用/禁用输入法 | 三键限制 |
| `disable-terminal-session-change-toast` | 关闭会话切换提示 | 三键限制 |
| `hide-soft-keyboard-on-startup` | 启动时是否请求输入法 | 三键限制 |
| `shortcut.create-session` | 硬件键盘新建会话 | 三键限制；`PROHIBITED.md` 禁止实体键盘快捷键设置 |
| `shortcut.next-session` | 硬件键盘下一会话 | 同上 |
| `shortcut.previous-session` | 硬件键盘上一会话 | 同上 |
| `disable-hardware-keyboard-shortcuts` | 关闭上述快捷键 | 同上 |
| `fullscreen` | 隐藏系统栏 | 三键限制 |
| `use-fullscreen-workaround` | 系统栏唤出方式 | 三键限制 |
| `ctrl-space-workaround` | 补发 Ctrl+空格 | 三键限制 |
| `back-key` | 返回键改发 ESC | 三键限制 |
| `volume-keys` | 音量减改为虚拟 Ctrl | 三键限制 |

证据：`TermuxPropertiesParser.parse` 的 `boolean`、`choice`、`integer`、`float`、`shortcut`、`extraKeys` 分支；`TermuxProperties` 的字段与默认值；`TerminalScreen`、`TerminalRuntime`、`MainActivity` 对上述字段的读取。

其中默认值即改变行为的项：

- `extra-keys` 默认把第一行的 `/` 与 `-` 换成 `DRAWER` 与 `SCROLL`，与 `DESIGN.md`「布局/按键和 Termux 完全相同」冲突。
- `bell-character` 默认 `vibrate`，与 `DESIGN.md`「忽略 bell character，不做出响应」冲突。
- `disable-terminal-session-change-toast` 默认 `false`，切换会话时实际弹出提示；`DESIGN.md` 未声明该提示。
- `volume-keys` 默认 `virtual`，`MainActivity.handleTermuxPropertyKey` 默认把音量减拦截为一次性 Ctrl；`DESIGN.md` 未声明该行为。
- `terminal-margin-horizontal` 默认 3dp、`terminal-margin-vertical` 默认 0dp、`terminal-toolbar-height` 默认 1 倍；默认视觉与 Termux 接近，但键面本身违规。

其余键默认值不改变现有行为，但非默认值会启用未声明能力。

### `colors.properties`

`TermuxPropertiesRepository.reloadNow` 用 `locate(home, "colors.properties")` 读取第二个文件，`ColorsPropertiesParser.parse` 解析前景、背景、光标与 16 个 ANSI 颜色；`RenderContract.colors` 经 `Bridge.setPaletteOverride` 下发到原生 `setPaletteOverride`，最终走 `theme.with_overrides`。

冲突：`config.md` 与 `PROHIBITED.md` 明确禁止解析 `colors.properties`；`PROHIBITED.md` 同时禁止自定义终端主题。该能力不是默认生效，但文件存在即生效。

### 光标闪烁

`terminal-cursor-blink-rate` 经 `TerminalRuntime.scheduleCursorBlink` 启动补帧协程，`RenderContract.cursorBlinkRateMs` 下发到 `Bridge.setCursorBlinkRate`，原生以 `CURSOR_BLINK_RATE_MS`、`BLINK_EPOCH`、`cursor_blink_phase_on` 计算亮灭相位。

冲突：`DESIGN.md` 要求光标「不闪烁」；`PROHIBITED.md` 禁止光标闪烁开关与速度。默认值 0 不闪，但能力与键面仍在。

### 响铃

`TerminalRuntime.EventDispatcher.handle` 在收到 `poll.bells` 时调用 `BellAlerter.ring`，后者按 `bell-character` 震动或播放提示音。

冲突：`DESIGN.md` 要求忽略 bell character，不做出响应。

### 修饰键栏默认布局

`TermuxProperties.DEFAULT_EXTRA_KEYS` 第一行为 `ESC DRAWER SCROLL HOME UP END PGUP`，第二行为 `TAB CTRL ALT LEFT DOWN RIGHT PGDN`。代码注释明确说明这不是 Termux 上游默认值。

冲突：`DESIGN.md` 要求固定 2 行 7 列，布局/按键和 Termux 完全相同。抽屉已有 `ModalNavigationDrawer` 的左缘滑动入口；滚动锁定不在 `DESIGN.md` 中声明。

### 测试跳过

`wb` 的「staging 删不净」用例以 `assumeFalse` 在前提不成立时跳过。`TESTING.md` 明确禁止跳过，要求缺少环境时失败而不是跳过或忽略。该提交未合入。

## 二、边界差异

### 配置重读时机

`config.md`：配置变更在下次启动会话时生效。代码只在 `TerminalRuntime.start` 与 `applySettings` 中调用 `termuxProperties.reloadNow()`：`start` 是进程启动，不是每次建会话，配置改动后新建会话不一定读到新值；`applySettings` 由系统配置变化与主题设置触发，重读后会把光标样式等热更新到现有会话。规范要求的「下次建会话时读取」没有单独实现，热更新则是规范未声明的额外行为。

### 超大文件保留旧值

`config.md`：单文件超过 64 KiB 视为无效并跳过。`TermuxPropertiesRepository.readFile` 返回 `null` 后，属性读取回退到 `lastGood.properties`。首次加载时 `lastGood` 是默认值，符合「跳过」；若先读到合法文件、之后文件被改成超大文件，则保留上一次的值，而不是回到默认值。规范未声明这种保留语义。

## 三、已核对一致

- 三个允许键的校验：`default-working-directory` 经 `resolveWorkingDirectory` 检查目录存在且为目录，否则回退家目录；`terminal-transcript-rows` 范围 100–50000，非法回退 2000；`terminal-cursor-style` 接受 `block`/`bar`/`beam`/`underline`，非法回退 `block`。
- 文件路径：`~/.termux/termux.properties` 优先，`~/.config/termux/termux.properties` 次之，只取首个可读常规文件，不合并。
- 符号链接与非常规文件：`Files.isRegularFile(..., NOFOLLOW_LINKS)` 跳过。
- 体积上限：`MAX_BYTES = 64 * 1024`，超过即视为无效。
- 编码：`readText(Charsets.UTF_8)`。
- 不写配置：仓储只读，不创建、不写入、不修改 `termux.properties`。
- 默认工作目录、回滚行数、光标样式的默认值与 `config.md` 一致。

## 四、`wb` 合并审计

已合入的 5 个提交：

- `style(android): 让恢复过来的文件过 spotless`
- `docs(openspec): 补上「陈旧帧不得呈现」这条要求`
- `fix(native): 滚动在途门控自带超时兜底`
- `fix(native): 空闲路径也要走滚动门控的超时兜底`
- `docs: 规范引用改用小节名而非行号`

未合入的 7 个提交及原因：

| 提交主题 | 未合入原因 |
| --- | --- |
| `feat(installer): 校验钉住引导归档的 sha256 并补 bootstrap-integrity 规范` | `PROHIBITED.md` 禁止 Bootstrap zip 的 sha256 sidecar 校验 |
| `fix(installer): 补上不存在的 encodeToHex` | 只服务于上述校验 |
| `chore(openspec): 归档 bootstrap-archive-checksum 并提升 bootstrap-integrity 规范` | 只服务于上述校验 |
| `test(installer): 让「staging 删不净」用例先确认前提成立` | `TESTING.md` 禁止跳过 |
| `docs(openspec): 修好 bootstrap-integrity 规范缺少的章节头` | 规范属于被禁止的校验能力 |
| `fix(settings): colors.properties 读失败保留上次覆盖` | `colors.properties` 被禁止解析 |
| `docs(openspec): 同步 isRenderableKey 与软键盘切换取值写法` | 软键盘切换取值属于被禁止的配置键 |

`origin/wb2` 与 `origin/wb` 的未合并部分内容相同，没有额外正确修改。

## 五、修复顺序

1. 把 `TermuxProperties` 收缩到三个键，删除 `ColorsProperties` 与对应原生覆盖通道。
2. 删除闪烁、响铃、会话提示、夜间覆盖、全屏、返回/音量键改写、硬件快捷键、软键盘启用/禁用、边距/键栏高度、启动键盘等派生能力。
3. 把修饰键栏恢复为 Termux 默认布局，保留固定 2 行 7 列。
4. 对齐配置重读时机与超大文件语义。
5. 更新 `openspec/specs` 中受影响的规范，删除禁止能力的规范与测试。
6. 逐项运行针对性测试、`cargo test`、`:app:testDebugUnitTest`、`detekt`、`spotless`。
