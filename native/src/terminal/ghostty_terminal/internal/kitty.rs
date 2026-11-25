use std::borrow::Cow;
use std::collections::HashMap;

use super::input_filter::{Clear, ClearKind};
use super::{Arc, KittyPlacementFrame, Terminal, grid_cols, grid_rows};

use libghostty_vt::kitty::graphics::PlacementIterator;
use libghostty_vt::render::{ImageIterator, ImageProtocol, RenderState};

// ── Kitty 图像与放置采集 ─────────────────────────────────────

/// 撤下全部可见放置的删除命令（`a=d,d=A`）。
///
/// `d=A` 撤下屏幕上的全部放置并释放其图像数据；`q=2` 让命令完全静默——
/// 删除成功本就不回包，失败也不回，终端不会因此多出日志或应答。
const DELETE_VISIBLE_PLACEMENTS: &[u8] = b"\x1b_Ga=d,d=A,q=2\x1b\\";

/// 按图像 id 撤下放置的删除命令（`a=d,d=I`）。
///
/// `d=I` 撤下该图像的全部放置并释放其图像数据。只能按图像 id 撤：上游按
/// 放置 id 删除时只认协议显式给出的放置 id，终端自行分配的放置 id 删不掉。
fn delete_image_command(image_id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\").into_bytes()
}

/// 清屏前置的 kitty 删除命令。
///
/// 清屏要撤下图像放置，而撤下必须发生在清屏之前：
///
/// - ED2（清活动区）：撤下全部可见放置。提示符行上的清屏走「旧行先滚进回滚
///   再清屏」的路径，可见放置会随旧行滚进回滚区，此后清屏不再撤下它们；
///   紧接着的 ED3 又会把回滚区的锚点搬到屏幕左上角，图像便复活在左上角并
///   盖住屏幕（用户报障：`kitten icat` 后输入 `clear`，图像不清除反而变大）。
///   提前撤下可见放置，图像便不进回滚区，也无从复活。
/// - ED3（清回滚区）：撤下锚在回滚区的放置。上游清回滚区时把放置的锚点搬到
///   被删页的左上角而不标记其失效，锚在回滚区的放置于是回到屏幕左上角。
pub(crate) fn clear_delete_commands(terminal: &Terminal<'_, '_>, kind: ClearKind) -> Vec<u8> {
    match kind {
        ClearKind::Display => DELETE_VISIBLE_PLACEMENTS.to_vec(),
        ClearKind::Scrollback => scrollback_delete_commands(terminal),
    }
}

/// 清屏前置 kitty 删除命令后的透传载荷。
///
/// 删除命令插在清屏序列之前：撤下必须发生在清屏之前，否则清屏会先把旧行
/// （连同图像放置）滚进回滚区，那时的放置已不在屏幕上，清屏便撤不下它们。
pub(crate) fn with_clear_deletes<'a>(
    terminal: &Terminal<'_, '_>,
    payload: Cow<'a, [u8]>,
    clears: &[Clear],
) -> Cow<'a, [u8]> {
    if clears.is_empty() {
        return payload;
    }
    let mut out = Vec::with_capacity(payload.len() + 64);
    let mut cursor = 0usize;
    for clear in clears {
        let commands = clear_delete_commands(terminal, clear.kind);
        if commands.is_empty() {
            continue;
        }
        out.extend_from_slice(&payload[cursor..clear.offset]);
        out.extend_from_slice(&commands);
        cursor = clear.offset;
    }
    out.extend_from_slice(&payload[cursor..]);
    Cow::Owned(out)
}

/// 撤下锚在回滚区的放置：按图像 id 逐个删除。
///
/// 视口不在活动区时（用户正在回看历史）无法区分「活动区」与「回滚区」，
/// 此时一律撤下：宁可少留图像，也不让清屏后的图像复活到屏幕左上角。
fn scrollback_delete_commands(terminal: &Terminal<'_, '_>) -> Vec<u8> {
    let mut iterator = match PlacementIterator::new() {
        Ok(iterator) => iterator,
        Err(error) => {
            log::warn!("kitty: placement iterator unavailable: {error}");
            return Vec::new();
        }
    };
    let Ok(graphics) = terminal.kitty_graphics() else {
        return Vec::new();
    };
    let Ok(mut placements) = iterator.update(&graphics) else {
        return Vec::new();
    };
    let viewport_active = terminal.viewport_active().unwrap_or(true);
    // 图像 id → 是否保留。有活动区放置的图像留下；含虚拟放置（Unicode 占位符）
    // 的图像也留下：虚拟放置跟着占位符单元走，占位符随回滚区一起被清掉，
    // 放置自然消失，按图像 id 撤下反而会带走同一图像在屏幕上的那一份。
    let mut images: HashMap<u32, bool> = HashMap::new();
    while placements.next().is_some() {
        let Ok(image_id) = placements.image_id() else {
            continue;
        };
        let Some(image) = graphics.image(image_id) else {
            continue;
        };
        let keep = images.entry(image_id).or_insert(false);
        if placements.is_virtual().unwrap_or(false) {
            *keep = true;
            continue;
        }
        let active =
            viewport_active && matches!(placements.viewport_pos(&image, terminal), Ok(Some(_)));
        if active {
            *keep = true;
        }
    }
    let mut removable: Vec<u32> = images
        .into_iter()
        .filter_map(|(image_id, keep)| (!keep).then_some(image_id))
        .collect();
    removable.sort_unstable();
    let mut commands = Vec::new();
    for image_id in removable {
        commands.extend(delete_image_command(image_id));
    }
    commands
}

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

/// 放置与视口是否有交集（决定它要不要进图集）。
///
/// 网格尺寸来自协议参数（`c`/`r`），客户端可给出 `u32::MAX` 量级的数：直接相加
/// 在 i32 上溢出，debug 构建直接 panic（JNI 边界上即整进程崩溃），release 构建
/// 回绕成负数——一个明明可见的放置被判成屏外，图像半边不画。故一律饱和运算。
fn intersects_viewport(
    viewport_col: i32,
    viewport_row: i32,
    grid_cols: u32,
    grid_rows: u32,
    rows: u32,
    cols: u32,
) -> bool {
    let end_row = viewport_row.saturating_add(i32::try_from(grid_rows).unwrap_or(i32::MAX));
    let end_col = viewport_col.saturating_add(i32::try_from(grid_cols).unwrap_or(i32::MAX));
    end_row > 0
        && viewport_row < i32::try_from(rows).unwrap_or(i32::MAX)
        && end_col > 0
        && viewport_col < i32::try_from(cols).unwrap_or(i32::MAX)
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
            if !intersects_viewport(viewport_col, viewport_row, grid_cols, grid_rows, rows, cols) {
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

#[cfg(test)]
mod tests {
    use super::intersects_viewport;

    /// 视口 24×80 的常规判据：四种「有交集」与三种「无交集」。
    #[test]
    fn viewport_intersection_covers_edges_and_offscreen_cases() {
        let (rows, cols) = (24, 80);
        assert!(
            intersects_viewport(10, 5, 4, 3, rows, cols),
            "完全落在视口内的放置必须进图集，否则可见图像永远不上屏"
        );
        assert!(
            intersects_viewport(0, -2, 4, 4, rows, cols),
            "顶部滚出两行的放置仍有两行可见，按交集判定必须保留"
        );
        assert!(
            intersects_viewport(-1, 0, 2, 2, rows, cols),
            "左侧滚出一列的放置仍有一列可见，不得因负列号丢弃"
        );
        assert!(
            intersects_viewport(78, 23, 8, 1, rows, cols),
            "右下角越界的放置仍从格子内开始，必须保留而非整块丢弃"
        );
        assert!(
            !intersects_viewport(0, -4, 4, 4, rows, cols),
            "整块滚出视口上方的放置没有任何可见像素，必须判为不可见"
        );
        assert!(
            !intersects_viewport(80, 0, 4, 2, rows, cols),
            "完全在视口右侧的放置没有任何可见像素，必须判为不可见"
        );
        assert!(
            !intersects_viewport(0, 24, 4, 2, rows, cols),
            "完全在视口下方的放置没有任何可见像素，必须判为不可见"
        );
    }

    /// 协议参数的网格尺寸可以大到 `u32::MAX`：相加必须在 i32 上饱和而不是溢出。
    ///
    /// 判别力：未饱和时 debug 构建在此 panic（JNI 边界上等于整进程崩溃），
    /// release 构建回绕成负数，一个从 (0,0) 开始、覆盖整屏的放置会被判成屏外。
    #[test]
    fn extreme_grid_size_is_saturated_not_overflowing() {
        assert!(
            intersects_viewport(0, 0, u32::MAX, u32::MAX, 24, 80),
            "起点在视口内、网格尺寸极大的放置必须可见，不得因饱和判成屏外"
        );
        assert!(
            !intersects_viewport(i32::MAX - 1, i32::MAX - 1, u32::MAX, u32::MAX, 24, 80),
            "起点在视口之外时只允许判为不可见，且不得溢出 panic"
        );
    }
}
