## 1. 依赖

- [x] `Cargo.toml`：`guillotiere = "0.7"` → `etagere = "0.3"`
- [x] `render/font/atlas.rs`：`guillotiere::` → `etagere::`（构造、3 处 `allocate`、`deallocate`）
- [x] `render/font/pipeline.rs`：`guillotiere::` → `etagere::`（`AtlasAllocator` 字段与 2 处构造）
- [x] `render/font/mod.rs`：`allocation_id: Option<guillotiere::AllocId>` → `etagere::AllocId`

## 2. 验证

- [x] `cargo check --all-targets` 通过
- [x] GPU CI `frame_and_atlas` 通过（2026-11-04 复核：CI 的 `scripts/check.nu` 在
      `nix develop` 里跑 `cargo test`，该环境按 `flake.nix` 把 `VK_ICD_FILENAMES`
      指向 Mesa lavapipe，故 CI 的 Vulkan 用例即在 lavapipe 下执行。本机同环境
      复跑 `cargo test --lib frame_and_atlas` 全绿：28 passed / 0 failed）
