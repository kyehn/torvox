# Design

## 问题定位

Failsafe 请求的链路只有「置标记 → 建会话时读标记」两段：

- `TerminalRuntime.requestFailsafeSession()` 置 `failsafeRequested`；
- `TerminalRuntime.buildConfig()` 读它（只读不消费），据此给出
  `Shell.SystemDefault` + 空 prefix；
- 建会话成功（`start()` / `createSession()`）后 `consumeFailsafeRequest()` 清标记。

断点在「谁在应用已启动时去建那个会话」：`start()` 因「已有会话」早退，`createSession()`
只在用户主动新建会话时才被调用。于是标记悬空，既不建会话也不消失。

## 方案选择

**处置决策放在 `TerminalViewModel`，不在 `MainActivity`、也不在 `TerminalRuntime`。**

- `MainActivity` 只知道 Intent，不掌握会话与 Surface 的状态；把「是否已有会话」的
  判断写在这里会让 Activity 依赖运行期内部结构。
- `TerminalRuntime` 掌握会话，但建会话需要 Surface，而 Surface 由视图侧持有
  （`TerminalViewModel.currentSurface`）。运行期拿不到 Surface 就无法建会话。
- `TerminalViewModel` 同时持有运行期（注入）与 Surface，且已有「新建会话」入口
  （`createSession()`），复用它即不新增第二条建会话路径。

**决策写成纯函数 `decideFailsafeIntent(hasSession, freshIntent)`**：三种情形各自的后果
互不相同（建会话 / 只置标记 / 完全忽略），用枚举显式表达比布尔组合更易读，也便于
表驱动测试覆盖边界。

## 「新投递」与「重复投递」

`MainActivity` 的 `launchMode` 是默认的 `standard`：快捷方式在应用已启动时被点，会
**新建一个 Activity 实例**（其 `onCreate` 走同一条 Intent 处理路径）；而配置变更
（密度、语言）也会重建 Activity，`onCreate` 再次带上**同一个**启动 Intent。

两者从 Intent 上无法区分，但后果相反：前者必须建会话，后者绝不能建。故以
`savedInstanceState == null` 判定「本次 Intent 是新投递」：

- `onCreate`：`freshIntent = savedInstanceState == null`（重建时 `savedInstanceState`
  非空，系统在重建前必定调用过 `onSaveInstanceState`）；
- `onNewIntent`：恒为新投递。

进程被杀后系统重建 Activity 属「`savedInstanceState` 非空」的一类，但此时运行期
已无会话，决策落在「只置标记」分支——首个会话仍是应急会话。

## Surface 未交付时先挂起

新投递的 Intent 到达时，Surface 未必可用：

- 快捷方式新建的 Activity 实例，其 Surface 要等布局之后才交付；
- 应用从后台被拉起时，Surface 异步重建。

`createSession()` 无 Surface 建不出会话（直接返回并记错误）。故请求先挂起
（`pendingFailsafeSession`），待 Surface 交付后补建。

挂起点选在 `TerminalViewModel.currentSurface` 的 setter：赋值即「Surface 已交付」的
唯一时刻，且覆盖全部交付路径（`surfaceCreated`、`surfaceChanged` 引起的重配、
`startRuntime`）。销毁路径把 Surface 置空时跳过补建——那不是「有新的可用窗口」。

`startRuntime` 中的赋值顺序相应调整为「尺寸先于 Surface」：补建会话需要尺寸就绪。

## 标记的生命周期不变

`failsafeRequested` 仍是「只读不消费」：读取发生在 `buildConfig()`，而 `buildConfig()`
也被设置变化路径 `applySettings()` 调用——在那里消费会让一次主题或配置变化把应急请求
静默吞掉。消费仍只发生在**建出会话成功**之后；建会话失败时标记保留，下一次尝试
仍应是应急会话。

## 验证方式

- 纯决策：表驱动单测覆盖四种（已有会话 × 新投递）组合。
- 接线：ViewModel 层单测以 mock 驱动运行期——应用已启动时请求会置标记，且 Surface
  交付后确实建出会话；重建时既不置标记也不建会话。
