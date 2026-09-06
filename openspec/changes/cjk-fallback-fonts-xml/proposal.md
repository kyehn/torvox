## Why

终端 CJK 字形当前靠家族名字符串匹配评分选择回退字体，与 Android `fonts.xml` 定义的语言特定回退链不一致，导致简中用户可能看到宋体或日文字形而非系统默认的 Noto Sans CJK SC。本变更让 Rust 渲染侧直接读取 `fonts.xml` 的 `<family lang>` 回退链，做到与 Termux（系统 Typeface 解析）相同的效果。

## What Changes

- 新增 `font_db.rs` 纯函数：解析 `fonts.xml` 文本，提取 monospace 目标文件名与各 locale 的 CJK 回退 `(filename, index)` 列表（含 `<font>` 的 `index` 属性，用于区分同一 TTC 内 SC/TC/JP/KR 变体）。
- `find_cjk_fallback_fonts()` 优先采用 `fonts.xml` 指定的 `(file_name, index)` 精确映射到 fontdb face（`FaceInfo.index` 即 TTC 内序号）；`index` 缺失或失配时才用现有 `locale_token_match` 按 family 名二次过滤。
- 解析失败或文件缺失时回退到现有评分扫描行为（只做加法，不改变现有成功路径）。
- `native/Cargo.toml` 新增 `roxmltree = "0.20"` 直接依赖（已在 Cargo.lock 中，无新版本引入）。
- 替换 `resolve_system_monospace_from_fonts_xml()` 内脆弱的纯文本搜索为同一解析函数。

## Capabilities

### New Capabilities

- `font/cjk-fallback`: CJK 回退字体必须与系统 `fonts.xml` 语言回退链一致的选择规则与回退语义。

### Modified Capabilities

（无。`openspec/specs/` 尚为空，本次同时建立长期规范。）

## Impact

- 影响代码：`native/src/render/font/font_db.rs`、`native/src/render/font/cjk.rs`、`native/Cargo.toml`。
- 新增依赖：`roxmltree 0.20`（纯安全 Rust，已为传递依赖，无 `unsafe`，符合核心数据路径禁 `unsafe` 要求）。
- 无 JNI/Kotlin 改动，无 API 破坏，无行为回退（失败路径与现有行为一致）。
- 规范依据：`docs/specification/DESIGN.md` 设置·字体选择、CJK 回退条款与终端·CJK 渲染条款。
