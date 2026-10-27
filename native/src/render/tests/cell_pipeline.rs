use super::*;

#[test]
fn blend_highlight_semi_transparent() {
    let base = [0.0, 0.0, 0.0, 1.0];
    let hl = [255, 255, 255, 128];
    let blended = blend_highlight(base, hl);
    assert!((blended[0] - 0.5).abs() < 0.01);
    assert!((blended[1] - 0.5).abs() < 0.01);
    assert!((blended[2] - 0.5).abs() < 0.01);
    assert!((blended[3] - 1.0).abs() < 0.01);
}

#[test]
fn search_highlight_blends_on_non_cursor_cell() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        background: [0.0, 0.0, 0.0, 1.0],
        ..test_cell_data('X' as u32, 0, 0)
    }];
    let cursor = test_cell_cursor(false);
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [255, 0, 0, 128],
    }];
    let instances = build_instances(
        &cell_data,
        test_config(1, 1, cell_width, cell_height, cursor, &highlights),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        cell.background[0] > 0.4,
        "highlighted cell background should have red tint from blending: {:?}",
        cell.background
    );
}

#[test]
fn cursor_cell_not_affected_by_search_highlight() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [0.0, 1.0, 0.0, 1.0],
        ..test_cell_data('A' as u32, 0, 0)
    }];
    let cursor = CellCursor {
        color: [0.5, 0.5, 1.0, 1.0],
        ..test_cell_cursor(true)
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [200, 0, 0, 200],
    }];
    let instances = build_instances(
        &cell_data,
        test_config(1, 1, cell_width, cell_height, cursor, &highlights),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.background, &[0.5, 0.5, 1.0, 0.7]),
        "cursor cell background should be cursor color (with block alpha), not highlight color"
    );
}

// ── Search highlight helpers and tests ───────────────────────────

#[test]
fn search_highlight_current_match_inverts_foreground_background() {
    // Current match: alpha >= 128 triggers foreground/background swap
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 2,
        end_col_exclusive: 5,
        color: [200, 100, 50, 160], // current match: alpha >= 128
    }];
    let by_row = group_highlights_by_row(&highlights);
    let hl = cell_highlight(0, 3, &by_row);
    assert!(hl.is_some(), "cell (0,3) should have highlight");
    let color = hl.expect("cell must have highlight");
    assert_eq!(color[3], 160, "alpha should be preserved");
}

#[test]
fn search_highlight_other_match_no_invert() {
    // Other match: alpha < 128 should NOT swap foreground and background
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 2,
        end_col_exclusive: 5,
        color: [100, 150, 200, 64], // other match: alpha < 128
    }];
    let by_row = group_highlights_by_row(&highlights);
    let hl = cell_highlight(0, 3, &by_row);
    assert!(hl.is_some(), "cell (0,3) should have highlight");
    let color = hl.expect("cell must have highlight");
    assert_eq!(color[3], 64, "alpha should be preserved");
}

#[test]
fn search_highlight_outside_range_not_found() {
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 2,
        end_col_exclusive: 5,
        color: [200, 100, 50, 160],
    }];
    let by_row = group_highlights_by_row(&highlights);
    // Before start
    assert!(cell_highlight(0, 1, &by_row).is_none());
    // After end (end_col_exclusive)
    assert!(cell_highlight(0, 5, &by_row).is_none());
    // Wrong row
    assert!(cell_highlight(1, 3, &by_row).is_none());
}

#[test]
fn search_highlight_zero_alpha_no_invert() {
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 10,
        color: [100, 150, 200, 0], // fully transparent
    }];
    let by_row = group_highlights_by_row(&highlights);
    let hl = cell_highlight(0, 5, &by_row);
    assert!(hl.is_some(), "should find highlight at row 0 col 5");
    assert_eq!(hl.expect("must have highlight")[3], 0);
}

#[test]
fn search_highlight_multiple_matches_same_row() {
    let highlights = vec![
        SearchHighlight {
            row: 0,
            start_col: 0,
            end_col_exclusive: 3,
            color: [200, 100, 50, 64], // other match
        },
        SearchHighlight {
            row: 0,
            start_col: 10,
            end_col_exclusive: 15,
            color: [200, 100, 50, 160], // current match
        },
    ];
    let by_row = group_highlights_by_row(&highlights);

    // First highlight (other match)
    let hl = cell_highlight(0, 1, &by_row);
    assert!(hl.is_some(), "cell (0,1) should have other match highlight");
    assert_eq!(
        hl.expect("other match must have highlight")[3],
        64,
        "other match alpha should be 64"
    );

    // Between highlights — no highlight
    assert!(
        cell_highlight(0, 5, &by_row).is_none(),
        "cell (0,5) should not be highlighted"
    );

    // Second highlight (current match)
    let hl = cell_highlight(0, 12, &by_row);
    assert!(
        hl.is_some(),
        "cell (0,12) should have current match highlight"
    );
    assert_eq!(
        hl.expect("current match must have highlight")[3],
        160,
        "current match alpha should be 160"
    );
}

// ── 生产取值钉 ──────────────────────────────────────────────────
//
// 搜索高亮的两个 alpha 常量（`cell_builder::SEARCH_HIGHLIGHT_*_ALPHA`）同时决定
// 代码里的字面量与「当前/其余命中」的观感差异：
//   - 当前命中 → 255（完全不透明）
//   - 其余命中 → 160（≥ 128，同样反色，只按不透明度淡出）
// 以下用例把渲染结果钉在这些取值上：改常量必须一并改这里的期望值。

#[test]
fn search_highlight_current_match_alpha_matches_production() {
    use crate::render::cell_builder::apply_search_highlight;

    // Production anchor: SEARCH_HIGHLIGHT_CURRENT_ALPHA = 255.
    let hl: [u8; 4] = [255, 200, 0, 255];

    let original_foreground: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // white text
    let original_background: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // black background
    let mut foreground = original_foreground;
    let mut background = original_background;

    apply_search_highlight(&mut foreground, &mut background, hl);

    // alpha >= 128 must swap foreground/background (inverse video): foreground becomes the
    // ORIGINAL background...
    assert!(
        f32_arrays_equal(&foreground, &original_background),
        "alpha=255 must swap foreground/background; foreground should become the original background: {:?}",
        foreground
    );
    // ...and the swapped background is fully covered by the opaque
    // highlight color.
    let expected_background = blend_highlight(original_background, hl);
    assert!(
        f32_arrays_equal(&background, &expected_background),
        "background should be highlight blended over the swapped background: {:?}",
        background
    );
    assert!(
        f32_arrays_equal(&background, &[1.0, 200.0 / 255.0, 0.0, 1.0]),
        "opaque highlight must fully replace the background: {:?}",
        background
    );
}

#[test]
fn search_highlight_other_match_alpha_matches_production() {
    use crate::render::cell_builder::apply_search_highlight;

    // Production anchor: SEARCH_HIGHLIGHT_OTHER_ALPHA = 160,
    // at or above the 128 swap threshold (all matches invert).
    let hl: [u8; 4] = [100, 150, 200, 160];

    let original_foreground: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let original_background: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let mut foreground = original_foreground;
    let mut background = original_background;

    apply_search_highlight(&mut foreground, &mut background, hl);

    // alpha >= 128 must swap (inverse video): foreground becomes the original background.
    assert!(
        f32_arrays_equal(&foreground, &original_background),
        "alpha=160 must swap foreground/background like every match: {:?}",
        foreground
    );
    // 底色从**原背景**混向高亮色，故 160 与 255 的底色必然不同；若拿交换后的
    // 背景（此时是原前景色）当基准，同色系的高亮会把 alpha 整个吃掉。
    let expected_background = blend_highlight(original_background, hl);
    assert!(
        f32_arrays_equal(&background, &expected_background),
        "background should be the highlight blended over the original background: {:?}",
        background
    );
    assert!(
        !f32_arrays_equal(&background, &original_background),
        "the alpha=160 blend must visibly change the background"
    );
}

/// 高亮色等于原前景色（默认主题无 `[colors.selection]`，选区色回退到前景）时，
/// 当前命中与其余命中 MUST 仍可区分。
///
/// 判别力：用交换后的背景做混合基准时，`blend(前景, 前景)` 与 alpha 无关，
/// 255 与 160 渲染成同一块像素——这正是用户报障的「两种高亮颜色应该不同」。
#[test]
fn other_match_stays_distinct_when_highlight_equals_foreground() {
    use crate::render::cell_builder::apply_search_highlight;

    let foreground_color: [f32; 4] = [
        0xF8 as f32 / 255.0,
        0xF8 as f32 / 255.0,
        0xF2 as f32 / 255.0,
        1.0,
    ];
    let background_color: [f32; 4] = [
        0x21 as f32 / 255.0,
        0x21 as f32 / 255.0,
        0x21 as f32 / 255.0,
        1.0,
    ];
    let highlight: [u8; 4] = [0xF8, 0xF8, 0xF2, 160];

    let mut current_foreground = foreground_color;
    let mut current_background = background_color;
    apply_search_highlight(
        &mut current_foreground,
        &mut current_background,
        [0xF8, 0xF8, 0xF2, 255],
    );

    let mut other_foreground = foreground_color;
    let mut other_background = background_color;
    apply_search_highlight(&mut other_foreground, &mut other_background, highlight);

    assert!(
        !f32_arrays_equal(&other_background, &current_background),
        "其余命中的底色必须与当前命中不同：other={other_background:?} current={current_background:?}"
    );
    assert!(
        !f32_arrays_equal(&other_background, &background_color),
        "其余命中必须与未高亮的背景不同：other={other_background:?}"
    );
    assert!(
        f32_arrays_equal(&other_foreground, &background_color),
        "其余命中的文字取原背景色：{:?}",
        other_foreground
    );
}

#[test]
fn selection_intersect_current_match_double_swap() {
    // Covers the highlight-on-terminal-selection path: the VT thread bakes
    // the tracked-selection inverse video into CellData (foreground/background pre-swapped
    // here), then apply_search_highlight runs on top. A current-match
    // highlight (production alpha 255 >= 128 from
    // SearchHighlightColors.CURRENT_MATCH_ALPHA) swaps AGAIN, so the two
    // swaps cancel — the cell keeps its original foreground while the
    // background becomes the fully-opaque highlight color.
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    // Terminal-baked selection: white-on-black becomes black-on-white.
    let cell_data = vec![CellData {
        foreground: [0.0, 0.0, 0.0, 1.0],
        background: [1.0, 1.0, 1.0, 1.0],
        underline_color: [0.0, 0.0, 0.0, 1.0],
        ..test_cell_data('X' as u32, 0, 0)
    }];
    let cursor = test_cell_cursor(false);
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [255, 200, 0, 255], // CURRENT_MATCH_ALPHA = 255 >= 128
    }];
    let instances = build_instances(
        &cell_data,
        test_config(1, 1, cell_width, cell_height, cursor, &highlights),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Double swap cancels: the text keeps its ORIGINAL foreground...
    assert!(
        f32_arrays_equal(&cell.foreground, &[1.0, 1.0, 1.0, 1.0]),
        "selection swap + highlight swap must cancel; foreground keeps original: {:?}",
        cell.foreground
    );
    // ...and the background is fully covered by the opaque highlight.
    assert!(
        f32_arrays_equal(&cell.background, &[1.0, 200.0 / 255.0, 0.0, 1.0]),
        "background should be the fully-opaque highlight color: {:?}",
        cell.background
    );
}

// ── Cursor shape tests ──

#[test]
fn cursor_geometry_anchored_to_cell_box() {
    // spec cursor-rendering「方块光标按整格几何」「光标标记锚在单元盒」：光标几何只由
    // 单元盒决定，与格内字形（空单元格取参考位图、文本格取该字形位图）无关。
    // 跟随位图会让同一个光标随内容变高变矮、降部字形还会把它带出行界。
    for style in [CursorStyle::Block, CursorStyle::Bar, CursorStyle::Underline] {
        for codepoint in [0x20, 'g' as u32, 'M' as u32] {
            let mut font_pipeline = ascii_font();
            let (cell_width, cell_height) = font_pipeline.cell_metrics();
            let instances = build_cursor_probe_instance(
                codepoint,
                [1.0, 1.0, 1.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
                0,
                true,
                &mut font_pipeline,
                style,
            );
            // 文本格上的 Bar/Underline 会在字形之后追加标记四边形，取最后一个。
            let cell = instances.last().expect("cursor must produce an instance");
            let (left, top) = (cell.quad_origin[0], cell.quad_origin[1]);
            let (width, height) = (cell.quad_size[0], cell.quad_size[1]);
            assert!(
                f32_eq(left, 0.0),
                "{style:?}/{codepoint:#x}: 光标左边界应为单元左边界 0，实得 {left}"
            );
            match style {
                CursorStyle::Block => {
                    assert!(
                        f32_eq(top, 0.0)
                            && f32_eq(width, cell_width)
                            && f32_eq(height, cell_height),
                        "{style:?}/{codepoint:#x}: 方块光标应为整格 {cell_width}×{cell_height}，实得 [{left},{top}]+[{width},{height}]"
                    );
                }
                CursorStyle::Bar => {
                    assert!(
                        f32_eq(top, 0.0) && f32_eq(top + height, cell_height),
                        "{style:?}/{codepoint:#x}: 竖线应纵向铺满单元 [0,{cell_height}]，实得 [{top},{}]",
                        top + height
                    );
                    assert!(
                        width > 0.0 && width < cell_width,
                        "{style:?}/{codepoint:#x}: 竖线宽度应窄于单元 {cell_width}，实得 {width}"
                    );
                }
                CursorStyle::Underline => {
                    assert!(
                        top > 0.0 && f32_eq(top + height, cell_height),
                        "{style:?}/{codepoint:#x}: 下划线底边应为单元底边 {cell_height}，实得 [{top},{}]",
                        top + height
                    );
                    assert!(
                        f32_eq(width, cell_width),
                        "{style:?}/{codepoint:#x}: 下划线宽度应为整格 {cell_width}，实得 {width}"
                    );
                }
            }
        }
    }
}

#[test]
fn cursor_not_rendered_when_visible_false() {
    let mut font_pipeline = ascii_font();
    let instances = build_hidden_cursor_probe_instance(0x20, &mut font_pipeline);
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // 非光标位置的空白格用默认背景色，而非光标色
    assert!(
        !f32_arrays_equal(&cell.background, &[1.0, 1.0, 1.0, 0.7]),
        "cursor cell should not have block alpha background when cursor_visible=false"
    );
}

#[test]
fn cursor_at_origin() {
    let mut font_pipeline = ascii_font();
    let instances = build_cursor_probe_instance(
        'A' as u32,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        true,
        &mut font_pipeline,
        CursorStyle::Block,
    );
    assert_eq!(
        instances.len(),
        1,
        "cursor at (0,0) must produce an instance"
    );
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.background, &[1.0, 1.0, 1.0, 0.7]),
        "cursor at origin must render with block alpha"
    );
}

#[test]
fn cursor_with_text_and_block_style() {
    let mut font_pipeline = ascii_font();
    let instances = build_cursor_probe_instance(
        'X' as u32,
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
        true,
        &mut font_pipeline,
        CursorStyle::Block,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Block cursor keeps the original foreground readable, background becomes cursor color×alpha.
    assert!(
        f32_arrays_equal(&cell.foreground, &[0.0, 1.0, 0.0, 1.0]),
        "block cursor on text: foreground should stay the original foreground"
    );
    assert!(
        f32_arrays_equal(&cell.background, &[1.0, 1.0, 1.0, 0.7]),
        "block cursor on text: background should be cursor color with block alpha"
    );
}

#[test]
fn cursor_color_custom_values() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        background: [0.0, 0.0, 0.0, 1.0],
        ..test_cell_data(0x20, 0, 0)
    }];
    let cursor = CellCursor {
        color: [0.5, 0.3, 0.8, 1.0],
        ..test_cell_cursor(true)
    };
    let instances = build_instances(
        &cell_data,
        test_config(1, 1, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.background, &[0.5, 0.3, 0.8, 0.7]),
        "custom cursor color should be reflected with block alpha multiplier"
    );
}

#[test]
fn render_paused_skips_frame() {
    let mut context = Renderer::new_with_no_surface();
    assert!(!context.render_paused, "should start unpaused");
    // 未配置 surface 时，未暂停状态下渲染必须失败
    assert!(
        context
            .render_frame_with_plan(
                &[],
                &[],
                false,
                &crate::render::cell_builder::FramePatch::default()
            )
            .is_err(),
        "expected error when not paused and no surface"
    );
    // When paused, render reports not-presented (Err) so the caller
    // does not advance last_frame: a consumed-but-never-presented New
    // frame would otherwise be lost forever (IME pause swallows output).
    context.set_render_paused(true);
    assert!(
        context
            .render_frame_with_plan(
                &[],
                &[],
                false,
                &crate::render::cell_builder::FramePatch::default()
            )
            .is_err(),
        "expected err when paused (frame not presented)"
    );
}

#[test]
fn render_paused_toggle_resumes_rendering() {
    let mut context = Renderer::new_with_no_surface();
    // Pause then unpause
    context.set_render_paused(true);
    assert!(
        context
            .render_frame_with_plan(
                &[],
                &[],
                false,
                &crate::render::cell_builder::FramePatch::default()
            )
            .is_err(),
        "paused reports not-presented"
    );
    context.set_render_paused(false);
    assert!(
        context
            .render_frame_with_plan(
                &[],
                &[],
                false,
                &crate::render::cell_builder::FramePatch::default()
            )
            .is_err(),
        "unpaused fails without surface (correct behavior)"
    );
}

#[test]
fn render_paused_remains_paused_after_multiple_frames() {
    let mut context = Renderer::new_with_no_surface();
    context.set_render_paused(true);
    for _ in 0..10 {
        assert!(
            context
                .render_frame_with_plan(
                    &[],
                    &[],
                    false,
                    &crate::render::cell_builder::FramePatch::default()
                )
                .is_err(),
            "paused render must stay not-presented across multiple frames"
        );
    }
}

#[test]
fn new_with_no_surface_starts_unpaused() {
    let context = Renderer::new_with_no_surface();
    assert!(!context.render_paused, "new context must start unpaused");
}

#[test]
fn set_render_paused_idempotent() {
    let mut context = Renderer::new_with_no_surface();
    context.set_render_paused(true);
    context.set_render_paused(true);
    assert!(
        context
            .render_frame_with_plan(
                &[],
                &[],
                false,
                &crate::render::cell_builder::FramePatch::default()
            )
            .is_err(),
        "double-pause still not-presented"
    );
}
// ══════════════════════════════════════════════════════════════════════════
// GPU Pipeline Benchmarks  (wgpu command encoding, submit)
// ══════════════════════════════════════════════════════════════════════════

/// Benchmark wgpu command encoding overhead: create encoder, begin render pass,
/// draw instances, end pass, submit. This tests the CPU-side graphics command
/// path that happens every frame.
#[test]
fn bench_gpu_command_encoding_overhead() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    // 构造最小的离屏渲染目标
    let (_texture, view) = bench_render_target(device);

    let iteration_count = 500;
    let start = Instant::now();
    for _ in 0..iteration_count {
        submit_clear_pass(device, queue, &view);
    }
    let elapsed = start.elapsed();
    let submissions_per_sec = iteration_count as f64 / elapsed.as_secs_f64();
    println!(
        "GPU command encoding + submit: {:.0} submissions/sec ({:.1}ms per submission)",
        submissions_per_sec,
        elapsed.as_millis() as f64 / iteration_count as f64,
    );
    assert!(
        submissions_per_sec > 100.0,
        "Command encoding too slow: {:.0} frames/sec (need >100)",
        submissions_per_sec,
    );
}

/// Benchmark full GPU pipeline throughput for rendering a screen of CellInstances:
/// buffer upload + command encoding + submit + poll. This mirrors the actual
/// render_frame_with_plan() path without requiring a swapchain surface.
#[test]
fn bench_gpu_full_submit_throughput() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    // Create off-screen target
    let (_texture, view) = bench_render_target(device);

    // Staging buffer + instance data
    let instance_data: Vec<CellInstance> = (0..1920)
        .map(|instance_index| CellInstance {
            quad_origin: [
                instance_index as f32 % 80.0 * 10.0,
                instance_index as f32 / 80.0 * 20.0,
            ],
            atlas_offset: [0.0, 0.0],
            atlas_size: [0.0, 0.0],
            foreground: [0.9, 0.9, 0.9, 1.0],
            background: [0.1, 0.1, 0.1, 1.0],
            underline_color: [0.9, 0.9, 0.9, 1.0],
            quad_size: [10.0, 20.0],
            flags: 0.0,
            bearing: [0.0, 0.0],
        })
        .collect();
    let bytes = bytemuck::cast_slice(&instance_data);
    let buf_size = bytes.len() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Bench VBO"),
        size: buf_size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let iteration_count = 200;
    let start = Instant::now();
    for _ in 0..iteration_count {
        queue.write_buffer(&buffer, 0, bytes);
        submit_clear_pass(device, queue, &view);
    }
    // 排空已提交的全部工作
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(2)),
    });
    let elapsed = start.elapsed();
    let fps = iteration_count as f64 / elapsed.as_secs_f64();
    let mb_per_sec =
        iteration_count as f64 * bytes.len() as f64 / 1_048_576.0 / elapsed.as_secs_f64();
    println!(
        "GPU full submit: {:.0} frames/sec ({:.1}ms per frame, {:.0} MB/s upload)",
        fps,
        elapsed.as_millis() as f64 / iteration_count as f64,
        mb_per_sec,
    );
    // Headless wgpu (Mesa Lavapipe) varies widely; set a conservative threshold
    assert!(
        fps > 30.0,
        "GPU full submit too slow: {:.0} fps (need >30)",
        fps,
    );
}

/// Benchmark the interaction between FontPipeline atlas and wgpu texture upload.
/// Measures the cost of creating + uploading a new atlas texture after glyph
/// cache warmup — this happens when new glyphs are encountered.
#[test]
fn bench_gpu_atlas_texture_upload() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::hint::black_box;
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    let iteration_count = 100;
    let start = Instant::now();
    for _ in 0..iteration_count {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas Bench"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Simulate uploading atlas data (background thread rasterized into Vec<u8>)
        let atlas_data = vec![0u8; 1024 * 1024 * 4];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024 * 4),
                rows_per_image: Some(1024),
            },
            wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
        );
        black_box(tex);
    }
    let elapsed = start.elapsed();
    let uploads_per_sec = iteration_count as f64 / elapsed.as_secs_f64();
    println!(
        "GPU atlas texture upload: {:.0} uploads/sec ({:.1}ms per 1024×1024 RGBA)",
        uploads_per_sec,
        elapsed.as_millis() as f64 / iteration_count as f64,
    );
    // Lavapipe may be slow; threshold is set conservatively
    assert!(
        uploads_per_sec > 5.0,
        "Atlas upload too slow: {:.0} uploads/sec (need >5)",
        uploads_per_sec,
    );
}

#[test]
fn all_static_pipelines_create_without_validation_errors() {
    // 每条生产用着色器管线都必须在开发环境提供的 wgpu 后端
    // （Mesa Lavapipe 软件 Vulkan）上编译通过。WGSL 编译错误正是在管线
    // 创建时暴露；下方的渲染用例已覆盖单元路径，本用例守护较少被走到的
    // KGP 管线。
    let Some((_instance, _adapter, device, _queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);

    let _ = crate::render::Renderer::create_cell_pipeline(&device, format);
    let _ = crate::render::Renderer::create_kgp_pipeline(&device, format);

    for (label, error) in [
        (
            "out-of-memory",
            futures::executor::block_on(oom_scope.pop()),
        ),
        (
            "validation",
            futures::executor::block_on(validation_scope.pop()),
        ),
    ] {
        assert!(
            error.is_none(),
            "pipeline creation raised {label} error: {error:?}"
        );
    }
}

// ── Context setter coverage (pure logic, no surface needed) ─────────────

/// 护栏：`write_texture` 在数据短于 `4 * width * height` 时 panic，而这份数据经 JNI
/// 跨越 FFI。少给一个字节曾足以让渲染线程被带走（表现为会话静默冻结而非崩溃）。
#[test]
fn kgp_atlas_short_payload_is_rejected_instead_of_panicking() {
    let mut context = Renderer::new_with_no_surface();
    let (width, height) = (4u32, 4u32);
    let short_by_one = vec![0u8; (width as usize) * 4 * (height as usize) - 1];
    context.set_kgp_atlas(&short_by_one, width, height);
    assert!(
        context.kgp_atlas_width == 0,
        "a short payload must not install an atlas"
    );
    // 恰好够长则接受。
    let exact = vec![0u8; (width as usize) * 4 * (height as usize)];
    context.set_kgp_atlas(&exact, width, height);
    assert_eq!(context.kgp_atlas_width, width);
    assert_eq!(context.kgp_atlas_height, height);
}

#[test]
fn kgp_atlas_zero_size_clears_texture() {
    let mut context = Renderer::new_with_no_surface();
    // Zero-size upload must clear any prior atlas instead of panicking.
    context.set_kgp_atlas(&[], 0, 0);
    assert!(context.kgp_texture.is_none());
    assert!(context.kgp_bind_group.is_none());
    assert_eq!(context.kgp_atlas_width, 0);
    assert_eq!(context.kgp_atlas_height, 0);
}

/// 超出设备上限的图集尺寸必须被拒绝，而不是装一个无效纹理。
///
/// `create_texture` 对超限尺寸返回无效句柄，装上去图像永远不出现，只有逐帧的
/// wgpu 校验错误——而尺寸来自 kitty 帧宽累加，属远端可控输入。
#[test]
fn kitty_atlas_beyond_the_device_limit_is_rejected() {
    let mut context = Renderer::new_with_no_surface();
    // 只把**一个**轴撑到超限，另一轴取 1：载荷于是只有 64KB 量级，
    // 「长度不足」那条早退不会替尺寸检查兜底——否则去掉尺寸检查本用例照样全绿。
    let oversized = context.device.limits().max_texture_dimension_2d + 1;
    let payload = vec![0u8; oversized as usize * 4];
    context.set_kgp_atlas(&payload, oversized, 1);
    assert!(
        context.kgp_texture.is_none(),
        "超限图集不得装入（{oversized} > 设备上限）"
    );
    assert_eq!(context.kgp_atlas_width, 0);
}

/// 换 Kitty 图集必须让下一帧全量重绘。
///
/// 漏掉时后果只在**清空**分支现形：图像消失后 `kgp_instances` 为空，
/// `should_render_partial_frame` 重新放行部分帧，而图像原先压住的那些行若并不脏
/// （kitty 图常叠在空白单元格上），此后每帧只画脏带——图像像素永久残留在屏幕上。
#[test]
fn replacing_the_kitty_atlas_invalidates_the_accumulator() {
    let mut context = Renderer::new_with_no_surface();
    let (width, height) = (4u32, 4u32);
    context.set_kgp_atlas(
        &vec![0u8; (width as usize) * 4 * (height as usize)],
        width,
        height,
    );
    context.frame_invalidated = false;

    // 换图集：新图集的 UV 布局与旧图集无关，未重绘的行留的是上一张图的残片。
    context.set_kgp_atlas(&[], 0, 0);
    assert!(
        context.frame_invalidated,
        "清空图集后下一帧必须全量重绘，否则旧图像像素永久残留"
    );

    context.frame_invalidated = false;
    context.set_kgp_atlas(
        &vec![0u8; (width as usize) * 4 * (height as usize)],
        width,
        height,
    );
    assert!(
        context.frame_invalidated,
        "换图集后下一帧必须全量重绘，否则留下上一张图的残片"
    );
}

/// Kitty 放置帧：整幅同色，`pixel_*` 决定显示尺寸（与源尺寸不等即缩放）。
fn kitty_frame(
    viewport_col: i32,
    source_width: u32,
    source_height: u32,
    pixel_width: u32,
    pixel_height: u32,
    rgba: [u8; 4],
) -> crate::terminal::ghostty_terminal::KittyPlacementFrame {
    crate::terminal::ghostty_terminal::KittyPlacementFrame {
        image_id: 1,
        viewport_col,
        viewport_row: 0,
        pixel_width,
        pixel_height,
        source_x: 0,
        source_y: 0,
        source_width,
        source_height,
        cell_offset_x: 0,
        cell_offset_y: 0,
        z: 0,
        image_width: source_width,
        image_height: source_height,
        image_rgba: rgba.repeat((source_width * source_height) as usize).into(),
    }
}

/// 图集里的相邻条目不得互相串色。
///
/// 多幅图像并排写进同一张图集，采样器却是双线性：纹素坐标一旦越过条目边界，
/// 过滤就会取到**邻条目**的纹素。放大显示时（显示尺寸大于源矩形）左缘会混入
/// 前一幅图的颜色；条目比图集矮时底缘会混入图集里的透明行，图像于是出现一道
/// 彩边或一条发虚的半透明边。
#[test]
fn kitty_atlas_entries_do_not_bleed_into_each_other() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    // 红图 1x3 在左（1:1 显示），绿图 2x2 在右且比图集矮（放大到 4x4 显示）：
    // 绿图左缘紧邻红图所在列，底缘下方是图集里的透明行。
    let frames = vec![
        kitty_frame(0, 1, 3, 1, 3, [255, 0, 0, 255]),
        kitty_frame(2, 2, 2, 4, 4, [0, 255, 0, 255]),
    ];
    let (atlas, atlas_width, atlas_height, instances) =
        crate::render::kitty::pack_and_build(&frames, 8.0, 8.0).expect("atlas");
    assert!(
        context.set_kgp_atlas(&atlas, atlas_width, atlas_height),
        "图集必须装载成功"
    );

    let frame = context.render_to_buffer(&[], &instances).expect("readback");
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let index = ((y * 50 + x) * 4) as usize;
        [
            frame[index],
            frame[index + 1],
            frame[index + 2],
            frame[index + 3],
        ]
    };
    // 绿图绘制在第 2 列（8 像素一格）起、共 4x4 像素。
    for y in 0..4 {
        for x in 16..20 {
            assert_eq!(
                pixel_at(x, y),
                [0, 255, 0, 255],
                "绿图 ({x},{y}) 混入了邻条目的像素"
            );
        }
    }
    // 红图 1:1 显示，同样不得被图集边界影响。
    for y in 0..3 {
        assert_eq!(
            pixel_at(0, y),
            [255, 0, 0, 255],
            "红图 (0,{y}) 混入了邻条目的像素"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
// Dirty-band computation (render-vulkan-performance)
// ══════════════════════════════════════════════════════════════════════
