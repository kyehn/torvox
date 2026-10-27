use super::*;

#[test]
fn cell_instance_size() {
    // 2+2+2+4+4+4+2+1+2 个 f32 = 23 × 4 = 92 字节。去掉从不参与运算的
    // 字形前进宽度后少了 4 字节；顶点属性表随之少一项（见 `cell_instance_attribs_locations`）。
    assert_eq!(std::mem::size_of::<CellInstance>(), 92);
}

/// 交换链尺寸判定表：缩放、下限、上限钳制三个不变量各有用例。
/// 判红方式：去掉钳制则前两条钳制断言判红；去掉下限则零尺寸两条判红。
#[test]
fn surface_extent_decision_table() {
    const NO_LIMIT: u32 = u32::MAX;

    // 缩放：`RENDER_SCALE` 为 1.0 时逐像素透传；期望按该常量推导，
    // 常量一旦改动本表即暴露意图而非静默跟随。
    assert_eq!(
        crate::render::context::surface_extent(1080, 2400, NO_LIMIT),
        (
            (1080.0 * crate::render::RENDER_SCALE) as u32,
            (2400.0 * crate::render::RENDER_SCALE) as u32
        )
    );

    // 上限钳制：超过 `max_texture_dimension_2d` 的尺寸必须被钳回该上限。
    assert_eq!(
        crate::render::context::surface_extent(8192, 4096, 4096),
        (4096, 4096)
    );
    // 只有一维超限时另一维不受影响。
    assert_eq!(
        crate::render::context::surface_extent(600, 9000, 8192),
        (600, 8192)
    );
    // 恰好等于上限时原样通过（钳制 MUST NOT 把合法尺寸改小）。
    assert_eq!(
        crate::render::context::surface_extent(2048, 2048, 2048),
        (2048, 2048)
    );

    // 下限：零尺寸（空窗口/尚未布局）不得原样交给 configure。
    assert_eq!(crate::render::context::surface_extent(0, 0, 4096), (1, 1));
    // 设备上报零上限（异常驱动）时钳制区间退化为 [1, 1]，MUST NOT panic。
    assert_eq!(
        crate::render::context::surface_extent(3000, 4000, 0),
        (1, 1)
    );
}

#[test]
fn scroll_px_offset_translates_viewport_down() {
    // Positive offset moves content down: NDC Y shrinks by 2*px/height.
    let base = orthographic_projection(800.0, 600.0);
    let moved = apply_scroll_px_offset(base, 30.0, 600.0);
    assert!((moved[3][1] - (1.0 - 30.0 * 2.0 / 600.0)).abs() < 1e-6);
    // Untouched rows (exact copy of the input rows).
    assert!(
        moved[0]
            .iter()
            .zip(base[0])
            .all(|(moved_value, base_value)| (moved_value - base_value).abs() < 1e-9)
    );
    assert!(
        moved[1]
            .iter()
            .zip(base[1])
            .all(|(moved_value, base_value)| (moved_value - base_value).abs() < 1e-9)
    );
    assert!(
        moved[2]
            .iter()
            .zip(base[2])
            .all(|(moved_value, base_value)| (moved_value - base_value).abs() < 1e-9)
    );
    // Zero offset / zero height are identity.
    let identity = apply_scroll_px_offset(base, 0.0, 600.0);
    assert!(
        identity
            .iter()
            .flatten()
            .zip(base.iter().flatten())
            .all(|(moved_value, base_value)| (moved_value - base_value).abs() < 1e-9)
    );
    let zero_height = apply_scroll_px_offset(base, 30.0, 0.0);
    assert!(
        zero_height
            .iter()
            .flatten()
            .zip(base.iter().flatten())
            .all(|(moved_value, base_value)| (moved_value - base_value).abs() < 1e-9)
    );
}

/// 渲染稳定性 spec §3「滚动不得有可见撕裂」：优先 vsync 约束的
/// Mailbox（新帧覆盖旧帧、无背压，滚动时最新帧胜出），其次
/// Fifo / AutoVsync；Immediate 仅作驱动只支持它时的兜底。
/// 见 context.rs select_present_mode。
#[test]
fn select_present_mode_prefers_vsync() {
    fn base_caps() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Rgba8Unorm],
            present_modes: vec![wgpu::PresentMode::Immediate],
            alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format_capabilities: vec![],
        }
    }

    // Mailbox（即时性 + vsync，无撕裂）在含 Immediate/Fifo 的候选里优先。
    let mut caps = base_caps();
    caps.present_modes = vec![
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
        wgpu::PresentMode::Fifo,
    ];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::Mailbox,
        "Mailbox 优先于 Immediate（vsync 防撕裂，滚动时最新帧胜出）"
    );

    // 无 Mailbox -> Fifo（标准 vsync）。
    let mut caps = base_caps();
    caps.present_modes = vec![
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Fifo,
        wgpu::PresentMode::AutoVsync,
    ];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::Fifo
    );

    // 无 Mailbox/Fifo -> AutoVsync。
    let mut caps = base_caps();
    caps.present_modes = vec![wgpu::PresentMode::Immediate, wgpu::PresentMode::AutoVsync];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::AutoVsync
    );

    // 驱动只支持 Immediate -> Immediate 兜底（无法避免撕裂时保持可用）。
    let base = base_caps();
    assert_eq!(
        Renderer::select_present_mode(&base),
        wgpu::PresentMode::Immediate
    );
}

#[test]
fn cell_instance_buffer_layout() {
    let layout = CellInstance::buffer_layout();
    assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);
    assert!(layout.array_stride > 0);
}

#[test]
fn cell_instance_pod_roundtrip() {
    let instance = CellInstance {
        quad_origin: [1.0, 2.0],
        atlas_offset: [0.5, 0.5],
        atlas_size: [0.1, 0.1],
        foreground: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [1.0, 1.0, 1.0, 1.0],
        quad_size: [3.0, 4.0],
        flags: 5.0,
        bearing: [8.0, 9.0],
    };
    let bytes = bytemuck::bytes_of(&instance);
    let back: &CellInstance = bytemuck::from_bytes(bytes);
    assert!(f32_arrays_equal(&back.quad_origin, &[1.0, 2.0]));
    assert!(f32_eq(back.flags, 5.0));
    assert!(f32_arrays_equal(&back.bearing, &[8.0, 9.0]));
}

#[test]
fn cell_instance_zeroable() {
    let instance: CellInstance = bytemuck::Zeroable::zeroed();
    assert!(f32_arrays_equal(&instance.quad_origin, &[0.0, 0.0]));
    assert!(f32_arrays_equal(
        &instance.foreground,
        &[0.0, 0.0, 0.0, 0.0]
    ));
    assert!(f32_eq(instance.flags, 0.0));
    assert!(f32_arrays_equal(&instance.bearing, &[0.0, 0.0]));
}

#[test]
fn gpu_uniforms_size() {
    // #[repr(C)] layout: 64 (mat4) + 8 (vec2) + 8 (std140 trailing padding) = 80.
    // Matches the WGSL `Uniforms`, whose struct size is likewise rounded to 16.
    assert_eq!(std::mem::size_of::<GpuUniforms>(), 80);
}

#[test]
fn orthographic_projection_basic() {
    let proj = orthographic_projection(100.0, 100.0);
    // [0][0] = 2/width
    assert!((proj[0][0] - 0.02).abs() < f32::EPSILON);
    // [1][1] = -2/height (flipped Y for the Vulkan swapchain)
    assert!((proj[1][1] + 0.02).abs() < f32::EPSILON);
    assert!((proj[3][1] - 1.0).abs() < f32::EPSILON, "translation Y");
    // [3][3] = 1 (result.w=1 for all vertices via Rust row-major→WGSL column-major)
    assert!((proj[3][3] - 1.0).abs() < f32::EPSILON);
    // Rust row-major → WGSL column-major: result[0]=sum_j Rust[j][0]*input_vector[j]
    // For input_vector=(1,1,0,1):
    //   result[0]=2/w*1 + 0*1 + 0*0 + (-1)*1 = 0.02-1 = -0.98
    //   result[1]=0*1 + (-2/h)*1 + 0*0 + 1*1 = -0.02+1 = 0.98
    //   result[3]=0*1 + 0*1 + 0*0 + 1*1 = 1
    let input_vector = [1.0_f32, 1.0, 0.0, 1.0];
    let mut result = [0.0_f32; 4];
    for component_index in 0..4 {
        result[component_index] = proj[0][component_index] * input_vector[0]
            + proj[1][component_index] * input_vector[1]
            + proj[2][component_index] * input_vector[2]
            + proj[3][component_index] * input_vector[3];
    }
    assert!(
        (result[0] - (-0.98)).abs() < 1e-6,
        "result[0]={}",
        result[0]
    );
    assert!((result[1] - (0.98)).abs() < 1e-6, "result[1]={}", result[1]);
    assert!((result[3] - 1.0).abs() < 1e-6, "result[3]={}", result[3]);
}

#[test]
fn cell_instance_attribs_locations() {
    let attribs = CellInstance::ATTRIBS;
    assert_eq!(attribs.len(), 9);
    assert_eq!(attribs[0].shader_location, 1);
    assert_eq!(attribs[1].shader_location, 2);
    assert_eq!(attribs[5].shader_location, 10);
    assert_eq!(attribs[8].shader_location, 8);
}

#[test]
fn orthographic_projection_zero_size() {
    let proj = orthographic_projection(0.0, 0.0);
    // 2.0 / 0.0 = inf
    assert!(proj[0][0].is_infinite());
}

#[test]
fn gpu_compute_write_color() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let output1 = run_compute_and_capture(&device, &queue, write_storage_buffer_shader());
    assert_ne!(
        output1,
        [0, 0, 0, 0],
        "write_color: shader produced all zeros — pipeline broken"
    );

    let output2 = run_compute_and_capture(&device, &queue, write_storage_buffer_shader());
    assert_eq!(output1, output2, "write_color: non-deterministic output");
}

#[test]
fn gpu_compute_blend() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let output1 = run_compute_and_capture(&device, &queue, blend_shader());
    assert_ne!(
        output1,
        [0, 0, 0, 0],
        "blend: shader produced all zeros — pipeline broken"
    );

    // Deterministic: second run produces same bytes
    let output2 = run_compute_and_capture(&device, &queue, blend_shader());
    assert_eq!(output1, output2, "blend: non-deterministic output");
}

#[test]
fn orthographic_projection_resize_changes_mapping() {
    let proj_wide = orthographic_projection(800.0, 400.0);
    let proj_small = orthographic_projection(800.0, 200.0);

    // Same world Y should map to different clip Y when height changes.
    let world_y = 100.0;
    let clip_y_400 = -2.0 * world_y / 400.0 + 1.0;
    let clip_y_200 = -2.0 * world_y / 200.0 + 1.0;
    assert!(
        clip_y_200 < clip_y_400,
        "smaller height -> same world Y maps lower in clip space"
    );

    // Verify using matrix directly (Rust row-major → WGSL column-major):
    // clip_y = proj[1][1] * world_y + proj[3][1]
    let result_400: f32 = proj_wide[1][1] * world_y + proj_wide[3][1];
    let result_200: f32 = proj_small[1][1] * world_y + proj_small[3][1];
    assert!(
        result_200 < result_400,
        "matrix clip_y at h=200 ({}) < h=400 ({})",
        result_200,
        result_400
    );
}

#[test]
fn orthographic_projection_resize_full_range() {
    // Compute WGSL clip_y = proj[0][1]*v[0] + proj[1][1]*v[1] + proj[2][1]*v[2] + proj[3][1]*v[3]
    // (Rust row-major → WGSL column-major multiplication)
    let clip_y = |proj: &[[f32; 4]; 4], y: f32| -> f32 {
        proj[0][1] * 0.0 + proj[1][1] * y + proj[2][1] * 0.0 + proj[3][1] * 1.0
    };

    // At 800x400 the entire surface height maps to clip [-1, 1].
    let proj = orthographic_projection(800.0, 400.0);
    let clip_y_top = clip_y(&proj, 0.0);
    let clip_y_bot = clip_y(&proj, 400.0);

    assert!(
        (clip_y_top - 1.0).abs() < 1e-6,
        "top of surface (y=0) -> clip_y=+1, got {}",
        clip_y_top
    );
    assert!(
        (clip_y_bot + 1.0).abs() < 1e-6,
        "bottom of surface (y=400) -> clip_y=-1, got {}",
        clip_y_bot
    );

    // At smaller height 800x250, bottom should still map to clip_y=-1.
    let proj_250 = orthographic_projection(800.0, 250.0);
    let clip_y_bot_250 = clip_y(&proj_250, 250.0);
    assert!(
        (clip_y_bot_250 + 1.0).abs() < 1e-6,
        "bottom of smaller surface (y=250) -> clip_y=-1, got {}",
        clip_y_bot_250
    );
}

#[test]
fn orthographic_projection_resize_gpu_uniforms() {
    let uniforms_800 = GpuUniforms {
        projection: orthographic_projection(800.0, 600.0),
        atlas_size: [1024.0, 1024.0],
        _padding: [0.0; 2],
    };
    let uniforms_400 = GpuUniforms {
        projection: orthographic_projection(800.0, 400.0),
        atlas_size: [1024.0, 1024.0],
        _padding: [0.0; 2],
    };

    // Same projection/atlas layout, different height
    assert_eq!(
        std::mem::size_of_val(&uniforms_800),
        std::mem::size_of_val(&uniforms_400)
    );
    // 投影矩阵的 Y 缩放不同
    assert!(
        (uniforms_800.projection[1][1] - uniforms_400.projection[1][1]).abs() > 0.001,
        "Y-scale should differ: 800={} 400={}",
        uniforms_800.projection[1][1],
        uniforms_400.projection[1][1]
    );
    let bytes = bytemuck::bytes_of(&uniforms_400);
    let back: &GpuUniforms = bytemuck::from_bytes(bytes);
    assert!(f32_eq(back.projection[1][1], uniforms_400.projection[1][1]));
}

#[test]
fn cursor_rendering_on_visible_cursor() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell_data = vec![
        CellData {
            background: [0.0, 0.0, 0.0, 1.0],
            ..test_cell_data('A' as u32, 0, 0)
        },
        CellData {
            background: [0.0, 0.0, 0.0, 1.0],
            ..test_cell_data(0, 0, 1)
        },
    ];
    let cursor = test_cell_cursor(true);
    let instances = build_instances(
        &cell_data,
        test_config(1, 2, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 2);
    let cursor_cell = &instances[0];
    // Block cursor alpha = cursor_color[3] * 0.7 (CURSOR_BLOCK_ALPHA constant)
    assert!(
        f32_arrays_equal(&cursor_cell.background, &[1.0, 1.0, 1.0, 0.7]),
        "cursor cell background should be white with block alpha when cursor_visible=true"
    );
    let non_cursor_cell = &instances[1];
    assert!(
        !f32_arrays_equal(&non_cursor_cell.background, &[1.0, 1.0, 1.0, 1.0]),
        "non-cursor cell background should NOT be white"
    );
}

#[test]
fn cursor_not_rendered_when_invisible() {
    let mut font_pipeline = ascii_font();
    let instances = build_hidden_cursor_probe_instance('A' as u32, &mut font_pipeline);
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        !f32_arrays_equal(&cell.background, &[1.0, 1.0, 1.0, 1.0]),
        "cursor cell should not have white background when cursor_visible=false"
    );
}

#[test]
fn reverse_video_applied_to_blank_cell() {
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut font_pipeline = ascii_font();
    let foreground = [1.0, 0.0, 0.0, 1.0];
    let background = [0.0, 0.0, 1.0, 1.0];
    let instances = build_cursor_probe_instance(
        0x20,
        foreground,
        background,
        1 << cell_flags::REVERSE,
        false,
        &mut font_pipeline,
        CursorStyle::Block,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Reverse video swaps foreground/background: blank cell background must become the foreground,
    // foreground must become the background.
    assert!(
        f32_arrays_equal(&cell.background, &foreground),
        "reversed blank cell background must equal foreground"
    );
    assert!(
        f32_arrays_equal(&cell.foreground, &background),
        "reversed blank cell foreground must equal background"
    );
}

/// 增量构建与全量构建的输出 MUST 逐字节一致。
///
/// 脏带路径只决定「哪些行重画」，行内容必须与全新全量渲染相同——termlib
/// `TerminalRedrawTest`（`changed.sameAs(fresh)`）在本仓的等效判据。判别力：增量
/// 路径若在复制干净行时错位一行、或漏掉光标行，两条输出立刻不同。
#[test]
fn cached_build_matches_full_build_byte_for_byte() {
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let (rows, cols) = (4u32, 5u32);
    let cells = non_default_grid(rows, cols);
    let cursor = CellCursor {
        row: 2,
        col: 1,
        ..test_cell_cursor(true)
    };
    let config = test_config(rows, cols, cell_width, cell_height, cursor, &[]);

    let mut full = Vec::new();
    assert!(
        crate::render::build_instances_from_cell_data(
            &cells,
            config,
            &mut font_pipeline,
            &mut full
        )
        .is_some(),
        "full build failed"
    );

    let mut cache = crate::render::cell_builder::CachedInstances::new(rows, cols);
    let all_dirty = vec![true; rows as usize];
    let mut first = Vec::new();
    assert!(
        crate::render::cell_builder::build_instances_cached(
            &cells,
            config,
            &mut font_pipeline,
            &all_dirty,
            &mut cache,
            &mut first,
        )
        .is_some(),
        "cache-populating build failed"
    );

    let mut dirty = vec![false; rows as usize];
    dirty[2] = true;
    let mut partial = Vec::new();
    assert!(
        crate::render::cell_builder::build_instances_cached(
            &cells,
            config,
            &mut font_pipeline,
            &dirty,
            &mut cache,
            &mut partial,
        )
        .is_some(),
        "partial build failed"
    );

    assert_eq!(
        bytemuck::cast_slice::<CellInstance, u8>(&full),
        bytemuck::cast_slice::<CellInstance, u8>(&partial),
        "增量构建的输出必须与全量构建逐字节一致（脏带只改画哪些行，不改行内容）"
    );
}

/// resize 循环 MUST 整缓存重建：实例数等于当前网格，不累积旧网格的行。
///
/// 判别力：行布局失效后若只按行尾偏移增量写，旧网格的行会残留在缓存里，
/// 输出实例数随网格切换累加（这里立刻判红）。
#[test]
fn resize_cycles_do_not_accumulate_stale_rows() {
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cursor = CellCursor::default();
    let mut cache = crate::render::cell_builder::CachedInstances::new(4, 5);
    for (rows, cols) in [(4u32, 5u32), (3, 6), (5, 4), (2, 8), (4, 5)] {
        let cells = non_default_grid(rows, cols);
        let config = test_config(rows, cols, cell_width, cell_height, cursor, &[]);
        let mut out = Vec::new();
        assert!(
            crate::render::cell_builder::build_instances_cached(
                &cells,
                config,
                &mut font_pipeline,
                &vec![true; rows as usize],
                &mut cache,
                &mut out,
            )
            .is_some(),
            "build for {rows}x{cols} failed"
        );
        assert_eq!(
            out.len(),
            (rows * cols) as usize,
            "resize 后实例数必须等于当前网格单元数（{rows}x{cols}），不得累积旧网格的行"
        );
    }
}

#[test]
fn cluster_cell_merged_precomposed_emits_single_primary() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let mut extras = [0u32; 7];
    extras[0] = 0x301;
    let cell_data = vec![CellData {
        grapheme_extra: extras,
        ..test_cell_data('e' as u32, 0, 0)
    }];
    let cursor = test_cell_cursor(false);
    let instances = build_configured_cell_instance(
        &cell_data,
        cursor,
        cell_width,
        cell_height,
        &mut font_pipeline,
    );
    assert_eq!(
        instances.len(),
        1,
        "precomposed cluster must emit exactly the primary quad, got {}",
        instances.len()
    );
    assert!(
        font_pipeline
            .caches
            .shape_cache
            .iter()
            .any(|(key, _)| key.text == "e\u{301}"),
        "cluster shape must be cached"
    );
}

#[test]
fn cluster_cell_multi_mark_shapes_positioned_overlays() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    // a + two combining marks shapes to two glyphs: base primary plus
    // one positioned overlay from the shaper.
    let mut extras = [0u32; 7];
    extras[0] = 0x301;
    extras[1] = 0x302;
    let cluster_cell = |row: u32, col: u32| CellData {
        grapheme_extra: extras,
        ..test_cell_data('a' as u32, row, col)
    };
    let blank_cell = |row: u32, col: u32| test_cell_data(' ' as u32, row, col);
    // 位于非零列与行的字素簇证明叠加层原点含完整网格原点，而非仅整形偏移。
    let cell_data = vec![
        cluster_cell(0, 0),
        blank_cell(0, 1),
        blank_cell(0, 2),
        cluster_cell(0, 3),
        blank_cell(1, 0),
        cluster_cell(1, 1),
        blank_cell(1, 2),
        blank_cell(1, 3),
    ];
    let cursor = CellCursor {
        row: 9,
        col: 9,
        ..test_cell_cursor(false)
    };
    let instances = build_instances(
        &cell_data,
        test_config(2, 4, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    // Primary glyph plus the positioned combining-mark overlay.
    assert!(
        instances.len() >= 2,
        "cluster cell must emit primary plus overlay, got {}",
        instances.len()
    );
    // 叠加层原点带有整形偏移：用同一次簇整形重算期望偏移，
    // 并要求恰在该位置出现实例（基准格位于第 0 列）。
    let shaped = font_pipeline.shape_run("a\u{301}\u{302}");
    assert!(
        shaped.len() > 1,
        "fixture must shape multi-mark cluster to several glyphs"
    );
    let expected_origin = [shaped[1].x_offset, shaped[1].y_offset];
    let is_shaped_overlay = |instance: &crate::render::CellInstance| {
        instance.atlas_size != [0.0; 2] && instance.quad_origin == expected_origin
    };
    assert!(
        instances.iter().any(is_shaped_overlay),
        "a glyph overlay must sit at the shaper offset {expected_origin:?}"
    );
    let expected_shifted = [3.0 * cell_width + shaped[1].x_offset, shaped[1].y_offset];
    let is_shifted_overlay = |instance: &crate::render::CellInstance| {
        instance.atlas_size != [0.0; 2] && instance.quad_origin == expected_shifted
    };
    assert!(
        instances.iter().any(is_shifted_overlay),
        "an overlay must sit at grid origin plus shaper offset {expected_shifted:?}"
    );
    let expected_row = [
        cell_width + shaped[1].x_offset,
        cell_height + shaped[1].y_offset,
    ];
    let is_row_overlay = |instance: &crate::render::CellInstance| {
        instance.atlas_size != [0.0; 2] && instance.quad_origin == expected_row
    };
    assert!(
        instances.iter().any(is_row_overlay),
        "an overlay must include the grid row origin {expected_row:?}"
    );
    // The cluster shaping ran through the shared shape cache.
    assert!(
        font_pipeline
            .caches
            .shape_cache
            .iter()
            .any(|(key, _)| key.text == "a\u{301}\u{302}"),
        "cluster shape must be cached"
    );
}

#[test]
fn cluster_cell_invalid_extra_falls_back_without_overlay() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    // An unrepresentable extra (lone surrogate) cannot join the cluster
    // string and cannot convert back to char: shaping is skipped and the
    // fallback drops it, leaving exactly the primary quad.
    let mut extras = [0u32; 7];
    extras[0] = 0xd800;
    let cell_data = vec![CellData {
        grapheme_extra: extras,
        ..test_cell_data('x' as u32, 0, 0)
    }];
    let cursor = test_cell_cursor(false);
    let instances = build_configured_cell_instance(
        &cell_data,
        cursor,
        cell_width,
        cell_height,
        &mut font_pipeline,
    );
    assert_eq!(
        instances.len(),
        1,
        "invalid extra must not produce overlays, got {}",
        instances.len()
    );
}

// ── Bearing correctness: Termux-aligned font metrics ──────────────

/// Verify bearing_y uses font baseline, not centering.
/// build_cell_instances_from_flat uses raw bearing_y = ascent_pixels - placement.top
/// (no centering, no clamping — the raw font baseline offset).
#[test]
fn bearing_y_uses_font_baseline_not_centering() {
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();

    let chars = ['A', 'g', 'p', '.', ','];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_y = ascent_pixels - info.placement.top as f32;

        let instances = build_single_cell_instance(ch, cell_width, cell_height, &mut font_pipeline);
        let cell = &instances[0];

        assert!(
            (cell.bearing[1] - expected_bearing_y).abs() < 1.0,
            "'{ch}' bearing_y={} should be font baseline {} (not centered)",
            cell.bearing[1],
            expected_bearing_y
        );
    }
}

/// Verify bearing_x uses font's natural left side bearing, not centering.
#[test]
fn bearing_x_uses_font_natural_bearing() {
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();

    let chars = ['A', 'i', 'l', 'W', 'M'];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_x = info.placement.left as f32;

        let instances = build_single_cell_instance(ch, cell_width, cell_height, &mut font_pipeline);
        let cell = &instances[0];

        assert!(
            (cell.bearing[0] - expected_bearing_x).abs() < 1.0,
            "'{ch}' bearing_x={} should be font natural bearing {}",
            cell.bearing[0],
            expected_bearing_x
        );
    }
}

/// 断言一行内所有非降部字符共用同一基线（位图底边）。位图顶边本就
/// 因字形而异（大写高度与小写高度不同），顶边相等反而会让文字错位。
/// 本用例保证字符之间没有纵向错位。
#[test]
fn all_chars_share_same_baseline_y() {
    let mut font_pipeline = ascii_font();
    let (cell_width, cell_height) = font_pipeline.cell_metrics();

    let chars = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
    let cell_data: Vec<crate::terminal::ghostty_terminal::CellData> = chars
        .iter()
        .enumerate()
        .map(|(i, &ch)| test_cell_data(ch as u32, 0, i as u32))
        .collect();
    let cursor = test_cell_cursor(false);
    let instances = build_instances(
        &cell_data,
        test_config(1, chars.len() as u32, cell_width, cell_height, cursor, &[]),
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), chars.len());

    // Baseline alignment: bitmap bottom (origin + bearing + bitmap height)
    // is identical for glyphs without descenders. atlas_size is
    // UV-normalized, so bitmap pixels are recovered with the atlas height.
    let bottoms: Vec<f32> = instances
        .iter()
        .map(|inst| inst.quad_origin[1] + inst.bearing[1] + inst.atlas_size[1] * 1024.0)
        .collect();
    for (index, bottom) in bottoms.iter().enumerate() {
        if chars[index] == 'y' {
            // Descender must reach below the baseline, never float above it.
            assert!(
                *bottom >= bottoms[0] - 1.0,
                "cell[{index}] ('y') bottom={bottom} must not float above baseline {}",
                bottoms[0]
            );
        } else {
            assert!(
                (bottom - bottoms[0]).abs() < 1.0,
                "cell[{index}] bottom={bottom} should match baseline bottom={}",
                bottoms[0]
            );
        }
    }
}

/// Verify CJK bearing_y is not centered.
/// build_cell_instances_from_flat uses raw bearing_y = ascent_pixels - placement.top.
#[test]
fn cjk_bearing_y_not_centered() {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    let ascent_pixels = font_pipeline.ascent_pixels();

    let cjk_chars = ['中', '文', '好'];
    for ch in cjk_chars {
        if let Some(info) = font_pipeline.glyph_information(ch) {
            let expected = ascent_pixels - info.placement.top as f32;

            let cell_data = vec![crate::terminal::ghostty_terminal::CellData {
                width: 2,
                ..test_cell_data(ch as u32, 0, 0)
            }];
            let cursor = test_cell_cursor(false);
            let (cell_width, cell_height) = font_pipeline.cell_metrics();
            let instances = build_instances(
                &cell_data,
                test_config(1, 2, cell_width, cell_height, cursor, &[]),
                &mut font_pipeline,
            );
            let cell = &instances[0];

            assert!(
                (cell.bearing[1] - expected).abs() < 1.0,
                "'{ch}' bearing_y={} should be font baseline {} (not centered)",
                cell.bearing[1],
                expected
            );
        }
    }
}

#[test]
fn gpu_background_plain_color_fill() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);

    let result = context
        .render_to_buffer(&[], &[])
        .expect("wgpu render must succeed");

    // 清屏色即默认主题背景（单源，不手写色值）。
    let default_background = crate::terminal::theme::default_dark().palette.background;
    let idx = (25 * 50 + 25) * 4;
    assert_eq!(
        result[idx], default_background.r,
        "center R should be default theme background R"
    );
    assert_eq!(
        result[idx + 1],
        default_background.g,
        "center G should be default theme background G"
    );
    assert_eq!(
        result[idx + 2],
        default_background.b,
        "center B should be default theme background B"
    );
    assert_eq!(result[idx + 3], 255, "center A should be 255");
}

#[test]
fn search_highlight_contains_cell() {
    let hl = SearchHighlight {
        row: 5,
        start_col: 3,
        end_col_exclusive: 8,
        color: [0, 255, 0, 128],
    };
    let mut by_row: HashMap<i32, Vec<&SearchHighlight>, foldhash::fast::RandomState> =
        HashMap::with_hasher(foldhash::fast::RandomState::default());
    by_row.insert(5, vec![&hl]);
    assert!(cell_highlight(5, 4, &by_row).is_some());
    assert!(cell_highlight(5, 3, &by_row).is_some());
    assert!(cell_highlight(5, 7, &by_row).is_some());
    assert!(cell_highlight(5, 8, &by_row).is_none()); // exclusive end
    assert!(cell_highlight(4, 4, &by_row).is_none()); // wrong row
}

#[test]
fn blend_highlight_basic() {
    let base = [0.0, 0.0, 0.0, 1.0];
    let red_hl = [255, 0, 0, 255];
    let blended = blend_highlight(base, red_hl);
    assert!(f32_arrays_equal(&blended, &[1.0, 0.0, 0.0, 1.0]));
}

#[test]
fn blend_highlight_zero_alpha() {
    let base = [0.2, 0.3, 0.4, 1.0];
    let transparent = [255, 0, 0, 0];
    let blended = blend_highlight(base, transparent);
    assert!(f32_arrays_equal(&blended, &base));
}
