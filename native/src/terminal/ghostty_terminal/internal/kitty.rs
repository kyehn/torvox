use std::collections::HashMap;

use super::*;

use libghostty_vt::render::{ImageIterator, ImageProtocol, RenderState};

// ── Kitty 图像与放置采集 ─────────────────────────────────────

/// Kitty 放置采集缓存：跨查询复用图像解码结果。
///
/// 图像内容随存储代数变化（同一 id 重新传输即换像素），故代数变化即整体作废。
#[derive(Default)]
pub(crate) struct KittyPlacementCache {
    generation: u64,
    /// image id → 解码结果（`None` 表示该图像不可解码，同样值得记住）。
    images: HashMap<u32, Option<(u32, u32, Arc<[u8]>)>>,
}

impl KittyPlacementCache {
    fn sync(&mut self, generation: u64) {
        if self.generation != generation {
            self.generation = generation;
            self.images.clear();
        }
    }
}

/// RGB 三元组 → RGBA8（alpha 固定不透明）。
///
/// RGB 图像与 PNG 解码器的 RGB 兜底分支都要这一步：两处各写一遍时，改动只落在
/// 一处，另一处的图像会少一个通道（颜色错乱且无报错）。
fn rgb_to_rgba(triples: &[[u8; 3]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(triples.len() * 4);
    for triple in triples {
        out.extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
    }
    out
}

impl super::GhosttyTerminal {
    /// 上游 Kitty 图像载荷归一化为 RGBA8：Ok(None) 为分片传输待定返回 None；
    /// RGB 直扩 alpha，灰度按亮度展开，PNG 经线程解码器已为 RGBA（按长度兜底）。
    pub(crate) fn kitty_image_to_rgba(
        image: &libghostty_vt::kitty::graphics::Image<'_>,
    ) -> Option<(u32, u32, Arc<[u8]>)> {
        use libghostty_vt::kitty::graphics::ImageFormat;
        let width = image.width().ok()?;
        let height = image.height().ok()?;
        let bytes = image.data().ok()??;
        let pixel_count = width.checked_mul(height)? as usize;
        let rgba = match image.format().ok()? {
            ImageFormat::Rgba => {
                if bytes.len() != pixel_count.checked_mul(4)? {
                    return None;
                }
                bytes.to_vec()
            }
            ImageFormat::Rgb => {
                if bytes.len() != pixel_count.checked_mul(3)? {
                    return None;
                }
                let (triples, _) = bytes.as_chunks::<3>();
                rgb_to_rgba(triples)
            }
            ImageFormat::Gray => {
                if bytes.len() != pixel_count {
                    return None;
                }
                let mut out = Vec::with_capacity(pixel_count * 4);
                for gray in bytes {
                    out.extend_from_slice(&[*gray, *gray, *gray, 255]);
                }
                out
            }
            ImageFormat::GrayAlpha => {
                if bytes.len() != pixel_count.checked_mul(2)? {
                    return None;
                }
                let (pairs, _) = bytes.as_chunks::<2>();
                let mut out = Vec::with_capacity(pixel_count * 4);
                for pair in pairs {
                    out.extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
                }
                out
            }
            ImageFormat::Png => {
                // 解码器输出 RGBA；按长度兜底 RGB（防御上游行为漂移）。
                if bytes.len() == pixel_count.checked_mul(4)? {
                    bytes.to_vec()
                } else if bytes.len() == pixel_count.checked_mul(3)? {
                    let (triples, _) = bytes.as_chunks::<3>();
                    rgb_to_rgba(triples)
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some((width, height, rgba.into()))
    }

    /// 取图像并解码为 RGBA；跨查询按 image id 缓存，避免同一图像
    /// 被多个放置块反复解码。
    fn cached_kitty_image(
        graphics: &libghostty_vt::kitty::graphics::Graphics<'_>,
        cache: &mut KittyPlacementCache,
        image_id: u32,
    ) -> Option<(u32, u32, Arc<[u8]>)> {
        if let Some(cached) = cache.images.get(&image_id) {
            return cached.clone();
        }
        let decoded = graphics
            .image(image_id)
            .and_then(|image| Self::kitty_image_to_rgba(&image));
        cache.images.insert(image_id, decoded.clone());
        decoded
    }

    /// 采集当前帧的全部可见 Kitty 放置：几何 + RGBA 像素。
    ///
    /// 几何取自上游渲染状态的图像列表：上游按协议口径给出放置的视口位置、
    /// 显示尺寸与源矩形，Unicode 占位符（`kitten icat` 走的那条）所需的
    /// 等比缩放与居中也在其中。本仓不再自行推导占位符锚点与尺寸——自行推导
    /// 只能得到图像的原始像素尺寸，画出来的是原图大小而非放置区域大小，
    /// 大图因此铺满并盖住屏幕。
    ///
    /// 像素按图像 id 从 kitty 存储取；上游把一幅图按行切成多个放置块
    /// （Unicode 占位符逐行一块），同一图像的各块共享一份解码结果。
    pub(crate) fn collect_kitty_placements<'alloc>(
        terminal: &Terminal<'alloc, '_>,
        cache: &mut KittyPlacementCache,
        render_state: &mut RenderState<'alloc>,
        image_iter: &mut ImageIterator<'alloc>,
    ) -> Vec<KittyPlacementFrame> {
        let snapshot = match render_state.update(terminal) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::warn!("kitty: render state update failed: {error}");
                return Vec::new();
            }
        };
        let mut iterations = match image_iter.update(&snapshot) {
            Ok(iterations) => iterations,
            Err(error) => {
                log::warn!("kitty: image iterator update failed: {error}");
                return Vec::new();
            }
        };
        let Ok(graphics) = terminal.kitty_graphics() else {
            return Vec::new();
        };
        match graphics.generation() {
            Ok(generation) => cache.sync(generation),
            // 生成戳读不到就无法判断缓存是否过期：整体作废，退回全量解码。
            Err(_) => *cache = KittyPlacementCache::default(),
        }
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let mut out = Vec::new();
        while let Some(placement) = iterations.next() {
            // 上游只把 Kitty 协议的放置放进图像列表；其余协议（如上游新增的
            // 图像协议）本仓不绘制，跳过即可。
            if !matches!(placement.protocol(), Ok(ImageProtocol::Kitty)) {
                continue;
            }
            let geometry = (
                placement.image_id(),
                placement.viewport_col(),
                placement.viewport_row(),
                placement.pixel_width(),
                placement.pixel_height(),
                placement.source_x(),
                placement.source_y(),
                placement.source_width(),
                placement.source_height(),
                placement.cell_offset_x(),
                placement.cell_offset_y(),
                placement.z(),
                placement.grid_cols(),
                placement.grid_rows(),
            );
            let (
                Ok(image_id),
                Ok(viewport_col),
                Ok(viewport_row),
                Ok(pixel_width),
                Ok(pixel_height),
                Ok(source_x),
                Ok(source_y),
                Ok(source_width),
                Ok(source_height),
                Ok(cell_offset_x),
                Ok(cell_offset_y),
                Ok(z),
                Ok(grid_cols),
                Ok(grid_rows),
            ) = geometry
            else {
                // 单个放置的几何读不到即跳过：读不到就没有可画的位置，
                // 画在错误位置比不画更难察觉。
                log::debug!("kitty: placement geometry unavailable, skipping");
                continue;
            };
            // 视口外的放置不进图集：图集按可见放置打包，屏外的图像
            // 只会白占预算与显存。
            let visible = viewport_row + i32::try_from(grid_rows).unwrap_or(i32::MAX) > 0
                && viewport_row < i32::try_from(rows).unwrap_or(i32::MAX)
                && viewport_col + i32::try_from(grid_cols).unwrap_or(i32::MAX) > 0
                && viewport_col < i32::try_from(cols).unwrap_or(i32::MAX);
            if !visible {
                continue;
            }
            let Some((image_width, image_height, image_rgba)) =
                Self::cached_kitty_image(&graphics, cache, image_id)
            else {
                log::debug!("kitty: image {image_id} failed to decode, skipping");
                continue;
            };
            if pixel_width == 0 || pixel_height == 0 {
                continue;
            }
            out.push(KittyPlacementFrame {
                image_id,
                viewport_col,
                viewport_row,
                pixel_width,
                pixel_height,
                source_x,
                source_y,
                source_width,
                source_height,
                cell_offset_x,
                cell_offset_y,
                z,
                image_width,
                image_height,
                image_rgba,
            });
        }
        out.sort_by_key(|frame| frame.z);
        out
    }
}
