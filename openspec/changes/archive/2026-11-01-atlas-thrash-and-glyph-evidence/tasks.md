## 1. 图集抖动必须出声

- [x] 发射遍数上限改为具名常量 `MAX_STABLE_ATTEMPTS`（3）
- [x] 遍数用尽仍不收敛时限频 warn：图集尺寸 + 网格 + 「工作集超出图集」
- [x] 不收敛时不写回实例缓存（留空，下一帧整体重建）

## 2. 帧内驱逐用例的前提可收敛

- [x] 图集取 160×160（churn 逐出、帧内搬迁、工作集留得住三者同时成立）
- [x] 断言 churn 真的推进了代际
- [x] 断言第二帧内部真的发生了搬迁

## 3. 字形缓存取证落在真正的查表上

- [x] `styled_screenful_of_codepoints_stays_cached` 直接对 `face_glyph_id_cache` 取证
- [x] 注释写明为何不经 `glyph_information_styled`（主字体覆盖汉字时该路径不查表）

## 4. CJK bearing 与 bearing 契约统一

- [x] 断言实例 bearing 等于 `glyph_bearing_y` 的契约值
- [x] 位图不高于单元格时额外断言「不居中」（用例名那条性质）

## 5. 门禁

- [x] `cargo clippy --all-targets -- -D warnings` 无错
- [x] `cargo fmt --check` 无差
- [x] `cargo test`（含 `tests/concurrency.rs`）全绿
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` 无警告
- [x] `semgrep` / `markdownlint-cli2` / `openspec validate --all --strict` 通过
