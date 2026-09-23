# fix-cjk-rendering — Tasks

- [x] 取证 CJK 字形渲染质量（发虚/大小不一致）：真机像素测量定位根因
- [x] 修复 CJK 字形水平 1.197x 拉伸（cell.wgsl scaled_x 改为 1:1 物理像素）
- [x] 真机验证：修复后 CJK 字形宽高与 FreeType 36.75px 参考完全一致（1.000x）
- [x] IME 中文输入链路验证（输入触发 / 首帧可见性 / 点击行为）
- [x] CJK 渲染性能说明（字形 atlas 缓存，稳态帧成本与西文一致）

## 取证摘要

- 设备：Pixel 6 emulator profile 1080x2400 @ 420dpi（raster_scale=2.625），zh-CN locale，
  系统字体 scale 1.0；主字体 DroidSansMono，CJK fallback NotoSansCJK-Regular.ttc index=2（SC）。
- 修复前 ink box（2-cell quad 内）：中 37x34 / 文 42x35 / 测 41x34 / 试 41x34，
  FreeType 参考 36.75px 无 hint：31x35 / 35x35 / 34x34 / 34x35 → 宽度比率 ≈1.19–1.21。
  ASCII 'c' 17x21 与参考一致（1.000x）。
- 根因：cell.wgsl `scaled_x = cell_px.x * glyph_adv_px / quad_size.x`。等宽西文
  advance == cell（22px）→ 比率 1.0 不显形；CJK quad=44px（2 cell）、advance≈36.75px
  → 每字形水平拉伸 44/36.75=1.197x。Nearest 采样下笔画增粗、比例失真（发虚、与英文
  宽高不协调）；垂直方向无缩放正常。
- 修复：`scaled_x = cell_px.x`，atlas 位图按物理像素 1:1 采样，`in_glyph` 按位图尺寸裁剪
  （quad 剩余区域走背景分支）。等宽西文输出不变；CJK 恢复 1:1。
- 修复后测量：中 31x34 / 文 35x35 / 测 34x34 / 试 35x34，与 FreeType 参考 1.000x 一致。
