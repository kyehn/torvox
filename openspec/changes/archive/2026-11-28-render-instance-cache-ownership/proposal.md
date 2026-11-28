# Proposal

## Why

原生渲染的实例构建在上一次提交后处于**编译不过**的状态：`build_instances_cached`
已从 `cell_builder` 删除，而 `pass/frame.rs`、基准与单元测试仍在调用它。`origin/main`
上的 `cargo check --all-targets` 直接报 `cannot find function build_instances_cached`，
本机与 Android 目标都编不过。

修复过程中暴露出三处结构性问题，它们正是这次删除留下的半截状态：

1. **实例有两份副本**：`Renderer` 既有行级缓存 `cell_cache`，又有独立的
   `cpu_instances`；每帧构建先把实例写进缓存，再整屏拷进 `cpu_instances`（约 100KB），
   下一帧又从缓存拷回来。缓存的注释写着「干净行原样复制缓存里的实例」，实际却是
   「整屏复制出去」。
2. **路径判定有两处**：调用方按「缓存兼容 + 掩码够长 + 代际一致」推断本帧是否走了
   增量，构建器内部按「掩码够长 + 缓存兼容 + 代际一致 + 并非全脏」再判一次。两处条件
   不等价：掩码全脏时调用方判「增量」而构建器实际做了全量重建，于是调用方拿全量重建
   的实例去做稀疏上传，干净行的槽位留在上一帧的陈旧实例上。
3. **性能门禁测的不是生产路径**：基准与分配预算用例走的是「一次性构建到调用方 Vec」
   的辅助入口，而生产走的是「跨帧复用缓存」。两者分配行为不同，门禁因此不覆盖生产。

## What Changes

- `CachedInstances` 成为本帧实例的**唯一副本**：`Renderer` 删去 `cpu_instances` 与
  `cell_full_mask_cache`，脏行就地重建、干净行原地保留
- 实例构建收敛为单一入口 `CachedInstances::rebuild(cell_data, config, font_pipeline,
  dirty_rows)`，并由它回报本帧走的是增量还是全量；调用方不再自行推断
- 删除辅助入口 `build_instances_from_cell_data`（其「构建到调用方 Vec」的语义不再是
  任何生产路径的行为），基准与分配预算用例改为直接测生产路径
- `partial_frame_bands` 的首个参数由「是否整帧重建」改为「是否走了增量构建」，与构建
  过程的回报同口径
- 规范补上「实例缓存是唯一副本」与「路径判定唯一且由构建过程回报」两条要求，并把 CI
  下限断言里引用的基准名更新为 `rebuild_all_rows`

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`：新增「实例缓存 MUST 是本帧实例的唯一副本」与「构建走哪条路径 MUST
  由构建过程回报」两条要求与场景。
- `performance-measurement`：CI 下限断言引用的基准名随入口收敛而更新，并新增「性能门禁
  MUST 测生产路径」的要求与场景。

## Impact

- `native/src/render/cell_builder.rs`：构建入口收敛为 `CachedInstances::rebuild`，
  删除 `build_instances_from_cell_data`
- `native/src/render/pass/frame.rs`、`native/src/render/pass/bands.rs`：按构建回报决定
  是否走部分帧
- `native/src/render/context/mod.rs`、`native/src/render/context/renderer.rs`：删去
  `cpu_instances` 与 `cell_full_mask_cache`
- `native/src/render/{cell_builder,pass}/tests/`、`native/src/render/tests/`、
  `native/benches/cell_builder.rs`、`native/tests/instance_alloc_budget.rs`：适配新入口
- 受影响的行为：构建降级为全量时不再走稀疏上传；每帧少一次整屏实例拷贝
