//! Kitty 图像放置的图集组装与 GPU 实例构建。
//!
//! VT 线程采集[`KittyPlacementFrame`]（含 RGBA8 像素与视口几何），本模块在渲染线程完成两步：
//! 1. 横条带图集打包：各源子矩形并排写入单张 RGBA 图集（单图常见情形零拷贝直传）。
//! 2. 实例构建：视口网格坐标映射为像素 quad（与单元格 quad 同约定：左上原点，Y 向下），
//!    UV 归一化到图集尺寸（KGP 着色器直接采样 UV，不做像素换算）。

use crate::render::KittyGraphicsInstance;
use crate::terminal::ghostty_terminal::KittyPlacementFrame;

/// 图集条目：放置在图集中的像素偏移（实例 UV 归一化用）。
pub(crate) struct AtlasEntry {
    atlas_x: u32,
    atlas_y: u32,
}

/// 组装图集：返回（图集 RGBA，宽，高，各放置对应的图集偏移）。
/// 源矩形钳制到图像边界；空输入返回 None（调用方保持旧图集/传空实例）。
fn pack_atlas(frames: &[KittyPlacementFrame]) -> Option<(Vec<u8>, u32, u32, Vec<AtlasEntry>)> {
    if frames.is_empty() {
        return None;
    }
    // 单图且全图显示：零拷贝直传，避免一次大内存复制。
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
                vec![AtlasEntry {
                    atlas_x: 0,
                    atlas_y: 0,
                }],
            ));
        }
    }
    let mut total_width: u32 = 0;
    let mut max_height: u32 = 0;
    let mut clamped: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(frames.len());
    for frame in frames {
        let rect = clamp_source(frame, frame.image_width, frame.image_height);
        if rect.2 == 0 || rect.3 == 0 {
            return None;
        }
        total_width = total_width.saturating_add(rect.2);
        max_height = max_height.max(rect.3);
        clamped.push(rect);
    }
    if total_width == 0 || max_height == 0 {
        return None;
    }
    let stride = total_width.checked_mul(4)? as usize;
    let mut atlas = vec![0u8; stride.saturating_mul(max_height as usize)];
    let mut entries = Vec::with_capacity(frames.len());
    let mut offset_x: u32 = 0;
    for (frame, (source_x, source_y, source_width, source_height)) in
        frames.iter().zip(clamped.iter())
    {
        copy_sub_rect(
            &frame.image_rgba,
            frame.image_width,
            &mut atlas,
            total_width,
            offset_x,
            0,
            *source_x,
            *source_y,
            *source_width,
            *source_height,
        );
        entries.push(AtlasEntry {
            atlas_x: offset_x,
            atlas_y: 0,
        });
        offset_x = offset_x.saturating_add(*source_width);
    }
    Some((atlas, total_width, max_height, entries))
}

/// 源矩形钳制到图像边界（防御上游行为漂移，避免越界 panic）。
fn clamp_source(frame: &KittyPlacementFrame, image_width: u32, image_height: u32) -> (u32, u32, u32, u32) {
    let source_x = frame.source_x.min(image_width);
    let source_y = frame.source_y.min(image_height);
    let source_width = frame
        .source_width
        .min(image_width.saturating_sub(source_x));
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
/// 完全滚出视口（右/下越界）或零尺寸的放置被跳过；顶部滚出（负行）保留，
/// 由 GPU 裁剪（与上游 viewport_pos 可为负的语义一致）。
pub(crate) fn build_kitty_instances(
    frames: &[KittyPlacementFrame],
    atlas_width: u32,
    atlas_height: u32,
    entries: &[AtlasEntry],
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

/// 一站式：打包图集 + 构建实例（FFI 渲染线程入口）。
/// 返回（图集 RGBA，宽，高，实例）。无可见放置时返回 None（调用方传空实例、不碰图集）。
pub fn pack_and_build(
    frames: &[KittyPlacementFrame],
    grid_cell_width: f32,
    grid_cell_height: f32,
) -> Option<(Vec<u8>, u32, u32, Vec<KittyGraphicsInstance>)> {
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

    #[test]
    fn single_full_image_packs_without_copy_change() {
        let frames = vec![test_frame()];
        let (atlas, width, height, entries) = pack_atlas(&frames).expect("atlas");
        assert_eq!((width, height), (1, 1));
        assert_eq!(atlas, vec![255, 0, 0, 255]);
        assert_eq!(entries.len(), 1);
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
    }
}
