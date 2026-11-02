# 任务

## 一、缺陷核定

- [x] 核实自研解析器的存在理由不成立：`ISO-8859-1` 转码只发生在
      `Properties.load(InputStream)`，`load(Reader)` 按字符读，给 `StringReader`
      即无转码。
- [x] 对照 Java Properties 语法，列出与自研实现的差异：空白分隔、注释行尾反斜杠、
      值尾随空白。
- [x] 确认「注释行以 `\` 结尾吞掉下一行」是缺陷：紧随其后的有效配置项被静默丢弃。
- [x] 对照上游：termux-app 用 `java.util.Properties` 读同一份文件，语法口径本就
      是 Java Properties。

## 二、实现

- [x] `PropertiesFile`：改为 `java.util.Properties.load(StringReader)`，删掉自研的
      分隔符扫描、续行拼接与转义还原。
- [x] `RecordingProperties`：记录每次写入的键值，把出现顺序与重复键留痕保住
      （`Properties` 本身是映射，后写覆盖先写且不留痕迹）。
- [x] `MALFORMED_UNICODE_ESCAPE`：非法 `\u` 转义（后随不足四位十六进制）先降级
      成字面量，避免 JDK 拒绝整份文件——保住「写坏一个键只废掉那个键」。
- [x] 值去首尾空白：与既有行为一致，数值键不容忍尾随空白。

## 三、回归钉

- [x] `a whitespace separated pair is read as key and value`：空白分隔被识别。
- [x] `a comment ending with a backslash does not swallow the next line`：注释行
      尾随反斜杠不再吞掉下一行。
- [x] `a malformed unicode escape only affects its own value`：非法 `\u` 只降级
      该值，其余键照常生效。
- [x] `a non ascii value survives parsing`：非 ASCII 值不被破坏。
- [x] 既有解析用例全部仍通过（重复键留痕、越界回退、未知键忽略、extra-keys
      迷你语法均未受影响）。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.settings.TermuxProperties*"`
      → 88 tests, 0 failed（含新增的 5 条 Robolectric 用例，跑 Android 自己的
      `java.util.Properties`）。
- [x] `:app:testDebugUnitTest` 全量 → BUILD SUCCESSFUL。
- [x] `detekt` 与 `spotlessCheck` → 通过。
- [x] `markdownlint-cli2` 与 `openspec validate --all --strict` → 通过。
