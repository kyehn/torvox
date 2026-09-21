## 1. 取证：残留/撕裂像素证据与代码路径

- [x] 1.1 模拟器滚动截图：`input swipe` + `screencap`，OCR/像素分析确认滚动后底部残影
      （scroll_into_history.png：底部网格行外出现 `2UU` 残影片段）。
- [x] 1.2 阅读渲染纵向滚动实现（pass.rs take_dirty_rect/upload_atlas/present、context.rs
      viewport_scroll_px、ffi.rs render_inner scrollOffset/scrollYPx）：
      亚像素偏移只平移新绘几何，累加器旧像素不动（部分路径叠加）。

## 2. 修复：滚动一致性门控（强制全量重绘）

- [x] 2.1 实现：`viewport_scroll_px != 0` 或相对上一呈现帧变化 → 本帧 `Clear(背景)` +
      绘制全部实例（禁用脏带）；归零稳定后恢复部分渲染（dsh commit df492ab：
      `is_scroll_offset_active` 门控 + `last_drawn_viewport_scroll_px` 基准）。
- [x] 2.2 单测：滚动激活/变化的决策表 + 全量路径像素语义 GPU 测试
      （`scroll_offset_active_decision_table` + `gpu_scroll_offset_full_redraw_has_no_stale_pixels`，
      后者断言无残留、边缘条带为背景）。
- [x] 2.3 `cargo test -p native --lib` 通过（551 passed, 0 failed，含既有渲染测试）。

## 3. 设备验证

- [x] 3.1 构建 APK 安装模拟器，生成滚动内容后 `input swipe`，修复后截图与修复前逐像素对比：
      底部条带无残影、滚动后画面完整。
- [x] 3.2 REPORT.md（像素证据 + 根因 + 修复对比）
      设备证据：`docs/verification/scroll-residual-tearing-device-evidence.md`
      （连续滚动 burst 8 帧逐像素 diff：底部条带无残影/无混合色；截图/像素数据齐备）。
      → 归档 change。