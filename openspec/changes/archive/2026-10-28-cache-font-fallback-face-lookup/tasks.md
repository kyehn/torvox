# 任务

- [x] 建 change（proposal/design/tasks/specs）
- [x] 样式面查表缓存改名并扩为「面 → 字形号」缓存（`render/font/mod.rs`、`glyph_cache.rs`）
- [x] 回退链查表改走缓存（`render/font/pipeline.rs`、`render/font/cjk.rs`）
- [x] 「解析不出」负缓存覆盖合成样式（`render/font/pipeline.rs`）
- [x] 补回归钉：`fallback_chain_face_lookup_is_cached`、`unresolvable_codepoint_is_cached_for_synthesized_runs`
- [x] 验证两条新用例在改动前失败、改动后通过
- [x] 原生侧门禁：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、`cargo test --all-targets`（743 例全绿）
- [x] `openspec validate --all --strict` 通过
