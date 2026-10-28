# 设计

## 事实一：桥的读取口径本就只有一个

`TestUtils.getBridge()` 是活动会话桥的唯一读口（主线程同步读，孵化中为
`null`）。各测试类的 `private fun bridge()` 只是把它包成「取不到即抛」，并
非另一套语义。把它提为 `requireBridge()`：等待仍由 `awaitBridge()` 负责，
`requireBridge()` 只负责「已经过了就绪门槛却拿不到」这一种失败的表达。

## 事实二：全量文本必须先泵事件队列

运行时输出收割由 `NativeBridge.pollEvent()` 驱动，不泵一次就读到的可能是上一
轮的文本。各类的 `currentText()` / `pumpAndText()` 都是同一个动作，故合为
`pumpedTerminalText()`。

两种副本在「桥为 null」时的表现不同：`currentText()` 返回 `null`，
`pumpAndText()` 抛错。合并后取 `null` 语义——调用点本就按可空处理（轮询判
据全是 `?.contains(...) == true`），而可空语义对「Activity 重建导致桥瞬时为
空」更稳：抛错会把瞬时态判成用例失败。

## 事实三：`awaitBridge()` 已是既有的就绪门槛

仓库里 15 处测试用的是 `composeTestRule.awaitBridge()`；另 5 处仍写着同义的
「轮询 + `assertNotNull`」副本。统一到 `awaitBridge()`：轮询参数（30s /
200ms）与失败消息本就是同一套。

## 事实四：输入法延迟用例的「拿焦点」是同一件事

`BackspaceLatencyTest` 与 `CjkComposingLatencyTest` 里的 `onUiThread` 与
`tapToFocus` 逐字相同，连注释都指向同一条路径（切换余波的有界抑制窗口）。
两者都搬到 `TestUtils`：注释保留「失焦会被拒收」这条判据来源。

## 取舍：为什么不建测试基类

剩余的 jscpd 克隆集中在**类声明骨架**（规则声明、`device` 字段、`setUp`），
而各类的就绪门槛确实不同：`waitForSession()`（含 prefix 安装）、
`waitForTerminalScreen()`、`waitForTerminalPixels()`（等原生 Surface 挂载）
各自对应不同的失败语义。抽成基类要把门槛变成可覆写方法，读者便看不出某个
用例到底等了哪一档——正是这些注释想守住的信息。故骨架保留逐份书写，只把
**逻辑**（读桥、泵文本、拿焦点、断言块）收进共享助手。

## 取舍：为什么共享助手放在 `TestUtils.kt`

`TestUtils.kt` 已经是 `AndroidComposeTestRule` 扩展函数的集中处
（`waitForSession`、`getBridge`、`awaitBridge`、`cleanUpTerminalState`…）。
新助手沿用同一形态与命名，不另立文件。

## 验证

- `jscpd --min-lines 20 --min-tokens 100`：`native/src` 与 `android/app/src`
  全仓 0 克隆；降到 `--min-lines 12 --min-tokens 60` 时剩余克隆全部是 import
  块与类声明骨架（各类的就绪门槛本就不同，见上）。
- `./gradlew :app:detekt :app:spotlessCheck :app:compileDebugAndroidTestKotlin`
  通过（`allWarningsAsErrors` 开启，删掉的私有副本不得留下未用导入）。
- 仪器测试在模拟器上跑通（受影响类：鼠标上报、粘贴按钮、会话抽屉、输入法像素、
  备用屏输入法重排、选择拖拽、退格延迟、组字延迟）。
