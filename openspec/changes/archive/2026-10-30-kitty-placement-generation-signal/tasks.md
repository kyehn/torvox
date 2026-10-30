# Tasks

## 1. 放置缓存改用上游放置生成戳

- [x] `KittyPlacementKeys` 去掉滚动偏移与回滚长度，纳入放置生成戳
- [x] `kitty_cache_invalidation` 以放置生成戳变化驱动重查
- [x] 删除视口差量平移通道（近似帧标志、上一帧视口键、平移分支）

## 2. 规范

- [x] `kitty-graphics` 改写「放置缓存按内容变化失效」
- [x] `kitty-graphics` 改写「清屏必须立即撤下图像放置并清除像素」
- [x] `kitty-graphics` 删除「视口滚动只平移放置缓存」

## 3. 验证

- [x] VT 侧回归用例：ED2 走 scrollClear 时撤下放置
- [x] 失效判定用例按新语义重写
- [x] `cargo test --lib` 全绿
