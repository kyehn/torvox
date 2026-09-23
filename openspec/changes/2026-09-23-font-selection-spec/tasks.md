# 字体选择按规范重实现

## 上下文

- `DESIGN.md :94`–`:104` 字体选择节、`TESTING.md :31 :39` 覆盖项为标准。
- native 侧已合规，只改 Kotlin/Compose 侧。

## 任务

- [ ] 2.1 `systemFonts()` 改 `XmlPullParser` 解析 `/system/etc/fonts.xml`（纯函数 + 读文件两层；缺失/不可解析 → 日志 + `IllegalStateException`）；删目录扫描/`Typeface` 回退/硬编码表/`cleanFontName`；重写 `SystemFontsTest`。
- [ ] 2.2 删除文件安装流：对话框首行、`onPickFontFile` 参数链、`customFontLauncher`、`installFontFile`、`getFileNameFromUri`、`sanitizeFontFileName`、`filesDir/fonts` 创建与缓存迁移、`pick_font_file` 文案；保留 `loadFontFile` JNI/Bridge/Runtime 仅作探测；`FontSwitchInstrumentedTest` 改断言。
- [ ] 2.3 `font.ttf`/`.ttc`/`.otf` 存在即默认：`applyFontSettings` 入口优先 `loadFontFile` 探测应用，无则走用户设置。
- [ ] 2.4 用户字体目录改为 `files/home/.termux/font`（列表扫描 + `setExtraFontPaths` 指向；含 `.ttc`）。
- [ ] 2.5 `setFontFamily` native 返回 false → 日志 + `SettingsRepository.clearFontFamily` 新增并清除该设置（`applyFontSettings` 与 `FontManager.setFontFamily` 两处）。
- [ ] 2.6 验证（`cargo test`、`testDebugUnitTest`、androidTest 编译、release APK 设备验证）后 `openspec archive`。
