# Proposal

## Why

真机 logcat 里两条与本仓直接相关的诊断暴露出两个独立缺陷：

1. `SLOW_CELL_BUILD build=128.857ms ... instances=1176`：同一帧的单元构建在
   毫秒级停顿上百毫秒。取证发现同一码位（U+23CE）被全库扫描了 9 次——**同一个
   码位每次出现都重扫一遍字体库**。
2. `W/native::android::ffi::render::frame]` 的慢帧诊断里，`render` 段动辄
   30~136ms：着色器编译与 2048×2048 图集分配**发生在渲染循环内**，第一帧必须
   等它们做完才画得出东西。

两者的共同点是「代价放在了不该放的位置」：解析结论本该只算一次，却在条件不
成立时被反复重算；管线建立本该在挂载路径上完成，却在每帧的兜底路径上被反复触发。

## What Changes

- **解析结论缓存的读取不再以样式或回退层有无为条件**。缓存记的是「这个码位由
  哪个面的哪个字形解析」这一**结论**，与粗体/斜体无关（粗斜是光栅化后的遮罩后
  处理，用的还是同一个字形号），也与「装没装 CJK 回退层」无关（回退层只是解析
  途径之一）。原先读取挂在 `has_cjk_fallback && !synthesized` 上，而写入没有这两
  个条件——设备上「主字体已覆盖 CJK」时回退层为空、全库扫描是唯一出路，扫描结论
  只写进表却不被读，于是每出现一次重扫一遍全库（`find_glyph_anywhere` 要遍历
  fontdb 全部 face 并逐个 `with_face_data` 重建 charmap）。
- **伪字形不入缓存**：字形号 0（`.notdef`，非空的空心方框）与私用区码位一律不记，
  否则「主字体没有这个码位」会被当成已解析，整条回退链被跳过。
- **管线建立移出渲染循环**：着色器编译与图集分配改由挂载路径（`attach_window_inner`，
  不占 UI 线程）主动完成，渲染路径只做兜底补建；建立耗时记入日志，代价仍然可见。
- **图集重传只覆盖缓存字形**：管线重建后 GPU 侧图集纹理是新建的空纹理，而 CPU 侧
  字形缓存全命中——没有任何字形被重新 blit，终端只剩背景、无文字且不自愈。原实现
  整幅标脏重传 16MB 图集，改为只取缓存字形的包围盒（实际用到通常只有几十 KB）。

## Impact

- 规格：
  - `cjk-rendering` ADDED「解析结论缓存的读取不得以样式或回退层为条件」。
  - `render-stability` ADDED「管线建立不得发生在渲染循环内」「图集重传只覆盖缓存
    字形」。
- 代码：
  - `native/src/render/font/pipeline.rs`、`native/src/render/font/cjk.rs`（解析结论
    缓存的读写条件、伪字形拒绝、全库扫描计数）。
  - `native/src/render/font/atlas.rs`、`native/src/render/context/renderer.rs`、
    `native/src/android/ffi/render/{frame,window}.rs`（管线建立位置与图集脏区）。
- 基准：`native/benches/{cell_builder,cjk_resolve}.rs` 的上限断言按修复后的实测
  重校（每格全库扫描的退化会被这两条断言直接判红）。
