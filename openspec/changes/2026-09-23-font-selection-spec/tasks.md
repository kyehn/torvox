# 字体选择按规范重实现

## 上下文

- `DESIGN.md :94`–`:104` 字体选择节、`TESTING.md :31 :39` 覆盖项为标准。
- native 侧已合规，只改 Kotlin/Compose 侧。

## 任务

- [x] 2.1 `systemFonts()` 改 DOM 解析 `/system/etc/fonts.xml`（纯函数 + 读文件两层；缺失/不可解析 → 日志 + `IllegalStateException`）；删目录扫描/`Typeface` 回退/硬编码表/`cleanFontName`；重写 `SystemFontsTest`。
- [x] 2.2 删除文件安装流：对话框首行、`onPickFontFile` 参数链、`customFontLauncher`、`installFontFile`、`getFileNameFromUri`、`sanitizeFontFileName`、`filesDir/fonts` 创建与缓存迁移、`pick_font_file` 文案；保留 `loadFontFile` JNI/Bridge/Runtime 仅作探测；`FontSwitchInstrumentedTest` 改断言。
- [x] 2.3 `font.ttf`/`.ttc`/`.otf` 存在即默认：`Bridge.setFontFamily` 内优先 `loadFontFile` 探测应用并返回应用结果，无则走传入值。
- [x] 2.4 用户字体目录改为 `files/home/.termux/font`（`setExtraFontPaths` 指向 + `lastExtraFontPaths` 去重；列表经 `listFontFamilies` 天然包含；含 `.ttc`）。
- [x] 2.5 存入 family 为 native 未知 → 日志 + `SettingsRepository.clearFontFamily` 新增并清除该设置（`FontManager.setFontFamily` 与 `loadFonts` 两处；`font.ttf` 覆盖时豁免）。
- [ ] 2.6 验证（`cargo test`、`testDebugUnitTest`、androidTest 编译、release APK 设备验证）后 `openspec archive`。
