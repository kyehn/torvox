# termux.properties 与 colors.properties 落地评估

本文记录本轮把 termux 生态的配置接进应用之后的实际行为、逐项的复杂度/稳定性/性能
评估，以及复查中改掉的回归。行为口径以 termux-app v0.119.0-beta.3 的源码为准；与本仓
`docs/specification/` 冲突之处单列在「规范冲突」一节——那些文件受保护，未获明确同意
不改。

`docs/compatibility-gap-audit.md` 是实现前的快照审计，文中「未实现」的判断已被本轮
改动取代。

## 逐项评估

「触点」指实现跨越的层数（配置解析 → 界面/原生 → 测试）。复杂度按触点与分支数估计，
稳定性看状态不一致与「静默不生效」的风险，性能看是否落在渲染或输入热路径上。

| 键 | 落地行为 | 触点 | 复杂度 | 稳定性 | 性能 |
| --- | --- | --- | --- | --- | --- |
| `extra-keys` | 键栏布局完全由配置决定；宏只解析不渲染 | 配置 → 键栏 → 测试 | 中 | 中：未知键名会静默消失，日志留痕 | 无（仅重组时计算） |
| `extra-keys-style` | 五档字形集合逐项对齐上游 `ExtraKeysConstants` | 配置 → 键名字形 | 低 | 低 | 无 |
| `extra-keys-text-all-caps` | 只影响字母标签，符号字形不变 | 配置 → 键名字形 | 低 | 低 | 无 |
| `terminal-toolbar-height` | 单键高度缩放，网格预留同源计算 | 配置 → 布局 → 网格 | 中 | 低 | 无（布局期计算） |
| `terminal-margin-horizontal` / `-vertical` | 内缩 SurfaceView，网格随尺寸收窄 | 配置 → 布局 → 网格 | 中 | 低 | 无 |
| `terminal-cursor-style` | 作为**默认**样式下发，程序 DECSCUSR 仍优先 | 配置 → JNI → VT | 中 | 低 | 无 |
| `terminal-cursor-blink-rate` | 原生按时基算相位，宿主按间隔补帧 | 配置 → JNI → 渲染循环 | 高 | 中：补帧节奏与空闲门控耦合 | 高：闪烁期间每半周期一次全量重绘 |
| `default-working-directory` | 解析后必须落在真实存在的目录上，否则回退家目录 | 配置 → 会话启动 | 低 | 低 | 可忽略（一次 stat） |
| `terminal-transcript-rows` | 回滚深度，越界回退默认 2000 | 配置 → 会话启动 | 低 | 低 | 可忽略（内存按行计） |
| `bell-character` | `on_bell` → 事件 → 震动/提示音/忽略 | 配置 → 原生回调 → 事件 → 系统 | 中 | 中：事件合并与最小间隔 | 低（合并后每会话最多一条） |
| `fullscreen` | 隐藏状态栏 | 配置 → 窗口 | 低 | 低 | 无 |
| `use-fullscreen-workaround` | 决定唤出后是临时栏还是默认行为 | 配置 → 窗口 | 低 | 低 | 无 |
| `night-mode` | 三态压在应用日夜判定之上，视图与原生同源 | 配置 → 主题解析 → 原生 | 中 | 低 | 无 |
| `soft-keyboard-toggle-behaviour` | `enable/disable` 翻转「软键盘可用与否」 | 配置 → 组合状态 → 输入法 | 中 | 中：禁用期必须让平台自动弹那条路也失效 | 无 |
| `hide-soft-keyboard-on-startup` | 默认不弹；写 `false` 在首帧后请求一次 | 配置 → 视图 → 输入法 | 低 | 低 | 无 |
| `disable-terminal-session-change-toast` | 切换会话时顶部提示 `[序号] 标题` | 配置 → ViewModel → 组合 | 中 | 低 | 无 |
| `back-key` | `escape` 时返回键发 ESC | 配置 → Activity | 低 | 低 | 无 |
| `volume-keys` | `virtual` 时音量减当一次性 Ctrl | 配置 → Activity → 修饰键状态 | 中 | 中：与用户手动切换修饰键的时序 | 无 |
| `ctrl-space-workaround` | 显式补发 NUL | 配置 → Activity | 低 | 低 | 无 |
| `shortcut.create/next/previous-session` | `Ctrl+Alt+<char>` 切换会话 | 配置 → Activity → ViewModel | 中 | 低 | 无 |
| `disable-hardware-keyboard-shortcuts` | 关掉上一条 | 配置 → Activity | 低 | 低 | 无 |
| `colors.properties` | 19 槽覆盖，未写的槽沿用基底主题 | 配置 → JNI → 主题 | 中 | 低：覆盖压在基底上，不累积 | 无（仅换主题/换配置时） |

系统侧同时落地：Wi-Fi 锁与唤醒锁同生命周期、Android 键码表补齐、工作目录（OSC 7）
跟踪、颜色方案与明暗变化上报、选区手势（长按选中 + 拖柄 + 菜单）。

## 本轮改掉的回归

以下五项是复查 `wb` 引入的实现时发现的**行为回退**，已按上游与本仓 `config.md` 修正。

| 项 | 现象 | 依据 | 改法 |
| --- | --- | --- | --- |
| 工作目录不校验存在性 | 写错的路径直接交给 PTY，会话起不来 | 上游 `TermuxSharedProperties.getDefaultWorkingDirectoryInternalPropertyValueFromValue` 检查存在/是目录/可读后回退默认；本仓 `config.md` 同义 | 解析后校验目录，不存在回退家目录 |
| 数值越界被钳到边界 | `terminal-transcript-rows = 99999` 得到 50000 行回滚 | 上游 `SharedProperties.getDefaultIfNotInRange` 一律回退默认；本仓 `config.md` 写明「非法回退 2000」 | 越界回退默认并记错误 |
| 闪烁间隔接受 1..99 | 50ms 间隔被当成有效值 | 上游 `TerminalView` 的有效区间是 100..2000，越界按「不闪」处理 | 只接受 0 或 100..2000 |
| 配置文件读取无上限、跟随符号链接 | 任意大的文件会被整个读进内存；链接可把读取引到别处 | 本仓 `config.md`：只认常规文件、超过 64 KiB 视为无效 | 读取前判大小，拒绝符号链接与非常规文件 |
| Wi-Fi 锁用低延迟模式 | 官方文档写明该模式「只在屏幕点亮且应用位于前台时生效」，灭屏即失效——正是这把锁要解决的场景 | termux 用 `WIFI_MODE_FULL_HIGH_PERF`；官方文档写明高性能模式在灭屏、后台仍生效 | 改用高性能模式 |

## 测试覆盖

- 配置解析：`android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`
  覆盖逐键取值、越界回退、非法值只影响自身、重复键、注释与转义。
- 文件读取规则：`TermuxPropertiesRepositoryTest.kt` 覆盖主路径优先、符号链接跳过、
  超大文件跳过、文件缺失不算错误。
- 键栏转换：`ModifierBarLayoutTest.kt` 覆盖布局、字形五档、大小写、宏与未知键。
- 工作目录：`ResolveWorkingDirectoryTest.kt` 覆盖 `~`、相对、绝对、缺失目录回退。
- Wi-Fi 锁：`TerminalForegroundServiceTest.kt` 覆盖取锁与释放、无会话不持锁。
- 原生侧：光标闪烁相位与空闲门控（`idle_repaint_gate_tests.rs`）、词选与全选
  （`ghostty_terminal/tests/lifecycle.rs`）、调色板覆盖（`terminal/theme/tests.rs`）、
  键码补齐（`android/keymap.rs`）。

## 规范冲突

以下条目与本轮实现冲突，但相关文件受保护（`docs/specification/` 需明确同意才改实际
内容），故只记录，不擅自改动。

- `PROHIBITED.md` 末条把「解析 `termux.properties` 中除 `default-working-directory`、
  `terminal-transcript-rows`、`terminal-cursor-style` 之外的键，以及 `colors.properties`」
  列为禁止项。本轮按用户明确指令实现了这些键。
- `DESIGN.md` 「忽略 bell character，不做出响应」与 `bell-character` 的实现冲突。
- `DESIGN.md` 「输入光标为方块样式……不闪烁」与 `terminal-cursor-blink-rate` 冲突；
  本轮取默认值 0（不闪），默认外观仍与该条一致。
- `config.md` 的键清单仍只有三个键，需与实际解析范围同步。
