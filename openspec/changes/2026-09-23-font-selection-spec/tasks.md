# 字体选择按规范重实现

## 上下文

- `DESIGN.md :94`–`:104` 字体选择节、`TESTING.md :31 :39` 覆盖项为标准。
- native 侧已合规，只改 Kotlin/Compose 侧。

## 任务

- [x] 2.1 `systemFonts()` 改 DOM 解析 `/system/etc/fonts.xml`（纯函数 + 读文件两层；缺失/不可解析 → 日志 + `IllegalStateException`）；删目录扫描/`Typeface` 回退/硬编码表/`cleanFontName`；重写 `SystemFontsTest`。
- [x] 2.2 删除文件安装流：对话框首行、`onPickFontFile` 参数链、`customFontLauncher`、`installFontFile`、`getFileNameFromUri`、`sanitizeFontFileName`、`filesDir/fonts` 创建与缓存迁移、`pick_font_file` 文案；保留 `loadFontFile` JNI/Bridge/Runtime 仅作探测；`FontSwitchInstrumentedTest` 改断言。
- [x] 2.3 `font.ttf`/`.ttc`/`.otf` 存在即默认：`Bridge.setFontFamily` 内优先 `loadFontFile` 探测应用并返回应用结果，无则走传入值。
- [x] 2.4 用户字体目录改为 `files/home/.termux/font`（`setExtraFontPaths` 指向 + `lastExtraFontPaths` 去重；列表经 `listFontFamilies` 天然包含；含 `.ttc`）。
- [x] 2.5 存入 family 经 native 返回值判定（`applyFontSettings` 返回 `Boolean?`；false → 日志 + `SettingsRepository.clearFontFamily`；null 无会话不处理；失败 toast 不再报成功）。
- [x] 2.6 `loadFonts` 致命错误直通崩溃（`IllegalStateException` 单独捕获重抛，不再吞入空列表）；列表恢复文档顺序（`fonts.xml` 在前，native 补充追加，只精确去重不排序；对话框同步去排序；默认名回退去掉硬编码）。
- [x] 2.7 `font.ttf` 覆盖探测按路径 + mtime + 大小缓存，同一文件只进库一次。
- [x] 2.8 native `find_font_by_name` 支持 `fonts.xml` 别名（别名→文件名→已加载 face 精确查找，不加载新文件；精确优先于模糊）。
- [x] 2.9 去硬编码回退（Kotlin `getDefaultFontName` 空回退，native `system_monospace_name` 空回退；`shaping` 空即 `Family::Monospace` 不变）。
- [x] 2.10 `loadFonts` 通用异常直通抛出（仅保留取消信号优先重抛），不再吞错置空列表；过时 `filesDir/fonts` 注释更新。
- [ ] 2.11 验证（`cargo test`、`testDebugUnitTest`、androidTest 编译、release APK 设备验证）后 `openspec archive`。
