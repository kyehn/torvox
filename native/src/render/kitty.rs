//! Kitty 图像放置的图集组装与 GPU 实例构建。
//!
//! VT 线程采集[`KittyPlacementFrame`]（含 RGBA8 像素与视口几何），本模块在渲染线程完成两步：
//! 1. 横条带图集打包：各源子矩形并排写入单张 RGBA 图集（单图全图常见情形零拷贝借用）。
//! 2. 实例构建：视口网格坐标映射为像素 quad（与单元格 quad 同约定：左上原点，Y 向下），
//!    UV 归一化到图集尺寸（KGP 着色器直接采样 UV，不做像素换算）。

use std::borrow::Cow;

use crate::render::KittyGraphicsInstance;
use crate::terminal::ghostty_terminal::KittyPlacementFrame;

/// 图集条目：放置在图集中的像素偏移（实例 UV 归一化用）。
pub(crate) struct AtlasEntry {
    atlas_x: u32,
    atlas_y: u32,
}

/// 图集字节预算：取图像存储上限（KGP_STORAGE_LIMIT）。
///
/// 图集是图像像素的**重排副本**：条带布局把每个放置的源矩形并排铺开，同一幅图像
/// 被放置 N 次就在图集里铺 N 份，故图集可以远大于图像存储。PTY 同时控制图像尺寸
/// 与放置数量，两者相乘即可把图集撑到数十 GB——先分配、再由 GPU 侧尺寸检查拒绝
/// 为时已晚：分配失败会让进程 abort，且没有任何日志。故预算必须在分配**之前**。
const MAX_ATLAS_BYTES: usize = crate::terminal::ghostty_terminal::KGP_STORAGE_LIMIT as usize;

/// 条带布局：各有效放置的钳制源矩形与图集偏移（与 frames 等长，无效帧占位零矩形）。
/// 单个无效帧被跳过而非拖垮整张图集；无有效帧时返回 None。
struct StripLayout {
    width: u32,
    height: u32,
    /// 与 frames 等长：有效帧为图集偏移，无效帧为 None。
    entries: Vec<Option<AtlasEntry>>,
    /// 与 frames 等长：有效帧为钳制源矩形，无效帧为零矩形。
    clamped: Vec<(u32, u32, u32, u32)>,
}

fn layout_strip(frames: &[KittyPlacementFrame]) -> Option<StripLayout> {
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let mut entries: Vec<Option<AtlasEntry>> = Vec::with_capacity(frames.len());
    let mut clamped: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(frames.len());
    for frame in frames {
        let rect = clamp_source(frame, frame.image_width, frame.image_height);
        if rect.2 == 0 || rect.3 == 0 {
            entries.push(None);
            clamped.push((0, 0, 0, 0));
            continue;
        }
        entries.push(Some(AtlasEntry {
            atlas_x: width,
            atlas_y: 0,
        }));
        clamped.push(rect);
        // 条目紧挨着排：采样越界由着色器按条目钳制（见 `kitty_graphics.wgsl` 的
        // 「按条目 clamp-to-edge」），图集里不必再为每个条目留沟槽像素。
        // 受检加法：饱和会把「超限」变成一个看似正常的巨大宽度，最终以分配器
        // abort 收场（PTY 可控的图像尺寸与放置数量即可触发）。
        let Some(next_width) = width.checked_add(rect.2) else {
            log::error!("kitty: atlas width overflows u32; dropping image");
            return None;
        };
        width = next_width;
        height = height.max(rect.3);
    }
    if width == 0 || height == 0 {
        return None;
    }
    Some(StripLayout {
        width,
        height,
        entries,
        clamped,
    })
}

/// 打包产物：（图集 RGBA，宽，高，与 frames 等长的条目表）。
///
/// 单图全图时图集即该帧的像素本身，零拷贝借用即可；横条带布局才需新分配。
/// 调用方只读（见 [`pack_and_build`] 与 `repack_kitty_atlas`），故借用可行。
type PackedAtlas<'a> = (Cow<'a, [u8]>, u32, u32, Vec<Option<AtlasEntry>>);

/// 图集字节数：跨步 `width * 4` 乘 `height`，受检且不超过 [`MAX_ATLAS_BYTES`]。
///
/// [`pack_atlas`] 与 [`layout_entries`] 必须共用同一判据：一方认为可打包、另一方
/// 拒绝时，复用路径会拿这份布局与已上传图集（永远是 0）比对，于是每帧重试打包、
/// 每帧刷一条警告，图像却永远不会出现。
fn atlas_byte_size(width: u32, height: u32) -> Option<usize> {
    let stride = width.checked_mul(4)?;
    let bytes = (stride as usize).checked_mul(height as usize)?;
    (bytes <= MAX_ATLAS_BYTES).then_some(bytes)
}

/// 组装图集：源矩形钳制到图像边界；无有效帧返回 None（调用方清空实例与图集）。
/// 输入总量由上游存储上限约束（KGP_STORAGE_LIMIT=64MiB，见 types.rs）；
/// 横条带布局存在矩形空洞，故输出像素数可大于输入面积和——图集字节数用受检算术
/// 计算，超出预算（[`MAX_ATLAS_BYTES`]）时明确记日志后放弃该图集，绝不先分配。
fn pack_atlas(frames: &[KittyPlacementFrame]) -> Option<PackedAtlas<'_>> {
    if frames.is_empty() {
        return None;
    }
    // 单图且全图显示：图集与帧像素同一份，借用即可（KGP 上限 64MiB，克隆即翻倍峰值）。
    if frames.len() == 1 {
        let frame = &frames[0];
        let (clamped_x, clamped_y, clamped_width, clamped_height) =
            clamp_source(frame, frame.image_width, frame.image_height);
        if clamped_width == 0 || clamped_height == 0 {
            return None;
        }
        if clamped_x == 0
            && clamped_y == 0
            && clamped_width == frame.image_width
            && clamped_height == frame.image_height
        {
            return Some((
                Cow::Borrowed(frame.image_rgba.as_ref()),
                frame.image_width,
                frame.image_height,
                vec![Some(AtlasEntry {
                    atlas_x: 0,
                    atlas_y: 0,
                })],
            ));
        }
    }
    let layout = layout_strip(frames)?;
    // 预算检查必须在分配**之前**：条带布局下同一幅图像被放置 N 次就会在图集里
    // 铺 N 份，PTY 控住图像尺寸与放置数量即可让图集达到数十 GB。分配失败会让
    // 进程 abort（无日志），且这份缓冲注定传不上 GPU。
    let Some(atlas_len) = atlas_byte_size(layout.width, layout.height) else {
        log::warn!(
            "kitty: atlas {}x{} overflows or exceeds the {MAX_ATLAS_BYTES}-byte budget; dropping image",
            layout.width,
            layout.height
        );
        return None;
    };
    let mut atlas = vec![0u8; atlas_len];
    for (index, frame) in frames.iter().enumerate() {
        let (Some(entry), (source_x, source_y, source_width, source_height)) =
            (&layout.entries[index], layout.clamped[index])
        else {
            continue;
        };
        copy_sub_rect(
            frame.image_rgba.as_ref(),
            frame.image_width,
            &mut atlas,
            layout.width,
            entry.atlas_x,
            0,
            source_x,
            source_y,
            source_width,
            source_height,
        );
    }
    Some((
        Cow::Owned(atlas),
        layout.width,
        layout.height,
        layout.entries,
    ))
}

/// 源矩形钳制到图像边界（防御上游行为漂移，避免越界 panic）。
fn clamp_source(
    frame: &KittyPlacementFrame,
    image_width: u32,
    image_height: u32,
) -> (u32, u32, u32, u32) {
    let source_x = frame.source_x.min(image_width);
    let source_y = frame.source_y.min(image_height);
    let source_width = frame.source_width.min(image_width.saturating_sub(source_x));
    let source_height = frame
        .source_height
        .min(image_height.saturating_sub(source_y));
    (source_x, source_y, source_width, source_height)
}

/// 子矩形复制（RGBA8，逐行 memcpy）。
// 标准 blit 签名（源矩形+目标矩形），单调用点，成组无收益。
fn copy_sub_rect(
    source: &[u8],
    source_width: u32,
    destination: &mut [u8],
    destination_width: u32,
    destination_x: u32,
    destination_y: u32,
    source_x: u32,
    source_y: u32,
    width: u32,
    height: u32,
) {
    let source_stride = source_width as usize * 4;
    let destination_stride = destination_width as usize * 4;
    let row_bytes = width as usize * 4;
    for row in 0..height as usize {
        let source_offset = (source_y as usize + row) * source_stride + source_x as usize * 4;
        let destination_offset =
            (destination_y as usize + row) * destination_stride + destination_x as usize * 4;
        // 越界即图像像素缓冲短于上游声明的尺寸（协议行为漂移）。静默 break 会把图集留成
        // 半截黑条，用户只看到「kitty 协议坏了」而无从归因，故出声后再停。
        let (Some(source_row), Some(destination_row)) = (
            source.get(source_offset..source_offset + row_bytes),
            destination.get_mut(destination_offset..destination_offset + row_bytes),
        ) else {
            log::error!(
                "KGP blit out of range at row {row} (source len {}, destination len {}, row bytes {row_bytes})",
                source.len(),
                destination.len(),
            );
            break;
        };
        destination_row.copy_from_slice(source_row);
    }
}

/// 构建 KGP 实例：视口网格坐标映射为像素 quad，UV 归一化。
/// 屏外剔除由上游 `viewport_visible` 在采集侧完成，本函数仅跳过零尺寸/
/// 零钳制条目；顶部滚出（负行）保留，由 GPU 裁剪（上游 viewport_pos 语义）。
/// entries 须与 frames 等长（`layout_entries` 产出，无效帧为 None）。
pub(crate) fn build_kitty_instances(
    frames: &[KittyPlacementFrame],
    atlas_width: u32,
    atlas_height: u32,
    entries: &[Option<AtlasEntry>],
    grid_cell_width: f32,
    grid_cell_height: f32,
) -> Vec<KittyGraphicsInstance> {
    if atlas_width == 0 || atlas_height == 0 || grid_cell_width <= 0.0 || grid_cell_height <= 0.0 {
        return Vec::new();
    }
    let atlas_width_float = atlas_width as f32;
    let atlas_height_float = atlas_height as f32;
    frames
        .iter()
        .zip(entries.iter())
        .filter_map(|(frame, entry)| {
            let entry = entry.as_ref()?;
            if frame.pixel_width == 0 || frame.pixel_height == 0 {
                return None;
            }
            let quad_origin = [
                frame.viewport_col as f32 * grid_cell_width + frame.cell_offset_x as f32,
                frame.viewport_row as f32 * grid_cell_height + frame.cell_offset_y as f32,
            ];
            let (_, _, clamped_width, clamped_height) =
                clamp_source(frame, frame.image_width, frame.image_height);
            if clamped_width == 0 || clamped_height == 0 {
                return None;
            }
            Some(KittyGraphicsInstance::new(
                quad_origin,
                [frame.pixel_width as f32, frame.pixel_height as f32],
                [
                    entry.atlas_x as f32 / atlas_width_float,
                    entry.atlas_y as f32 / atlas_height_float,
                ],
                [
                    clamped_width as f32 / atlas_width_float,
                    clamped_height as f32 / atlas_height_float,
                ],
                1.0,
            ))
        })
        .collect()
}

/// 无像素拷贝的布局计算（图集未变时滚动/缩放只重建实例）。
/// 返回（图集宽，高，与 frames 等长的条目表）。
/// 与 [`pack_atlas`] 共用预算判据：无法打包的布局也不得进入复用路径。
pub(crate) fn layout_entries(
    frames: &[KittyPlacementFrame],
) -> Option<(u32, u32, Vec<Option<AtlasEntry>>)> {
    let layout = layout_strip(frames)?;
    atlas_byte_size(layout.width, layout.height)?;
    Some((layout.width, layout.height, layout.entries))
}

/// 打包产物：（图集 RGBA，宽，高，实例）。
type AtlasInstances<'a> = (Cow<'a, [u8]>, u32, u32, Vec<KittyGraphicsInstance>);

/// 一站式：打包图集 + 构建实例（FFI 渲染线程入口）。
/// 无可见放置时返回 None（调用方清空实例与图集）。
pub fn pack_and_build<'a>(
    frames: &'a [KittyPlacementFrame],
    grid_cell_width: f32,
    grid_cell_height: f32,
) -> Option<AtlasInstances<'a>> {
    let (atlas, atlas_width, atlas_height, entries) = pack_atlas(frames)?;
    let instances = build_kitty_instances(
        frames,
        atlas_width,
        atlas_height,
        &entries,
        grid_cell_width,
        grid_cell_height,
    );
    Some((atlas, atlas_width, atlas_height, instances))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::KittyPlacementFrame;

    fn test_frame() -> KittyPlacementFrame {
        KittyPlacementFrame {
            image_id: 1,
            viewport_col: 2,
            viewport_row: 3,
            pixel_width: 8,
            pixel_height: 16,
            source_x: 0,
            source_y: 0,
            source_width: 1,
            source_height: 1,
            cell_offset_x: 0,
            cell_offset_y: 0,
            z: 0,
            image_width: 1,
            image_height: 1,
            image_rgba: vec![255, 0, 0, 255].into(),
        }
    }

    fn green_frame_at(col: i32, row: i32) -> KittyPlacementFrame {
        KittyPlacementFrame {
            image_id: 2,
            viewport_col: col,
            viewport_row: row,
            pixel_width: 4,
            pixel_height: 4,
            source_x: 0,
            source_y: 0,
            source_width: 2,
            source_height: 2,
            cell_offset_x: 0,
            cell_offset_y: 0,
            z: 1,
            image_width: 2,
            image_height: 2,
            image_rgba: vec![
                0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ]
            .into(),
        }
    }

    #[test]
    fn single_full_image_packs_without_copy_change() {
        let frames = vec![test_frame()];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas");
        assert_eq!((width, height), (1, 1));
        assert_eq!(atlas, vec![255, 0, 0, 255]);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_some());
    }

    #[test]
    fn instances_map_viewport_grid_to_pixels_with_normalized_uv() {
        let frames = vec![test_frame()];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas");
        let instances = build_kitty_instances(&frames, width, height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].quad_origin, [16.0, 48.0]);
        assert_eq!(instances[0].quad_size, [8.0, 16.0]);
        assert_eq!(instances[0].atlas_offset, [0.0, 0.0]);
        assert_eq!(instances[0].atlas_region, [1.0, 1.0]);
        assert_eq!(atlas.len(), 4);
    }

    #[test]
    fn empty_frames_yield_no_atlas() {
        assert!(pack_atlas(&[]).is_none());
        assert!(layout_entries(&[]).is_none());
    }

    /// 条目在图集里紧挨着排，UV 覆盖的正是条目自己的纹素。
    ///
    /// KGP 采样器是双线性，显示尺寸不等于源子矩形时纹素坐标会越过条目边界，
    /// 越界取样由着色器按条目钳制（`kitty_graphics.wgsl` 的「按条目
    /// clamp-to-edge」）。图集里因此不必留沟槽像素：留了只会让图集变宽，
    /// 并让 UV 口径多出一处分叉。
    #[test]
    fn strip_entries_are_packed_side_by_side() {
        let frames = vec![test_frame(), green_frame_at(5, 0)];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas");
        // 1 像素的第一张 + 2 像素的第二张，无沟槽。
        assert_eq!((width, height), (3, 2));
        assert_eq!(entries[1].as_ref().map(|entry| entry.atlas_x), Some(1));
        let texel = |x: u32, y: u32| {
            let offset = (y * width + x) as usize * 4;
            atlas[offset..offset + 4].to_vec()
        };
        assert_eq!(texel(0, 0), vec![255, 0, 0, 255], "第一张图占据首列");
        assert_eq!(texel(1, 0), vec![0, 255, 0, 255], "第二张图紧接第一张");
        assert_eq!(texel(0, 1), vec![0, 0, 0, 0], "未被任何图覆盖的行保持透明");

        let instances = build_kitty_instances(&frames, width, height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 2);
        // UV 按子矩形归一化：第二帧偏移 1/3、宽 2/3。
        assert_eq!(instances[1].atlas_offset, [1.0 / 3.0, 0.0]);
        assert_eq!(instances[1].atlas_region, [2.0 / 3.0, 1.0]);
        assert_eq!(instances[1].quad_origin, [40.0, 0.0]);
        assert_eq!(atlas.len(), 3 * 2 * 4);
    }

    #[test]
    fn invalid_frame_is_skipped_without_killing_atlas() {
        let mut bad = test_frame();
        bad.source_width = 99;
        bad.source_x = 10;
        let frames = vec![bad, green_frame_at(0, 0)];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas survives");
        assert_eq!((width, height), (2, 2), "无效帧不占图集空间");
        assert!(entries[0].is_none());
        let instances = build_kitty_instances(&frames, width, height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 1);
        assert_eq!(atlas.len(), 2 * 2 * 4);
    }

    /// 图集超出字节预算时必须在分配**之前**被拒绝。
    ///
    /// 判别力：预算检查若放在分配之后，PTY 控住图像尺寸与放置数量即可让图集
    /// 达到数十 GB——分配失败会让进程 abort（无任何日志），画面直接消失且无从
    /// 归因。放置帧只声明尺寸不携带像素，故本用例不需要真的备出那份大缓冲。
    #[test]
    fn oversized_atlas_is_rejected_before_allocation() {
        let mut huge = test_frame();
        huge.image_width = 9_000_000;
        huge.image_height = 1;
        huge.source_width = 9_000_000;
        huge.source_height = 1;
        // 同一幅图像放两处：条带布局把它的源矩形并排铺开，图集宽度翻倍。
        let frames = vec![huge.clone(), huge];
        assert!(
            pack_atlas(&frames).is_none(),
            "超预算的图集必须在分配像素缓冲之前被拒绝"
        );
        // 复用路径必须与打包同判据：放行则会每帧与已上传图集（0）比对失败而重试打包。
        assert!(
            layout_entries(&frames).is_none(),
            "超预算布局不得进入复用路径"
        );
    }

    #[test]
    fn negative_viewport_row_is_kept_for_gpu_clipping() {
        let frames = vec![green_frame_at(0, -2)];
        let (atlas_width, atlas_height, entries) = layout_entries(&frames).expect("layout");
        let instances =
            build_kitty_instances(&frames, atlas_width, atlas_height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].quad_origin, [0.0, -32.0]);
    }
}
