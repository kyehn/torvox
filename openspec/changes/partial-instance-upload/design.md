# 设计

## 现状

`render_frame_with_plan` 在部分帧下：

1. 构建 `cpu_instances`（含干净行缓存实例）。
2. 上传整份 `instances` 到 `instance_buffer`（`clear_end` 之后）。
3. 只绘制 `dirty_bands` 对应的实例切片。

因此上传字节数与全量帧相同，只是绘制减少。在 24×80 网格、60fps 下，约 1920 个实例
× 64 字节 = 122KB/帧；部分帧若只脏 1 行，实际需要上传不到 1KB，浪费约 120KB/帧。

## 改进方案

### 上传范围

部分帧时，对每个 `DirtyBand`：

- 计算其实例切片 `[band.instance_start, band.instance_end)` 在 `instances` 中的字节范围。
- 按 `clear_end + band.instance_start * stride` 的偏移写入对应切片。
- 干净行的槽位保留上一帧内容（不写入），反正不会被绘制。

全量帧保持现有全量上传。

### 容量保证

`ensure_instance_capacity` 已按 `draw_end`（`clear_end + instances.len() * stride`）扩容，
部分帧下 `instances.len()` 仍为全量行数，故容量足够；只写脏带切片不会越界。

### 与累加器的关系

累加器（`accumulator_view`）在部分帧下以 `Load` 叠加；脏带清除实例已覆盖脏带像素矩形。
上传脏带实例即可，干净行的实例槽位 stale 不影响，因为它们不会被 draw。

### 上传顺序

保持清除实例先上传（offset 0），再上传各脏带实例（各自 offset）。各脏带之间无重叠，
可按 `bands` 顺序依次 `write_buffer`。

## 测试

- 表驱动：给定 `dirty_bands` 与 `instances` 长度，上传偏移与长度正确。
- 边界：空脏带列表时不上传实例；`instance_start == instance_end` 的带跳过。
- 容量：部分帧下 `instance_buffer` 容量仍按全量计算，不因上传切片而缩小。
