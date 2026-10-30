# 图集分配器 guillotiere → etagere

## Why

`render/font` 用 `guillotiere 0.7` 做字形图集的 2D 纹理分配。`guillotiere 0.7.0` 发布于 2021-09，此后无新版本，实质停滞；而它的精神继任者 `etagere`（同一作者 nical）持续维护（最新 `0.3.0`，8 个月前发布），并被 `wgpu`、`resvg` 等采用为标准图集分配器。两者 API 几乎一致（`AtlasAllocator::new`/`size2`/`allocate`/`deallocate`/`AllocId`、矩形 `Rectangle.min.{x,y}` 同形），替换成本极低。

DESIGN 要求「依赖、工具尽量使用最新版本」「优先使用库」，BUILD 要求「依赖使用最新稳定版本、不得降级」。把停滞依赖替换为维护活跃的同构替代品，符合依赖卫生原则。

## What Changes

- 移除 `guillotiere`，引入 `etagere = "0.3"`。
- `render/font/{atlas,pipeline,mod}.rs` 中 `guillotiere::` 命名空间整体改为 `etagere::`（构造、`allocate`/`deallocate`、`AllocId` 字段类型）。
- 分配器行为不变：仍是「分配失败 → 驱逐 LRU → 重建」流程，仅底层打包算法由 guillotine 换成 shelf；对任意合法非重叠打包，正确性一致，且 `frame_and_atlas` 既有测试在 GPU CI 下覆盖。

## Impact

- 受影响能力：字形图集（`render/font`）。
- 风险：打包算法改变会影响图集内矩形排布，只影响坐标、不影响正确性；本仓无 GPU，行为正确性依赖 `frame_and_atlas` 系列测试在带 Vulkan（Mesa lavapipe）的 CI 下验证（TESTING 要求缺 GPU 时失败而非跳过）。

## 状态

- [x] 替换依赖与命名空间
- [x] `cargo check --all-targets` 通过（编译验证，etagere v0.3.0 入 lock、guillotiere 移除）
- [ ] GPU CI 下 `frame_and_atlas` 测试通过（行为验证；本仓无 GPU，待 CI）
