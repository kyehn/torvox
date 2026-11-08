# Kotlin 测试的价值、稳定性与断言强度审查

本文记录一次针对 Kotlin 测试代码的审查：逐个测试文件核对断言强度、等待方式与
跨语言契约的钉子，把每一处命中逐条判定，并修掉查得出的真实缺口。审查不修改
`docs/STATUS.md`，不改动受保护文件。

## 审查范围与证据

- 范围：`android/app/src/test` 与 `android/app/src/androidTest` 下的全部 Kotlin
  测试文件（195 个文件、476 个用例），以及 Rust 侧事件序列化契约
  （`native/src/event.rs`）。
- 断言扫描：按 `assert[A-Z]\w*`、`fail(`、`error(`、`verify(`、`coVerify(`、
  `confirmVerified`、`check(`、`require(` 统计断言点，共 3328 处；没有「有用例
  却零断言」的文件，也没有「断言点数少于用例数」的文件。
- 逐函数扫描：只有 3 个函数体内没有直接断言点——Cucumber 的 `setUp`/`tearDown`
  两个钩子与 `SelectionTapDismissTest` 的取值助手 `selectionStateForTest`，三者
  都不承载判定。
- 跳过扫描：`@Ignore`、`@Disabled`、`TODO()`、`assumeTrue`/`assumeFalse` 共 0 处。
- 恒真断言扫描：`assertEquals(x, x)`、`assertTrue(x == x)` 共 0 处。
- 等待方式扫描：`Thread.sleep` 共 121 处，按「手势节拍」「线程同步」「等异步结果」
  三类归类。
- 跨语言契约核对：Rust `Event` 的五个变体与 Kotlin `PollEvent` 的五个变体，逐个
  核对事件标签与字段名，并检查两侧是否都有快照钉。

## 发现

### Rust 侧事件快照只钉住一半变体

`native/src/event.rs` 的序列化快照原先只覆盖 `bell`、`desktop_notification` 与
`exit`，`clipboard` 与 `clipboard_read` 没有 Rust 侧快照。Kotlin 侧
`ParseEventTest` 与 `PollEventTest` 用硬编码 JSON 覆盖了全部五个变体，两侧靠注释
互指而没有机器校验：标签或字段名一旦漂移，kotlinx-serialization 会抛
UnknownField/MissingField，事件整批丢弃——用户刚复制的内容消失、OSC 52 读请求
无人应答，远端把空剪贴板粘出来。

处置：补 `clipboard_tests`，把两个变体的序列化结果钉成逐字 JSON。判别力验证：
给变体加 `#[serde(rename = "clipboardRead")]` → 快照判红；给字段加
`#[serde(rename = "requestId")]` → 快照判红；还原后转绿。

### 「不崩溃」形态的用例名掩盖了真实断言

`…_does_not_crash` 命名共 5 处。其中 4 处其实已有具体断言（文档 id、root id、
设置页返回按钮、会话数），只有名字在说「不崩溃」；名实不符会让人以为这些用例
只验存活，评审时按弱断言处理。

处置：按实际断言改名——`queryDocument_subset_projection_returns_document_id`、
`subset_projection_returns_root_id`、`settings_open_shows_settings_screen`、
`add_two_sessions_consecutively`。

### 修饰键栏连按用例只断言界面还在

`ModifierBarTest` 的两条连按用例（ESC、方向键）唯一的断言是 `TerminalScreen`
仍显示。按键写进 PTY 后若把 shell 的行编辑器逼进异常态，界面照样正常显示——这两
条用例证明不了输入链路活着。

处置：连按后送一条探针命令并等它落格（`assertSessionStillResponds`）：先等 shell
提示符，再用 ETX 清掉 ESC 留下的 meta 前缀，最后送 `printf` 探针并轮询终端文本。
断言从「界面还在」升级为「会话仍可交互」。

### 文本搜索端到端用例用固定睡眠等异步搜索

搜索在输入线程上异步执行（150ms 防抖加原生搜索），而 `TextSearchEndToEndTest`
用 `waitForOutput()`（1500ms）与 `waitForSearchStable()`（500ms）等它。固定睡眠
两头都不靠：慢模拟器上计数还没算出来就断言，快机器上白等；而断言本身又只到
「计数节点存在」——查没查到都成立。

处置：两处裸睡换成落格门控（`awaitTerminalContains`）与计数门控
（`awaitSearchCount`/`awaitSelectedCount`/`awaitNoResultsCount`）。计数文案从资源
串推导：`countPattern` 用固定数字填一遍资源串，再把数字段换成数字匹配，逐段引用
而非整串引用——`Regex.escape` 会把整串包进 `\Q…\E`，数字段随之变成字面量，正则
就再也匹配不到真实计数（该坑由设备上的失败日志定位：`当前文本：共 92 个`
始终不匹配）。

顺带把断言变实：命中类用例从「计数节点存在」升级为「计数落定且给出命中个数」；
多行用例断言 30 行内容全部命中（计数不少于 30）；无结果用例先搜到真实命中、再
换成查不到的串，等计数从「共 N 个」翻回「无结果」——「无结果」文案在搜索跑完前
就已经显示，只断言它存在证明不了搜索跑过。

## 明确不作处置的项

- **`SettingsRepositoryTest` 的两处 `Thread.sleep(700)`**：仓库收集器跑在真实 IO
  调度器上，虚拟时间不适用；用例注释已写明原因与 2 倍余量。
- **`BehaviorInstrumentedTest` 与 `StickyCtrlInterruptInstrumentedTest` 的等待**：
  前者等的是长按弹窗这类无障碍层级里查不到、只能靠像素判决的中间态；后者的最终
  判据是 `rc=130` 这一真实退出码。两者都有具体断言兜底。
- **合成手势里的 `Thread.sleep`**：`down` 与 `up` 之间的间隔、拖动步进的节拍，是
  被测输入本身的时间形状，没有可查询的替代判据。
- **竞态用例里的 `Thread.sleep`**：看门狗、输入批缓冲、渲染线程拆卸三处用例靠睡眠
  让线程推进，属竞态测试的固有做法，已在并发审查留档中逐条判定。
- **崩溃式仪器用例**：`ModifierBarTest` 的取消语义用例、`RenderPauseTest`、
  `SessionCreationInstrumentedTest` 均已带具体断言（选中态、节点存在、会话数）。

## 扫描命中逐条判定

### 断言强度

195 个测试文件、476 个用例、3328 处断言点，没有「有用例却零断言」的文件，也
没有断言点数少于用例数的文件。用例里的断言以 `coVerify(exactly = …)`、
`assertIsDisplayed()`、`assertExists()` 等 Compose 断言形式出现，只匹配
`assertEquals|assertTrue|…` 的扫描会把它们误判为无断言。

### 跳过与恒真

`@Ignore`/`@Disabled`/`TODO()`/`assume*` 0 处；`assertEquals(x, x)` 与
`assertTrue(x == x)` 0 处。

### 等待方式

`Thread.sleep` 共 121 处，分属手势节拍、线程同步、等异步结果三类。等异步结果这
一类里最典型的两处（文本搜索的落格与计数等待）已换成门控；其余各处都带有具体
断言兜底，或等的是没有查询接口的中间态。

## 复现方法

- 断言扫描：按 `assert[A-Z]\w*|fail\(|error\(|verify\(|coVerify\(|confirmVerified|check\(|require\(`
  统计 `android/app/src/test` 与 `android/app/src/androidTest` 下的断言点。
- 跨语言契约：Rust 侧 `native/src/event.rs` 的 `clipboard_tests`、
  `notification_tests`、`exit_tests`、`bell_tests`；Kotlin 侧
  `android/app/src/test/java/terminal/emulator/bridge/PollEventTest.kt` 与
  `ParseEventTest.kt`。
- 判别力验证：给 `Event::ClipboardRead` 变体加 `#[serde(rename = "clipboardRead")]`
  或给 `request_id` 字段加 `#[serde(rename = "requestId")]`，对应快照即判红。
- 行为验证：`android/` 下运行 `:app:testDebugUnitTest` 与
  `:app:connectedDebugAndroidTest`（`ModifierBarTest`、
  `TextSearchEndToEndTest`、`RenderPauseTest`、`SessionCreationInstrumentedTest`、
  `DocumentsProviderInstrumentedTest`）。
