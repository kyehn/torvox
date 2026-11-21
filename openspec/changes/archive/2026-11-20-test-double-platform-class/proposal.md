# Proposal

## Why

Android 侧回归用例若在测试进程里继承平台框架类（如 `android.view.KeyEvent`）来伪造
输入，会与 mockk 的 inline agent 相撞：agent 安装时对已加载类做重转换，撞上
Robolectric 插桩过的框架类后转换失败，JVM 打印
`java.lang.instrument ASSERTION FAILED ... transform method call failed`，并把
`java.lang.StackTraceElement$HashedModules` 标记为初始化失败；此后测试工作进程的
未捕获异常处理器再抛 `NoClassDefFoundError`，整个 Gradle 测试工作进程死亡。

后果不是「一条用例失败」，而是整个测试任务以 `java.io.EOFException` 失败、其余用例
的结果全部丢失——一次伪造方式的失当会淹没整个套件的结论。

本次复核中，「死键丢弃重音的日志等级」回归用例正是这样把测试任务弄崩的：工作进程
死亡后 Gradle 等待至 12 分钟才以 `EOFException` 收场，诊断信息只有上面两行 JVM 断言。

## What Changes

- 被测代码抽出字符级入口：死键合成与丢弃诊断从平台按键事件过滤中分离
  （`TerminalViewModel.resolveUnicodeChar`），字符级行为不必经 `KeyEvent` 即可验证。
- 新增 `code-health` 要求：平台输入的字符级行为必须可独立验证；回归用例
  MUST NOT 继承平台框架类来伪造输入。
- 补齐正常路径日志等级的回归用例：死键丢弃重音、会话关闭竞态下到达的输入、
  Activity 重建时旧窗口的销毁回调、快捷方式请求应急会话。

## 不做什么

- 不改 `mockk` / Robolectric 的依赖版本或测试框架配置：崩溃的根因是「测试进程里
  继承插桩过的框架类」这一替身形态，不是版本组合。
- 不以跳过该用例、降低断言强度或调低日志级别处置崩溃。
- 不把死键合成逻辑迁出 `TerminalViewModel`：待合成状态归 ViewModel 所有，字符级
  入口只是把平台无关的那一层单独暴露出来。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `code-health`：新增「平台输入的字符级行为必须可独立验证」。

## Impact

- Affected specs: `code-health`（新增一条要求）。
- Affected code: `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`
  （抽出字符级入口）、`android/app/src/test/java/terminal/emulator/DeadKeyLogSeverityTest.kt`
  与 `android/app/src/test/java/terminal/emulator/runtime/NormalPathLogSeverityTest.kt`
  （新增回归用例）。
- 行为变化：无。生产路径的判定顺序与日志内容与改动前逐条一致。
