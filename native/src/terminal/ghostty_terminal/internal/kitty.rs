use std::collections::HashMap;

use super::super::unicode_placement;
use super::*;

// ── Kitty 图像与放置采集 ─────────────────────────────────────

#[derive(Default)]
pub(crate) struct KittyPlacementCache {
    generation: u64,
    cols: u32,
    /// image id → 解码结果（`None` 表示该图像不可解码，同样值得记住）。
    images: HashMap<u32, Option<(u32, u32, Arc<[u8]>)>>,
    /// 绝对缓冲行 → (占位符布局签名, 该行扫描结果)。
    rows: HashMap<u64, (u64, Vec<unicode_placement::PlaceholderCell>)>,
    /// 扫描计数，仅测试使用。
    #[cfg(test)]
    pub(crate) stats: CacheStats,
}

impl KittyPlacementCache {
    pub(crate) fn sync(&mut self, generation: u64, cols: u32) {
        if self.generation != generation || self.cols != cols {
            self.generation = generation;
            self.cols = cols;
            self.images.clear();
            self.rows.clear();
        }
    }
}

/// 把 Kitty 的样式颜色折算为图像/放置 id（与上游 colorToId 一致：
/// 取 RGB 的 24 位，调色板索引直接作 id）。
fn kitty_color_to_id(color: libghostty_vt::style::StyleColor) -> u32 {
    use libghostty_vt::style::StyleColor;
    match color {
        StyleColor::None => 0,
        StyleColor::Palette(index) => index.0 as u32,
        StyleColor::Rgb(rgb) => ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32,
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
    /// 被多个放置或占位符分组反复解码。
    pub(crate) fn cached_kitty_image(
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

    /// 采集视口内全部可见 Kitty 放置：普通放置由 graphics 存储的 pin 解算，
    /// Unicode 占位符放置按可见占位符单元反推整图锚点。两者按 z 排序。
    pub(crate) fn collect_kitty_placements(
        terminal: &Terminal,
        cache: &mut KittyPlacementCache,
        row_signatures: Option<&[u64]>,
    ) -> Vec<KittyPlacementFrame> {
        use libghostty_vt::kitty::graphics::PlacementIterator;
        let Ok(graphics) = terminal.kitty_graphics() else {
            return Vec::new();
        };
        match graphics.generation() {
            Ok(generation) => cache.sync(generation, grid_cols(terminal)),
            // 生成戳读不到就无法判断缓存是否过期：整体作废，退回全量扫描。
            Err(_) => *cache = KittyPlacementCache::default(),
        }
        let Ok(mut iterator) = PlacementIterator::new() else {
            return Vec::new();
        };
        let Ok(mut placements) = iterator.update(&graphics) else {
            return Vec::new();
        };
        let mut virtual_meta = HashMap::new();
        let mut out = Vec::new();
        while let Some(placement) = placements.next() {
            match placement.is_virtual() {
                Ok(true) => {
                    // 虚拟放置的 z / 像素偏移只存在于放置本身；占位符单元只编码
                    // 图像行列。元数据读不到就跳过该放置（宁可少画，不画错位置）。
                    let meta = (|| {
                        Ok::<_, libghostty_vt::error::Error>((
                            placement.image_id()?,
                            placement.placement_id()?,
                            placement.z()?,
                            placement.x_offset()?,
                            placement.y_offset()?,
                        ))
                    })();
                    match meta {
                        Ok((image_id, placement_id, z, x_offset, y_offset)) => {
                            virtual_meta.insert((image_id, placement_id), (z, x_offset, y_offset));
                        }
                        Err(error) => {
                            log::warn!("kitty: virtual placement metadata failed: {error}");
                        }
                    }
                    continue;
                }
                Ok(false) => {}
                // 读不到即按虚拟放置跳过：宁可少画一幅图，也不把不可见的放置画出来。
                Err(error) => {
                    log::warn!("kitty: is_virtual failed, skipping placement: {error}");
                    continue;
                }
            }
            let Ok(image_id) = placement.image_id() else {
                continue;
            };
            let Some(image) = graphics.image(image_id) else {
                log::debug!("kitty: 图像 {image_id} 已从 graphics 存储中消失，跳过");
                continue;
            };
            let Some((image_width, image_height, image_rgba)) =
                Self::cached_kitty_image(&graphics, cache, image_id)
            else {
                log::debug!("kitty: 图像 {image_id} 解码失败（格式或像素长度不支持），跳过");
                continue;
            };
            let Ok(info) = placement.placement_render_info(&image, terminal) else {
                continue;
            };
            if !info.viewport_visible
                || info.pixel_width == 0
                || info.pixel_height == 0
                || info.source_width == 0
                || info.source_height == 0
            {
                continue;
            }
            out.push(KittyPlacementFrame {
                image_id,
                viewport_col: info.viewport_col,
                viewport_row: info.viewport_row,
                pixel_width: info.pixel_width,
                pixel_height: info.pixel_height,
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
                // 偏移与 z 读不到即丢弃该放置：留 0 会把图像画到错误位置并
                // 破坏叠加次序，而那与「没画」同样不可接受，只是更难察觉。
                cell_offset_x: match placement.x_offset() {
                    Ok(offset) => offset,
                    Err(error) => {
                        log::warn!("kitty: x_offset failed, skipping placement: {error}");
                        continue;
                    }
                },
                cell_offset_y: match placement.y_offset() {
                    Ok(offset) => offset,
                    Err(error) => {
                        log::warn!("kitty: y_offset failed, skipping placement: {error}");
                        continue;
                    }
                },
                z: match placement.z() {
                    Ok(z) => z,
                    Err(error) => {
                        log::warn!("kitty: z failed, skipping placement: {error}");
                        continue;
                    }
                },
                image_width,
                image_height,
                image_rgba,
            });
        }
        out.extend(Self::collect_unicode_placements(
            terminal,
            &graphics,
            cache,
            &virtual_meta,
            row_signatures,
        ));
        out.sort_by_key(|frame| frame.z);
        out
    }

    /// 扫描视口内带 `has_kitty_virtual_placeholder` 标记的行，解码占位符单元
    /// 并反推整图锚点。
    ///
    /// shortcut: 整图按锚点绘制，依赖占位符覆盖整幅图；若客户端只用子矩形
    /// 占位符做图像内滚动，图会画出占位区，届时再按可见块裁剪。
    pub(crate) fn collect_unicode_placements(
        terminal: &Terminal,
        graphics: &libghostty_vt::kitty::graphics::Graphics<'_>,
        cache: &mut KittyPlacementCache,
        virtual_meta: &HashMap<(u32, u32), (i32, u32, u32)>,
        row_signatures: Option<&[u64]>,
    ) -> Vec<KittyPlacementFrame> {
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        if rows == 0 || cols == 0 {
            return Vec::new();
        }
        let mut cells = Vec::new();
        // 纯滚动时同一绝对行的占位符不变，`row_signatures` 命中即可复用上次扫描，
        // 免掉每格 4 次 FFI；落定后的同布局重查同理。绝对行以 VT 自己的滚动条
        // 偏移为基准（回看历史时视口顶行不是回滚长度）。
        let cache_base_row = row_signatures.and_then(|_| match terminal.scrollbar() {
            Ok(scrollbar) => Some(scrollbar.offset),
            Err(error) => {
                log::warn!("kitty: scrollbar query failed, row cache disabled: {error}");
                None
            }
        });
        let mut rows_in_view = HashMap::new();
        #[cfg(test)]
        let (mut reused_count, mut scanned_count) = (0u64, 0u64);
        for row in 0..rows {
            let signature = row_signatures
                .and_then(|signatures| signatures.get(row as usize))
                .copied();
            if signature == Some(0) {
                continue;
            }
            let absolute_row = cache_base_row.map(|base| base + u64::from(row));
            if let (Some(signature), Some(absolute_row)) = (signature, absolute_row) {
                if let Some((cached_signature, cached)) = cache.rows.get(&absolute_row)
                    && *cached_signature == signature
                {
                    let mut cached = cached.clone();
                    cached.iter_mut().for_each(|cell| cell.grid_row = row);
                    cells.extend(cached.iter().copied());
                    rows_in_view.insert(absolute_row, (signature, cached));
                    #[cfg(test)]
                    {
                        reused_count += 1;
                    }
                    continue;
                }
            } else if signature.is_none() {
                let Ok(grid_ref) =
                    terminal.grid_ref(Point::Viewport(PointCoordinate { x: 0, y: row }))
                else {
                    continue;
                };
                let Ok(row_ref) = grid_ref.row() else {
                    continue;
                };
                if !row_ref.has_kitty_virtual_placeholder().unwrap_or(false) {
                    continue;
                }
            }
            let mut row_cells = Vec::new();
            for col in 0..cols {
                let Ok(grid_ref) = terminal.grid_ref(Point::Viewport(PointCoordinate {
                    x: col as u16,
                    y: row,
                })) else {
                    continue;
                };
                let Ok(cell) = grid_ref.cell() else {
                    continue;
                };
                if !matches!(
                    cell.codepoint(),
                    Ok(codepoint) if codepoint == unicode_placement::PLACEHOLDER
                ) {
                    continue;
                }
                let Ok(style) = grid_ref.style() else {
                    continue;
                };
                let image_id = kitty_color_to_id(style.fg_color);
                if image_id == 0 {
                    continue;
                }
                let placement_id = kitty_color_to_id(style.underline_color);
                let mut buffer = ['\0'; 8];
                let Ok(len) = grid_ref.graphemes(&mut buffer) else {
                    continue;
                };
                // 至少要有基字符 + 行记号 + 列记号；不足即非完整占位符。
                if len < 3 {
                    continue;
                }
                let marks = &buffer[1..len];
                let (Some(&row_mark), Some(&col_mark)) = (marks.first(), marks.get(1)) else {
                    continue;
                };
                let (Some(image_row), Some(image_col)) = (
                    unicode_placement::diacritic_index(row_mark as u32),
                    unicode_placement::diacritic_index(col_mark as u32),
                ) else {
                    continue;
                };
                let image_id = match marks.get(2) {
                    Some(&high_mark) => {
                        let Some(high) = unicode_placement::diacritic_index(high_mark as u32)
                        else {
                            continue;
                        };
                        if high > 0xFF {
                            continue;
                        }
                        image_id | (high << 24)
                    }
                    None => image_id,
                };
                row_cells.push(unicode_placement::PlaceholderCell {
                    grid_row: row,
                    grid_col: col,
                    image_id,
                    placement_id,
                    image_row,
                    image_col,
                });
            }
            #[cfg(test)]
            {
                scanned_count += 1;
            }
            if let (Some(signature), Some(absolute_row)) = (signature, absolute_row) {
                cells.extend(row_cells.iter().copied());
                rows_in_view.insert(absolute_row, (signature, row_cells));
            } else {
                cells.extend(row_cells);
            }
        }
        if cache_base_row.is_some() {
            cache.rows = rows_in_view;
        }
        #[cfg(test)]
        cache.stats.record(reused_count, scanned_count);
        let mut out = Vec::new();
        for anchor in unicode_placement::anchors(&cells) {
            // 与上游一致：占位符未编码放置 id（underline 未设置）时，
            // 退而取该图像的第一个虚拟放置；显式编码了 id 却不匹配则不猜。
            let meta = virtual_meta
                .get(&(anchor.image_id, anchor.placement_id))
                .or_else(|| {
                    if anchor.placement_id == 0 {
                        virtual_meta
                            .iter()
                            .find(|((image_id, _), _)| *image_id == anchor.image_id)
                            .map(|(_, meta)| meta)
                    } else {
                        None
                    }
                });
            let Some(&(z, x_offset, y_offset)) = meta else {
                log::debug!(
                    "kitty: virtual placement ({}, {}) missing metadata, skipping",
                    anchor.image_id,
                    anchor.placement_id
                );
                continue;
            };
            let Some((image_width, image_height, image_rgba)) =
                Self::cached_kitty_image(graphics, cache, anchor.image_id)
            else {
                continue;
            };
            if image_width == 0 || image_height == 0 {
                continue;
            }
            out.push(KittyPlacementFrame {
                image_id: anchor.image_id,
                viewport_col: anchor.grid_col,
                viewport_row: anchor.grid_row,
                pixel_width: image_width,
                pixel_height: image_height,
                source_x: 0,
                source_y: 0,
                source_width: image_width,
                source_height: image_height,
                cell_offset_x: x_offset,
                cell_offset_y: y_offset,
                z,
                image_width,
                image_height,
                image_rgba,
            });
        }
        out
    }
}
