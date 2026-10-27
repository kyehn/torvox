# Proposal

## Why

Kitty 图集把多幅图像并排写进同一张纹理，采样器是双线性。显示尺寸与源矩形不等时
（放大显示，或 `source_*` 子矩形裁剪），片元中心的纹素坐标会越过条目边界，双线性
过滤于是取到**邻条目**的纹素：左缘混入前一幅图的颜色；条目比图集矮时底缘混入图集
里的透明行，图像出现一道彩边或一条发虚的半透明边。

一屏多图（`lsix`、`icat`）或图像小于放置尺寸时必现：用户看到的是每幅图左缘一条
上一幅图颜色的竖条，与图像内容无关。

## What Changes

- 采样坐标按条目钳制到本条目的首末纹素中心：边缘取本条目自己的边缘纹素
  （「按条目 clamp-to-edge」），越界取样不再发生。
- 图集不再为每个条目留 1 像素沟槽：钳制之后沟槽无用，留着只会让图集变宽，
  并让 UV 口径多出一处分叉。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `kitty-graphics`：新增「图像边缘采样不得跨条目串色」要求。

## Impact

- `native/shaders/kitty_graphics.wgsl`：采样坐标按条目钳制（顶点阶段传出条目的
  纹素矩形，片元阶段钳制采样坐标）。
- `native/src/render/kitty.rs`：图集布局去掉沟槽列，条目紧挨着排。
- `native/src/render/pass/readback.rs`：回读脚手架补画 KGP 叠加层（仅测试脚手架，
  生产零调用）。
- `native/src/render/tests/cell_pipeline.rs`：新增 GPU 回归用例（相邻条目不得串色）。
