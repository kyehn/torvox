## 1. 依赖

- [x] `Cargo.toml`：`guillotiere = "0.7"` → `etagere = "0.3"`
- [x] `render/font/atlas.rs`：`guillotiere::` → `etagere::`（构造、3 处 `allocate`、`deallocate`）
- [x] `render/font/pipeline.rs`：`guillotiere::` → `etagere::`（`AtlasAllocator` 字段与 2 处构造）
- [x] `render/font/mod.rs`：`allocation_id: Option<guillotiere::AllocId>` → `etagere::AllocId`

## 2. 验证

- [x] `cargo check --all-targets` 通过
- [ ] GPU CI `frame_and_atlas` 通过
