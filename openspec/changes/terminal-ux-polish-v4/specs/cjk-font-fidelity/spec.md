# Spec: cjk-font-fidelity

## Requirement

系统为 zh/ja/ko 时，若主字体不支持 CJK，终端必须以 **Noto Sans CJK（或同等无衬线 CJK）** 回退渲染 CJK 字符，且做到：设置页显示首项 == 实际渲染首项、无衬线必胜衬线、无模糊、首屏极速。

## Design — priority + token + cache

- **Priority**：`cjk_family_priority = base(known/generic/locale/fallback) + localeBoost(6) - serifPenalty(32)`。Serif 罚 32 保证任何位图/矢量组合下 Sans 胜 Serif（证明见 design.md）。
- **Token 边界**：`localeTag` 匹配 `family.split(|c: !alphanumeric).any(|tok| tok==tag)`，避免 `misc` 误命中 `sc`。
- **扫描多数投票**：`test_chars=['中','日','가']` 各探针，`outlineHits>bitmapHits` 判向量。
- **Outline 缓存**：`GlyphCache.outline_cache LRU<(ID,GID),bool> 10k`，`glyph_source_is_outline` 先查缓存，miss 才建 scaler+Render。
- **显示保序**：`cjk_fallback_names()` 按 `cjk_fallback_ids` 顺序保序去重（Seen），归一化仅对 `cjk && !serif` 合并为 `Noto Sans CJK`；`cjk_fallback_names_sorted()` 另提供排序版。

## Scenarios

- Given 系统 locale `zh-CN` 且主字体 `Liberation Mono`（不支持 CJK），when `cjk_fallback_ids` 取 Top3，then sorted 优先级 Sans SC > Sans JP > Serif SC，且 Serif 因 -32 未入 Top3。
- Given `family="misc symbols"` 且 locale `sc`，when 评分，then token 边界使 `misc` 不获 `sc` 加分（旧 `contains` 会误加）。
- Given 混合位图/矢量字体（如 Droid Sans Fallback 含位图中文），when 扫描向量判定，then 多数投票正确判位图，不因首字符恰为矢量而误判。
- Given 首屏 14 个中文字符，when 首次渲染，then `scan` 阶段 scaler build ≤3 次（ outline 缓存命中），首屏 <400ms，二次相同字符 <16ms（`cjk_glyph_cache` 命中）。
- Given 设置页读取 `cjk_fallback_names()[0]` 且渲染取 `cjk_fallback_ids[0]`，when 日志对比，then 二者家族名一致（`FALLBACK_HIT` 与 display 首项同一 `Not...`）。

## Verification

- 单测 `cjk_priority_tests::sans_cjk_outranks_serif_cjk`（含 locale sc/jp/kr）、`locale_token_boundary_misc_not_sc`、`outline_majority_vote`、`fallback_first_item_consistency` 8 用例。
- `cargo test --workspace` 997 pass + 日志审计 `FALLBACK_CANDIDATE -15 vs -47` + `CJK_FALLBACK found 3`。
- 模拟器：`echo 中文渲染速度测试` cat 后 `screencap` 像素采样无宋体衬线、`dumpsys gfxinfo` 首帧 <400ms。
