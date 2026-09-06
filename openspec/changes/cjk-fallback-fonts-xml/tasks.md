## 1. 解析函数与依赖

- [ ] 1.1 `native/Cargo.toml` 新增 `roxmltree = "0.20"` 直接依赖，`cargo metadata -p native` 确认版本与 Cargo.lock 一致且无新增传递依赖
- [ ] 1.2 `font_db.rs` 新增纯函数 `parse_fonts_xml_families(xml: &str) -> (Vec<String>, Vec<(String, Vec<(String, u32)>)>)`（monospace 文件名 + 保序的 `(lang, [(filename, index)])`，`index` 缺省为 0），根元素兼容 `familyset`/`fontconfig`，未知元素忽略，无属性 `<font>` 标签按文本提取，`cargo check -p native` 通过
- [ ] 1.3 `font_db.rs` 新增 `locale_to_fonts_xml_langs(locale: &str) -> Vec<&str>` 多候选映射（zh-CN→zh-Hans/zh-CN/zh；zh-TW→zh-Hant/zh-TW/zh-HK/zh；ja/ko 直投；其他为空），单元测试覆盖全部映射分支

## 2. 替换脆弱解析与 CJK 接入

- [ ] 2.1 用 `parse_fonts_xml_families` 重写 `resolve_system_monospace_from_fonts_xml()`（保留签名与 `None` 回退语义），`cargo test -p native font_db` 通过
- [ ] 2.2 `cjk.rs::find_cjk_fallback_fonts()` 接入：`fonts.xml` 命中的 `(file_name, index)` 精确映射 face ID（`Path::file_name` 小写归一化匹配 `Source::File`/`SharedFile` 路径 + `face.index` 比对；`index` 缺失或失配时用现有 `locale_token_match` 二次过滤）前置 + 去重后接原 `scan_fallback_candidates` 结果，`cargo test -p native font::` 全过
- [ ] 2.3 读取路径顺序 `/system/etc/fonts.xml` → `/system/etc/fonts_fallback.xml`，任一解析失败/无命中时行为与改前一致（以 `non_cjk_locale_no_fallback` 等既有测试为证）

## 3. 测试与真机验证

- [ ] 3.1 新增单元测试：内嵌从 API 35 模拟器拉取的 AOSP 格式片段（`zh-Hans→NotoSansCJK-Regular.ttc index=2` 等）覆盖 monospace 提取、`<family lang>` 保序提取、`index` 属性提取、无属性 `<font>` 标签、损坏 XML 回退空结果，`cargo test -p native font_db` 通过
- [ ] 3.2 新增单元测试：`zh-CN`/`zh-TW`/`ja`/`ko`/`en-US` 五档 locale 的端到端回退选择（含同一 TTC 内 index 精确命中 SC/TC/JP/KR 变体，`index` 失配时回退 family 名过滤），`cargo test -p native cjk` 通过
- [ ] 3.3 真机/模拟器验证（已部分完成 2026-09-06，API 35 模拟器）：干净安装后 `I native::android::ffi: setSystemLocale: zh-CN` 首次到达 Rust 侧，`noto sans cjk sc` 获 `eff_pri=21`（locale boost 生效）；附带发现并修复 spawn 前 `setSystemLocale` 被 `sessionId == 0` 静默丢弃的时序 bug（spawn 后补调，见 TerminalRuntime.kt）。剩余：中文字符实际渲染 E2E（`adb input text` 不支持中文，需其他输入方式），以及 `adb shell cat /system/etc/fonts.xml` 与 logcat `CJK_FALLBACK` 的一致性复核
- [ ] 3.4 `openspec validate --change cjk-fallback-fonts-xml` 通过，无 clippy 新增警告（生产代码无 `#[allow]`）
