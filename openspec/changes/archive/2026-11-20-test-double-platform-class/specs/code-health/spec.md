# code-health Specification

## ADDED Requirements

### Requirement: 平台输入的字符级行为必须可独立验证

被测代码处理平台输入事件（按键、触摸）时，字符级或参数级的决策 MUST 有不依赖平台
框架类的入口：平台事件级的过滤与字符级的处理 MUST 分开，回归用例 MUST 走字符级
入口验证字符级行为。

回归用例 MUST NOT 在测试进程里继承平台框架类来伪造输入。宿主 JVM 单测里，平台框架
类经 Robolectric 插桩后加载；继承它会与 mockk 的 inline agent 相撞，使 JVM 的字节码
转换失败并把测试工作进程带崩（`transform method call failed` 与
`NoClassDefFoundError: java.lang.StackTraceElement$HashedModules`），结果是整个测试
任务失败而非单条用例失败。

#### Scenario: 需要伪造返回特定字符的按键

- **WHEN** 回归用例需要一个返回特定 Unicode 字符（如死键字符）的按键输入
- **THEN** MUST 调用被测代码的字符级入口，MUST NOT 继承 `android.view.KeyEvent`
      等平台框架类来伪造该输入

#### Scenario: 平台框架类替身使测试工作进程崩溃

- **WHEN** 测试进程内出现继承平台框架类的替身，导致测试工作进程死亡
- **THEN** MUST 判为替身形态缺陷并改用字符级入口，MUST NOT 以跳过该用例、
      降低断言强度或调低日志级别处置

#### Scenario: 字符级入口不得改变平台路径的判定顺序

- **WHEN** 把字符级处理从平台事件处理中抽出
- **THEN** 平台路径的判定顺序、无内容时的返回值与死键被消费时的返回值 MUST 与
      抽出前逐条一致
