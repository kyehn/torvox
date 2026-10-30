# kitty 放置缓存改用上游放置生成戳

## Why

用户报障：`kitten icat` 显示的图像在终端里 `clear` 一次清不掉，要 clear 两次才消失。

复核后定位到一条确定的失效判定缺陷：**渲染侧把「视口平移」当成了「放置没变」**。

上游 `eraseDisplay(.complete)` 在主屏且最后一行非空内容是提示符时，先 `scrollClear()`
（把提示符以上的旧行滚进回滚），再撤下图像放置。此时：

1. 图像存储生成戳不变——图像本身还留在回滚里，没有被删除；
2. 可见占位符布局签名不变——`kitten icat` 走 APC 直接放置，视口里没有占位符单元；
3. 视口顶行变了——回滚长度增长而滚动偏移仍为 0。

旧实现按「视口顶行差量平移已有放置帧」处理这一路，既不重查放置也不清像素，撤下的
图像就留在累加器里，直到下一次重查（用户体感：clear 一下不清，两下才清）。

平移近似帧这条通道本身也与上游契约冲突：上游 `RenderState::image_placements_generation`
写明「放置被增删、移动、随滚动或重排都会推进它，值变了就读一次列表」。视口滚动推进
它，撤下放置也推进它——两者本该共用同一个信号。

## What Changes

- 放置缓存键去掉「滚动偏移 + 回滚长度」这对视口键，改为纳入上游的放置生成戳。
- 删除视口差量平移通道：平移近似帧、停稳补查、滚动中不平移等状态一并移除。
- 放置生成戳一变就重查放置并整帧重绘，撤下、滚动、重排共用一条路径。
- `kitty-graphics` 规范改写「放置缓存按内容变化失效」与「清屏必须立即撤下图像放置并
  清除像素」，并删除「视口滚动只平移放置缓存」。

## Impact

- `native/src/android/ffi.rs`：放置键结构、失效判定、渲染态字段。
- `native/src/android/ffi/render/frame.rs`：放置键构造与放置帧应用。
- `native/src/android/ffi/kitty_placement_keys_tests.rs`：失效判定用例。
- `native/src/terminal/ghostty_terminal/tests/selection_search.rs`：VT 侧回归用例。
- 规范：`kitty-graphics`。
- 行为：ED2 清屏后图像立即消失；滚动时按上游契约重查放置。
