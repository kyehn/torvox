# 任务

## 上下文

- `font_db.rs:23` `CACHED_FONT_DB` 为 `OnceLock`，区域族由 `current_locale()` 选出。
- `ffi.rs:2965` `set_current_locale` 仅由 JNI `setSystemLocale` 写入；该 JNI 只在 sessionId != 0 时到达。
- `TerminalRuntime.kt:2009` 同步 `setExtraFontPaths` → `render_state_mut()` 定型字体库。
- `pipeline.rs:268` `set_system_locale` 只清缓存重发现，不补装区域族。
- `AndroidManifest.xml:66` `android:permission` 在提供器访问路径上不被强制。
- `TerminalDocumentsProvider.kt:243` 裸 `ParcelFileDescriptor.open`，无关闭回调与广播。
- `DocumentMutations.kt:20,114,144,276` 与 `TerminalDocumentsProvider.kt:102-106` 用 `require()`。

## 任务

- [x] 1.1 `font_db.rs` 新增纯函数 `missing_region_fallback_faces`（库 + fonts.xml + locale → 缺失的 (文件名, 索引)）
- [x] 1.2 `font_db.rs` 新增 `load_region_fallback_faces`（只增补缺失面，已装入跳过）
- [x] 1.3 `pipeline.rs` 的 `set_system_locale` 在重发现回退层前补装区域族
- [x] 2.1 单测：库在 locale 之前定型时区域族被报为缺失
- [x] 2.2 单测：补装后不再报缺失（同名不同索引只补缺的那个面）
- [x] 2.3 单测：非 CJK locale 不补装
- [x] 3.1 `AndroidManifest.xml` 把 `android:permission` 换成平台强制的 `readPermission`+`writePermission`
- [x] 4.1 `TerminalDocumentsProvider.openDocument` 带 `OnCloseListener` 广播写回
- [x] 4.2 `parseOpenModeFallback` 改抛 `FileNotFoundException`
- [x] 5.1 `DocumentMutations` 目录校验改抛 `FileNotFoundException`
- [x] 5.2 `sanitize` 只校验不改写（拒绝 `/`、`\`、`.`、`..`、空名）
- [x] 5.3 `DocumentMutations` 新增 `notifyWritten` 供外部写回广播
- [x] 6.1 同步修正 `DocumentsProviderTest.kt` 两处受契约变更影响的用例
- [x] 6.2 `DocumentsProviderInstrumentedTest.kt` 补跨进程写入回写用例
- [x] 7.1 验证 `scripts/check-rust.nu`
- [x] 7.2 验证 `scripts/build-android-libs.nu --profile dev x86_64`
- [x] 7.3 验证 `scripts/check-gradle.nu`
- [x] 7.4 模拟器以 `persist.sys.locale=zh-CN` 复跑取证循环，汉字不再是豆腐块
