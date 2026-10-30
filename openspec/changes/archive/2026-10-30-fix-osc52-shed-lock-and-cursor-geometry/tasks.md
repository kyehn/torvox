## 1. OSC 52 洪泛削减改用注册表读锁

- [x] `clipboard.rs` 削减分支 `wlock_session_registry()` → `session_handle()`
- [x] 注释写明「读锁而非写锁」的理由（等 `pty_write_order` 时长无上界）
- [x] guard 在取到 `Arc` 后即释放，写 PTY 落在锁外

## 2. 帧内未作答计数缓存

- [x] `shed_counts` 只缓存已超限会话（削减分支不注册槽位 → 计数帧内不变）
- [x] 未超限会话逐条重算，避免缓存失准放宽上限

## 3. 宽字符光标归位防下溢

- [x] `build_cursor` 改 `saturating_sub`
- [x] 规范 `cursor-rendering` 同步改述并写明兜底语义

## 4. 宽字符光标几何：注释与规范改如实

- [x] `cell_builder.rs` 注释说明竖线不乘 `cell_span` 的理由
- [x] 规范补「竖线宽 MUST NOT 乘列跨度」一条，消除歧义
- [x] 未改实现：竖线是固定细条，乘列跨度会在宽字符上变成两倍粗的短横

## 5. 附带修正

- [x] `renderer.rs` 文档块归位（拆回 `mark_surface_invalidated`）
- [x] `pipeline.rs` `[None; 128]` → `ASCII_UPPER_BOUND as usize`
