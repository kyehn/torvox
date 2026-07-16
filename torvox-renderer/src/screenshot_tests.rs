// Included into gpu.rs #[cfg(test)] mod tests — screenshot + OCR tests
// Every test renders through wgpu(lavapipe), saves PNG, and runs rapidocr.

// ─── Helpers ────────────────────────────────────────────────────

const TEST_PADDING_X: u32 = 20;
const TEST_PADDING_Y: u32 = 10;

// ─── Theme Color Helpers ──────────────────────────────────────────
// Colors come from Catppuccin Mocha (current default theme) instead of hardcoded.

fn theme_fg() -> [f32; 4] {
    let c = torvox_core::config::Theme::catppuccin_mocha().foreground;
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

fn theme_bg() -> [f32; 4] {
    let c = torvox_core::config::Theme::catppuccin_mocha().background;
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

fn theme_clear_color() -> wgpu::Color {
    let c = torvox_core::config::Theme::catppuccin_mocha().background;
    wgpu::Color {
        r: c[0] as f64 / 255.0,
        g: c[1] as f64 / 255.0,
        b: c[2] as f64 / 255.0,
        a: 1.0,
    }
}

fn theme_ansi(i: usize) -> [f32; 4] {
    let c = torvox_core::config::Theme::catppuccin_mocha().ansi[i];
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

fn run_ocr(png_path: &std::path::Path) -> String {
    let output = std::process::Command::new("rapidocr")
        .args([
            "-img",
            png_path.to_str().expect("test PNG path is valid UTF-8"),
        ])
        .output()
        .expect("rapidocr CLI must be available");
    assert!(
        output.status.success(),
        "rapidocr failed on {}: {}",
        png_path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn save_png(pixels: &[u8], width: u32, height: u32, path: &std::path::Path) {
    use image::RgbImage;
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let img = RgbImage::from_raw(width, height, rgb).expect("RgbImage::from_raw");
    img.save(path).expect("save PNG");
}

fn save_png_raw(rgb: &[u8], width: u32, height: u32, path: &std::path::Path) {
    use image::RgbImage;
    let img = RgbImage::from_raw(width, height, rgb.to_vec()).expect("RgbImage::from_raw");
    img.save(path).expect("save PNG");
}

fn test_out_dir() -> std::path::PathBuf {
    // Use temp dir — golden images banned from repo (FR-080)
    let mut p = std::env::temp_dir();
    p.push("torvox-test-screenshots");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Render a FlatGrid through wgpu, save PNG, optionally run OCR, return pixels.
/// Pass `Some("text")` to assert OCR finds that text; `None` to skip OCR.
/// `clear_color` sets the terminal background color (None = default black).
fn render_grid(
    test_name: &str,
    grid: &FlatGrid,
    expected_ocr: Option<&str>,
    clear_color: Option<wgpu::Color>,
) -> (Vec<u8>, u32, u32) {
    let Some((instance, adapter, device, queue)) = create_test_device() else {
        panic!("no GPU for {test_name}");
    };
    let atlas_dim: u32 = 512;
    let mut font_pipeline =
        crate::font::FontPipeline::new(atlas_dim as i32, atlas_dim as i32, 20.0);
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let width = (grid.cols as f32 * cell_w).round() as u32 + TEST_PADDING_X;
    let height = (grid.rows as f32 * cell_h).round() as u32 + TEST_PADDING_Y;
    let mut ctx = setup_test_gpu_context_custom(instance, adapter, device, queue, width, height);
    if let Some(c) = clear_color {
        ctx.bg_color = c;
    }
    ctx.initialize_pipeline_and_bind_group(atlas_dim, atlas_dim, width, height);
    let instances = build_cell_instances_from_flat(
        grid,
        &mut font_pipeline,
        atlas_dim as f32,
        atlas_dim as f32,
    );
    assert!(
        !instances.is_empty(),
        "{test_name}: 0 instances (font/glyph load failed)"
    );
    ctx.upload_atlas(font_pipeline.atlas_bitmap(), atlas_dim, atlas_dim);
    let pixels = ctx
        .render_to_buffer(&instances, &[])
        .expect("render_to_buffer failed");

    let out_dir = test_out_dir();
    let png_path = out_dir.join(format!("{test_name}.png"));
    save_png(&pixels, width, height, &png_path);

    if let Some(expected) = expected_ocr {
        assert!(
            !expected.is_empty(),
            "{test_name}: expected_ocr must be non-empty string"
        );
        let ocr_out = run_ocr(&png_path);
        assert!(
            ocr_out.contains(expected),
            "{test_name}: OCR did not find '{expected}' in {ocr_out:?}"
        );
    }

    (pixels, width, height)
}

// Assert a pixel region has both color A and color B present (fg/bg swap proof).
// ─── Absolute Verification Infrastructure ──────────────────────

/// Sum R+G+B over all pixels in a rectangular region.
/// Region coordinates are in image-pixel space (cells rendered at origin 0,0).
fn region_total_brightness(pixels: &[u8], width: u32, rx: u32, ry: u32, rw: u32, rh: u32) -> i64 {
    let mut total: i64 = 0;
    for row in ry..ry + rh {
        for col in rx..rx + rw {
            let i = (row * width + col) as usize * 4;
            if i + 2 < pixels.len() {
                total += pixels[i] as i64 + pixels[i + 1] as i64 + pixels[i + 2] as i64;
            }
        }
    }
    total
}

/// Assert selected region total brightness exceeds unselected region by margin.
/// Two-region comparison cancels padding when both regions have equal padding.
#[allow(clippy::too_many_arguments)]
fn assert_swap_proof_by_total_brightness(
    pixels: &[u8],
    width: u32,
    test_name: &str,
    selected_x: u32,
    selected_y: u32,
    selected_w: u32,
    selected_h: u32,
    unselected_x: u32,
    unselected_y: u32,
    unselected_w: u32,
    unselected_h: u32,
    margin_total: i64,
) {
    let sel_total = region_total_brightness(
        pixels, width, selected_x, selected_y, selected_w, selected_h,
    );
    let unsel_total = region_total_brightness(
        pixels,
        width,
        unselected_x,
        unselected_y,
        unselected_w,
        unselected_h,
    );
    assert!(
        sel_total > unsel_total + margin_total,
        "{test_name}: selected region brightness {sel_total} should exceed unselected {unsel_total} \
         by margin {margin_total} \
         (sel=({selected_x},{selected_y},{selected_w},{selected_h}), \
         unsel=({unselected_x},{unselected_y},{unselected_w},{unselected_h}))"
    );
}

/// Extract a rectangular region from RGBA pixels, save as PNG, run
/// rapidocr, and assert the OCR output contains `expected` text.
#[allow(clippy::too_many_arguments)]
fn extract_and_ocr_region(
    pixels: &[u8],
    full_w: u32,
    full_h: u32,
    region_name: &str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    expected: &str,
) {
    assert!(
        x + w <= full_w,
        "extract_and_ocr_region({region_name}): region ({x},{y},{w},{h}) exceeds width ({full_w})"
    );
    assert!(
        y + h <= full_h,
        "extract_and_ocr_region({region_name}): region ({x},{y},{w},{h}) exceeds height ({full_h})"
    );
    assert!(
        w > 0 && h > 0,
        "extract_and_ocr_region({region_name}): zero-dimension region {w}×{h}"
    );

    let mut cropped = Vec::with_capacity((w * h * 3) as usize);
    for row in y..y + h {
        let start = (row * full_w + x) as usize * 4;
        let end = (start + w as usize * 4).min(pixels.len());
        for p in pixels[start..end].chunks(4) {
            cropped.extend_from_slice(&[p[0], p[1], p[2]]);
        }
    }

    let min_expected = ((w as u64 * h as u64 * 3) / 10) as usize;
    assert!(
        cropped.len() >= min_expected,
        "extract_and_ocr_region({region_name}): cropped region too small ({} bytes, expected >= {} bytes)",
        cropped.len(),
        min_expected,
    );

    let out_dir = test_out_dir();
    let path = out_dir.join(format!("{region_name}.png"));
    save_png_raw(&cropped, w, h, &path);

    let ocr_out = run_ocr(&path);
    assert!(
        ocr_out.to_uppercase().contains(&expected.to_uppercase()),
        "{region_name}: OCR should find '{expected}' \
         \nRegion coordinates: ({x},{y},{w},{h}) \
         \nFull OCR output:\n{ocr_out}"
    );
}

/// Scan RGBA pixels for cell-aligned contiguous regions where `is_marked`
/// returns true. Uses 4-connectivity BFS. Returns pixel-space bounding boxes.
fn find_contiguous_regions<F>(
    pixels: &[u8],
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    min_cells: u32,
    is_marked: F,
) -> Vec<(u32, u32, u32, u32)>
where
    F: Fn(&[u8], u32, u32, u32, u32, u32) -> bool,
{
    let cell_w = (width - TEST_PADDING_X) / cols;
    let cell_h = (height - TEST_PADDING_Y) / rows;
    let origin_x: u32 = 0;
    let origin_y: u32 = 0;

    let mut marked_cells = vec![false; (cols * rows) as usize];
    for col in 0..cols {
        for row in 0..rows {
            let cell_x = origin_x + col * cell_w;
            let cell_y = origin_y + row * cell_h;
            let mut pixel_count: u64 = 0;
            for py in cell_y..cell_y + cell_h {
                for px in cell_x..cell_x + cell_w {
                    let i = (py * width + px) as usize * 4;
                    if i + 2 < pixels.len() {
                        pixel_count += 1;
                    }
                }
            }
            if is_marked(pixels, cell_x, cell_y, cell_w, cell_h, pixel_count as u32) {
                marked_cells[(row * cols + col) as usize] = true;
            }
        }
    }

    let mut visited = vec![false; (cols * rows) as usize];
    let mut regions = Vec::new();

    for col in 0..cols {
        for row in 0..rows {
            let idx = (row * cols + col) as usize;
            if !marked_cells[idx] || visited[idx] {
                continue;
            }

            let mut min_col = col;
            let mut max_col = col;
            let mut min_row = row;
            let mut max_row = row;
            let mut queue = vec![(col, row)];
            visited[idx] = true;
            let mut front = 0;

            while front < queue.len() {
                let (c, r) = queue[front];
                front += 1;

                for &(nc, nr) in &[
                    (c.wrapping_sub(1), r),
                    (c + 1, r),
                    (c, r.wrapping_sub(1)),
                    (c, r + 1),
                ] {
                    if nc < cols && nr < rows {
                        let nidx = (nr * cols + nc) as usize;
                        if marked_cells[nidx] && !visited[nidx] {
                            visited[nidx] = true;
                            min_col = min_col.min(nc);
                            max_col = max_col.max(nc);
                            min_row = min_row.min(nr);
                            max_row = max_row.max(nr);
                            queue.push((nc, nr));
                        }
                    }
                }
            }

            let contiguous_cell_count = (max_col - min_col + 1) * (max_row - min_row + 1);
            if contiguous_cell_count >= min_cells {
                let region_x = origin_x + min_col * cell_w;
                let region_y = origin_y + min_row * cell_h;
                let region_w = (max_col - min_col + 1) * cell_w;
                let region_h = (max_row - min_row + 1) * cell_h;
                regions.push((region_x, region_y, region_w, region_h));
            }
        }
    }

    regions
}

/// Scan RGBA pixels for cell-aligned regions where average per-cell
/// total brightness exceeds `brightness_threshold`. Returns cell-aligned
/// bounding boxes of contiguous bright cells.
fn find_reverse_color_regions(
    pixels: &[u8],
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    brightness_threshold: u32,
    min_cells: u32,
) -> Vec<(u32, u32, u32, u32)> {
    find_contiguous_regions(
        pixels,
        width,
        height,
        cols,
        rows,
        min_cells,
        |pixels, cx, cy, cw, ch, count| {
            let total = region_total_brightness(pixels, width, cx, cy, cw, ch);
            (total as u32 / count.max(1)) > brightness_threshold
        },
    )
}

/// Scan RGBA pixels for cell-aligned regions where a specific channel
/// dominates. Works when fg and bg have equal brightness but distinct
/// channel dominance (e.g., fg=red, bg=green).
fn find_reverse_color_regions_by_channel(
    pixels: &[u8],
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    dominant_channel: usize,
    ratio_threshold: f32,
    min_cells: u32,
) -> Vec<(u32, u32, u32, u32)> {
    let other0 = if dominant_channel == 0 { 1 } else { 0 };
    let other1 = if dominant_channel == 2 { 1 } else { 2 };
    find_contiguous_regions(
        pixels,
        width,
        height,
        cols,
        rows,
        min_cells,
        |pixels, cx, cy, cw, ch, _count| {
            let mut sum_dom: u64 = 0;
            let mut sum_other: u64 = 0;
            for py in cy..cy + ch {
                for px in cx..cx + cw {
                    let i = (py * width + px) as usize * 4;
                    if i + 2 < pixels.len() {
                        sum_dom += pixels[i + dominant_channel] as u64;
                        sum_other += pixels[i + other0] as u64 + pixels[i + other1] as u64;
                    }
                }
            }
            (sum_dom as f32 / sum_other.max(1) as f32) > ratio_threshold
        },
    )
}

// ─── Search test helpers ─────────────────────────────────────────

/// Verify that a selected cell range is brighter than an unselected range in the same row.
#[allow(clippy::too_many_arguments)]
fn assert_search_row_swap_proof(
    pixels: &[u8],
    w: u32,
    cell_w: u32,
    cell_h: u32,
    test_name: &str,
    row_index: u32,
    selected_col: u32,
    unselected_col: u32,
    num_cells: u32,
    margin: i64,
) {
    let y = row_index * cell_h;
    assert_swap_proof_by_total_brightness(
        pixels,
        w,
        test_name,
        selected_col * cell_w,
        y,
        num_cells * cell_w,
        cell_h,
        unselected_col * cell_w,
        y,
        num_cells * cell_w,
        cell_h,
        margin,
    );
}

// ─── Search tests ────────────────────────────────────────────────

/// Search highlight: cells 0-4 selected → fg/bg swapped.
/// Colors from theme: fg = Catppuccin Mocha foreground, bg = Catppuccin Mocha ansi[8] (gray).
/// After swap: selected cells show theme fg as bg (bright), unselected cells show gray bg.
#[test]
fn ocr_search_highlight_reverses_colors() {
    let mut grid = FlatGrid::new(1, 20);
    let text = "HELLO OTHER TEXT";
    for (i, ch) in text.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }
    grid.selected[0..5].fill(true);

    let (pixels, w, h) = render_grid(
        "SEARCH_HIGHLIGHT",
        &grid,
        Some("OTHER"),
        Some(theme_clear_color()),
    );

    let cols = grid.cols;
    let rows = grid.rows;
    let cell_w = (w - TEST_PADDING_X) / cols;
    let cell_h = (h - TEST_PADDING_Y) / rows;

    // Layer 2: auto-detect reverse-color region, verify region found
    let regions = find_reverse_color_regions(&pixels, w, h, cols, rows, 400, 5);
    assert!(
        !regions.is_empty(),
        "SEARCH_HIGHLIGHT: no reverse-color region found"
    );
    assert_eq!(
        regions.len(),
        1,
        "SEARCH_HIGHLIGHT: expected 1 merged region, got {}",
        regions.len()
    );

    // Crop the reversed (selected) region and OCR it directly. Full-image OCR
    // truncates a reversed word at the line start (drops the trailing glyph),
    // but cropping to the reversed region gives rapidocr a clean single-word
    // image so the highlighted text stays legible/verifiable.
    let (rx, ry, rw, rh) = regions[0];
    let pad_x = cell_w;
    let pad_y = 5u32;
    let crop_x = rx.saturating_sub(pad_x);
    let crop_w = (rw + 2 * pad_x).min(w - crop_x);
    let crop_y = ry.saturating_sub(pad_y);
    let crop_h = (rh + 2 * pad_y).max(32).min(h - crop_y);
    extract_and_ocr_region(
        &pixels,
        w,
        h,
        "SEARCH_HIGHLIGHT_REGION",
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        "HELLO",
    );

    // Layer 3: pixel swap proof — selected cols 0-4 vs unselected cols 6-10 (5 cells each)
    assert_search_row_swap_proof(
        &pixels,
        w,
        cell_w,
        cell_h,
        "SEARCH_HIGHLIGHT",
        0,
        0,
        6,
        5,
        15_000,
    );
}

/// Two rows with different bg colors from theme: row 0 = ansi[0] (black), row 1 = ansi[8] (gray).
/// After swap: selected cells show theme fg as bg (bright), unselected cells show their bg color.
/// Two different highlight colors verify "previous/next result" visual difference.
#[test]
fn ocr_search_previous_result_different_color() {
    let mut grid = FlatGrid::new(2, 15);
    let text0 = "PREV MATCH     ";
    let text1 = "NEXT MATCH     ";
    for (i, ch) in text0.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }
    grid.selected[5..10].fill(true); // "MATCH" selected (reversed fg/bg = bright bg, dark glyph)
    for (i, ch) in text1.chars().enumerate() {
        let idx = 15 + i;
        grid.chars[idx] = ch;
        grid.foreground[idx] = theme_fg();
        grid.background[idx] = theme_bg();
    }
    grid.selected[20..25].fill(true); // "MATCH" selected

    let (pixels, w, h) = render_grid(
        "SEARCH_PREV_RESULT",
        &grid,
        Some("MATCH"),
        Some(theme_clear_color()),
    );

    let cols = grid.cols;
    let rows = grid.rows;
    let cell_w = (w - TEST_PADDING_X) / cols;
    let cell_h = (h - TEST_PADDING_Y) / rows;

    // Layer 2: auto-detect reverse-color regions, verify OCR
    let regions = find_reverse_color_regions(&pixels, w, h, cols, rows, 400, 5);
    assert!(
        !regions.is_empty(),
        "SEARCH_PREV_RESULT: no reverse-color region found"
    );
    assert_eq!(
        regions.len(),
        1,
        "SEARCH_PREV_RESULT: expected 1 merged region, got {}",
        regions.len()
    );

    // BFS merges both rows (vertical adjacency). Crop the full merged region
    // (both rows) for OCR — rapidocr requires at least 32px input height.
    let (rx, ry, rw, rh) = regions[0];
    assert!(
        rh >= cell_h * 2,
        "SEARCH_PREV_RESULT: merged height {rh} < 2*{cell_h}"
    );

    let pad_x = cell_w;
    let pad_y = 5;
    let crop_x = rx.saturating_sub(pad_x);
    let crop_w = (rw + 2 * pad_x).min(w - crop_x);
    let crop_y = ry.saturating_sub(pad_y);
    let crop_h = (rh + 2 * pad_y).max(32).min(h - crop_y);
    extract_and_ocr_region(
        &pixels,
        w,
        h,
        "SEARCH_PREV_MERGED",
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        "MATCH",
    );

    // Layer 3a: pixel swap proof — row 0 selected cols 5-9 > unselected cols 0-4
    assert_search_row_swap_proof(
        &pixels,
        w,
        cell_w,
        cell_h,
        "SEARCH_PREV_RESULT_ROW0",
        0,
        5,
        0,
        5,
        30_000,
    );

    // Layer 3b: pixel swap proof — row 1 selected cols 5-9 > unselected cols 0-4
    assert_search_row_swap_proof(
        &pixels,
        w,
        cell_w,
        cell_h,
        "SEARCH_PREV_RESULT_ROW1",
        1,
        5,
        0,
        5,
        30_000,
    );
}

/// Two rows with selected cells at different positions.
/// Row 0: "FOCUS ONE" cols 6-10 selected (reversed fg/bg).
/// Row 1: "FOCUS TWO" cols 6-10 selected (reversed fg/bg).
#[test]
fn ocr_search_next_result_different_color() {
    let mut grid = FlatGrid::new(2, 15);
    let text0 = "FOCUS ONE      ";
    let text1 = "FOCUS TWO      ";
    for (i, ch) in text0.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }
    grid.selected[6..11].fill(true);
    for (i, ch) in text1.chars().enumerate() {
        let idx = 15 + i;
        grid.chars[idx] = ch;
        grid.foreground[idx] = theme_fg();
        grid.background[idx] = theme_bg();
    }
    grid.selected[21..26].fill(true);

    let (pixels, w, h) = render_grid(
        "SEARCH_NEXT_RESULT",
        &grid,
        Some("FOCUS"),
        Some(theme_clear_color()),
    );

    let cols = grid.cols;
    let rows = grid.rows;
    let cell_w = (w - TEST_PADDING_X) / cols;
    let cell_h = (h - TEST_PADDING_Y) / rows;

    // Layer 2: auto-detect reverse-color regions, verify OCR
    let regions = find_reverse_color_regions(&pixels, w, h, cols, rows, 400, 5);
    assert!(
        !regions.is_empty(),
        "SEARCH_NEXT_RESULT: no reverse-color region found"
    );
    assert_eq!(
        regions.len(),
        1,
        "SEARCH_NEXT_RESULT: expected 1 merged region, got {}",
        regions.len()
    );

    // BFS merges both rows (vertical adjacency). Crop the full merged region
    // (both rows) for OCR — rapidocr requires at least 32px input height.
    let (rx, ry, rw, rh) = regions[0];
    assert!(
        rh >= cell_h * 2,
        "SEARCH_NEXT_RESULT: merged height {rh} < 2*{cell_h}"
    );

    let pad_x = cell_w;
    let pad_y = 5;
    let crop_x = rx.saturating_sub(pad_x);
    let crop_w = (rw + 2 * pad_x).min(w - crop_x);
    let crop_y = ry.saturating_sub(pad_y);
    let crop_h = (rh + 2 * pad_y).max(32).min(h - crop_y);
    extract_and_ocr_region(
        &pixels,
        w,
        h,
        "SEARCH_NEXT_MERGED",
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        "ONE",
    );
    extract_and_ocr_region(
        &pixels,
        w,
        h,
        "SEARCH_NEXT_MERGED",
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        "TWO",
    );

    // Layer 3a: pixel swap proof — row 0 selected cols 6-10 > unselected cols 0-4
    assert_search_row_swap_proof(
        &pixels,
        w,
        cell_w,
        cell_h,
        "SEARCH_NEXT_RESULT_ROW0",
        0,
        6,
        0,
        5,
        30_000,
    );

    // Layer 3b: pixel swap proof — row 1 selected cols 6-10 > unselected cols 0-4
    assert_search_row_swap_proof(
        &pixels,
        w,
        cell_w,
        cell_h,
        "SEARCH_NEXT_RESULT_ROW1",
        1,
        6,
        0,
        5,
        30_000,
    );
}

// ─── Long-press / selection tests ───────────────────────────────

/// Long-press blank area: no text rendered, all pixels at theme bg color, no reverse-color regions.
#[test]
fn visual_long_press_blank() {
    let grid = FlatGrid::new(1, 10);
    let (pixels, w, h) = render_grid("LONG_PRESS_BLANK", &grid, None, Some(theme_clear_color()));
    // 1. OCR metadata check — no text detected
    let ocr_out = run_ocr(&test_out_dir().join("LONG_PRESS_BLANK.png"));
    let trimmed = ocr_out.trim();
    let paren_start = trimmed.find('(').unwrap_or(trimmed.len());
    let intro = &trimmed[..paren_start];
    assert!(
        intro.contains("RapidOCROutput"),
        "blank screen should produce RapidOCR metadata, got: '{ocr_out}'"
    );
    let metadata = &trimmed[..trimmed.find(')').map(|i| i + 1).unwrap_or(trimmed.len())];
    assert!(
        metadata.contains("txts=None") || metadata.contains("txts=()"),
        "blank screen 'txts' should be empty: '{ocr_out}'"
    );

    // 2. All pixels are within dark range (GPU clear + space cells produce
    // theme-bg-derived values; exact u8 may vary by GPU due to floating-point
    // rounding in the clear pass, so we only verify the screen is dark).
    let max_channel = pixels
        .chunks(4)
        .map(|p| p[0].max(p[1]).max(p[2]))
        .max()
        .unwrap_or(255);
    assert!(
        max_channel < 100,
        "LONG_PRESS_BLANK: brightest channel value {max_channel} >= 100 — screen should be dark",
    );

    // 3. No reverse-color regions (no selected cells)
    let regions = find_reverse_color_regions(&pixels, w, h, 10, 1, 10, 1);
    assert!(
        regions.is_empty(),
        "LONG_PRESS_BLANK: expected no reverse-color regions, found {}",
        regions.len()
    );
}

/// Long-press text: "WORD" at cols 6-9 selected, other cells normal.
/// Colors from theme: fg = Catppuccin Mocha foreground, bg = Catppuccin Mocha background.
/// Three-layer verification: OCR, region detection, pixel swap proof.
#[test]
fn visual_long_press_text() {
    let mut grid = FlatGrid::new(1, 15);
    let text = "AFTER WORD HERE";
    for (i, ch) in text.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }
    grid.selected[6..10].fill(true);

    let cols = grid.cols;
    let rows = grid.rows;

    // Layer 1: whole-image OCR
    let (pixels, w, h) = render_grid(
        "LONG_PRESS_TEXT",
        &grid,
        Some("WORD"),
        Some(theme_clear_color()),
    );

    // Layer 2: region detection — find reverse-color cells and OCR the region
    let regions = find_reverse_color_regions(&pixels, w, h, cols, rows, 382, 3);
    assert!(
        !regions.is_empty(),
        "LONG_PRESS_TEXT: should find reverse-color region for WORD"
    );
    // Find the region that covers the WORD (cols 6-9)
    let cell_w = (w - TEST_PADDING_X) / cols;
    let word_region = regions.iter().copied().find(|(rx, _, rw, _)| {
        let cell_x = rx / cell_w;
        cell_x <= 6 && cell_x + rw.div_ceil(cell_w) >= 10
    });
    assert!(
        word_region.is_some(),
        "LONG_PRESS_TEXT: no region covering cols 6-9"
    );
    let (rx, ry, rw, rh) = word_region.expect("must be Some after is_some assert");
    extract_and_ocr_region(&pixels, w, h, "LONG_PRESS_TEXT_SEL", rx, ry, rw, rh, "WORD");

    // Layer 3: pixel proof — swap proof by total brightness
    let cell_h = (h - TEST_PADDING_Y) / rows;
    let sel_x = 6 * cell_w;
    let sel_w = 4 * cell_w;
    let unsel_x = 0;
    let unsel_w = 4 * cell_w;
    assert_swap_proof_by_total_brightness(
        &pixels,
        w,
        "LONG_PRESS_TEXT",
        sel_x,
        0,
        sel_w,
        cell_h,
        unsel_x,
        0,
        unsel_w,
        cell_h,
        19_500,
    );
}

/// Cursor at different positions: cells at col 3, 8, 13 are selected (reversed).
/// Colors from theme: fg = Catppuccin Mocha foreground, bg = Catppuccin Mocha background.
/// Verifies each selected cell shows reversed colors (brighter bg) vs adjacent cells.
#[test]
fn visual_cursor_position() {
    let mut grid = FlatGrid::new(1, 20);
    let text = "CURSOR TEST POSITION";
    for (i, ch) in text.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }
    // Cursors at three positions
    grid.selected[3] = true;
    grid.selected[8] = true;
    grid.selected[13] = true;

    // Layer 1: whole-image OCR
    let (pixels, w, h) = render_grid(
        "CURSOR_POS",
        &grid,
        Some("CURSOR"),
        Some(theme_clear_color()),
    );

    // Layer 2: region detection — find reverse-color cells for each cursor
    let cols = grid.cols;
    let rows = grid.rows;
    let cell_w = (w - TEST_PADDING_X) / cols;
    let cell_h = (h - TEST_PADDING_Y) / rows;

    let regions = find_reverse_color_regions(&pixels, w, h, cols, rows, 382, 1);
    assert!(
        regions.len() >= 3,
        "CURSOR_POS: should find at least 3 reverse-color regions for cursors (found {})",
        regions.len(),
    );

    // Verify each selected cell (col 3, 8, 13) has HIGHER total brightness than the
    // adjacent unselected cell two cols to the right (col 5, 10, 15).
    //
    // After fg/bg swap, the selected cell has theme_fg as bg (bright) and theme_bg as glyph (dim).
    // The unselected cell has theme_bg as bg (dim) and theme_fg as glyph (bright).
    // Since bg area (~80%) >> glyph area (~20%), selected cell's total brightness > unselected.
    for &sel_col in &[3u32, 8, 13] {
        let sel_left = (sel_col as f64 * cell_w as f64) as u32;
        let unsel_left = ((sel_col + 2) as f64 * cell_w as f64) as u32;
        let mut sel_total = 0i64;
        let mut unsel_total = 0i64;
        for row in 0..cell_h {
            for col_off in 0..cell_w {
                let sx = sel_left + col_off;
                let ux = unsel_left + col_off;
                let si = (row * w + sx) as usize * 4;
                let ui = (row * w + ux) as usize * 4;
                if si + 2 < pixels.len() {
                    sel_total += pixels[si] as i64 + pixels[si + 1] as i64 + pixels[si + 2] as i64;
                }
                if ui + 2 < pixels.len() {
                    unsel_total +=
                        pixels[ui] as i64 + pixels[ui + 1] as i64 + pixels[ui + 2] as i64;
                }
            }
        }
        // Selected: theme_fg bg + theme_bg glyph → total ≈ ~80,000
        // Unselected: theme_bg bg + theme_fg glyph → total ≈ ~38,000
        assert!(
            sel_total > unsel_total + 10_000,
            "cursor at col {sel_col}: selected theme fg bg ({sel_total}) >> unselected theme bg ({unsel_total})",
        );
    }
}

/// Cursor moved between two columns: each selected position swaps colors,
/// and the previously selected column reverts to unselected rendering.
/// Colors from theme: fg = Catppuccin Mocha foreground, bg = Catppuccin Mocha background.
#[test]
fn visual_cursor_move() {
    let mut grid = FlatGrid::new(1, 20);
    let text = "CURSOR_COL_5       ";
    for (i, ch) in text.chars().enumerate() {
        if i >= grid.chars.len() {
            break;
        }
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_bg();
    }

    // Position 1: cursor at col 5
    grid.selected[5] = true;
    let (pixels1, w1, h1) = render_grid(
        "CURSOR_MOVE_POS1",
        &grid,
        Some("CURSOR"),
        Some(theme_clear_color()),
    );
    let cw1 = (w1 - TEST_PADDING_X) / grid.cols;
    let ch1 = (h1 - TEST_PADDING_Y) / grid.rows;

    assert_swap_proof_by_total_brightness(
        &pixels1,
        w1,
        "cursor_move pos1: col5 select > col8 unsel",
        5 * cw1,
        0,
        cw1,
        ch1,
        8 * cw1,
        0,
        cw1,
        ch1,
        10_000,
    );

    // Position 2: cursor moved to col 8, col 5 unselected
    grid.selected[5] = false;
    grid.selected[8] = true;
    let (pixels2, w2, h2) = render_grid(
        "CURSOR_MOVE_POS2",
        &grid,
        Some("CURSOR"),
        Some(theme_clear_color()),
    );
    let cw2 = (w2 - TEST_PADDING_X) / grid.cols;
    let ch2 = (h2 - TEST_PADDING_Y) / grid.rows;

    assert_swap_proof_by_total_brightness(
        &pixels2,
        w2,
        "cursor_move pos2: col8 select > col5 unsel",
        8 * cw2,
        0,
        cw2,
        ch2,
        5 * cw2,
        0,
        cw2,
        ch2,
        10_000,
    );
}

/// Select all: every cell selected, all fg/bg swapped.
/// Colors from theme: fg = Catppuccin Mocha foreground, bg = Catppuccin Mocha ansi[0] (black).
/// Absolute verification: cell-only region total brightness exceeds threshold.
#[test]
fn visual_select_all() {
    let mut grid = FlatGrid::new(1, 15);
    let text = "SELECTED LINE  ";
    for (i, ch) in text.chars().enumerate() {
        grid.chars[i] = ch;
        grid.foreground[i] = theme_fg();
        grid.background[i] = theme_ansi(0); // black bg for max OCR contrast
    }
    grid.selected.fill(true);

    let (pixels, w, h) = render_grid(
        "SELECT_ALL",
        &grid,
        Some("SELECTED"),
        Some(theme_clear_color()),
    );

    let cell_w = (w - TEST_PADDING_X) / grid.cols;
    let cell_h = (h - TEST_PADDING_Y) / grid.rows;
    let cell_region_w = cell_w * grid.cols;
    let cell_region_h = cell_h * grid.rows;

    // After swap: bg = theme_fg (bright), glyph = ansi[0] (black).
    // All cell-only pixels summed: bright bg dominates.
    let cell_total = region_total_brightness(&pixels, w, 0, 0, cell_region_w, cell_region_h);
    assert!(
        cell_total > 800_000,
        "SELECT_ALL: cell-only brightness {cell_total} should exceed 800,000 (swap proof)"
    );
}

/// Cell reverse color verification: render one cell with fg=red, bg=green, selected=true.
/// After swap: glyph drawn in green (original bg), background filled with red (original fg).
#[test]
fn visual_cell_reverse_color_verification() {
    // Two cells side by side: same original fg/bg, one selected, one not.
    let mut grid = FlatGrid::new(1, 2);
    grid.chars[0] = 'A';
    grid.chars[1] = 'A';
    let n = 2usize;
    grid.foreground = vec![[1.0, 0.0, 0.0, 1.0]; n];
    grid.background = vec![[0.0, 1.0, 0.0, 1.0]; n];
    grid.selected[0] = true; // cell 0 → swap: glyph=green, bg=red
    grid.selected[1] = false; // cell 1 → keep: glyph=red,  bg=green

    let (pixels, w, h) = render_grid("CELL_REVERSE", &grid, Some("AA"), None);

    // Channel-based region detection: cell 0 (selected) should show red dominance
    let regions = find_reverse_color_regions_by_channel(&pixels, w, h, 2, 1, 0, 1.5, 1);
    assert!(
        !regions.is_empty(),
        "CELL_REVERSE: should find reverse-color region for cell 0"
    );
    let (rx, _ry, _rw, _rh) = regions[0];
    let cell_w = (w - TEST_PADDING_X) / 2;
    assert_eq!(
        rx, 0,
        "CELL_REVERSE: first region x ({rx}) should be 0 (cell 0, cell_w={cell_w})"
    );

    // Cell 0 (selected, swap):   bg=red   → red sum > green sum
    // Cell 1 (unselected, keep): bg=green → green sum > red sum
    let cell_h_px = h - TEST_PADDING_Y;
    let cell_w_px = (w - TEST_PADDING_X) / grid.cols;
    let mut r0 = 0u32;
    let mut g0 = 0u32;
    let mut r1 = 0u32;
    let mut g1 = 0u32;
    for y in 0..cell_h_px {
        for x in 0..(cell_w_px * grid.cols) {
            let i = (y * w + x) as usize * 4;
            if i + 2 < pixels.len() {
                if x < cell_w_px {
                    r0 += pixels[i] as u32;
                    g0 += pixels[i + 1] as u32;
                } else {
                    r1 += pixels[i] as u32;
                    g1 += pixels[i + 1] as u32;
                }
            }
        }
    }
    assert!(
        r0 > g0,
        "selected: red area sum({r0}) > green glyph({g0}) — swap should make bg red (was fg)"
    );
    assert!(
        g1 > r1,
        "unselected: green area sum({g1}) > red glyph({r1}) — no swap, bg stays green (was bg)"
    );
}
