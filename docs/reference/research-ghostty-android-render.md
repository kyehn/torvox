# 深度研究：ghostty-android 渲染层 — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：<https://github.com/sylirre/ghostty-android-terminal>
> 前置：`research-ghostty-android.md`（选择系统，亲自精读）；本文补充**渲染层**（onDraw/drawRowText/kitty graphics/壁纸）

## 1. onDraw 分层（TerminalView.java:1509-1600，亲自精读）

```
onDraw:
1. snapshotSmooth（平滑滚动时取 viewport + 上方一行，:1514-1524）
2. canvas.drawColor(snapshot.defaultBg()) ← 主题背景
3. drawBackgroundImage(canvas) ← 壁纸（center-crop + alpha 叠加）
4. canvas.translate(0, offsetPx) ← 平滑滚动偏移
5. drawRowBackground（先所有背景 run，:1540-1543） ← 背景先画，字形不被覆盖
6. updateGraphics() + drawImages(canvas, true) ← kitty graphics z<0（背景上、文字下）
7. updateCursorBlink() + updateTextBlink() ← 光标/文字闪烁状态更新
8. drawCursor(canvas) ← 光标
9. drawRowText（逐行，:1548-1552） ← 文字
10. drawImages(canvas, false) ← kitty graphics z>=0（文字上，kitty 默认）
11. drawSizeOverlay + drawSelectionHandles + drawBellFlash
```

**torvox 对比**：torvox 的 wgpu 渲染在 shader 中处理背景/字形/光标（单 pass 实例化），z-order 由实例顺序决定。ghostty-android 的分层顺序（背景→图→光标→文字→图）是 CPU 渲染的必要顺序，torvox 的 GPU 实例化天然满足。**bell flash**（drawBellFlash :1580-1593：白色 alpha 96*phase 矩形 + postInvalidateDelayed(16)）——torvox 有 flashBell 等价（已实现）。

## 2. drawRowText run 批处理（TerminalView.java:1800-1870，亲自精读）

**核心算法**（:1803-1835）：

- 同一 fg/attr 的 ASCII 连续字形合并为 run → 一次 `canvas.drawText(runText)`（:1832）
- **run 中断条件**（:1819-1825）：cp==0（空单元格）/ fg 或 attr 变化 / ATTR_WIDE（宽字形前进 2 格）/ grapheme cluster（组合字符必须整簇交给字体整形）/ **cp > 0x7F（非 ASCII 可能走 fallback 字体、advance 不同）**
- 下划线独立绘制（drawUnderline，run 覆盖 [runStart, x) 全宽）
- **blink 关闭阶段**（:1811-1813）：blink 单元格 cp 置 0——字形消失但背景保留
- 宽字符/簇/非 ASCII 单独 drawText 并"钉"在网格列（:1837-1866）——避免 fallback 字体 advance 推挤后续单元格

**torvox 对比**：torvox 的 GPU 渲染每个 CellInstance 独立 quad + swash 栅格化——无 run 批处理需求。但"非 ASCII 走 fallback"的坑（torvox 的 cjk.rs 处理）与"宽字符 2 格 advance"（torvox is_wide + grapheme_extra 堆叠）已对应实现。blink 文字消失背景保留——torvox 的 cursor blink 处理类似。

## 3. kitty graphics（:190-196, :955, :1621-1622）

- `imageCache: Map<Integer, Bitmap>` 按 image id 缓存解码位图（GFX_STRIDE ints）
- `imagePaint = new Paint(Paint.FILTER_BITMAP_FLAG)`
- `clearImageCache()`：终端重置/换终端时清缓存
- 两档 z-order：z<0 在背景上文字下，z>=0 在文字上（kitty 默认）

**torvox 对比**：torvox 的 libghostty-vt 内部处理 kitty graphics（CellData 层面不暴露图像），渲染由 ghostty 管理。ghostty-android 是 Java 侧手动解码——torvox 不需要（VT 引擎已处理）。

## 4. 壁纸（:885-907, drawBackgroundImage :1595-1640）

- `setBackgroundImage(Bitmap)`：**take ownership**（替代旧 bitmap）
- center-crop（短轴填满、长轴均匀裁剪）+ `backgroundImageAlpha` 叠加在主题背景上（低透明度保文字可读）
- 无图像/无尺寸时 no-op

**torvox 对比**：torvox 的 setBackgroundImage（Rust set_bg_image 纹理 + shader 采样）等价功能。ghostty-android 的 center-crop 语义可对照（torvox 是否 center-crop？待查）。

## 5. 平滑滚动（:1514-1524, drawRowBackground/Text 的 srcRow/top 分离）

- `pixelScrollOffset > 0` 时 `snapshotSmooth(snapshot, aboveSnapshot)` 取 viewport + 上方一行
- 网格绘制向下偏移 offsetPx，顶部露出条由"行 -1"的 aboveSnapshot 填充
- `drawRowText(canvas, snapshot, y, sc, y*cellHeight)`——**源行与目标像素分离**（smoothScroll 路径）

**torvox 对比**：torvox 渲染无子行像素偏移（整行滚动）。这是 CPU 渲染的平滑度技巧，GPU 终端通常不做（ghostty 桌面版也不做）。低优先。

## 6. 结论

ghostty-android 渲染层是 CPU Canvas 渲染的精细范本（run 批处理、fallback 隔离、z-order 分层、平滑滚动）。torvox 的 GPU 渲染在架构上无需这些技巧（shader 天然处理），但以下可对照核查：**壁纸 center-crop 语义**、**blink 时文字消失背景保留**、**bell flash 渐变**。kitty graphics 由 libghostty-vt 内部处理（torvox 无需 Java 侧实现）。
