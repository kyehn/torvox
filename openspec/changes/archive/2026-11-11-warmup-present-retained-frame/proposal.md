# Proposal

## Why

`docs/STATUS.md` 的未决问题「切换后台返回终端闪烁」在开孔遮罩一侧收口后，还剩
原生侧的一处：新 surface 挂上、渲染循环的首帧尚未产出时，`warmup` 呈现的是一块
背景色而不是用户离开时看到的那一帧。判据用错了——它看的是「累加器里的内容还新不
新鲜」，而暂停期写入的输出恰好让内容变陈旧，那一帧正是用户离开时看到的画面。

## What Changes

- 原生侧的保留帧判据从「内容还新鲜」改为「内容**已渲染**」：累加器里只要有过一帧
  渲染结果，尺寸与格式又与新交换链一致，新 surface 挂上时就呈现它。
- 新增 `frame_content_valid` 标志与累加器生命周期同步：累加器重建（首帧、尺寸/
  格式变化）时置假，成功渲染后置真；`frame_invalidated` 保持原义（下一帧能否走
  部分渲染路径），两者不再混用。
- 主规约 `render-stability` 新增一条要求，写明保留帧的判据与退路。

## Capabilities

### Modified Capabilities

- `render-stability`：新 surface 挂上时保留帧的**呈现判据**（内容已渲染 vs 内容
  还新鲜）与退路（从未渲染过 / 尺寸格式不符时清成背景色）。

## Impact

- `native/src/render/context/mod.rs`
- `native/src/render/context/renderer.rs`
- `native/src/render/context/surface.rs`
- `native/src/render/pass/frame.rs`
- `native/src/render/pass/tests.rs`
- `openspec/specs/render-stability/spec.md`

## 取舍与缺口

- 判据只能落在「内容已渲染」上：内容新鲜度管的是**下一帧**能否走部分渲染路径，
  两者语义不同，混用会让「返回时闪一下」在暂停期有输出时必现（`setRenderPaused(false)`
  本身就会把累加器置陈旧）。
- 从未渲染过的累加器内容未定义，呈现它等于把未初始化纹理端到屏幕上，故必须退回
  清屏。
- 尺寸或格式与新交换链不一致时 `copy_texture_to_texture` 不合法，同样退回清屏。
