## Context

动机见 proposal.md。现状与约束：

- `native/src/render/font/font_db.rs:87-115` 的 `resolve_system_monospace_from_fonts_xml()` 用纯文本搜索（`content.find("name=\"monospace\"")` + 手工切 `<font>` 标签）解析 `/system/etc/fonts.xml`，只取 monospace 首个字体，不解析 CJK 回退链；`<font>` 无属性时（`<font>X.ttf</font>`）匹配 `"<font "` 会漏检。
- `native/src/render/font/cjk.rs:76-133` 的 `find_cjk_fallback_fonts()` 靠家族名字符串匹配评分近似系统回退链，在 OEM 定制字体设备上可能失配。
- `fontdb 0.23` 的 `FaceInfo.source` 为 `Source::File(PathBuf)`（`load_font_file` 默认行为），可直接按文件名映射，无需模糊匹配。
- `roxmltree 0.20.0` 已在 `Cargo.lock`（经 `fontconfig-parser` → `fontdb` 引入），提升为直接依赖不引入新版本；纯安全 Rust，符合核心路径禁 `unsafe`。
- `fontconfig-parser` 要求 XML 根元素为 `<fontconfig>`（其 `parser.rs` 硬校验），而 AOSP `fonts.xml` 根元素为 `<familyset>`，故不可复用，已排除。
- minSdk 33；不考虑 OEM 定制；无 Kotlin/JNI 改动预算（保持改动面最小）。

## Goals / Non-Goals

**Goals:**

- CJK 回退选择以 `fonts.xml` 的 `<family lang>` 链为首要依据，与 Termux/系统行为一致。
- 解析失败时行为与现状完全一致（零回退风险）。
- 解析逻辑可单元测试（纯函数 + 内嵌 AOSP 格式片段）。

**Non-Goals:**

- 不处理 OEM 自定义字体路径与私有可能格式（按 AGENTS.md 简洁原则，只覆盖 AOSP 标准结构）。
- 不改字形光栅化、atlas、字重合成逻辑。
- 不新增 JNI 方法与 Kotlin 代码。

## Decisions

### D1: 用 roxmltree 直接解析，而非复用 fontconfig-parser

- Why：`fontconfig-parser` 根元素校验与 `fonts.xml`（`<familyset>`）不兼容；其能力（match/edit/alias 语义）远超所需，引入反而增加理解成本。
- Alternative（已排除）：Kotlin 侧 `Typeface.getFontFamily()` 查询后经 JNI 回传——返回的是 family 名而非文件路径，Rust 侧仍需模糊映射，且跨语言改动面更大。

### D2: 解析函数为纯函数 `parse_fonts_xml_families(xml: &str)`

- Why：文件 IO 与解析分离，单元测试可内嵌 AOSP 格式片段，无需 Android 环境；`#[cfg(not(target_os = "android"))]` 下返回空，保持桌面构建与现有测试行为不变。
- 输出：`(monospace_filenames: Vec<String>, lang_fallbacks: Vec<(String, Vec<String>)>)`——monospace 文件名列表与 `(lang, filenames)` 列表，保留文档顺序即优先级。

### D3: 用 `(file_name, index)` 精确映射，family 名匹配仅为次级策略

- Why：实测（API 35 模拟器）四个 CJK locale 块指向**同一** `NotoSansCJK-Regular.ttc`，仅 `index` 不同（zh-Hans→2，zh-Hant→3，ja→0，ko→1，与 TTC 标准变体顺序 JP/KR/SC/TC=0/1/2/3 吻合）。文件名级匹配无法区分变体——这正是“中文显示为 JP 字形”的可能根因，而当前实现完全忽略 index。
- fontdb 保证：`load_font_source`/`load_fonts_from_file` 按 `0..n` 顺序拆 TTC，`FaceInfo.index` 即 TTC 内序号（fontdb 0.23 `lib.rs` 实证）。故 `fonts.xml` 的 `(filename, index)` 可精确映射到 fontdb face：先比 `Path::file_name`（小写归一化），再比 `face.index`。
- 次级策略：`index` 属性缺失或映射失配时，才用现有 `locale_token_match` 按 family 名过滤（复用 `cjk.rs` 已有逻辑，不新增匹配规则）。
- 无属性 `<font>` 标签（如 `<font>X.ttf</font>`）按标签文本提取，自然覆盖（现有纯文本搜索找 `"<font "` 会漏检此类标签，本设计顺带修复）。

### D4: 只做加法——fonts.xml 结果作为最高优先级前置

- Why：`find_cjk_fallback_fonts()` 现有评分扫描保留为补充与失败回退；`fonts.xml` 命中的 ID 前置 + 去重后接原扫描结果。任何解析/映射失败都等价于“`fonts.xml` 未提供提示”，行为与现状一致。

### D5: locale→lang 映射用多候选列表

- Why：AOSP 用 `zh-Hans`/`zh-Hant`，部分旧设备与文档用 `zh-CN`；匹配时按候选顺序取首个命中的 `<family lang>` 块。`zh-CN`→`[zh-Hans, zh-CN, zh, und-Hani]`；`zh-TW`/`zh-HK`→`[zh-Hant, zh-TW, zh-HK, zh, und-Hani]`；`ja`→`[ja]`；`ko`→`[ko]`；其他→空（走原逻辑）。`und-Hani` 为零成本末位兜底（实测 API 35 无此块，但 BCP-47 语义下合法）。

## Risks / Trade-offs

- [Risk] `fonts.xml` 头部官方 DEPRECATED 声明（要求应用停止解析、改用 `SystemFonts#getAvailableFonts`/`ASystemFontIterator`，格式随时会变）→ Mitigation：本变更不新增对该文件的依赖面（既有 `resolve_system_monospace_from_fonts_xml` 已在读取）；`ASystemFontIterator` 只给文件列表、无 fallback 语义，不可替代；D4 失败回退保证格式漂移时行为退化到现状而非崩溃。接受该 trade-off 并在此记录。
- [Risk] `fonts.xml` 在不同 API 级别结构漂移（如新增属性、`fonts_fallback.xml` 分流）→ Mitigation：根元素兼容 `familyset`/`fontconfig` 两种；读取路径按 `/system/etc/fonts.xml` → `/system/etc/fonts_fallback.xml` 顺序尝试；未知元素一律忽略而非报错。
- [Risk] 文件名大小写/分隔符差异导致映射失败 → Mitigation：比较时统一小写并将 `-`/`_`/` ` 归一化（沿用 `pipeline.rs` 现有归一化惯例）；失败即回退原逻辑。
- [Risk] `roxmltree` 直接依赖提升后未来版本漂移 → Mitigation：按 DESIGN.md 依赖政策用 `"0.20"` 非固定写法，随 `fontdb` 升级自然对齐。

## Migration Plan

- 无需迁移：纯增量改动，无数据格式、无设置项、无 API 变更；回滚即 revert 三个文件。
- 验证：`cargo test -p native font::` 全过；单元测试使用从真机拉取的 `fonts.xml` 片段（含 `zh-Hans→index=2` 结构）；模拟器实测（API 35）已确认 `setSystemLocale` 到达 Rust 侧且 sc 获 locale boost（`eff_pri=21`），附带修复 spawn 前调用被 `sessionId == 0` 丢弃的时序 bug。中文字符渲染 E2E 待补（`adb input text` 不支持中文）。

## Open Questions

- 无。需真机验证的项（`fonts_fallback.xml` 是否存在、TTC 变体 family 命名）已在 tasks 中列为验证步骤，不影响方案与任务分解。
