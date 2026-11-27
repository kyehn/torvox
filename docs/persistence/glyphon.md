# glyphon 深度审查（0.12.0 / HEAD `49dc8f7bafa8091f4d71521fd62ee6f647b556f5`）

## 范围与基线

- 上游：`https://github.com/grovesNL/glyphon`，审查时 HEAD 为 `49dc8f7bafa8091f4d71521fd62ee6f647b556f5`，`Cargo.toml` 版本 `0.12.0`。
- 许可：MIT OR Apache-2.0 OR Zlib；依赖 `wgpu 30`、`cosmic-text 0.19`、`etagere 0.3`、`lru 0.16.2`、`rustc-hash 2.1.1`。
- 实读文件：`Cargo.toml`、`README.md`、`src/lib.rs`、`src/cache.rs`、`src/custom_glyph.rs`、`src/error.rs`、`src/text_atlas.rs`、`src/text_render.rs`、`src/shader.wgsl`、`src/viewport.rs`、`examples/{hello-world,text-sizes,custom-glyphs}.rs`、`benches/{prepare,state}.rs`；另用 `git show 0.7.0:...` 对照 zelland 钉住的版本。
- 本仓定位：仅作参考留档。本仓终端渲染走 `native/src/render` 的 wgpu 直绘 + 自有字形图集，不新增 glyphon 依赖，也不改受保护的 `Cargo.toml`。

## 1. 分层方式与输入顺序

| 层 | 载体 | 职责 |
| --- | --- | --- |
| 公共 API | `src/lib.rs` | 导出 `Cache`、`Viewport`、`TextRenderer`、`TextAtlas`、`TextArea`、`TextBounds`、`rasterize_with_resvg`（后者需 example feature） |
| 共享 GPU 资源 | `src/cache.rs::Cache` / `InnerCache` | `Arc<Inner>` 持有 sampler、shader、已建渲染管线；多 renderer 共用，按 `(format, multisample, depth_stencil)` 线性查表 |
| 图集 | `src/text_atlas.rs::TextAtlas` / `InnerAtlas` | 两套 `BucketedAtlasAllocator` + `LruCache`（mask 单通道 / color 多通道），含增长、裁剪、LRU 淘汰 |
| 整形与顶点 | `src/text_render.rs::TextRenderer` | 把 `cosmic_text::Buffer` 的布局转成 GPU 顶点，管理 uniform buffer 与 oversized buffer |
| 着色器 | `src/shader.wgsl` | 顶点解包、颜色空间转换、纹理采样、alpha 混合 |
| 视口 | `src/viewport.rs::Viewport` | 把 `Resolution` 写入 uniform buffer，供 shader 换算 NDC |

一帧的输入顺序（`TextRenderer::prepare_with_depth_and_custom`）：

1. `viewport.update(queue, resolution)` 上传分辨率 uniform；
2. 清空 `glyph_vertices`；
3. 每个 `TextArea` 先处理 `custom_glyphs`（走自定义 rasterizer，按 `metadata -> depth` 回调决定深度）；
4. 再以 `layout_runs().skip_while(!visible).take_while(visible)` 遍历可见行与字形；
5. 每个字形先查 mask 图集，未命中再查 color 图集；未命中则 `get_image_uncached` 栅格化、`try_allocate` 分配槽位、写纹理；
6. 裁剪到 `GlyphBounds` 后压入 `GlyphToRender`；
7. 用户另开 render pass 调 `render(atlas, viewport, pass)`，内部 upload 顶点并 draw。

## 2. 关键机制

### 2.1 缓存与淘汰（`cache.rs` / `text_atlas.rs`）

- 键：`GlyphonCacheKey { font_id, glyph_id, subpixel_bin, content_type, custom_id }`；自定义字形用 `CustomGlyphCacheKey { id, width, height, subpixel_bin }`，颜色不进键（颜色在顶点里）。
- `InnerAtlas::generation` 是帧号；`trim()` 自增（wrapping），命中时把 `GlyphDetails::last_used` 设为当前代。`try_allocate` 沿 `peek_lru()` 淘汰，跳过 `last_used == generation` 的"本帧在用"项。这是 0.12 对 0.7 的重写：0.7 用 `HashSet<GlyphonCacheKey>` 记 `glyphs_in_use`，跨帧状态多一处同步点。
- `grow()` 把图集翻倍后，对全部缓存键重新 `get_image_uncached` 并 `write_texture` 回新纹理；分配坐标不变，重传的是像素。
- 双图集初值 256×256，上限取 `device.limits().max_texture_dimension_2d`；都放不下时返回 `PrepareError::AtlasFull`。

### 2.2 几何与着色（`text_render.rs::prepare_glyph` / `shader.wgsl`）

- `GlyphToRender` 把 `dim`、`uv` 各打包成两个 `u16`；WGSL 以 `Uint32` 读取再按低/高 16 位拆包，隐含小端布局。
- `ColorMode::Accurate` 用 `Rgba8UnormSrgb` 颜色图集，shader 对文字颜色逐通道 `pow(x, 2.2)` 做 sRGB→线性，在线性空间混合；`ColorMode::Web` 用 `Rgba8Unorm` 并跳过转换以贴近浏览器。
- sampler 为 Nearest，字形按物理像素尺寸栅格化后 1:1 放置；不做双线性过滤。
- 裁剪：`TextBounds` 先与 `[0, resolution]` 求交，再用 `right_shift`/`bottom_shift` 同步扣减 `atlas_x`/`atlas_y`，保证 UV 与裁剪后矩形一致。

### 2.3 自定义字形（`custom_glyph.rs`）

- 调用方实现 `RasterizeCustomGlyph`，返回 `RasterizedCustomGlyph`；`custom_glyphs` 实例携带 `id`、`width/height`、`subpixel_bin`、`metadata`，先于普通文本处理，方便用同一图集承载图片（本仓 kitty 图形关心这条路径）。
- 关键契约：rasterizer 必须确定性。同一 key 在图集增长时会被重新栅格化；若此刻返回 `None` 或像素尺寸变化，`grow()` 的 `unwrap()` / 写入长度会失败。上游 README/API 未把这条写成显式约束。
- 无失效接口：图片内容改变而 `id`、尺寸不变时，缓存不会刷新；动画/替换内容必须换 `id`，否则旧像素会一直被复用。

## 3. 缺陷、边界与未验证项

| 严重度 | 结论 | 依据 |
| --- | --- | --- |
| 高 | **仓库没有任何单元测试**：`src/`、`examples/`、`benches/` 中 `#[test]` 与 `#[cfg(test)]` 零命中；正确性依赖 examples 与下游集成 | 对克隆树执行 `grep -rn '#\[test\]\|#\[cfg(test)\]'` 为空 |
| 中 | `RenderError::{RemovedFromAtlas, ScreenResolutionChanged}` 在 `render()` 中从不返回；`prepare` 与 `render` 的分辨率/图集一致性完全由调用方保证，错配只会画错而不会报错 | `src/error.rs` 与 `src/text_render.rs::render` |
| 中 | `grow()` 对缓存键逐个 `get_image_uncached(...).unwrap()`；自定义 rasterizer 一旦不稳定或字体后端偶发返回 `None` 即 panic；同时图集翻倍会造成单帧全量重栅格化尖峰 | `src/text_atlas.rs::InnerAtlas::grow` |
| 中 | `image.placement.width/height as u16`、`top/left as i16` 均无 checked cast；超大字号下可能回绕后写出错误尺寸/越界 | `src/text_render.rs::prepare_glyph` |
| 中 | `SwashContent::SubpixelMask` 被当作普通 Mask 处理（content_type 仅 Mask/Color），不提供 LCD 子像素抗锯齿 | `src/text_render.rs::GetGlyphImageResult`、`src/shader.wgsl` |
| 低 | sRGB 转换在顶点着色器逐通道 `pow`，每个字形 4 个顶点都要算；CJK 密集行数千字形时是可观的 ALU 成本，可用查表或预乘颜色替代 | `src/shader.wgsl::vs_main` |
| 低 | 顶点 UV/dim 按小端拆包；大端平台会错位（Android/arm64 为小端，实际无碍） | `src/text_render.rs::GlyphToRender` 与 WGSL `unpack` |
| 低 | `generation` 用 `wrapping_add`，极端长会话回绕时本帧标记可能与旧值同号 | `src/text_atlas.rs::trim` |
| 边界 | 没有 damage 追踪：每次 `prepare` 仍遍历全部可见 run；行级脏跟踪必须由调用方自己维护（本仓已有） | `src/text_render.rs::prepare_with_depth_and_custom` |

## 4. 版本漂移：zelland 钉的是 0.7.0

zelland 0.2.3 的 `src-tauri/Cargo.toml` 使用 `glyphon 0.7` + `wgpu 23` + `cosmic-text 0.12`。`0.7.0..HEAD` 的相关差异：

| 变更 | 0.7.0 状态 | 对 zelland 的影响 |
| --- | --- | --- |
| `fa9a3f8 Fix the is_run_visible calculation` | `is_run_visible` 用 `run.line_top`/`line_height` 未乘 `text_area.scale` | zelland 传入 `scale = 1.0`，此例不受影响；若后续按 `cell_height` 缩放则可见行判断会偏 |
| `9dd9376 Fix: Y-axis clipping uses resolution.height instead of width` | 0.7 已按 `height` 求交 | 不是 zelland 的缺陷 |
| `a21a2fd Fix slice out of bounds` | 0.7 写法为 `buffer.slice(..)[..contents.len()]` | 0.7 写法本身安全 |
| `5698d9e Clamp max against min for large glyphs` | 0.7 无 clamp | 极端 bounds 下可出现负区间；zelland 用 `(0,0)-(width,height)`，风险低 |
| `49dc8f7 Switch to generation-based usage tracker` | 0.7 用 `HashSet` 记在用量 | 0.7 跨帧状态更多；zelland 每帧 `prepare` 后调 `atlas.trim()`，未踩到已知失效场景 |
| 依赖升级 | wgpu 23→30、cosmic-text 0.12→0.19、etagere 0.2.10→0.3 | 0.7 无法直接吸收新整形/性能修正；升级需连带改受保护清单外的多个 crate 版本 |

## 5. 基准体系与复算方式

- 上游基准：`benches/prepare.rs`（构造缓冲后测 `prepare`，区分"新字形栅格化"与"缓存命中"）、`benches/state.rs`（状态/管线构建）；`cargo bench` 可复算，本仓不引入。
- 复算命令：
  - `git clone https://github.com/grovesNL/glyphon /tmp/glyphon && git -C /tmp/glyphon rev-parse HEAD`
  - `git -C /tmp/glyphon grep -n '#\[test\]\|#\[cfg(test)\]' HEAD -- src examples benches`
  - `git -C /tmp/glyphon show 0.7.0:src/text_render.rs`
- 生成物复现校验：本留档所有结论以固定 commit 的源码为准；上游移动 HEAD 后需按上述命令重新取基线，不能沿用本文件的行号。

## 6. 处置结论

| 项 | 判定 | 依据 |
| --- | --- | --- |
| 引入 glyphon 作本仓文本渲染 | 不采纳：本仓已有 wgpu 直绘与自有字形图集，`Cargo.toml` 受保护且 `DESIGN.md` 未声明该依赖 | `docs/specification/DESIGN.md`、`docs/specification/PROHIBITED.md` |
| 双图集 + LRU + 帧代淘汰算法 | 仅作结构参照，不新增抽象 | `src/text_atlas.rs::try_allocate`；本仓 `native/src/render/font` 已承担同职责 |
| 自定义字形作为图片进入同一图集 | 仅参照其请求结构（`id`/尺寸/subpixel/scale）与"内容变更须换 id"约束 | `src/custom_glyph.rs::RasterizeCustomGlyph` |
| `ColorMode::Web` 的浏览器色准取舍 | 不采纳：本仓主题色由自身规范定义，不做浏览器对齐 | `openspec/specs/theme-configuration/spec.md` |
| 上游零单测、调试帧、错误枚举空转等做法 | 不得反向采纳 | `src/error.rs`、`src/text_render.rs`、`benches/` |

跳过：未运行 `cargo bench`（需 GPU 与图形环境）；风险是性能判断仅来自代码路径分析，无实测数字。
