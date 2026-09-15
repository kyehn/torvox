//! Kitty 图像放置的图集组装与 GPU 实例构建。
//!
//! VT 线程采集[`KittyPlacementFrame`]（含 RGBA8 像素与视口几何），本模块在渲染线程完成两步：
//! 1. 横条带图集打包：各源子矩形并排写入单张 RGBA 图集（单图全图常见情形单拷贝直传）。
//! 2. 实例构建：视口网格坐标映射为像素 quad（与单元格 quad 同约定：左上原点，Y 向下），
//!    UV 归一化到图集尺寸（KGP 着色器直接采样 UV，不做像素换算）。

use crate::render::KittyGraphicsInstance;
use crate::terminal::ghostty_terminal::KittyPlacementFrame;

/// 图集条目：放置在图集中的像素偏移（实例 UV 归一化用）。
pub(crate) struct AtlasEntry {
    atlas_x: u32,
    atlas_y: u32,
}

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
        width = width.saturating_add(rect.2);
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
type PackedAtlas = (Vec<u8>, u32, u32, Vec<Option<AtlasEntry>>);

/// 组装图集：源矩形钳制到图像边界；无有效帧返回 None（调用方清空实例与图集）。
/// 输入总量由上游存储上限约束（KGP_STORAGE_LIMIT=64MiB，见 types.rs）；
/// 横条带布局存在矩形空洞（输出像素数可大于输入面积和），此处饱和/受检算术
/// 仅防溢出 panic，不另设未声明的截断。
fn pack_atlas(frames: &[KittyPlacementFrame]) -> Option<PackedAtlas> {
    if frames.is_empty() {
        return None;
    }
    // 单图且全图显示：单拷贝直传（借用下无法真正零拷贝）。
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
                frame.image_rgba.clone(),
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
    let stride = layout.width.checked_mul(4)? as usize;
    let mut atlas = vec![0u8; stride.saturating_mul(layout.height as usize)];
    for (index, frame) in frames.iter().enumerate() {
        let (Some(entry), (source_x, source_y, source_width, source_height)) =
            (&layout.entries[index], layout.clamped[index])
        else {
            continue;
        };
        copy_sub_rect(
            &frame.image_rgba,
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
    Some((atlas, layout.width, layout.height, layout.entries))
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
#[allow(clippy::too_many_arguments)]
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
        let Some(source_row) = source.get(source_offset..source_offset + row_bytes) else {
            break;
        };
        let Some(destination_row) =
            destination.get_mut(destination_offset..destination_offset + row_bytes)
        else {
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
pub(crate) fn layout_entries(
    frames: &[KittyPlacementFrame],
) -> Option<(u32, u32, Vec<Option<AtlasEntry>>)> {
    let layout = layout_strip(frames)?;
    Some((layout.width, layout.height, layout.entries))
}

/// 打包产物：（图集 RGBA，宽，高，实例）。
type AtlasInstances = (Vec<u8>, u32, u32, Vec<KittyGraphicsInstance>);

/// 一站式：打包图集 + 构建实例（FFI 渲染线程入口）。
/// 无可见放置时返回 None（调用方清空实例与图集）。
pub fn pack_and_build(
    frames: &[KittyPlacementFrame],
    grid_cell_width: f32,
    grid_cell_height: f32,
) -> Option<AtlasInstances> {
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
            image_rgba: vec![255, 0, 0, 255],
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
            ],
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

    #[test]
    fn strip_packs_two_images_side_by_side() {
        let frames = vec![test_frame(), green_frame_at(5, 0)];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas");
        assert_eq!((width, height), (3, 2));
        assert_eq!(entries.len(), 2);
        let instances = build_kitty_instances(&frames, width, height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 2);
        // 第二帧 UV 指向条带偏移 1/3 处，宽 2/3。
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
        assert_eq!((width, height), (2, 2));
        assert!(entries[0].is_none());
        let instances = build_kitty_instances(&frames, width, height, &entries, 8.0, 16.0);
        assert_eq!(instances.len(), 1);
        assert_eq!(atlas.len(), 2 * 2 * 4);
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
