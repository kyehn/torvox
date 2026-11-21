# Design

## 一、崩溃机制

现场（Gradle 9.9.2 + JDK 21 + Robolectric 4.18.1 + mockk 1.x）：

```text
DeadKeyLogSeverityTest > dead key without a precomposed form is a debug line STANDARD_ERROR
WARNING: A Java agent has been loaded dynamically (byte-buddy-agent-1.18.2.jar)
*** java.lang.instrument ASSERTION FAILED ***: "!errorOutstanding" with message transform method call failed at src/java.instrument/share/native/libinstrument/JPLISAgent.c line: 884
Gradle Test Executor 1 finished executing tests.
java.lang.NoClassDefFoundError: Could not initialize class java.lang.StackTraceElement$HashedModules
Exception: java.lang.NoClassDefFoundError thrown from the UncaughtExceptionHandler in thread "Test worker"
```

时序：测试类加载时，`FakeKeyEvent : android.view.KeyEvent` 让平台类经 Robolectric 的
插桩类加载器进入 JVM；用例内首次 `mockk` 触发 mockk 的 inline agent 安装，agent 对
已加载类做重转换，撞上插桩过的框架类后转换失败，JVM 记下未结转换错误并把
`StackTraceElement$HashedModules` 标记为初始化失败；此后任何取栈轨迹的动作都抛
`NoClassDefFoundError`，工作进程的未捕获异常处理器再抛一次，进程死亡。

判定依据：去掉「继承 `android.view.KeyEvent`」这一步（改走字符级入口）后，同一测试
文件与同一夹具立即通过；仓库内既有的 Robolectric + mockk 用例（它们 mock 的是
final 类，但不继承框架类）一直正常。故触发条件是「继承插桩过的框架类」与
「inline agent 安装」两者并存，而非 mockk 本身或 Robolectric 本身。

## 二、被否决的替代方案

| 方案 | 否决理由 |
| --- | --- |
| 用 mockk 造 `KeyEvent` 替身 | mockk 对非 final 类同样走 inline agent，agent 仍要重转换 `android.view.KeyEvent`；且替身类由哪个类加载器定义受 Robolectric 沙箱影响，跨加载器继承会再失败。 |
| 整体改用真实实例、彻底不用 mockk | `SettingsRepository`、`TermuxPropertiesRepository`、`TerminalRuntime` 都是 final 类，替身只能靠 mockk；且真实 `SettingsRepository` 会拉起 DataStore 协程，把用例的确定性交给 I/O。 |
| 放弃这条回归用例 | 丢掉的是「正常路径不得记为 warn」的唯一钉子：日志等级改回 `warn` 时不会有任何用例判红。 |
| 调整 mockk/Robolectric 版本 | 崩溃发生在 agent 与插桩类的交互上，不是版本缺陷；换版本不能给出一条可复核的判据。 |

## 三、落地方案

生产代码把「字符级」与「平台事件级」分开：

- 平台事件级留在 `TerminalViewModel.handleLayoutAwareHardwareKey`：动作、软键盘、
  设备号、来源、修饰键与 AltGr 的过滤，以及最终把字符交给 bridge。
- 字符级移入 `TerminalViewModel.resolveUnicodeChar(unicodeChar, hasAltGr)`：死键的
  判定与暂存、待合成组合符的取出与合成、无预组合形式时丢弃重音的诊断日志，
  返回「要发给终端的字符」，死键本身返回 `null`（表示已消费，等下一次击键合成）。

返回值语义与原实现逐条对应：原路径先判 `unicodeChar == 0` 返回 `false`，再判死键
返回 `true`，其余走合成与派发；新路径把后三步搬进字符级入口，调用点保持
「`null` → 已消费返回 `true`；`<= 0` → 无内容返回 `false`」。

## 四、回归用例的判别力

两条用例都断言**等级**而非「有没有日志」：同一条消息改成 `warn` 仍然有日志，只有钉住
等级才能挡住「正常路径被升级成告警」的回归。已实测：把死键丢弃重音的日志改回
`warn`，`DeadKeyLogSeverityTest` 立即判红；改回 `debug` 后通过。
