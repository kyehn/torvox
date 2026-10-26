use super::acquire::{AcquireLane, acquire_worker_tx, drain_in_flight_acquires};
use super::bands::{instance_upload_layout, partial_frame_bands};
use crate::render::Renderer;

#[test]
fn acquire_worker_is_singleton() {
    let tx1 = acquire_worker_tx();
    let tx2 = acquire_worker_tx();
    assert!(
        std::ptr::eq(tx1, tx2),
        "acquire_worker must return the same sender each call"
    );
}

// ── 在途取帧排干（AcquireLane）判定表 ──

fn idle_lane() -> AcquireLane {
    AcquireLane {
        in_flight: std::sync::Mutex::new(0),
        idle: std::sync::Condvar::new(),
    }
}

/// 无在途取帧时排干必须立即成功：`attach_surface` 的首次挂载与一切正常
/// surface 切换都靠这条路径直接放行，绝不能凭空等待。
#[test]
fn drain_succeeds_immediately_when_idle() {
    assert!(idle_lane().drain(std::time::Duration::from_millis(0)));
}

/// 有在途取帧时排干必须在期限后**失败**，而不是假装成功——
/// 假装成功正是同窗口重建撞 `ERROR_NATIVE_WINDOW_IN_USE_KHR` 的原因。
#[test]
fn drain_times_out_while_a_request_is_in_flight() {
    let lane = idle_lane();
    lane.begin();
    assert!(!lane.drain(std::time::Duration::from_millis(10)));
}

/// 工作线程销账后，已在等待的排干方必须被唤醒：release→attach 同址的
/// 恢复路径靠这条唤醒在同一次调用里完成重建，而不是退回拒绝分支。
#[test]
fn drain_wakes_up_when_the_in_flight_request_finishes() {
    let lane = std::sync::Arc::new(idle_lane());
    lane.begin();
    let finisher = std::sync::Arc::clone(&lane);
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        finisher.finish();
    });
    assert!(lane.drain(std::time::Duration::from_secs(5)));
    worker.join().expect("finisher thread joins");
}

/// 全局 lane 在任何取帧之前处于排干态：首个 `attach_surface`（尚无旧 surface）
/// 不得被守卫拦住。
#[test]
fn global_lane_is_drained_before_any_acquire() {
    assert!(drain_in_flight_acquires(std::time::Duration::from_millis(
        0
    )));
}

// ── should_render_partial 判定表 ──

/// `(ready, invalidated, bands, kgp) -> partial`
fn partial(args: (bool, bool, usize, bool)) -> bool {
    Renderer::should_render_partial(args.0, args.1, args.2, args.3)
}

/// 已构建、尺寸匹配、代际一致、含脏行。
fn built_cache() -> crate::render::cell_builder::CachedInstances {
    let mut cache = crate::render::cell_builder::CachedInstances::new(4, 2);
    cache.update_for_test(&[2usize, 3, 3, 6]);
    cache
}

/// 两段上传的字节边界：清除段与真实例在同一缓冲内不得重叠，且末段终点须等于
/// 两段合计（容量按此一次算足；先写清除段再为真实例扩容会把清除段丢掉）。
#[test]
fn instance_upload_layout_table() {
    let stride = std::mem::size_of::<crate::render::CellInstance>() as u64;
    assert!(
        stride > 0 && stride.is_multiple_of(4),
        "顶点偏移须 4 字节对齐"
    );
    for (clear_count, draw_count) in [(0usize, 0usize), (0, 5), (3, 5), (1, 0)] {
        let (clear_end, draw_end) = instance_upload_layout(clear_count, draw_count);
        assert_eq!(
            clear_end,
            clear_count as u64 * stride,
            "{clear_count}/{draw_count}"
        );
        assert_eq!(
            draw_end,
            (clear_count + draw_count) as u64 * stride,
            "{clear_count}/{draw_count}"
        );
        assert!(clear_end <= draw_end, "真实例起点不得早于清除段终点");
    }
}

#[test]
fn dirty_rows_yield_bands_when_cache_matches() {
    let cache = built_cache();
    let bands = partial_frame_bands(
        false,
        Some(&cache),
        4,
        2,
        0,
        Some(&[false, true, false, true]),
    )
    .expect("cache 一致且非全量重建时必须给出脏带");
    assert_eq!(bands.len(), 2);
    assert_eq!((bands[0].start_row, bands[0].end_row_exclusive), (1, 2));
    assert_eq!((bands[0].instance_start, bands[0].instance_end), (2, 3));
}

/// 回归钉：掩码与行数不齐时必须走安全分支，绝不越界。
///
/// `compute_dirty_bands` 产出的行号会直达 `cache.band_slice` →
/// `row_ends[row]`，而 `row_ends` 的长度只由 `is_compatible(rows, cols)`
/// 保证与 `rows` 相等，**不**由掩码长度保证。此前掩码被原样喂进去，唯一的
/// 防线是 `band_slice` 里的 `debug_assert!`——release 构建形同虚设，
/// 渲染线程会当场越界 panic。判别力看：获得与 4 行等长掩码相同的带。
#[test]
fn overlong_dirty_mask_is_truncated_not_indexed_out_of_bounds() {
    let cache = built_cache();
    // 掩码比行数长（前 4 位与等长用例完全相同，多出的 3 位全是脏行）。
    let bands = partial_frame_bands(
        false,
        Some(&cache),
        4,
        2,
        0,
        Some(&[false, true, false, true, true, true, true]),
    )
    .expect("超长掩码应截断后照常给出脏带");
    // 与等长掩码的结果逐字段一致：多出的位必须被忽略。
    let expected = partial_frame_bands(
        false,
        Some(&cache),
        4,
        2,
        0,
        Some(&[false, true, false, true]),
    )
    .expect("等长对照");
    assert_eq!(bands.len(), expected.len());
    for (band, want) in bands.iter().zip(expected.iter()) {
        assert_eq!(
            (band.start_row, band.end_row_exclusive),
            (want.start_row, want.end_row_exclusive)
        );
        assert_eq!(
            (band.instance_start, band.instance_end),
            (want.instance_start, want.instance_end)
        );
    }
    assert!(
        bands.iter().all(|band| band.start_row < 4),
        "行号必须落在 [0, rows) 内"
    );
}

/// 掩码比行数短时不能取到全部行：宁可整屏重画，也不拿不完整的脏带做增量更新
/// （漏掉的行会一直停在累加器里的旧像素上）。
#[test]
fn short_dirty_mask_falls_back_to_a_full_redraw() {
    let cache = built_cache();
    assert!(
        partial_frame_bands(false, Some(&cache), 4, 2, 0, Some(&[true, true])).is_none(),
        "掩码短于行数时必须放弃稀疏带"
    );
}

/// 全量重建（换字体、缩放、缓存失效）时每格 UV 与几何都换了：稀疏带会让
/// 干净行沿用累加器里的旧像素，必须强制全量重绘。这是「换字体后整屏仍是
/// 旧字」的回归钉。
#[test]
fn full_rebuild_never_yields_partial_bands() {
    let cache = built_cache();
    assert!(
        partial_frame_bands(
            true,
            Some(&cache),
            4,
            2,
            0,
            Some(&[false, true, false, true])
        )
        .is_none()
    );
}

/// 缓存未构建时（尺寸上「兼容」但一行数据都没有）不得据稀疏带做部分渲染。
#[test]
fn unbuilt_cache_yields_no_bands() {
    let cache = crate::render::cell_builder::CachedInstances::new(4, 2);
    assert!(partial_frame_bands(false, Some(&cache), 4, 2, 0, Some(&[true; 4])).is_none());
}

/// 尺寸变化（缩放）与图集代际变化（UV 已搬迁）同样强制全量。
#[test]
fn incompatible_size_or_generation_yields_no_bands() {
    let cache = built_cache();
    assert!(partial_frame_bands(false, Some(&cache), 8, 2, 0, Some(&[true; 4])).is_none());
    assert!(partial_frame_bands(false, Some(&cache), 4, 2, 7, Some(&[true; 4])).is_none());
}

#[test]
fn missing_cache_or_mask_yields_no_bands() {
    let cache = built_cache();
    assert!(partial_frame_bands(false, None, 4, 2, 0, Some(&[true; 4])).is_none());
    assert!(partial_frame_bands(false, Some(&cache), 4, 2, 0, None).is_none());
}

/// 空脏掩码不产生带——与构建行为一致，无带即由 `should_render_partial` 的
/// `band_count > 0` 判为全量重绘。
#[test]
fn empty_mask_yields_no_bands() {
    let cache = built_cache();
    let bands = partial_frame_bands(false, Some(&cache), 4, 2, 0, Some(&[false; 4]))
        .expect("路径本身可用，只是没有脏行");
    assert!(bands.is_empty());
    assert!(!Renderer::should_render_partial(
        true,
        false,
        bands.len(),
        false
    ));
}

// ── 滚动一致性门控（fix-scroll-residual-tearing） ──────────────

/// 滚动一致性门控决策表：仅当亚行余量相对上一呈现帧发生变化时强制全量。
#[test]
fn scroll_offset_active_decision_table() {
    let active = Renderer::is_scroll_offset_active;
    // 偏移为零且与上一帧一致：可走部分路径。
    assert!(!active(0.0, 0.0), "rest state must allow partial bands");
    // 余量稳定（含拖动中停住）：不再强制全量，避免空闲帧每帧全量重绘。
    assert!(
        !active(15.375, 15.375),
        "stable non-zero offset must allow partial bands"
    );
    // 开始拖动：余量从 0 变为非零，强制全量。
    assert!(active(15.375, 0.0), "starting a drag must force full");
    // 回落至零：余量变化，强制全量。
    assert!(active(0.0, 15.375), "settling back to zero must force full");
    // 余量变化：强制全量。
    assert!(active(1.0, 2.0), "offset change must force full");
    // 亚 EPSILON 的变化视为浮点噪声，不强制全量。
    assert!(
        !active(0.0, f32::EPSILON),
        "sub-epsilon change must not force full"
    );
}

/// 脏带清除实例：每带一个纯色四边形，覆盖该带完整像素矩形（空单元有光标时只发
/// 光标标记，格内其余区域不被本帧覆盖，会残留陈旧光标像素）。
#[test]
fn band_clear_instances_cover_band_rect() {
    let renderer = Renderer::new_with_no_surface();
    let bands = vec![
        crate::render::cell_builder::DirtyBand {
            start_row: 0,
            end_row_exclusive: 1,
            instance_start: 0,
            instance_end: 80,
        },
        crate::render::cell_builder::DirtyBand {
            start_row: 3,
            end_row_exclusive: 5,
            instance_start: 240,
            instance_end: 400,
        },
    ];
    let cell_height = 44.0;
    let clears = renderer.band_clear_instances(&bands, cell_height, 1080, 2400);
    assert_eq!(clears.len(), 2, "one clear quad per band");
    assert_eq!(clears[0].quad_origin, [0.0, 0.0]);
    assert_eq!(clears[0].quad_size, [1080.0, cell_height]);
    assert_eq!(clears[1].quad_origin, [0.0, 3.0 * cell_height]);
    assert_eq!(clears[1].quad_size, [1080.0, 2.0 * cell_height]);
    // 纯色填充：无字形，清除色由背景承担。
    assert_eq!(clears[0].atlas_size, [0.0; 2]);
    assert_eq!(clears[0].background[3], 1.0);
    // 退化几何不产生清除。
    assert!(
        renderer
            .band_clear_instances(&bands, 0.0, 1080, 2400)
            .is_empty()
    );
}

/// 脏带清除不得侵入相邻行的边界像素：向外取整会在选区/搜索高亮的整行着色里
/// 留下 1px 背景色缝隙（「文本选择 不同行间存在空隙」）。分数单元高
/// （模拟器实测 44.625）才暴露，整数高看不出。
#[test]
fn band_clear_range_does_not_bleed_into_neighbour_rows() {
    let cell_height = 44.625_f32;
    // 单元四边形只填充中心落进行跨度的像素行：行 1 = 中心在 [44.625, 89.25)
    // 的像素 = 45..88，故清除范围必须是 [45, 89)，不含像素 44（属行 0）
    // 与 89（属行 2）。
    assert_eq!(
        Renderer::band_clear_pixel_range(1, 2, cell_height, 2400),
        (45, 89)
    );
    // 行 3..5：中心在 [133.875, 223.125) 的像素 = 134..222。
    assert_eq!(
        Renderer::band_clear_pixel_range(3, 5, cell_height, 2400),
        (134, 223)
    );
    // 连续多带与单带覆盖同一像素集合（分带不得改变覆盖）。
    for row in 1..5 {
        let (single_lo, single_hi) =
            Renderer::band_clear_pixel_range(row, row + 1, cell_height, 2400);
        let (band_lo, band_hi) = Renderer::band_clear_pixel_range(1, 5, cell_height, 2400);
        if row == 1 {
            assert_eq!((single_lo, single_hi), (band_lo, band_lo + 44));
        }
        assert!(single_lo >= band_lo && single_hi <= band_hi);
    }
    // 整屏脏带仍覆盖到最后一行的最后一个像素，且不越出帧高。
    assert_eq!(
        Renderer::band_clear_pixel_range(0, 24, cell_height, 2400),
        (0, 1071)
    );
    assert_eq!(
        Renderer::band_clear_pixel_range(0, 24, cell_height, 1070),
        (0, 1070),
        "帧高上限仍然生效"
    );
}

#[test]
fn partial_happy_path() {
    assert!(partial((true, false, 1, false)));
}

#[test]
fn partial_requires_accumulator() {
    assert!(!partial((false, false, 1, false)));
}

#[test]
fn partial_requires_invalidated_false() {
    assert!(!partial((true, true, 1, false)));
}

#[test]
fn partial_requires_bands() {
    assert!(!partial((true, false, 0, false)));
}

/// 脏带与 kitty 图像相交判据（表驱动）：图像像素被脏带擦掉时必须重画图像。
#[test]
fn kitty_overlay_is_redrawn_only_when_its_pixels_are_wiped() {
    // 图像四边形：原点 (0, 0)，尺寸 44x44 像素（占第 0、1 两行，行高 22）。
    let instance = crate::render::KittyGraphicsInstance::new(
        [0.0, 0.0],
        [44.0, 44.0],
        [0.0, 0.0],
        [1.0, 1.0],
        1.0,
    );
    let instances = [instance];
    let band = |start: usize, end: usize| crate::render::cell_builder::DirtyBand {
        start_row: start,
        end_row_exclusive: end,
        instance_start: 0,
        instance_end: 0,
    };
    // 脏带与图像所在行相交 → 必须重画（否则图像被擦掉一半）。
    assert!(Renderer::kgp_touched_by_band(&instances, &band(0, 1), 22.0));
    assert!(Renderer::kgp_touched_by_band(&instances, &band(1, 2), 22.0));
    // 脏带在图像之外 → 不必重画（图像像素原样保留，重复混合会让半透明图越叠越实）。
    assert!(!Renderer::kgp_touched_by_band(
        &instances,
        &band(2, 3),
        22.0
    ));
    // 单元格高度未知时保守重画。
    assert!(Renderer::kgp_touched_by_band(&instances, &band(9, 10), 0.0));
}

#[test]
fn partial_rejected_when_kitty_overlay_changed() {
    // 叠加层本帧发生变化（新增/替换/删除图像）：上一帧的图像像素仍在累加器里，
    // 不整帧重绘就会留下残影。
    assert!(!partial((true, false, 1, true)));
}

#[test]
fn partial_allowed_while_kitty_overlay_is_static() {
    // 图像保持不变时叠加层每帧照画，文本层照旧走脏带：一张静止的图片不该把
    // 终端钉在全量帧的代价上（SwiftShader 实测全量帧 ~500ms → 1-2fps）。
    assert!(partial((true, false, 1, false)));
}
// ── 滚动门控合成判定表：partial 候选与 scroll 门控的合成 ──
// 覆盖生产合成（render_frame_with_plan 经 should_render_partial_frame 求值）：
// 归零稳定基线接受部分渲染；滚动激活（含惯性滚动中、归零复位瞬间、
// 持屏拖动非零保持）一律强制全量，不依赖 GPU。
/// `(accumulator_ready, frame_invalidated, band_count, kgp_overlay_dirty, scroll_active) -> partial`
fn planned_frame(decision: (bool, bool, usize, bool, bool)) -> bool {
    let (accumulator_ready, frame_invalidated, band_count, kgp_overlay_dirty, scroll_active) =
        decision;
    Renderer::should_render_partial_frame(
        accumulator_ready,
        frame_invalidated,
        band_count,
        kgp_overlay_dirty,
        scroll_active,
    )
}

#[test]
fn scroll_gate_composition_accepts_clean_partial() {
    // 归零且稳定：累加器在、未失效、有脏带、无 overlay、无滚动 → 部分。
    assert!(planned_frame((true, false, 1, false, false)));
}

#[test]
fn scroll_gate_composition_forces_full_on_hold_nonzero() {
    // 滚动激活（偏移非零保持），即使累加器/脏带齐全也必须全量。
    assert!(!planned_frame((true, false, 1, false, true)));
}

#[test]
fn scroll_gate_composition_survives_predicate_flips() {
    // 其余四个谓词位各自独立翻转时，scroll 门控始终压过部分判定；
    // 复位瞬间（scroll_active 仍为真）绝不允许脏带残留路径。
    for accumulator_ready in [true, false] {
        for frame_invalidated in [true, false] {
            for band_count in [0usize, 1usize] {
                for kgp_overlay_dirty in [true, false] {
                    assert!(
                        !planned_frame((
                            accumulator_ready,
                            frame_invalidated,
                            band_count,
                            kgp_overlay_dirty,
                            true
                        )),
                        "scroll_active must always veto partial",
                    );
                }
            }
        }
    }
}
