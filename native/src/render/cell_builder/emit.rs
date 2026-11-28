//! 单元实例的发射：一行网格 → GPU 实例。
//!
//! 与 [`super`]（单元数据的差分、缓存与预热）分开的原因：这一组函数只回答
//! 「一条单元记录该画成哪些四边形」——光标几何、空白格四边形、字形与组合记号的
//! 叠加都在此，与帧级缓存无关。

use std::collections::HashMap;
use std::sync::Arc;

use foldhash::fast::RandomState;

use crate::render::CellInstance;
use crate::terminal::CursorStyle;
use crate::terminal::ghostty_terminal::{KITTY_PLACEHOLDER, cell_flags};

use super::{
    BAR_CURSOR_WIDTH_FRACTION, CURSOR_MARKER_MINIMUM_THICKNESS, CellCursor, SearchHighlight,
    UNDERLINE_CURSOR_HEIGHT_FRACTION, apply_search_highlight, blank_cell_needs_quad,
    cell_highlight, cluster_text_into, codepoint_to_char,
};

/// 竖线/下划线光标标记在单元盒内的几何；方块光标返回 `None`。
///
/// 空单元格与文本单元格两条路径共用同一份判据：分叉会让同一个光标在两种格
/// 子里画出不同的粗细/高度，且这类偏差没有任何用例会覆盖（两种格子各测一遍
/// 才发现）。
///
/// 标记锚在单元盒，不锚字形位图（spec cursor-rendering「光标标记锚在单元盒」）：
/// 位图高度随字符变化，锚它会让同一个光标在空单元格与文本格之间变高变矮，
/// 降部字形还会把标记带出行界。
fn cursor_marker_geometry(
    cursor: CellCursor,
    origin: [f32; 2],
    cell_width: f32,
    cell_height: f32,
    cell_span: f32,
) -> Option<([f32; 2], [f32; 2])> {
    match cursor.style {
        // 整格就是光标盒，无需另发标记。
        CursorStyle::Block => None,
        // 竖线光标：单元格左侧细竖条，高度为整格。
        // 宽度**不**乘 `cell_span`：宽字符上乘了会变成两倍粗的短横，不再是竖线；
        // 宽字符只有头列一条记录，原点即字形原点，竖条已覆盖到字形。
        CursorStyle::Bar => Some((
            origin,
            [
                (cell_width * BAR_CURSOR_WIDTH_FRACTION).max(CURSOR_MARKER_MINIMUM_THICKNESS),
                cell_height,
            ],
        )),
        // 下划线光标：整格底部细横条，宽度覆盖整格。
        CursorStyle::Underline => {
            let marker_height = (cell_height * UNDERLINE_CURSOR_HEIGHT_FRACTION)
                .max(CURSOR_MARKER_MINIMUM_THICKNESS);
            Some((
                [origin[0], origin[1] + cell_height - marker_height],
                [cell_width * cell_span, marker_height],
            ))
        }
    }
}

/// 无字形的纯色四边形：空单元格背景、光标标记、字形缺失时的兜底。
///
/// `atlas_offset`/`atlas_size` 全零 = 采样不到任何字形轮廓，着色器只取背景色；
/// `bearing` 同理无意义。三处原先各写一遍同一份字面量，改动其中一处就会让
/// 光标在空单元格与文本格之间不一致，故收在此处。
fn push_flat_instance(
    instances: &mut Vec<CellInstance>,
    origin: [f32; 2],
    size: [f32; 2],
    cell_data_entry: &crate::terminal::ghostty_terminal::CellData,
    foreground: [f32; 4],
    background: [f32; 4],
    flags: u32,
) {
    instances.push(CellInstance {
        quad_origin: origin,
        atlas_offset: [0.0; 2],
        atlas_size: [0.0; 2],
        foreground,
        background,
        underline_color: cell_data_entry.underline_color,
        quad_size: size,
        flags: flags as f32,
        bearing: [0.0; 2],
    });
}

/// 构建单个网格行的实例。全量与增量构建器共用，
/// 使两条路径的单元级逻辑保持一致。
// 渲染热路径：参数由双构建路径共享调用，成组改结构体只增间接无收益。
pub(super) fn append_row_instances(
    cell_width: f32,
    cell_height: f32,
    ascent_pixels: f32,
    raster_scale: f32,
    atlas_width: f32,
    atlas_height: f32,
    cursor: CellCursor,
    highlights_by_row: &HashMap<i32, Vec<&SearchHighlight>, RandomState>,
    default_background: [f32; 4],
    font_pipeline: &mut crate::render::font::FontPipeline,
    instances: &mut Vec<CellInstance>,
    cell_row: &[crate::terminal::ghostty_terminal::CellData],
    cluster_text: &mut String,
) {
    // 无搜索高亮（常见情形）时整行跳过逐格查表：`cell_highlight` 要按行号哈希
    // 查表，逐格一次在整屏重建里就是 1920 次哈希；空表在行级一判即知。
    let has_highlights = !highlights_by_row.is_empty();
    for cell_data_entry in cell_row {
        // 非法码位由 codepoint_to_char 记录（每个构建都记，不再只有 debug），
        // 避免终端内容缺陷在 release 里被静默替换成方框而不留痕迹。
        let character = codepoint_to_char(cell_data_entry.codepoint);
        let cell_span = cell_data_entry.width.max(1) as f32;
        let quad_origin = [
            cell_data_entry.col as f32 * cell_width,
            cell_data_entry.row as f32 * cell_height,
        ];
        let mut foreground = cell_data_entry.foreground;
        let mut background = cell_data_entry.background;
        // SGR 7 反显：交换前景与背景色
        // 检查反显属性（新布局中为第 2 位，与旧路径
        // pack_style_flags → shader 所用的 `cell.reverse` 位位置一致）。
        // 与 termux TerminalRenderer.java:182-187 一致（选区与
        // reverseVideo 归入同一处前景/背景交换），也与 Ghostty 的
        // 渲染器反显处理一致。
        if (cell_data_entry.flags >> cell_flags::REVERSE) & 1 == 1 {
            std::mem::swap(&mut foreground, &mut background);
        }

        // 搜索高亮叠加（施加在终端烘焙的
        // 选区反显之上）。
        if has_highlights
            && let Some(hl) =
                cell_highlight(cell_data_entry.row, cell_data_entry.col, highlights_by_row)
        {
            apply_search_highlight(&mut foreground, &mut background, *hl);
        }
        // (spec cursor-rendering "宽字符光标几何"): 宽字符
        // 光标必须覆盖整个字形。build_cell_data 为每个宽字符只发出一条
        // width=2 的 CellData（尾部网格列没有自己的
        // CellData——review-1 已验证 internal/rendering.rs 会消费两列），故
        // 首条记录的 `cell_span` 已跨整个字形：光标盒按 cell_span 取宽即横跨两列。
        // 不存在需要另行标记的间隔单元记录。
        //
        // 三种样式里只有方块与**下划线**把 cell_span 乘进宽度（下面两处分支的
        // `quad_size` / `[cell_width * cell_span, marker_height]`）；**竖线不乘**——
        // 它是固定宽度的细条，乘了 cell_span 会在宽字符上变成两倍粗的短横，
        // 不再是竖线。竖线靠 `quad_origin` 锚在字形左边缘即可覆盖到字形：
        // 宽字符只有头列一条记录，原点即字形原点。
        let is_cursor = cursor.visible
            && cell_data_entry.row == cursor.row
            && cell_data_entry.col == cursor.col;
        // SGR 8：与 xterm 一致，保留背景与光标，跳过字形及全部前景装饰。
        let concealed = (cell_data_entry.flags >> cell_flags::CONCEAL) & 1 == 1;
        let emitted_flags = if concealed { 0 } else { cell_data_entry.flags };
        let mut effective_foreground = foreground;
        let mut effective_background = background;
        // 整格四边形：字形、空单元格背景与下划线光标宽度都取它；竖线/下划线
        // 光标标记由 [`cursor_marker_geometry`] 从它收缩。字形四边形**不**用
        // 收缩后的尺寸，否则组合记号等会被光标标记尺寸裁掉。
        let quad_size = [cell_width * cell_span, cell_height];
        // 方块光标恒为整格矩形（spec cursor-rendering「方块光标按整格几何」）：
        // termux 的 TerminalRenderer 把光标画在 [基线 − 行高, 基线]，即整格。
        // 跟随字形位图会让同一光标随格内字符变高变矮、越出本行——空单元格取
        // 'M' 位图高度，文本单元格取该字形位图高度，两者不等。
        if is_cursor && matches!(cursor.style, CursorStyle::Block) {
            // 方块光标整格反色（termux `invertCursorTextColor`，见
            // `docs/persistence/termux-app.md`）：背景取光标色、文字取原格背景色。
            // 旧实现把光标色按 0.7 叠在格背景上、前景不动，深色主题下白字压在
            // 半透明光标上对比度极低，「光标遮住文字」即由此而来。反色后文字与
            // 光标底的对比度由主题保证（光标色与背景色本就必须可区分）。
            // 取 `background` 而非 `default_background`：SGR 反显与搜索高亮已把
            // 该格的目标底色算进 `background`，用默认底色会让高亮格的光标文字
            // 变成另一种颜色。
            effective_background = cursor.color;
            effective_foreground = background;
        }
        if concealed
            || character == ' '
            || character == '\0'
            || cell_data_entry.codepoint == 0
            || cell_data_entry.codepoint == KITTY_PLACEHOLDER
        {
            // 与清除色同色的空白格不必发射：全量帧已由 Clear 铺底，
            // 部分帧由脏带清除实例铺底；省略后滚动整屏重绘的实例数按
            // 有效字符数而非网格面积增长。
            if !blank_cell_needs_quad(
                emitted_flags,
                is_cursor,
                effective_background,
                default_background,
            ) {
                continue;
            }
            // 空单元格：无光标时只发一个铺满全格的四边形；Bar/Underline 光标
            // 收缩为单元盒内的细标记，方块光标整格就是光标盒
            // （`cursor_marker_geometry` 返回 `None`）。
            //
            // 标记锚在单元盒，不锚字形位图（spec cursor-rendering
            // 「光标标记锚在单元盒」）：位图高度随字符变化，锚它会让同一个光标
            // 在空单元格与文本格之间变高变矮，降部字形还会把标记带出行界。
            let marker = if is_cursor {
                cursor_marker_geometry(cursor, quad_origin, cell_width, cell_height, cell_span)
            } else {
                None
            };
            // 标记只盖住细条，而全量帧以 Clear(主题背景) 铺底、部分帧只清脏带：
            // 非默认底色（选区/SGR 48）或装饰线必须另有整格四边形承载，
            // 否则细光标所在的空白格重绘后会丢掉底色与下划线。
            if marker.is_none()
                || blank_cell_needs_quad(
                    emitted_flags,
                    false,
                    effective_background,
                    default_background,
                )
            {
                push_flat_instance(
                    instances,
                    quad_origin,
                    quad_size,
                    cell_data_entry,
                    effective_foreground,
                    effective_background,
                    emitted_flags,
                );
            }
            if let Some((marker_origin, marker_size)) = marker {
                push_flat_instance(
                    instances,
                    marker_origin,
                    marker_size,
                    cell_data_entry,
                    effective_foreground,
                    cursor.marker_background(),
                    emitted_flags,
                );
            }
            continue;
        }

        // 非空单元格：先发字形四边形，再在其上追加光标标记
        // （Bar/Underline 时才有），使细竖线/下划线
        // 在字形之上可见。
        // 主体字形——单元带 bold/italic 标志时使用同族
        // styled 字面，否则合成）。
        let cell_bold = (cell_data_entry.flags >> cell_flags::BOLD) & 1 == 1;
        let cell_italic = (cell_data_entry.flags >> cell_flags::ITALIC) & 1 == 1;
        // 组合字形延续符的簇整形（组合记号、emoji ZWJ
        // 序列）：整簇只整形一次，使每个记号落到字体
        // 指定的位置。无附加符的单元完全跳过整形，
        // 常见情形零开销。整形文本写入跨单元复用的缓冲，
        // 命中整形缓存时整条路径不做堆分配。
        let mut cluster_shaped: Option<Arc<[crate::render::font::ShapedGlyphInfo]>> = None;
        if cell_data_entry
            .grapheme_extra
            .iter()
            .any(|&codepoint| codepoint != 0)
        {
            cluster_text_into(cluster_text, character, &cell_data_entry.grapheme_extra);
            if cluster_text.chars().count() > 1 {
                cluster_shaped =
                    Some(font_pipeline.shape_run_styled(cluster_text, cell_bold, cell_italic));
            }
        }
        // 整簇整形成单个字形（ZWJ emoji）时，它取代主体字形
        // 成为主体四边形；带位置的记号交由下面的 overlay
        // 循环处理。
        let merged_cluster_glyph = cluster_shaped
            .as_deref()
            .filter(|glyphs| glyphs.len() == 1)
            .and_then(|glyphs| glyphs.first())
            .and_then(|glyph| {
                font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id)
            });
        let has_merged_glyph = merged_cluster_glyph.is_some();
        if let Some(info) = merged_cluster_glyph
            .or_else(|| font_pipeline.glyph_information_styled(character, cell_bold, cell_italic))
        {
            let (atlas_offset, atlas_size, bearing_x) = info.atlas_uv(atlas_width, atlas_height);
            // info.height 是光栅化位图高度，单位为物理像素（已乘 raster_scale）
            // ——与物理网格单元高度同单位，居中回退同样按物理像素给出。
            // 该判定与叠加层共用 `glyph_bearing_y`：两处分叉会让组合记号/ZWJ
            // 表情与基字形错开，而那类偏移没有任何用例覆盖。
            let glyph_h_px = info.height as f32;
            let (bearing_y, _) = crate::render::font::pipeline::glyph_bearing_y(
                ascent_pixels,
                raster_scale,
                info.placement.top as f32,
                glyph_h_px,
                cell_height,
            );
            // 方块光标不加修饰：quad 本身就是整格光标盒。
            instances.push(CellInstance {
                quad_origin,
                atlas_offset,
                atlas_size,
                foreground: effective_foreground,
                background: effective_background,
                underline_color: cell_data_entry.underline_color,
                quad_size,
                flags: cell_data_entry.flags as f32,
                bearing: [bearing_x, bearing_y],
            });

            // 竖线/下划线光标：在字形之上追加细标记（原文颜色不动，
            // 标记盖在上层保证可见）；标记锚在单元盒，不锚字形位图。
            if is_cursor
                && let Some((marker_origin, marker_size)) =
                    cursor_marker_geometry(cursor, quad_origin, cell_width, cell_height, cell_span)
            {
                push_flat_instance(
                    instances,
                    marker_origin,
                    marker_size,
                    cell_data_entry,
                    effective_foreground,
                    cursor.marker_background(),
                    cell_data_entry.flags,
                );
            }

            // 组合字形延续码点（组合记号、emoji ZWJ 等），
            // 作为 overlay 实例渲染在主体字形之上，位置由上面的
            // 簇整形给出；整形无可用结果时按码点回退
            // 渲染。
            if let Some(glyphs) = cluster_shaped.as_deref()
                && glyphs.len() > 1
            {
                for glyph in &glyphs[1..] {
                    if let Some(info) =
                        font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id)
                    {
                        let shaped_origin = [
                            quad_origin[0] + glyph.x_offset,
                            quad_origin[1] + glyph.y_offset,
                        ];
                        instances.push(font_pipeline.shaped_overlay_instance(
                            &info,
                            crate::render::font::OverlayQuad {
                                origin: shaped_origin,
                                size: quad_size,
                                foreground: effective_foreground,
                                background: effective_background,
                                deco: cell_data_entry.underline_color,
                                flags: cell_data_entry.flags as f32,
                            },
                            cell_height,
                        ));
                    }
                }
            } else if !has_merged_glyph {
                for codepoint in &cell_data_entry.grapheme_extra {
                    if *codepoint == 0 {
                        continue;
                    }
                    if let Some(overlay) = font_pipeline.overlay_glyph_instance(
                        *codepoint,
                        crate::render::font::OverlayQuad {
                            origin: quad_origin,
                            size: quad_size,
                            foreground: effective_foreground,
                            background: effective_background,
                            deco: cell_data_entry.underline_color,
                            flags: cell_data_entry.flags as f32,
                        },
                        cell_height,
                    ) {
                        instances.push(overlay);
                    }
                }
            }
        } else {
            // 图集中找不到字形——推入一个空背景四边形，使
            // 单元格背景（含选区/高亮）可见。
            // 字体图集重建后字形即出现。
            push_flat_instance(
                instances,
                quad_origin,
                quad_size,
                cell_data_entry,
                effective_foreground,
                effective_background,
                cell_data_entry.flags,
            );
        }
    }
}
