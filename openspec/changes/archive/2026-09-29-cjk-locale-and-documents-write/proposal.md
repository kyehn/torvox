## Why

设备取证（emulator-5554 / API 35 / `persist.sys.locale=zh-CN`）复现两处缺陷：

- **CJK 区域族永久缺失**：`.mkshrc` 令 shell 启动时 `printf` 中文测试，同屏 ASCII 标点正常而
  4 个汉字全是豆腐块。logcat 显示 `CJK_FALLBACK: found 0 fallback fonts`，
  且 `setSystemLocale: zh-CN` 确已到达 native。根因是调用顺序：
  `Bridge.onSession` 在 `sessionId == 0` 时丢弃调用，spawn 前的 `setSystemLocale` 从未进入 native；
  随后的同步 `setExtraFontPaths` 经 `render_state_mut()` 创建管线并以 `current_locale() == ""`
  构建字体库（`loaded 2 font files, 2 faces`，无 `NotoSansCJK-Regular.ttc`）；
  `CACHED_FONT_DB` 是 `OnceLock`，进程存活期间不再重建；spawn 后重放的 `setSystemLocale`
  只能按 (文件名, index) 在**已在库中**的面里查找，于是仍然 0 命中。
  既有 `cjk-rendering` 测试全部先 `try_load_cjk_fonts` 手工灌库，替生产代码补上了缺失的前置条件，
  故缺陷对测试不可见（`nix develop` 下 103 passed）。
- **文档提供器权限属性写错位置**：清单写的是 `android:permission`，该字段填
  `ProviderInfo.permission`，内容提供器访问路径根本不读它——实测 shell uid 无任何授权
  即可 `content read` 成功，提供者对全设备应用裸奔。而 AOSP `DocumentsProvider.attachInfo`
  真正强制的是 `readPermission` 与 `writePermission` 同时为 `MANAGE_DOCUMENTS`，
  当前写法是靠"属性写了但填错字段"侥幸通过该检查。另有：`openDocument` 返回裸
  `ParcelFileDescriptor`，外部写回后不广播，文件选择器看不到变化；
  `require()` 抛 `IllegalArgumentException` 跨 Binder 会打崩 DocumentsUI；
  `sanitize()` 静默改写合法文件名（`a..b.txt` → `a_b.txt`）且返回的 docId 与请求不符。

## What Changes

- 区域回退族改为**增补**而非冻结：`font_db::load_region_fallback_faces` 按 locale 把 fonts.xml 的
  `lang` 块族补装进活动字体库；`FontPipeline::set_system_locale` 在重发现回退层前调用它。
  只增补不重建——`fontdb::ID` 是库内序号，重建会使主字体 `font_id` 指向另一个面。
  缺陷自此与 Kotlin 侧调用顺序无关，`createSession` 的竞态一并消失。
- 新增纯函数 `missing_region_fallback_faces`（库 + fonts.xml + locale → 缺失的 (文件名, 索引)），
  单测锁定"库在 locale 之前定型"这一真实回归场景。
- 清单把无效的 `android:permission` 换成平台强制的 `readPermission` + `writePermission`
  （均为 `MANAGE_DOCUMENTS`）：补上真正的访问控制，SAF 客户端凭逐 URI 授权不受影响。
- `openDocument` 改用带 `OnCloseListener` 的 `ParcelFileDescriptor.open`，外部写回后广播文档与父目录。
- `DocumentMutations` / `parseOpenModeFallback` 的 `require()` 改为抛 `FileNotFoundException`。
- `sanitize()` 改为只校验不改写：拒绝含路径分隔符、`.`、`..`、空白的名字。
- 补跨进程写入仪器测试（经 `ContentResolver.openOutputStream` 写回并验证落盘）。

## Capabilities

### New Capabilities

- `documents-provider`: 系统文件选择器所见用户文件的暴露与可写回契约。

### Modified Capabilities

- `cjk-rendering`: 区域回退族的装载时机（不再依赖 locale 早于字体库构建）。

## Impact

- `native/src/render/font/font_db.rs`、`native/src/render/font/pipeline.rs`。
- `android/app/src/main/AndroidManifest.xml`。
- `android/app/src/main/java/terminal/emulator/{TerminalDocumentsProvider,DocumentMutations}.kt`。
- `DocumentsProviderTest.kt` 两处用例随契约修正；`DocumentsProviderInstrumentedTest.kt` 补写入用例。

## Non-Goals

- 不改 `queryChildDocuments` 的 `sortOrder`：SAF 排序为可选提示，根未声明相关能力标志，
  实现它属于规范未声明的新逻辑。
- 不改根目录（`filesDir/home`）、链接语义与 `FLAG_SUPPORTS_*` 声明。
- 不改保护文件（`.github/`、`scripts/`、`flake.nix`、`docs/specification/` 等）。
