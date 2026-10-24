# Tasks

## 1. 键位映射

- [x] 1.1 `KEYCODE_BREAK` → `key::Key::Pause`，
      验证：`pause_break_key_maps_to_pause`
- [x] 1.2 逐键对照 Termux `KeyHandler` 确认其余条目无缺口（`KEYCODE_BACK` 归系统返回）

## 2. 修饰键栏长按阈值

- [x] 2.1 阈值改取 `viewConfiguration.longPressTimeoutMillis`，越界退回 400ms，
      锁定与自动重复共用，验证：`ModifierBarLongPressTest`（区间内 / 越界两组）
- [x] 2.2 仪器化复核长按锁定仍生效，
      验证：`ModifierBarTest`（14 例，模拟器）

## 3. 缩窄重排覆盖

- [x] 3.1 主屏缩窄保留完整正文，验证：`shrink_reflow_keeps_wrapped_text`
- [x] 3.2 备用屏缩窄只裁剪不重排且不崩溃，验证：`shrink_alt_screen_clips_without_reflow`
- [x] 3.3 删除测试文件里「RS-001 是最大缺口」的过期备注

## 4. 规范

- [x] 4.1 三个能力的增量落在本 change 的 `specs/` 下，
      验证：`openspec validate 2026-10-24-research-audit-followups --strict`
- [x] 4.2 六份研究的条目逐条给出核验结论（`design.md` 对照表），
      不做无证据的「已实现」声明

## 5. 集成验证

- [x] 5.1 Rust：`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib`（703 例）
- [x] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck`
- [x] 5.3 模拟器：`ModifierBarTest` 全绿

## Workflow follow-up

- `openspec validate ... --strict` 通过后归档，三项增量同步进主 spec。
- 归档后执行 `openspec validate --all --strict`。
