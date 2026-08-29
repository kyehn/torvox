# Spec: cjk-font-fidelity (v5 — 构建门禁 + 日志审计)

## Requirement

系统为 zh/ja/ko 时，若主字体不支持 CJK，终端必须以 **Noto Sans CJK（或同等无衬线 CJK）** 回退渲染 CJK 字符，且做到：设置页显示首项 == 实际渲染首项、无衬线必胜衬线、无模糊、首屏极速；**构建产物必须含 `.so` 且可验证**。

## Design — 保持 v4 核心，补门禁与审计

- **Priority**：`cjk_family_priority = base(known/generic/locale/fallback) + localeBoost(6) - serifPenalty(32)`。Serif 罚 32 保证任何位图/矢量组合下 Sans 胜 Serif（证明见 design.md）
- **Token 边界**：`localeTag` 匹配 `family.split(|c: !alphanumeric).any(|tok| tok==tag)`，避免 `misc` 误命中 `sc`
- **扫描多数投票**：`test_chars=['中','日','가']` 各探针，`outlineHits>bitmapHits` 判向量
- **Outline 缓存**：`GlyphCache.outline_cache LRU 10k`，命中 0.2µs
- **显示保序**：`cjk_fallback_names()` 按 `cjk_fallback_ids` 顺序保序去重，归一化仅对 `cjk && !serif` 合并；`cjk_fallback_names_sorted()` 另提供排序版
- **v5 门禁**：`build-android-libs.nu → jniLibs/libnative.so`（`readelf` 无 ghostty NEEDED，15M-60M）→ `build-apk.nu → 90M APK` 含 `.so`（`unzip -l | grep .so` 非空）

## Scenarios

- Given 构建后，when `unzip -l app-release.apk | grep .so`，then 命中 1+ 条，`readelf --dynamic libnative.so | grep NEEDED | grep ghostty` 为空
- Given 系统 locale `zh-CN` 且主字体不支持 CJK，when `cjk_fallback_ids` 取 Top3，then sorted 优先级 Sans SC > Sans JP > Serif SC，且 Serif 因 -32 未入 Top3
- Given `family="misc symbols"` 且 locale `sc`，when 评分，then token 边界使 `misc` 不获 `sc` 加分
- Given 首屏 14 个中文字符，when 首次渲染（产物正确），then `FALLBACK_CANDIDATE` 日志 `sans -15 vs serif -47`，首屏 <400ms，二次 <16ms
- Given 设置页读取 `cjk_fallback_names()[0]` 且渲染取 `cjk_fallback_ids[0]`，when 日志对比，then 二者家族名一致

## Verification

- 单测 `cjk_priority_tests::sans_cjk_outranks_serif_cjk`、`locale_token_boundary_misc_not_sc`、`serif_penalty_guards_vector_vs_bitmap` 5 用例 + `is_cjk_candidate` 27 用例
- 构建门禁 `unzip -l` + `readelf` + `stat` 脚本化
- 模拟器 `logcat FALLBACK_CANDIDATE` + `screencap` 像素采样无宋体
