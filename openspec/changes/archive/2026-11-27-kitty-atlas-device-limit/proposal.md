# Proposal

## Why

图集布局的可服务性此前只有「字节预算」一项判据，设备纹理边长上限（`max_texture_dimension_2d`）
只在 GPU 侧检查（`set_kgp_atlas`）。两处后果：

1. **逐帧重试打包与逐帧刷警告**：布局宽度超出设备上限时 GPU 侧拒绝，纹理尺寸归零；
   渲染线程的复用分支拿布局尺寸与已上传图集尺寸（恒为 0）比对，必然不等，于是走
   「整包重传」分支——每帧重算布局、每帧尝试打包、每帧写一条警告，图像却永远不会出现。
   终端持续输出时该分支逐帧执行，警告按帧率刷进 logcat，违反 `logcat-hygiene` 的
   「逐帧诊断必须限频」。
2. **判据分叉**：预算判据只有一处实现（`atlas_byte_size`），设备上限判据却在 GPU
   侧单独实现，两条路径（打包、复用）看到的可服务性口径不一致。

根因与上一轮（`2026-10-28-kitty-atlas-reuse-budget`）同源：同一条不变量被两个调用点
各实现一遍，只改一侧必然留下分叉。

## What Changes

- 抽出唯一判据 `check_atlas(plan, max_side)`：尺寸须落在设备纹理边长上限内，
  字节数须受检算术且在预算内；打包路径与复用路径共用。
- 设备上限由渲染器按当前设备给出（`Renderer::max_texture_side`），经布局层传入；
  GPU 侧的同款检查保留为纵深防御。
- 复用路径只判成败、不记拒绝日志（打包路径已记）：逐帧重试时复用路径记日志会把
  一条诊断刷成日志洪水。
- 回归测试补上「超设备上限的布局在两条路径上一致被拒」。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `kitty-graphics`：补上「图集布局必须落在设备纹理边长上限内」的要求与场景。

## Impact

- `native/src/render/kitty.rs`：新增 `AtlasPlan` 与唯一判据 `check_atlas`；
  `pack_atlas` / `layout_entries` / `pack_and_build` 接受设备上限参数
- `native/src/render/context/renderer.rs`：新增 `max_texture_side()`
- `native/src/android/ffi.rs`、`native/src/android/ffi/render/frame.rs`：传入设备上限
- 受影响的行为：超设备上限的 Kitty 图像不再每帧重试打包与刷日志；图像仍不显示
  （设备装不下，与上一轮一致），但代价降为一次布局计算
