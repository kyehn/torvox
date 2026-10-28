# Proposal

## Why

上一轮把图集预算校验放进了**打包**路径（`pack_atlas`），但**复用**路径
（`layout_entries`）仍然只算布局、不看预算，于是两条路径对「这份布局能不能出图」
给出不同答案。

渲染线程的复用分支先取 `layout_entries` 的成功结果，再拿布局尺寸与已上传图集尺寸
比对：超预算布局永远打包失败、已上传图集尺寸恒为 0，两者永不相等，于是每一帧都
重走整包重传路径——每帧重算布局、每帧尝试打包、每帧写一条警告，图像却永远不会
出现。滚动时该分支逐帧执行，警告按帧率刷进 logcat，违反 `logcat-hygiene` 的
「逐帧诊断必须限频」。

根因不在某一处判断写错，而在**同一条不变量被两个调用点各实现一遍**：只改打包侧
就必然留下分叉。

## What Changes

- 抽出唯一判据 `atlas_byte_size(width, height)`：受检跨步 × 高，且不超过
  `MAX_ATLAS_BYTES`；打包与复用两条路径都必须经过它。
- `layout_entries` 在返回布局前应用该判据：无法打包的布局不得进入复用路径。
- 回归测试改为断言两条路径**同时**拒绝超预算布局（旧断言把分叉当成了契约）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `kitty-graphics`：补上「复用路径必须与打包共用同一预算判据」的要求，替换上一轮
  「超预算布局仍可算出布局本身」的场景。

## Impact

- `native/src/render/kitty.rs`：新增 `atlas_byte_size`，`pack_atlas` 与
  `layout_entries` 共用；测试断言更新
- 受影响的行为：超预算的 Kitty 图像不再每帧重试打包与刷日志；图像仍不显示
  （与上一轮一致），但代价降为一次布局计算
