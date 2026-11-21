# 任务

## 一、定位

- [x] 复现测试工作进程死亡：Gradle 报 `java.io.EOFException`，JUnit 结果文件缺失
- [x] 定位崩溃点：mockk 的 inline agent 安装后，JVM 报 `transform method call failed`
      与 `NoClassDefFoundError: java.lang.StackTraceElement$HashedModules`
- [x] 判定触发条件：继承 Robolectric 插桩过的 `android.view.KeyEvent` 与 inline agent
      安装并存；去掉继承这一步后同一夹具立即通过

## 二、实现

- [x] `TerminalViewModel` 抽出字符级入口 `resolveUnicodeChar(unicodeChar, hasAltGr)`，
      死键合成与丢弃诊断搬入其中，平台事件过滤留在原处
- [x] 回归用例改走字符级入口，不再继承平台框架类
- [x] 新增 `code-health` 要求：平台输入的字符级行为必须可独立验证

## 三、验证

- [x] 判别力实测：死键丢弃重音的日志改回 `warn` → 用例判红；改回 `debug` → 通过
- [x] `:app:testDebugUnitTest`：通过
- [x] `:app:detekt :app:spotlessCheck :app:lintDebug`：通过
- [x] `openspec validate --all --strict`：通过
- [x] `markdownlint-cli2`：通过
- [x] `semgrep scan --error --config .semgrep/rules.yml`：0 findings
