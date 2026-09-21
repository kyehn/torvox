## Why

用户主诉：上下滚动时“底部残留”（滚动后旧内容残影留在屏幕底部）与“滚动撕裂”。

模拟器像素取证（API-35 SwiftShader，`adb shell input swipe` + `screencap`）：

- 滚动进历史后（scrollOffset=32），屏幕底部网格行下方出现不属于当前视口的残影行片段
  （OCR 读到 `2UU`/伤损行，`evidence/scroll_into_history.png`），即“旧内容残影留在屏幕底部”。
- 滚动期间 logcat：`render_frame: presented … (partial=true, bands=1)` 每帧仅一个带。

代码路径取证：

- `viewport_scroll_px`（setScrollYPx，亚像素余量）只作用于**当帧新绘制几何**的投影平移
  （`context.rs cell_uniforms → apply_scroll_px_offset`），**不会移动累加器（frame_texture）
  里上一帧已呈现的像素**；纹理拷贝无法亚像素平移。
- partial（脏带）路径用 `LoadOp::Load` 叠在旧累加器内容上：带矩形按新的投影偏移绘制，
  旧偏移位置与屏幕边缘条带（顶部 `[0, scroll_px)`、底部 `[rows*cell_h+scroll_px, height)`）
  的旧像素不被覆盖 → 残影/残留（模拟器 `2UU` 即该机制）。
- 拖动中 idle 重绘只标光标/高亮行脏（ffi.rs render_inner），这些行以新偏移重复绘制，
  旧位置像素残留在累加器 → 底部光标行/提示符多重残影 = 感知上的“滚动撕裂”。
- `scroll_up_rows` 滚动 blit（pass.rs）为死代码（ffi.rs 恒为 None），且 frame_texture
  缺 `COPY_DST`，即使启用也会校验失败；其语义也只覆盖整行、不覆盖亚像素余量。

结论：残留/撕裂根因 = 部分渲染（脏带）把“上一帧旧像素”与“投影平移的新几何”混合呈现，
亚像素滚动期间无自洽的整体移动机制。

## What Changes

- 渲染帧决策新增滚动一致性门控：`viewport_scroll_px != 0` 或自上一呈现帧以来发生变化时，
  强制全量自包含重绘（`LoadOp::Clear(背景)` + 绘制全部实例），禁用脏带部分路径；
  偏移归零且稳定后再恢复部分渲染。滚动期间每帧画面完整自洽，边缘条带显示清屏背景
  （该条带本应在下一次行进位时由 VT 推送的新行填充），不再有旧像素残留。

## Impact

- native/src/render/pass.rs（帧决策/清屏/绘制范围）
- native/src/render/context.rs（新增 last_drawn_viewport_scroll_px 状态）
- 不改 Kotlin 渲染循环、不改构建文件、不改散列 shader 语义。