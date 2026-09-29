# 字体选择按规范重实现

## 背景

`DESIGN.md` 字体选择节（`:94`–`:104`）与现状多处相悖：`SystemFonts.kt` 用硬编码目录扫描 + `Typeface` 回退 + 手动改写去重，未解析 `fonts.xml`（违 `:95` `:103` `:104`）；字体对话框首行“从文件选择…”（违 `:104`）；SAF 安装流复制文件到 `filesDir/fonts`（违 `:98` 从不复制/移动文件）；`font.ttf` 默认（`:99`）与 `.termux/fonts` 目录（`:101`）无任何实现；设置错误仅 toast（违 `:97` 重置应用数据）。

native 侧已合规（`ASystemFontIterator` 平台枚举、`font_db::parse_fonts_xml_families`、`list_monospace_fonts` 由 fontdb 元数据枚举），本次只改 Kotlin/Compose 侧，native 仅复用既有 `loadFontFile`（探测）与 `setExtraFontPaths`。

## 改动

- `systemFonts()` 改为经 native 取列表：`availableFontFamilies(list_monospace_fonts())`，
  即直接复用渲染侧字体库（其内容即 `fonts.xml` 声明的文件集加用户投放目录），
  列表按文档顺序、按族名精确去重，不归一不排序。**原计划的 Kotlin 侧
  `XmlPullParser` 解析 `fonts.xml` 未采用**：渲染侧已解析过一次，再解析一份会引入
  第二个事实来源，且平台 `SystemFonts` 无公开族名访问器。列表为空 → 日志 +
  抛 `IllegalStateException` 崩溃（`:96`）；`fonts.xml` 缺失/不可解析由 native abort。
- 删除文件安装流：对话框首行、`customFontLauncher`、`installFontFile`、`getFileNameFromUri`、`sanitizeFontFileName`、`filesDir/fonts` 创建与缓存迁移、`pick_font_file` 文案；保留 `loadFontFile` JNI 仅作 `font.ttf` family 探测。
- `font.ttf`（兼 `.ttc` `.otf`）存在即默认：`Bridge.setFontFamily` 内优先 `loadFontFile` 探测应用并返回应用结果，无则走传入值（覆盖启动、设置变更全部入口；`TerminalRuntime` 零改动）。
- 用户字体目录改为 `files/home/.termux/fonts`，`setExtraFontPaths` 指向它；列表经 `listFontFamilies` 天然包含，无手动判断。
- `setFontFamily` native 返回 false → 日志 + 清除该设置（`SettingsRepository.clearFontFamily` 新增）。

## 影响

- 测试：`SystemFontsTest` 重写；`FontSwitchInstrumentedTest` 的 pick-file 用例改断言无该行；`cargo test` `testDebugUnitTest` 通过；release APK 设备验证（`TESTING.md :31` font.ttf 复制 + 列表选择）。
- 不碰：native 字体管线、单主字体模型、实际字体信息框（已合 `:106`）。
