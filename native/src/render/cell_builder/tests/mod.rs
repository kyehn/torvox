//! 单元格实例构建器的测试夹具。
//!
//! 外观类（颜色、光标、搜索高亮）与缓存类（增量重建、图集驱逐）各自成模块，
//! 共享的单元构造与整帧构建夹具留在本文件。

use super::*;
use crate::terminal::ghostty_terminal::CellData;

mod appearance;
mod cache;

fn cell_data(
    row: u32,
    col: u32,
    character: char,
    foreground: [f32; 4],
    background: [f32; 4],
    flags: u32,
) -> CellData {
    CellData {
        codepoint: character as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground,
        background,
        underline_color: foreground,
        flags,
        row,
        col,
    }
}

fn build(
    cells: &[CellData],
    cursor: CellCursor,
    highlights: &[SearchHighlight],
) -> Vec<CellInstance> {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    let mut instances = Vec::new();
    let result = build_instances_from_cell_data(
        cells,
        CellInstanceConfig {
            rows: 24,
            cols: 80,
            grid_cell_width: 1024.0 / 80.0,
            grid_cell_height: 1024.0 / 24.0,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: highlights,
            default_background: [0.0; 4],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(
        result.is_some(),
        "build should succeed with a font pipeline"
    );
    instances
}
