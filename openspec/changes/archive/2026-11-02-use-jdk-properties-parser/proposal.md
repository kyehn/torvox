# Proposal

## Why

`termux.properties` 的语法层由本仓自研的 `PropertiesFile` 解析（约 95 行），把
Java Properties 语法又实现了一遍：分隔符扫描、续行拼接、转义还原各有一套。

这份自研实现当初的理由写在代码注释里：「不用 `java.util.Properties`……它会对
ISO-8859-1 做转码，非 ASCII 键值（中文路径）会被静默破坏」。**这条理由不成立**：
ISO-8859-1 转码只发生在 `Properties.load(InputStream)`；`load(Reader)` 按字符读，
给一个 `StringReader` 就不会有任何转码。于是本仓为一条不存在的约束自研了一整份
语法实现，而它与 JDK 参考实现之间的差异无人跟踪。

自研实现与 Java Properties 实际有几处行为差：

| 输入 | 自研实现 | Java Properties |
| --- | --- | --- |
| `key value`（空白分隔） | 整行当成键，值丢失 | 键 `key`、值 `value` |
| 注释行以 `\` 结尾 | 吞掉**下一行**（当续行处理） | 注释整行丢弃，下一行照常解析 |
| 尾随空白 | 值被裁掉 | 值保留尾随空白 |

其中「注释行以 `\` 结尾吞掉下一行」是真缺陷：用户在注释里写了一个路径末尾带反斜杠
的说明，紧随其后的有效配置项会被静默吃掉。

## What Changes

- `PropertiesFile` 改为委托 JDK 的 `java.util.Properties`（`load(Reader)` +
  `StringReader`）：续行、转义、分隔符规则随 JDK 维护，本仓不再自研语法。
- 保留「重复键留痕」这一既有行为：`Properties` 是映射，写入时后写覆盖先写且不留
  痕迹，故用一层记录每次写入的子类把出现顺序与重复一并记下来。
- 保留「写坏一个键只废掉那个键」的口径：JDK 对非法 `\u` 转义抛异常并拒绝整份
  文件，故先把这类转义降级为字面量再交给 JDK 解析。
- 值去首尾空白：Java Properties 保留尾随空白，而 `terminal-transcript-rows` 这类
  数值键不容忍尾随空白，裁掉后与既有行为一致。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `termux-configuration`：「properties 语法」补齐分隔符集合与「非法转义只降级该值」
  的容错口径。

## 不做什么

- 不引入第三方 properties 库：Java Properties 格式的参考实现就在 JDK 里
  （`java.util.Properties`），再套一层库只是多一份依赖与一份转译。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`。
- 不改「未知键忽略」「数值越界回退默认」「重复键后者生效且留痕」三条既有口径。
- 不改 `extra-keys` 的迷你语法：那份语法允许单引号与裸键名，标准 JSON 解析器
  反而不收，与本次改动无关。

## Impact

- Affected specs: `termux-configuration`（「properties 语法」一条 MODIFIED Requirement）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt`
    （`PropertiesFile` 及其两个辅助声明）
  - `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`
    （新增语法层回归用例）
