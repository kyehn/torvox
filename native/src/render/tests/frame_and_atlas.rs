use super::*;

#[cfg(test)]
mod dirty_band_tests {
    use crate::render::cell_builder::{DirtyBand, compute_dirty_bands};

    #[test]
    fn empty_mask_yields_no_bands() {
        assert!(compute_dirty_bands(&[]).is_empty());
        assert!(compute_dirty_bands(&[false, false, false]).is_empty());
    }

    #[test]
    fn single_row_is_single_band() {
        assert_eq!(compute_dirty_bands(&[false, true, false]), vec![(1, 2)]);
    }

    #[test]
    fn contiguous_rows_merge_into_one_band() {
        assert_eq!(
            compute_dirty_bands(&[true, true, false, true]),
            vec![(0, 2), (3, 4)]
        );
    }

    #[test]
    fn all_dirty_is_one_full_band() {
        assert_eq!(compute_dirty_bands(&[true; 5]), vec![(0, 5)]);
    }

    #[test]
    fn trailing_and_leading_edges() {
        assert_eq!(
            compute_dirty_bands(&[true, false, true, true, true]),
            vec![(0, 1), (2, 5)]
        );
        assert_eq!(compute_dirty_bands(&[false, false, true]), vec![(2, 3)]);
    }

    #[test]
    fn covers_all_rows_detection() {
        let full = DirtyBand {
            start_row: 0,
            end_row_exclusive: 24,
            instance_start: 0,
            instance_end: 100,
        };
        let partial = DirtyBand {
            start_row: 22,
            end_row_exclusive: 24,
            instance_start: 90,
            instance_end: 100,
        };
        assert!(full.covers_all_rows(24));
        assert!(!partial.covers_all_rows(24));
    }

    /// Band slice resolution against a populated row cache: the band's
    /// instance slice must exactly cover the per-row slices it spans.
    #[test]
    fn band_slice_matches_row_slices() {
        use crate::render::cell_builder::CachedInstances;
        let mut cache = CachedInstances::new(4, 2);
        // 模拟一次构建：第 0 行 2 个实例、第 1 行 1 个、第 2 行 0 个、
        // 第 3 行 3 个。
        cache.update_for_test(
            &[2usize, 3, 3, 6], // row_ends (cumulative)
        );
        let (start, end) = cache.band_slice(1, 3);
        assert_eq!((start, end), (2, 3)); // rows 1..3 → instances [2..3)
        let (start, end) = cache.band_slice(0, 4);
        assert_eq!((start, end), (0, 6));
        let (start, end) = cache.band_slice(2, 3);
        assert_eq!((start, end), (3, 3)); // empty middle row
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Content-bottom row (ime-content-aware-shift)
// ══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
// ══════════════════════════════════════════════════════════════════════════
// Surface loss → invalidation decision (render-surface-loss-recovery)
// ══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod surface_loss_tests {
    use crate::render::context::surface_loss_transition;

    /// A single Lost/Outdated can be a SurfaceFlinger scaling race that recovers on
    /// the next reconfigure — it must NOT invalidate (that would force a rebuild on
    /// every resize).
    #[test]
    fn single_surface_loss_keeps_surface() {
        assert_eq!(surface_loss_transition(0, false, true), (1, false));
    }

    /// Consecutive losses mean the window's BufferQueue is gone (abandoned): mark the
    /// surface invalid so the next attach rebuilds instead of reconfiguring forever.
    #[test]
    fn consecutive_losses_invalidate_surface() {
        let (streak, invalidated) = surface_loss_transition(1, false, true);
        assert_eq!((streak, invalidated), (2, true));
    }

    /// A frame that skipped (worker busy / acquire timeout) proves the surface is
    /// still alive, so the streak resets.
    #[test]
    fn skipped_frame_resets_streak() {
        assert_eq!(surface_loss_transition(1, false, false), (0, false));
        assert_eq!(surface_loss_transition(2, true, false), (0, true));
    }

    /// 回归钉：连续「仅本帧跳过」永不判死 surface。
    ///
    /// 取纹理期限被压到帧级（两条取纹理路径共用 `ACQUIRE_TIMEOUT`），理由是这把
    /// 期限就是 UI 线程在一次 GPU 卡死时最多要等的时间——前提是超时只降级为
    /// `Skipped`（下一帧重试）而不是判死窗口。若跳过会推进失效计数，这个期限就
    /// 会把一次慢帧变成永久黑屏，故此处显式钉住。
    #[test]
    fn repeated_skips_never_invalidate_surface() {
        let mut streak = 0;
        let mut invalidated = false;
        // 远超 surface 级失效门槛（连续 2 次）的跳过次数。
        for _ in 0..64 {
            let next = surface_loss_transition(streak, invalidated, false);
            streak = next.0;
            invalidated = next.1;
            assert!(!invalidated, "跳过不得判死 surface");
            assert_eq!(streak, 0, "跳过证明 surface 仍存活，计数须清零");
        }
    }

    /// Already-invalid surfaces stay invalid (idempotent): the host is expected to
    /// deliver a fresh ANativeWindow, and no duplicate error log is emitted.
    #[test]
    fn invalidation_is_idempotent() {
        assert_eq!(surface_loss_transition(2, true, true), (3, true));
        assert_eq!(surface_loss_transition(7, true, true), (8, true));
    }
}

// GPU 初始化并发 → 只初始化一次（render-gpu-init-race）
// ══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod gpu_init_race_tests {
    use crate::render::context::{INITIALIZATION_COUNT, try_global_gpu};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    /// 预热线程与渲染线程在启动瞬间并发调用取 GPU，完整的 Vulkan 初始化
    /// （建 Instance、request_adapter、request_device）在进程内必须只发生一次。
    ///
    /// 真机 logcat 实证的回归护栏：修复前 `GPU adapter` 与 `GPU device created`
    /// 各打印两遍、分属两个线程，两次初始化中第二次的产物被直接丢弃。
    ///
    /// 断言进程内的初始化总次数而非「各线程拿到同一引用」：`OnceLock` 一旦填满，
    /// 后者对修复前后的代码同样成立，对本竞态零判别力；且预热后再断言亦属恒真——
    /// 填满后 `init_gpu` 不可能再执行。竞态只发生在**首次**初始化，故断言
    /// 「成功初始化过且仅一次」，无论本进程内 GPU 是被本用例还是被并行用例首先
    /// 拉起，该断言都成立。
    #[test]
    fn concurrent_lookups_initialize_gpu_once() {
        // 并发数取多核上限，使 check-then-act 窗口尽可能被命中。不预先初始化：
        // 预热会把竞态窗口关掉。
        let thread_count = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4)
            .clamp(2, 16);
        let barrier = Arc::new(std::sync::Barrier::new(thread_count));
        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    try_global_gpu()
                })
            })
            .collect();

        let mut instances = HashSet::new();
        for handle in handles {
            let gpu = handle
                .join()
                .expect("GPU lookup thread must not panic")
                .expect("GPU must be available");
            // 取到的正是同一个全局实例（引用为 'static），故其地址唯一标识它。
            instances.insert(gpu as *const _ as usize);
        }

        assert_eq!(
            instances.len(),
            1,
            "all threads must observe one GPU instance, got {instances:?}",
        );
        assert_eq!(
            INITIALIZATION_COUNT.load(Ordering::Relaxed),
            1,
            "GPU 初始化在进程内只能成功一次（竞态回归：并发下多建一次，其产物被丢弃）",
        );
    }
}

#[cfg(test)]
mod last_content_row_tests {
    use crate::render::cell_builder::last_content_row;
    use crate::terminal::ghostty_terminal::CellData;

    /// 行优先单元数据（宽字符的占位单元已被终端省略，行长因此可变）。
    fn cell(row: u32, codepoint: u32) -> CellData {
        super::super::test_cell_data(codepoint, row, 0)
    }

    /// 3 行 × 4 列，内容只落在 `marks` 给出的 (行, 列)。
    fn grid_3x4(marks: &[(u32, u32, u32)]) -> Vec<CellData> {
        let mut cells = Vec::with_capacity(12);
        for row in 0..3u32 {
            for col in 0..4u32 {
                let codepoint = marks
                    .iter()
                    .find(|(r, c, _)| *r == row && *c == col)
                    .map_or(0, |(_, _, cp)| *cp);
                cells.push(cell(row, codepoint));
            }
        }
        cells
    }

    #[test]
    fn empty_viewport_has_no_content_row() {
        assert_eq!(last_content_row(&[], 3), None);
        assert_eq!(last_content_row(&grid_3x4(&[]), 3), None);
    }

    /// 空格与制表不撑起下沿：空白行像素上与背景无异，若计入则稀疏会话退化为
    /// 「网格填满」，输入法弹出又会整体上移把首行推出屏幕。
    #[test]
    fn blank_rows_do_not_count_as_content() {
        let cells = grid_3x4(&[
            (0, 0, u32::from(b'$')),
            (1, 0, b' '.into()),
            (2, 0, b'\t'.into()),
        ]);
        assert_eq!(last_content_row(&cells, 3), Some(0));
    }

    #[test]
    fn bottom_row_content_wins() {
        let cells = grid_3x4(&[(0, 0, u32::from(b'$')), (2, 3, u32::from(b'x'))]);
        assert_eq!(last_content_row(&cells, 3), Some(2));
    }

    #[test]
    fn middle_row_content_is_the_bottom_when_last_row_blank() {
        let cells = grid_3x4(&[(1, 2, u32::from(b'y'))]);
        assert_eq!(last_content_row(&cells, 3), Some(1));
    }

    /// 宽字符只产出一个单元（续列被省略），行号仍取自 `CellData.row`，
    /// 不依赖 `行号 × 列宽` 的下标推算。
    #[test]
    fn wide_char_rows_use_cell_row_field() {
        let cells = vec![
            cell(0, u32::from(b'a')),
            cell(0, 0),
            CellData {
                width: 2,
                ..cell(1, '中' as u32)
            },
        ];
        assert_eq!(last_content_row(&cells, 2), Some(1));
    }

    /// 行号越界的陈旧单元不得影响下沿（否则位移按不可见数据裁剪）。
    #[test]
    fn stale_out_of_range_rows_are_ignored() {
        let cells = vec![cell(0, u32::from(b'$')), cell(9, u32::from(b'x'))];
        assert_eq!(last_content_row(&cells, 3), Some(0));
        assert_eq!(last_content_row(&[cell(9, u32::from(b'x'))], 3), None);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// End-to-End Pipeline Benchmarks  (terminal write → CellData → instances)
// Moved from terminal::ghostty_terminal::tests: they drive the render
// pipeline, and terminal modules must not reference render (rust-arch).
// ══════════════════════════════════════════════════════════════════════════

/// 归属说明：度量对象虽为终端快照，但本用例与相邻渲染 bench 共享
/// `GPU_BENCH_LOCK`（防 Lavapipe 并行争用），故置于渲染 bench 套件内；
/// 终端输入仅作夹具。阈值为单防抖地板（见本文件头注释）。
/// Benchmark the full CPU-side pipeline: write terminal content → flush →
/// receive CellData → build CellInstances. This simulates the complete
/// per-frame data path before GPU submission.
#[test]
fn bench_end_to_end_cpu_pipeline_latency() {
    use std::hint::black_box;
    use std::time::Instant;

    use crate::terminal::ghostty_terminal::GhosttyTerminal;

    let mut terminal_under_test = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    let mut font_pipeline = super::super::font::FontPipeline::new(1024, 1024, 14.0);

    // Simulate a realistic screen: fill with text content
    let content = b"user@host:~$ cargo build --release --features=test-util\n   Compiling native v0.1.0\n    Finished `release` profile [optimized] target(s) in 0.42s\n";
    let iteration_count = 20; // 20 screens

    let start = Instant::now();
    for _ in 0..iteration_count {
        terminal_under_test.vt_write(content);
        terminal_under_test.flush();
        let cell_data = terminal_under_test.receive_cell_data();
        let (cells, cursor_info) = cell_data.expect("should receive CellData after flush");

        let cursor = CellCursor {
            row: cursor_info.row,
            col: cursor_info.col,
            visible: cursor_info.visible,
            style: cursor_info.style,
            ..test_cell_cursor(cursor_info.visible)
        };
        let mut instances = Vec::new();
        super::super::build_instances_from_cell_data(
            &cells,
            test_config(24, 80, 1024.0 / 80.0, 1024.0 / 24.0, cursor, &[]),
            &mut font_pipeline,
            &mut instances,
        );
        let count = black_box(instances.len());
        black_box(count);
    }
    let elapsed = start.elapsed();
    let ms_per_frame = elapsed.as_millis() as f64 / iteration_count as f64;
    let fps = iteration_count as f64 / elapsed.as_secs_f64();
    println!(
        "End-to-end CPU pipeline: {:.1}ms per frame ({:.0} fps) — terminal write + CellData + build_instances",
        ms_per_frame, fps,
    );
    // Must complete within two frame budgets: the single-run cost is
    // ~8ms/frame, but the full test suite runs tests in parallel and CPU
    // contention (especially with the software-Vulkan benchmarks) pushes
    // wall time well past the 16ms single-frame budget. Note the 32ms
    // bound only catches >=4x regressions; it is primarily an
    // anti-flake guard, not a precise performance gate.
    assert!(
        ms_per_frame < 32.0,
        "End-to-end CPU pipeline too slow: {:.1}ms per frame (need <32ms)",
        ms_per_frame,
    );
}

#[test]
fn merged_cluster_emits_single_primary_without_ghost_overlays() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    // 👨‍👩‍👧 shapes to one glyph: it must replace the primary quad,
    // not stack component overlays on top (ghosting).
    // 彩色 emoji 无 outline 光栅时合并字形查不到，走逐码点 overlay
    // 回退（与真机 NotoColorEmoji 行为一致）：此时不断言数量，只断言
    // 每个实例 UV 合法且无 panic。
    let mut extras = [0u32; 7];
    extras[0] = 0x200d;
    extras[1] = 0x1f469;
    extras[2] = 0x200d;
    extras[3] = 0x1f467;
    let cell_data = vec![CellData {
        width: 2,
        grapheme_extra: extras,
        ..test_cell_data(0x1f468, 0, 0)
    }];
    let cursor = test_cell_cursor(false);
    let instances = build_configured_cell_instance(
        &cell_data,
        cursor,
        cell_width,
        cell_height,
        &mut font_pipeline,
    );
    assert!(
        !instances.is_empty(),
        "emoji ZWJ cell must produce at least one instance"
    );
    for instance in &instances {
        assert!(
            instance.atlas_size[0] >= 0.0 && instance.atlas_size[1] >= 0.0,
            "overlay fallback instances must carry valid UVs"
        );
    }
}

#[test]
fn same_glyph_at_different_cells_samples_identical_atlas_region() {
    // 回归网（d 像 a 类字形混淆）：同一字符在不同单元格必须采样同一图集
    // 区域，仅 quad 原点随位置偏移；UV 与位置无关。
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell = |row: u32, col: u32| test_cell_data('d' as u32, row, col);
    let cell_data = vec![cell(0, 0), cell(0, 5)];
    let cursor = CellCursor {
        col: 99,
        ..test_cell_cursor(false)
    };
    let instances = build_instances(
        &cell_data,
        test_config(1, 6, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 2, "two cells must emit two quads");
    assert_eq!(
        instances[0].atlas_offset, instances[1].atlas_offset,
        "same glyph must sample the same atlas region"
    );
    assert_eq!(
        instances[0].atlas_size, instances[1].atlas_size,
        "same glyph must sample the same atlas extent"
    );
    let expected_dx = 5.0 * cell_width;
    assert!(
        (instances[1].quad_origin[0] - instances[0].quad_origin[0] - expected_dx).abs() < 1e-4,
        "quad origin must shift exactly by columns"
    );
}

#[test]
fn distinct_glyphs_sample_distinct_atlas_regions() {
    // 回归网（d 像 a 类字形混淆）：不同字符必须采样不同图集区域。
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell = |ch: char, col: u32| test_cell_data(ch as u32, 0, col);
    let cell_data = vec![cell('d', 0), cell('a', 1)];
    let cursor = CellCursor {
        col: 99,
        ..test_cell_cursor(false)
    };
    let instances = build_instances(
        &cell_data,
        test_config(1, 2, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 2, "two cells must emit two quads");
    assert!(
        instances[0].atlas_offset != instances[1].atlas_offset
            || instances[0].atlas_size != instances[1].atlas_size,
        "d and a must sample distinct atlas regions"
    );
}

#[test]
fn italic_d_rasterizes_distinct_from_regular() {
    // 斜体 d 右上溢出裁剪回归：合成斜体位图必须与正体不同且非空，
    // 顶部行必须有前景像素（被裁则顶部全空，形似 a）。
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut pipeline = ascii_font();
    let regular = pipeline
        .glyph_information_styled('d', false, false)
        .expect("regular d");
    let italic = pipeline
        .glyph_information_styled('d', false, true)
        .expect("italic d");
    assert!(
        (italic.atlas_x, italic.atlas_y, italic.width, italic.height)
            != (
                regular.atlas_x,
                regular.atlas_y,
                regular.width,
                regular.height
            ),
        "italic d must occupy a distinct atlas region"
    );
    let bitmap = pipeline.atlas_bitmap();
    let atlas_width = pipeline.atlas_dimensions().0 as usize;
    let top_rows_foreground = (0..italic.height.min(3))
        .flat_map(|atlas_row| {
            (0..italic.width).map(move |atlas_column| {
                let pixel_column = (italic.atlas_x + atlas_column) as usize;
                let pixel_row = (italic.atlas_y + atlas_row) as usize;
                bitmap[(pixel_row * atlas_width + pixel_column) * 4]
            })
        })
        .filter(|&alpha| alpha > 0)
        .count();
    assert!(
        top_rows_foreground > 0,
        "italic d 顶部必须有前景像素，否则右上被裁"
    );
    let _ = cell_flags::ITALIC;
}

#[test]
fn first_build_leaves_pending_dirty_rect_covering_all_glyphs() {
    use crate::terminal::ghostty_terminal::CellData;
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut font_pipeline = ascii_font();
    // 模拟 render_inner Phase 1：先取走构建前的脏区（ASCII 预热字形）。
    let _pre_frame_upload = font_pipeline.take_dirty_rect();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let text = "abcdefghijklmnop";
    let cell_data: Vec<CellData> = text
        .char_indices()
        .map(|(col, ch)| CellData {
            flags: 1 << cell_flags::ITALIC,
            ..test_cell_data(ch as u32, 0, col as u32)
        })
        .collect();
    let cursor = CellCursor {
        col: 99,
        ..test_cell_cursor(false)
    };
    let instances = build_instances(
        &cell_data,
        test_config(1, text.len() as u32, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), text.len(), "每个字形一个 quad");
    let (rect_x, rect_y, rect_w, rect_h) = font_pipeline
        .take_dirty_rect()
        .expect("构建后必须有待上传脏区（首帧上传契约）");
    for ch in text.chars() {
        let info = font_pipeline
            .glyph_information_styled(ch, false, true)
            .expect("斜体字形必须可解析");
        assert!(
            info.atlas_x as u32 >= rect_x
                && info.atlas_y as u32 >= rect_y
                && (info.atlas_x as u32 + info.width as u32) <= rect_x + rect_w
                && (info.atlas_y as u32 + info.height as u32) <= rect_y + rect_h,
            "字形 '{ch}' 区域 ({},{} {}x{}) 必须落在待上传脏区 ({rect_x},{rect_y} {rect_w}x{rect_h}) 内",
            info.atlas_x,
            info.atlas_y,
            info.width,
            info.height
        );
    }
    assert!(
        font_pipeline.take_dirty_rect().is_none(),
        "取走脏区后不得残留待上传区域"
    );
}

/// 重复渲染一致性（spec render-stability 1：幂等）：同一批单元格第二次
/// 构建必须全部命中字形缓存——不产生新脏区（GPU 纹理无需更新）、实例
/// 与首帧逐字节一致、已上传字形像素不被改写。
#[test]
fn repeat_build_is_identical_and_produces_no_new_dirty_rect() {
    use crate::terminal::ghostty_terminal::CellData;
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut font_pipeline = ascii_font();
    let _ = font_pipeline.take_dirty_rect();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let text = "abcdefghijklmnop";
    let cell_data: Vec<CellData> = text
        .char_indices()
        .map(|(col, ch)| CellData {
            // 交替斜体/正体：两种样式路径都必须幂等。
            flags: if col % 2 == 0 {
                1 << cell_flags::ITALIC
            } else {
                0
            },
            ..test_cell_data(ch as u32, 0, col as u32)
        })
        .collect();
    let cursor = CellCursor {
        col: 99,
        ..test_cell_cursor(false)
    };
    let config = test_config(1, text.len() as u32, cell_width, cell_height, cursor, &[]);
    let mut first = Vec::new();
    assert!(
        crate::render::build_instances_from_cell_data(
            &cell_data,
            config,
            &mut font_pipeline,
            &mut first
        )
        .is_some(),
        "首帧构建失败"
    );
    let rect = font_pipeline
        .take_dirty_rect()
        .expect("首帧构建必须产生待上传脏区");
    let first_bitmap = font_pipeline.atlas_bitmap().to_vec();
    let (atlas_w, atlas_h) = font_pipeline.atlas_dimensions();
    let upload_bytes = snapshot_atlas_rect(&first_bitmap, atlas_w as usize, rect);
    assert!(
        upload_bytes.iter().any(|&b| b > 0),
        "首帧脏区必须包含字形像素"
    );

    // 第二次构建：字形缓存命中，实例与首帧一致，无新上传区。
    let mut second = Vec::new();
    assert!(
        crate::render::build_instances_from_cell_data(
            &cell_data,
            config,
            &mut font_pipeline,
            &mut second
        )
        .is_some(),
        "重复构建失败"
    );
    let first_bytes = bytemuck::cast_slice::<CellInstance, u8>(&first);
    let second_bytes = bytemuck::cast_slice::<CellInstance, u8>(&second);
    assert_eq!(
        first_bytes, second_bytes,
        "重复构建实例必须与首帧逐字节一致"
    );
    assert!(
        font_pipeline.take_dirty_rect().is_none(),
        "重复构建全部命中缓存，不得产生新脏区"
    );
    let second_bitmap = font_pipeline.atlas_bitmap();
    let upload_after = snapshot_atlas_rect(second_bitmap, atlas_w as usize, rect);
    assert_eq!(upload_bytes, upload_after, "重复帧不得改写已上传字形像素");
    let _ = atlas_h;
}

/// 滚动一致性（fix-scroll-residual-tearing）GPU 契约测试：
/// 带视口像素偏移的全量重绘（非部分路径：Clear + 全部实例 + 偏移投影）
/// 必须画面自洽——上边缘条带为清屏背景、行内容整体下移、
/// 偏移归零后的全量重绘与滚动前逐字节一致（无任何残留像素）。
#[test]
fn gpu_scroll_offset_full_redraw_has_no_stale_pixels() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    let (surface_width, surface_height) = (50u32, 50u32);
    let (row_count, cell_height_px) = (8u32, 5.0f32);
    let scroll_px = 3.0f32;
    let stripe_colors: [[u8; 3]; 8] = [
        [200, 0, 0],
        [0, 200, 0],
        [0, 0, 200],
        [200, 200, 0],
        [200, 0, 200],
        [0, 200, 200],
        [255, 128, 0],
        [128, 0, 255],
    ];
    // 每行一条 has_glyph=0（atlas_size=0）的纯色满宽平铺条：着色器按
    // background 直绘，等价部分路径的 band_clear_instances 平铺块。
    let instances: Vec<CellInstance> = (0..row_count)
        .map(|row| {
            let color = [
                stripe_colors[row as usize][0] as f32 / 255.0,
                stripe_colors[row as usize][1] as f32 / 255.0,
                stripe_colors[row as usize][2] as f32 / 255.0,
                1.0,
            ];
            CellInstance {
                quad_origin: [0.0, row as f32 * cell_height_px],
                atlas_offset: [0.0; 2],
                atlas_size: [0.0; 2],
                foreground: color,
                background: color,
                underline_color: color,
                quad_size: [surface_width as f32, cell_height_px],
                flags: 0.0,
                bearing: [0.0; 2],
            }
        })
        .collect();
    // 清屏背景即默认主题背景（单源，不手写色值）。
    let default_background = crate::terminal::theme::default_dark().palette.background;
    let background = [
        default_background.r,
        default_background.g,
        default_background.b,
        255,
    ];

    // 帧 A：偏移 0 的全量重绘（滚动前基线）。
    let frame_at_rest = context.render_to_buffer(&instances, &[]).unwrap();

    // 帧 B：偏移 scroll_px 的全量重绘（fix 后的滚动帧语义）。
    context.set_viewport_scroll_px(scroll_px);
    context.refresh_cell_uniforms(surface_width as f32, surface_height as f32);
    let frame_scrolled = context.render_to_buffer(&instances, &[]).unwrap();

    // 帧 C：偏移归零后的全量重绘（滚动结束帧语义）。
    context.set_viewport_scroll_px(0.0);
    context.refresh_cell_uniforms(surface_width as f32, surface_height as f32);
    let frame_back = context.render_to_buffer(&instances, &[]).unwrap();

    let pixel_at = |buffer: &[u8], pixel_x: u32, pixel_y: u32| -> [u8; 4] {
        let index = ((pixel_y * surface_width + pixel_x) * 4) as usize;
        [
            buffer[index],
            buffer[index + 1],
            buffer[index + 2],
            buffer[index + 3],
        ]
    };

    // 帧 B：上边缘条带 [0, scroll_px) 必须是清屏背景色（该条带在
    // 行进位前本应显示上一行尚未到达的内容，背景即自洽表现）。
    for pixel_row in 0..scroll_px as u32 {
        for pixel_column in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, pixel_column, pixel_row),
                background,
                "top edge strip must be cleared background (x={pixel_column}, y={pixel_row})"
            );
        }
    }
    // 帧 B：行 r 内容整体下移，占据 [scroll_px + r*cell_height_px, scroll_px + (r+1)*cell_height_px)。
    for row in 0..row_count {
        let row_middle_y = (scroll_px as u32)
            + (row as f32 * cell_height_px) as u32
            + (cell_height_px * 0.5) as u32;
        let expected = [
            stripe_colors[row as usize][0],
            stripe_colors[row as usize][1],
            stripe_colors[row as usize][2],
            255,
        ];
        for pixel_column in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, pixel_column, row_middle_y),
                expected,
                "row {row} must sit at shifted position (x={pixel_column}, y={row_middle_y})"
            );
        }
    }
    // 帧 B：下边缘条带（网格底线之下）为清屏背景色。
    let grid_bottom_px = (scroll_px as u32) + (row_count as f32 * cell_height_px) as u32;
    for pixel_row in grid_bottom_px..surface_height {
        for pixel_column in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, pixel_column, pixel_row),
                background,
                "bottom edge strip must be cleared background (x={pixel_column}, y={pixel_row})"
            );
        }
    }
    // 帧 C：滚动结束重绘后与滚动前基线逐字节一致——中间任何残留
    // 像素都会破坏该相等性（此即用户主诉“底部残留”的判定）。
    assert_eq!(
        frame_back, frame_at_rest,
        "full redraw after scroll settle must be byte-identical to pre-scroll"
    );
}

/// 被拒的图集必须把「没装上」告诉调用方，否则实例会滞留。
///
/// 判别力：拒绝只清纹理与尺寸，调用方若照旧写回按旧图集算好的实例，`kgp_present`
/// 便恒真——绑定组恒 `None` 图像永不出现，**部分帧增量渲染在该会话剩余生命周期内彻底
/// 失效**，且每帧都因「上传尺寸漂移」重打包一次。把 `false` 一律改成 `true` 即复现。
#[test]
fn a_rejected_kitty_atlas_reports_the_rejection() {
    let mut context = Renderer::new_with_no_surface();
    let (width, height) = (4u32, 4u32);
    assert!(
        context.set_kgp_atlas(
            &vec![0u8; (width as usize) * 4 * (height as usize)],
            width,
            height
        ),
        "恰好够长的载荷必须被接受"
    );

    // 三条拒绝路径都要说出来：长度不足、零尺寸（清空）、超设备上限。
    let short_by_one = vec![0u8; (width as usize) * 4 * (height as usize) - 1];
    assert!(
        !context.set_kgp_atlas(&short_by_one, width, height),
        "长度不足必须报告未装载"
    );
    assert!(
        !context.set_kgp_atlas(&[], 0, 0),
        "零尺寸（清空）必须报告未装载"
    );
    let oversized = context.device.limits().max_texture_dimension_2d + 1;
    assert!(
        !context.set_kgp_atlas(&vec![0u8; oversized as usize * 4], oversized, 1),
        "超设备上限必须报告未装载"
    );
}
