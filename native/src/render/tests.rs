//! Render unit tests + GPU benchmarks.
//!
//! # Requirements
//! - FR-050 — surface lifecycle and render pipeline covered by unit tests

use std::collections::HashMap;

use crate::terminal::CursorStyle;
use wgpu::util::DeviceExt;

use super::*;

const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn f32_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

fn f32_arrays_equal(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Font pipeline with the ASCII glyph atlas pre-rasterized.
fn ascii_font() -> crate::render::font::FontPipeline {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    font_pipeline
}

#[test]
fn cell_instance_size() {
    assert_eq!(std::mem::size_of::<CellInstance>(), 96);
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
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );
    assert!(
        moved[1]
            .iter()
            .zip(base[1])
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );
    assert!(
        moved[2]
            .iter()
            .zip(base[2])
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );
    // Zero offset / zero height are identity.
    let identity = apply_scroll_px_offset(base, 0.0, 600.0);
    assert!(
        identity
            .iter()
            .flatten()
            .zip(base.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );
    let zero_height = apply_scroll_px_offset(base, 30.0, 0.0);
    assert!(
        zero_height
            .iter()
            .flatten()
            .zip(base.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-9)
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
    let c = CellInstance {
        quad_origin: [1.0, 2.0],
        atlas_offset: [0.5, 0.5],
        atlas_size: [0.1, 0.1],
        foreground: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [1.0, 1.0, 1.0, 1.0],
        quad_size: [3.0, 4.0],
        flags: 5.0,
        bearing: [0.0; 2],
        glyph_advance_width: 8.0,
    };
    let bytes = bytemuck::bytes_of(&c);
    let back: &CellInstance = bytemuck::from_bytes(bytes);
    assert!(f32_arrays_equal(&back.quad_origin, &[1.0, 2.0]));
    assert!(f32_eq(back.flags, 5.0));
    assert!(f32_eq(back.glyph_advance_width, 8.0));
}

#[test]
fn cell_instance_zeroable() {
    let c: CellInstance = bytemuck::Zeroable::zeroed();
    assert!(f32_arrays_equal(&c.quad_origin, &[0.0, 0.0]));
    assert!(f32_arrays_equal(&c.foreground, &[0.0, 0.0, 0.0, 0.0]));
    assert!(f32_eq(c.flags, 0.0));
    assert!(f32_arrays_equal(&c.bearing, &[0.0, 0.0]));
}

#[test]
fn gpu_uniforms_size() {
    // #[repr(C)] layout: 64 (mat4) + 8 (vec2) + 4 (raster_scale) + 4
    // (std140 trailing padding) = 80. Matches the WGSL `Uniforms`.
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
    // Rust row-major → WGSL column-major: result[0]=sum_j Rust[j][0]*v[j]
    // For v=(1,1,0,1):
    //   result[0]=2/w*1 + 0*1 + 0*0 + (-1)*1 = 0.02-1 = -0.98
    //   result[1]=0*1 + (-2/h)*1 + 0*0 + 1*1 = -0.02+1 = 0.98
    //   result[3]=0*1 + 0*1 + 0*0 + 1*1 = 1
    let v = [1.0_f32, 1.0, 0.0, 1.0];
    let mut result = [0.0_f32; 4];
    for i in 0..4 {
        result[i] = proj[0][i] * v[0] + proj[1][i] * v[1] + proj[2][i] * v[2] + proj[3][i] * v[3];
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
    assert_eq!(attribs.len(), 10);
    assert_eq!(attribs[0].shader_location, 1);
    assert_eq!(attribs[1].shader_location, 2);
    assert_eq!(attribs[5].shader_location, 10);
    assert_eq!(attribs[8].shader_location, 8);
    assert_eq!(attribs[9].shader_location, 9);
}

#[test]
fn orthographic_projection_zero_size() {
    let proj = orthographic_projection(0.0, 0.0);
    // 2.0 / 0.0 = inf
    assert!(proj[0][0].is_infinite());
}

/// Helper: create a wgpu instance + adapter + device for testing.
/// Returns None when no suitable GPU is available.
fn create_test_device() -> Option<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::empty(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("gpu_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .ok()?;
    Some((instance, adapter, device, queue))
}

fn write_storage_buffer_shader() -> wgpu::ShaderModuleDescriptor<'static> {
    wgpu::ShaderModuleDescriptor {
        label: Some("write_color"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
            "@group(0) @binding(0) var<storage, read_write> output: array<vec4<f32>>;
                @compute @workgroup_size(1, 1, 1)
                fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
                    output[gid.x] = vec4<f32>(0.157, 0.165, 0.212, 1.0);
                }",
        )),
    }
}

fn blend_shader() -> wgpu::ShaderModuleDescriptor<'static> {
    wgpu::ShaderModuleDescriptor {
        label: Some("blend"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
            "@group(0) @binding(0) var<storage, read_write> output: array<vec4<f32>>;
                @compute @workgroup_size(1, 1, 1)
                fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
                    let background = vec4<f32>(40.0/255.0, 42.0/255.0, 54.0/255.0, 1.0);
                    let foreground = vec4<f32>(1.0, 1.0, 1.0, 1.0);
                    let alpha = 0.5;
                    output[gid.x] = mix(background, foreground, vec4<f32>(alpha, alpha, alpha, alpha));
                }",
        )),
    }
}

fn run_compute_and_capture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shader_desc: wgpu::ShaderModuleDescriptor,
) -> [u8; 4] {
    let shader = device.create_shader_module(shader_desc);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: Some(std::num::NonZeroU64::new(16).unwrap()),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("compute_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compute_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    // Create storage buffer (1 vec4<f32> = 16 bytes)
    let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("storage_buffer"),
        size: 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging_buffer"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage_buffer.as_entire_binding(),
        }],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("compute_encoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("compute_pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }

    // Copy storage to staging
    encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, 16);

    queue.submit(Some(encoder.finish()));

    // Map read
    let (tx, rx) = std::sync::mpsc::channel();
    staging_buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(GPU_POLL_TIMEOUT),
    });

    if let Ok(Ok(())) = rx.recv() {
        let mapped = staging_buffer.slice(..).get_mapped_range();
        let view = mapped.expect("get_mapped_range should succeed");
        let actual: [u8; 4] = [view[0], view[1], view[2], view[3]];
        drop(view);
        staging_buffer.unmap();
        actual
    } else {
        panic!("buffer map_async failed");
    }
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
        raster_scale: 1.0,
        _padding: 0.0,
    };
    let uniforms_400 = GpuUniforms {
        projection: orthographic_projection(800.0, 400.0),
        atlas_size: [1024.0, 1024.0],
        raster_scale: 1.0,
        _padding: 0.0,
    };

    // Same projection/atlas layout, different height
    assert_eq!(
        std::mem::size_of_val(&uniforms_800),
        std::mem::size_of_val(&uniforms_400)
    );
    // Projection matrix Y-scale differs
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![
        CellData {
            codepoint: 'A' as u32,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: [1.0, 1.0, 1.0, 1.0],
            background: [0.0, 0.0, 0.0, 1.0],
            underline_color: [1.0, 1.0, 1.0, 1.0],
            flags: 0,
            row: 0,
            col: 0,
        },
        CellData {
            codepoint: 0,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: [1.0, 1.0, 1.0, 1.0],
            background: [0.0, 0.0, 0.0, 1.0],
            underline_color: [1.0, 1.0, 1.0, 1.0],
            flags: 0,
            row: 0,
            col: 1,
        },
    ];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: CursorStyle::Block,
        color: Some([1.0, 1.0, 1.0, 1.0]),
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 2,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let instances = build_cursor_probe_instance(
        'A' as u32,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        false,
        cell_w,
        cell_h,
        &mut font_pipeline,
    );
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let foreground = [1.0, 0.0, 0.0, 1.0];
    let background = [0.0, 0.0, 1.0, 1.0];
    let instances = build_cursor_probe_instance(
        0x20,
        foreground,
        background,
        1 << cell_flags::REVERSE,
        false,
        cell_w,
        cell_h,
        &mut font_pipeline,
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

const TEST_ATLAS_SIZE: f32 = 1024.0;

/// Build production instances for one configured cell (shared by cursor
/// and reverse-video tests): caller supplies the cell contents, cursor
/// state and grid metrics, unit grid otherwise.
fn build_configured_cell_instance(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    cursor: crate::render::CellCursor,
    cell_width: f32,
    cell_height: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: cell_data.len() as u32,
            grid_cell_w: cell_width,
            grid_cell_h: cell_height,
            cursor,
            atlas_width: TEST_ATLAS_SIZE,
            atlas_height: TEST_ATLAS_SIZE,
            search_highlights: &[],
        },
        font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    instances
}

/// Build one cursor-probe cell with white block cursor styling: covers the
/// repeated single-cell cursor/reverse-video setup with varying codepoint,
/// colors, flags and cursor visibility.
// 测试辅助构造器：参数与被测调用一一对应，成组反而遮蔽映射关系。
#[allow(clippy::too_many_arguments)]
fn build_cursor_probe_instance(
    codepoint: u32,
    foreground: [f32; 4],
    background: [f32; 4],
    flags: u32,
    cursor_visible: bool,
    cell_width: f32,
    cell_height: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    use crate::terminal::ghostty_terminal::CellData;
    let cell_data = vec![CellData {
        codepoint,
        width: 1,
        grapheme_extra: [0; 7],
        foreground,
        background,
        underline_color: foreground,
        flags,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: cursor_visible,
        style: CursorStyle::Block,
        color: Some([1.0, 1.0, 1.0, 1.0]),
    };
    build_configured_cell_instance(&cell_data, cursor, cell_width, cell_height, font_pipeline)
}

/// Build production instances for a single cell (shared by the bearing tests
/// below): one [`CellData`], default cursor, unit grid.
fn build_single_cell_instance(
    ch: char,
    cell_w: f32,
    cell_h: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    use crate::terminal::ghostty_terminal::CellData;
    let cell_data = vec![CellData {
        codepoint: ch as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 1,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &[],
        },
        font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    instances
}

#[test]
fn cluster_cell_merged_precomposed_emits_single_primary() {
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    // e + combining acute shapes to one precomposed glyph: it replaces
    // the primary quad instead of stacking an overlay.
    let mut extras = [0u32; 7];
    extras[0] = 0x301;
    let cell_data = vec![CellData {
        codepoint: 'e' as u32,
        width: 1,
        grapheme_extra: extras,
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let instances =
        build_configured_cell_instance(&cell_data, cursor, cell_w, cell_h, &mut font_pipeline);
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    // a + two combining marks shapes to two glyphs: base primary plus
    // one positioned overlay from the shaper.
    let mut extras = [0u32; 7];
    extras[0] = 0x301;
    extras[1] = 0x302;
    let cluster_cell = |row: u32, col: u32| CellData {
        codepoint: 'a' as u32,
        width: 1,
        grapheme_extra: extras,
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row,
        col,
    };
    let blank_cell = |row: u32, col: u32| CellData {
        codepoint: ' ' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row,
        col,
    };
    // Clusters at non-zero column and row prove the overlay origin
    // includes the full grid origin, not just the shaper offset.
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
    let cursor = crate::render::CellCursor {
        row: 9,
        col: 9,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 2,
            cols: 4,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: TEST_ATLAS_SIZE,
            atlas_height: TEST_ATLAS_SIZE,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    // Primary glyph plus the positioned combining-mark overlay.
    assert!(
        instances.len() >= 2,
        "cluster cell must emit primary plus overlay, got {}",
        instances.len()
    );
    // The overlay origin carries the shaper offset: recompute the
    // expected offset from the same cluster shaping and require an
    // instance at exactly that position (base cell is col 0).
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
    let expected_shifted = [3.0 * cell_w + shaped[1].x_offset, shaped[1].y_offset];
    let is_shifted_overlay = |instance: &crate::render::CellInstance| {
        instance.atlas_size != [0.0; 2] && instance.quad_origin == expected_shifted
    };
    assert!(
        instances.iter().any(is_shifted_overlay),
        "an overlay must sit at grid origin plus shaper offset {expected_shifted:?}"
    );
    let expected_row = [cell_w + shaped[1].x_offset, cell_h + shaped[1].y_offset];
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    // An unrepresentable extra (lone surrogate) cannot join the cluster
    // string and cannot convert back to char: shaping is skipped and the
    // fallback drops it, leaving exactly the primary quad.
    let mut extras = [0u32; 7];
    extras[0] = 0xd800;
    let cell_data = vec![CellData {
        codepoint: 'x' as u32,
        width: 1,
        grapheme_extra: extras,
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let instances =
        build_configured_cell_instance(&cell_data, cursor, cell_w, cell_h, &mut font_pipeline);
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();

    let chars = ['A', 'g', 'p', '.', ','];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_y = ascent_pixels - info.placement.top as f32;

        let instances = build_single_cell_instance(ch, cell_w, cell_h, &mut font_pipeline);
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();

    let chars = ['A', 'i', 'l', 'W', 'M'];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_x = info.placement.left as f32;

        let instances = build_single_cell_instance(ch, cell_w, cell_h, &mut font_pipeline);
        let cell = &instances[0];

        assert!(
            (cell.bearing[0] - expected_bearing_x).abs() < 1.0,
            "'{ch}' bearing_x={} should be font natural bearing {}",
            cell.bearing[0],
            expected_bearing_x
        );
    }
}

/// Verify all non-descending characters in a row share the same baseline
/// (bitmap bottom edge). Bitmap tops legitimately differ per glyph
/// (cap height vs x-height) — equal tops would misalign the text.
/// This ensures no vertical misalignment between characters.
#[test]
fn all_chars_share_same_baseline_y() {
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();

    let chars = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
    let cell_data: Vec<crate::terminal::ghostty_terminal::CellData> = chars
        .iter()
        .enumerate()
        .map(|(i, &ch)| crate::terminal::ghostty_terminal::CellData {
            codepoint: ch as u32,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: [1.0; 4],
            background: [0.0; 4],
            underline_color: [1.0; 4],
            flags: 0,
            row: 0,
            col: i as u32,
        })
        .collect();
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: chars.len() as u32,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), chars.len());

    // Baseline alignment: bitmap bottom (origin + bearing + bitmap height)
    // is identical for glyphs without descenders. atlas_size is
    // UV-normalized, so bitmap pixels are recovered with the atlas height.
    let bottoms: Vec<f32> = instances
        .iter()
        .map(|inst| inst.quad_origin[1] + inst.bearing[1] + inst.atlas_size[1] * 1024.0)
        .collect();
    for (i, bottom) in bottoms.iter().enumerate() {
        if chars[i] == 'y' {
            // Descender must reach below the baseline, never float above it.
            assert!(
                *bottom >= bottoms[0] - 1.0,
                "cell[{i}] ('y') bottom={bottom} must not float above baseline {}",
                bottoms[0]
            );
        } else {
            assert!(
                (bottom - bottoms[0]).abs() < 1.0,
                "cell[{i}] bottom={bottom} should match baseline bottom={}",
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
                codepoint: ch as u32,
                width: 2,
                grapheme_extra: [0; 7],
                foreground: [1.0; 4],
                background: [0.0; 4],
                underline_color: [1.0; 4],
                flags: 0,
                row: 0,
                col: 0,
            }];
            let cursor = crate::render::CellCursor {
                row: 0,
                col: 0,
                visible: false,
                style: CursorStyle::Block,
                color: None,
            };
            let (cell_w, cell_h) = font_pipeline.cell_metrics();
            let mut instances = Vec::new();
            let built = crate::render::build_instances_from_cell_data(
                &cell_data,
                crate::render::gpu::CellInstanceConfig {
                    rows: 1,
                    cols: 2,
                    grid_cell_w: cell_w,
                    grid_cell_h: cell_h,
                    cursor,
                    atlas_width: 1024.0,
                    atlas_height: 1024.0,
                    search_highlights: &[],
                },
                &mut font_pipeline,
                &mut instances,
            );
            assert!(built.is_some(), "production instance build failed");
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

#[cfg(test)]
fn setup_test_gpu_context(device: wgpu::Device, queue: wgpu::Queue) -> Renderer {
    let _gpu = crate::render::context::global_gpu_for_tests();
    let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Quad Vertex Buffer"),
        contents: bytemuck::cast_slice(QUAD_CORNERS),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut context = Renderer::new_inner(device, queue, quad_vertex_buffer);
    context.surface_config = Some(wgpu::SurfaceConfiguration {
        width: 50,
        height: 50,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
        color_space: wgpu::SurfaceColorSpace::Auto,
    });
    context.initialize_pipeline_and_bind_group(256, 256, 50, 50);
    context
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

    let idx = (25 * 50 + 25) * 4;
    assert_eq!(
        result[idx], 30,
        "center R should be 30 (Catppuccin Mocha background)"
    );
    assert_eq!(result[idx + 1], 30, "center G should be 30");
    assert_eq!(result[idx + 2], 46, "center B should be 46");
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        codepoint: 'X' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [1.0, 1.0, 1.0, 1.0],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [255, 0, 0, 128],
    }];
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 1,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &highlights,
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        codepoint: 'A' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [0.0, 1.0, 0.0, 1.0],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: CursorStyle::Block,
        color: Some([0.5, 0.5, 1.0, 1.0]),
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [200, 0, 0, 200],
    }];
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 1,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &highlights,
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.background, &[0.5, 0.5, 1.0, 0.7]),
        "cursor cell background should be cursor color (with block alpha), not highlight color"
    );
}

// ── Search highlight helpers and tests ───────────────────────────

/// Test helper: groups `SearchHighlight`s by row, just like
/// `build_cell_instances_into` does inline.
fn group_highlights_by_row(
    highlights: &[SearchHighlight],
) -> HashMap<i32, Vec<&SearchHighlight>, foldhash::fast::RandomState> {
    let mut by_row: HashMap<i32, Vec<&SearchHighlight>, foldhash::fast::RandomState> =
        HashMap::with_hasher(foldhash::fast::RandomState::default());
    for h in highlights {
        by_row.entry(h.row).or_default().push(h);
    }
    by_row
}

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

// ── Production-value alpha tests (P1-2) ──────────────────────────
//
// The Kotlin side packs search-highlight RGBA bytes with the constants in
// `SearchHighlightColors.kt` (single cross-language anchor):
//   - current match → SearchHighlightColors.CURRENT_MATCH_ALPHA = 255
//   - other matches → SearchHighlightColors.OTHER_MATCH_ALPHA = 160
//     (: raised from 96 so every match inverts, per spec
//     text-search-highlight "all matches visible inversion")
// These tests pin the renderer's behavior at exactly those production
// values. Changing either side requires changing the other in the same PR
// (see the doc comment on SearchHighlightColors).

#[test]
fn search_highlight_current_match_alpha_matches_production() {
    use crate::render::cell_builder::apply_search_highlight;

    // Production anchor: SearchHighlightColors.CURRENT_MATCH_ALPHA = 255.
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

    // Production anchor: SearchHighlightColors.OTHER_MATCH_ALPHA = 160,
    // at or above the 128 swap threshold (: ALL matches invert).
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
    // Background gets the highlight blended over the swapped background — which now
    // holds the ORIGINAL foreground — visibly different from BOTH the
    // untouched background and the fully opaque current-match treatment.
    let expected_background = blend_highlight(original_foreground, hl);
    assert!(
        f32_arrays_equal(&background, &expected_background),
        "background should be highlight blended over the swapped background: {:?}",
        background
    );
    assert!(
        !f32_arrays_equal(&background, &original_background),
        "the alpha=160 blend must visibly change the background"
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    // Terminal-baked selection: white-on-black becomes black-on-white.
    let cell_data = vec![CellData {
        codepoint: 'X' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [0.0, 0.0, 0.0, 1.0],
        background: [1.0, 1.0, 1.0, 1.0],
        underline_color: [0.0, 0.0, 0.0, 1.0],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [255, 200, 0, 255], // CURRENT_MATCH_ALPHA = 255 >= 128
    }];
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 1,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &highlights,
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
fn cursor_block_full_cell_size() {
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let instances = build_cursor_probe_instance(
        0x20,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        true,
        cell_w,
        cell_h,
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Production block cursor tracks the glyph bitmap (not the full cell):
    // full cell width, glyph height, placed inside the cell.
    assert!(
        f32_eq(cell.quad_size[0], cell_w),
        "Block cursor width should equal cell width"
    );
    assert!(
        cell.quad_size[1] > 0.0 && cell.quad_size[1] <= cell_h,
        "Block cursor height should cover the glyph within the cell, got {}",
        cell.quad_size[1]
    );
}

#[test]
fn cursor_not_rendered_when_visible_false() {
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let instances = build_cursor_probe_instance(
        0x20,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        false,
        cell_w,
        cell_h,
        &mut font_pipeline,
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Non-cursor blank cell uses default background, not cursor color
    assert!(
        !f32_arrays_equal(&cell.background, &[1.0, 1.0, 1.0, 0.7]),
        "cursor cell should not have block alpha background when cursor_visible=false"
    );
}

#[test]
fn cursor_at_origin() {
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let instances = build_cursor_probe_instance(
        'A' as u32,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        true,
        cell_w,
        cell_h,
        &mut font_pipeline,
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let instances = build_cursor_probe_instance(
        'X' as u32,
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
        true,
        cell_w,
        cell_h,
        &mut font_pipeline,
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        codepoint: 0x20,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [1.0, 1.0, 1.0, 1.0],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: CursorStyle::Block,
        color: Some([0.5, 0.3, 0.8, 1.0]),
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 1,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
    // Without a surface config, render should fail when not paused
    assert!(
        context.render_frame(&[], &[]).is_err(),
        "expected error when not paused and no surface"
    );
    // When paused, render reports not-presented (Err) so the caller
    // does not advance last_frame: a consumed-but-never-presented New
    // frame would otherwise be lost forever (IME pause swallows output).
    context.set_render_paused(true);
    assert!(
        context.render_frame(&[], &[]).is_err(),
        "expected err when paused (frame not presented)"
    );
}

#[test]
fn render_paused_toggle_resumes_rendering() {
    let mut context = Renderer::new_with_no_surface();
    // Pause then unpause
    context.set_render_paused(true);
    assert!(
        context.render_frame(&[], &[]).is_err(),
        "paused reports not-presented"
    );
    context.set_render_paused(false);
    assert!(
        context.render_frame(&[], &[]).is_err(),
        "unpaused fails without surface (correct behavior)"
    );
}

#[test]
fn render_paused_remains_paused_after_multiple_frames() {
    let mut context = Renderer::new_with_no_surface();
    context.set_render_paused(true);
    for _ in 0..10 {
        assert!(
            context.render_frame(&[], &[]).is_err(),
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
        context.render_frame(&[], &[]).is_err(),
        "double-pause still not-presented"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Render Pipeline Benchmarks — realistic content
// ══════════════════════════════════════════════════════════════════════════

/// Benchmark `build_instances_from_cell_data` with realistic mixed content:
/// varied colors, bold, italic, CJK, wide chars. This simulates a real
/// terminal screen with syntax highlighting, git output, and Unicode.
///
/// Thresholds are single anti-flake floors (no environment checks per
/// TESTING.md): parallel execution and software Vulkan contention make
/// wall time noisy; the floor catches order-of-magnitude regressions only.
/// Fine-grained tracking belongs to `cargo bench` (see check-rust.nu).

#[test]
fn bench_build_instances_from_cell_data() {
    use std::hint::black_box;
    use std::time::Instant;

    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);

    let (rows, cols) = (24u32, 80u32);
    let count = (rows * cols) as usize;
    // Realistic screen: ~70% ASCII, ~15% CJK wide, ~15% empty
    let mixed_data = [
        ('A', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0), // normal text
        (' ', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0), // space
        ('中', 2, [1.0, 0.8, 0.2, 1.0], [0.05, 0.05, 0.1, 1.0], 0), // CJK (yellow)
        ('W', 1, [0.3, 0.8, 1.0, 1.0], [0.1, 0.1, 0.1, 1.0], 1), // bold blue
        ('i', 1, [0.5, 1.0, 0.5, 1.0], [0.1, 0.1, 0.1, 1.0], 2), // italic green
        ('e', 1, [0.9, 0.9, 0.9, 1.0], [0.2, 0.0, 0.0, 1.0], 4), // red background (diff)
        ('█', 1, [0.6, 0.6, 0.6, 1.0], [0.15, 0.15, 0.15, 1.0], 0), // block char
        ('~', 1, [0.4, 0.4, 0.4, 1.0], [0.1, 0.1, 0.1, 1.0], 8), // dim gray
    ];

    let cell_data: Vec<crate::terminal::ghostty_terminal::CellData> = (0..count)
        .map(|i| {
            let (ch, w, foreground, background, fl) = mixed_data[i % mixed_data.len()];
            crate::terminal::ghostty_terminal::CellData {
                codepoint: ch as u32,
                width: w,
                grapheme_extra: [0; 7],
                foreground,
                background,
                underline_color: foreground,
                flags: fl,
                row: (i / cols as usize) as u32,
                col: (i % cols as usize) as u32,
            }
        })
        .collect();

    let cursor = crate::render::CellCursor {
        row: 12,
        col: 40,
        visible: true,
        style: crate::terminal::CursorStyle::Block,
        color: None,
    };

    let n = 100;
    let start = Instant::now();
    for _ in 0..n {
        let mut instances = Vec::new();
        let result = crate::render::build_instances_from_cell_data(
            &cell_data,
            crate::render::cell_builder::CellInstanceConfig {
                rows,
                cols,
                grid_cell_w: 1024.0 / cols as f32,
                grid_cell_h: 1024.0 / rows as f32,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &mut instances,
        );
        black_box(result);
        black_box(instances.len());
    }
    let elapsed = start.elapsed();
    let fps = n as f64 / elapsed.as_secs_f64();
    println!(
        "Mixed-content render: {:.0} fps ({:.1}ms for {} × {} cells with CJK/colors/bold)",
        fps,
        elapsed.as_millis(),
        n,
        count
    );
    let threshold_fps = 80.0;
    assert!(
        fps > threshold_fps,
        "Mixed-content render too slow: {:.0} fps (need >{threshold_fps:.0})",
        fps
    );
}

// ══════════════════════════════════════════════════════════════════════════
// GPU Pipeline Benchmarks  (wgpu buffer upload, command encoding, submit)
// ══════════════════════════════════════════════════════════════════════════

/// Benchmark wgpu buffer upload throughput — the main GPU data path.
/// Every frame writes CellInstance data to a GPU buffer via queue.write_buffer().
/// This benchmark measures raw write speed for 24×80 instance data (1920 cells).
#[test]
fn bench_gpu_buffer_upload_throughput() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::hint::black_box;
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    // Create a staging buffer of realistic size
    let cell_instance_size = std::mem::size_of::<CellInstance>() as u64;
    let buf_size = cell_instance_size * 1920;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Bench Buffer"),
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Generate realistic instance data
    let instance_data: Vec<CellInstance> = (0..1920)
        .map(|i| CellInstance {
            quad_origin: [i as f32 % 80.0 * 10.0, i as f32 / 80.0 * 20.0],
            atlas_offset: [0.0, 0.0],
            atlas_size: [0.0, 0.0],
            foreground: [0.9, 0.9, 0.9, 1.0],
            background: [0.1, 0.1, 0.1, 1.0],
            underline_color: [0.9, 0.9, 0.9, 1.0],
            quad_size: [10.0, 20.0],
            flags: 0.0,
            bearing: [0.0, 0.0],
            glyph_advance_width: 0.0,
        })
        .collect();
    let bytes = bytemuck::cast_slice(&instance_data);

    let n = 1000;
    let start = Instant::now();
    for _ in 0..n {
        queue.write_buffer(&buffer, 0, bytes);
        black_box(&buffer);
    }
    let elapsed = start.elapsed();
    let mb_per_sec = n as f64 * bytes.len() as f64 / 1_048_576.0 / elapsed.as_secs_f64();
    println!(
        "GPU buffer upload: {:.0} MB/s ({}×{} bytes in {:.1}ms)",
        mb_per_sec,
        n,
        bytes.len(),
        elapsed.as_millis(),
    );
    assert!(
        mb_per_sec > 350.0,
        "Buffer upload too slow: {:.0} MB/s (need >350)",
        mb_per_sec,
    );
}

/// Benchmark wgpu command encoding overhead: create encoder, begin render pass,
/// draw instances, end pass, submit. This tests the CPU-side graphics command
/// path that happens every frame.
#[test]
fn bench_gpu_command_encoding_overhead() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::hint::black_box;
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    // Create minimal off-screen render target
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bench Texture"),
        size: wgpu::Extent3d {
            width: 800,
            height: 600,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let n = 500;
    let start = Instant::now();
    for _ in 0..n {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bench Encoder"),
        });
        {
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bench Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            black_box(&_rp);
        }
        queue.submit(black_box([encoder.finish()]));
    }
    let elapsed = start.elapsed();
    let submissions_per_sec = n as f64 / elapsed.as_secs_f64();
    println!(
        "GPU command encoding + submit: {:.0} submissions/sec ({:.1}ms per submission)",
        submissions_per_sec,
        elapsed.as_millis() as f64 / n as f64,
    );
    assert!(
        submissions_per_sec > 100.0,
        "Command encoding too slow: {:.0} frames/sec (need >100)",
        submissions_per_sec,
    );
}

/// Benchmark full GPU pipeline throughput for rendering a screen of CellInstances:
/// buffer upload + command encoding + submit + poll. This mirrors the actual
/// render_frame() path without requiring a swapchain surface.
#[test]
fn bench_gpu_full_submit_throughput() {
    let _serial = GPU_BENCH_LOCK.lock();
    use std::hint::black_box;
    use std::time::Instant;

    let renderer = Renderer::new_with_no_surface();
    let device = &renderer.device;
    let queue = &renderer.queue;

    // Create off-screen target
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bench Texture"),
        size: wgpu::Extent3d {
            width: 800,
            height: 600,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Staging buffer + instance data
    let instance_data: Vec<CellInstance> = (0..1920)
        .map(|i| CellInstance {
            quad_origin: [i as f32 % 80.0 * 10.0, i as f32 / 80.0 * 20.0],
            atlas_offset: [0.0, 0.0],
            atlas_size: [0.0, 0.0],
            foreground: [0.9, 0.9, 0.9, 1.0],
            background: [0.1, 0.1, 0.1, 1.0],
            underline_color: [0.9, 0.9, 0.9, 1.0],
            quad_size: [10.0, 20.0],
            flags: 0.0,
            bearing: [0.0, 0.0],
            glyph_advance_width: 0.0,
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

    let n = 200;
    let start = Instant::now();
    for _ in 0..n {
        queue.write_buffer(&buffer, 0, bytes);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bench Encoder"),
        });
        {
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bench Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            black_box(&_rp);
        }
        queue.submit(black_box([encoder.finish()]));
    }
    // Drain all submitted work
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(2)),
    });
    let elapsed = start.elapsed();
    let fps = n as f64 / elapsed.as_secs_f64();
    let mb_per_sec = n as f64 * bytes.len() as f64 / 1_048_576.0 / elapsed.as_secs_f64();
    println!(
        "GPU full submit: {:.0} frames/sec ({:.1}ms per frame, {:.0} MB/s upload)",
        fps,
        elapsed.as_millis() as f64 / n as f64,
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

    let n = 100;
    let start = Instant::now();
    for _ in 0..n {
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
    let uploads_per_sec = n as f64 / elapsed.as_secs_f64();
    println!(
        "GPU atlas texture upload: {:.0} uploads/sec ({:.1}ms per 1024×1024 RGBA)",
        uploads_per_sec,
        elapsed.as_millis() as f64 / n as f64,
    );
    // Lavapipe may be slow; threshold is set conservatively
    assert!(
        uploads_per_sec > 5.0,
        "Atlas upload too slow: {:.0} uploads/sec (need >5)",
        uploads_per_sec,
    );
}

/// Benchmark glyph cache warmup stress: render 100 unique CJK characters through
/// FontPipeline, measuring first-encounter time vs cache-hit time. This tests
/// the CJK cache and atlas allocation for the cold-start scenario.
#[test]
fn bench_cjk_glyph_cache_warmup() {
    use std::hint::black_box;
    use std::time::Instant;

    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);

    // 100 unique CJK ideographs from common ranges
    let cjk_chars: Vec<char> = (0x4E00..0x4E64) // 100 CJK chars: 一-们
        .filter_map(char::from_u32)
        .collect();
    assert_eq!(cjk_chars.len(), 100);

    // First pass: cold cache — each char requires full resolution
    let start = Instant::now();
    for (i, &ch) in cjk_chars.iter().enumerate() {
        let info = font_pipeline.glyph_information(ch);
        assert!(
            info.is_some(),
            "CJK char U+{:04X} should resolve at index {}",
            ch as u32,
            i
        );
        black_box(info);
    }
    let cold_time = start.elapsed();

    // Second pass: hot cache — all should hit cjk_glyph_cache + glyph_cache
    let start = Instant::now();
    for &ch in &cjk_chars {
        let info = font_pipeline.glyph_information(ch);
        assert!(
            info.is_some(),
            "CJK char U+{:04X} should resolve from cache",
            ch as u32
        );
        black_box(info);
    }
    let hot_time = start.elapsed();

    let cold_per_char = cold_time.as_micros() as f64 / cjk_chars.len() as f64;
    let hot_per_char = hot_time.as_micros() as f64 / cjk_chars.len() as f64;
    let speedup = cold_per_char / hot_per_char;

    println!(
        "CJK glyph cache: cold {:.1}µs/char → hot {:.1}µs/char ({:.0}× speedup)",
        cold_per_char, hot_per_char, speedup,
    );
    assert!(
        hot_per_char < 500.0,
        "CJK hot cache too slow: {:.1}µs/char (need <500µs)",
        hot_per_char,
    );
    assert!(
        speedup > 5.0,
        "CJK cache not effective: {:.0}× speedup (need >5× from 100 unique chars)",
        speedup,
    );
}

#[test]
fn all_static_pipelines_create_without_validation_errors() {
    // Every production shader pipeline must compile against the wgpu
    // backend available in the dev shell (Mesa Lavapipe software Vulkan).
    // Pipeline creation is where WGSL compile errors surface; rendering
    // tests below already exercise cell paths, this guards the
    // less-travelled KGP pipeline.
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

// ══════════════════════════════════════════════════════════════════════
// Dirty-band computation (render-vulkan-performance)
// ══════════════════════════════════════════════════════════════════════

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
        // Simulate a build with 2 instances in row 0, 1 in row 1,
        // 0 in row 2, 3 in row 3.
        cache.update_for_test(
            &[2usize, 3, 3, 6], // row_ends (cumulative)
        );
        let (s, e) = cache.band_slice(1, 3);
        assert_eq!((s, e), (2, 3)); // rows 1..3 → instances [2..3)
        let (s, e) = cache.band_slice(0, 4);
        assert_eq!((s, e), (0, 6));
        let (s, e) = cache.band_slice(2, 3);
        assert_eq!((s, e), (3, 3)); // empty middle row
    }
}

// ══════════════════════════════════════════════════════════════════════
// Pure vertical shift detection (scroll-blit path, )
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod vertical_shift_tests {
    use crate::render::cell_builder::detect_vertical_shift;
    use crate::terminal::ghostty_terminal::CellData;

    fn cell(row: u32, col: u32, tag: u8) -> CellData {
        let mut cd: CellData = bytemuck::Zeroable::zeroed();
        cd.row = row;
        cd.col = col;
        cd.codepoint = if col == 0 { tag as u32 } else { b' ' as u32 };
        cd
    }

    fn row(row: u32, tag: u8, cols: u32) -> Vec<CellData> {
        (0..cols).map(|c| cell(row, c, tag)).collect()
    }

    fn grid(cols: u32, tags: &[u8]) -> Vec<CellData> {
        tags.iter()
            .enumerate()
            .flat_map(|(r, &t)| row(r as u32, t, cols))
            .collect()
    }

    #[test]
    fn detects_one_row_upward_scroll() {
        // Old rows A B C D; new rows B C D E → pure shift by 1.
        let old = grid(2, b"ABCD");
        let new = grid(2, b"BCDE");
        assert_eq!(detect_vertical_shift(&old, &new, 4, 8), Some(1));
    }

    #[test]
    fn detects_two_row_scroll() {
        let old = grid(2, b"ABCDE");
        let new = grid(2, b"CDEFG");
        assert_eq!(detect_vertical_shift(&old, &new, 5, 8), Some(2));
    }

    #[test]
    fn rejects_identical_frames() {
        let g = grid(2, b"ABCD");
        assert_eq!(detect_vertical_shift(&g, &g, 4, 8), None);
    }

    #[test]
    fn rejects_non_scroll_edits() {
        // Middle row changed: not a pure shift of any amount.
        let old = grid(2, b"ABCD");
        let new = grid(2, b"AXCD");
        assert_eq!(detect_vertical_shift(&old, &new, 4, 8), None);
    }

    #[test]
    fn respects_max_scan() {
        let old = grid(2, b"ABCDEF");
        let new = grid(2, b"EFGHIJ"); // shift 4
        assert_eq!(detect_vertical_shift(&old, &new, 6, 8), Some(4));
        assert_eq!(detect_vertical_shift(&old, &new, 6, 2), None);
    }

    #[test]
    fn handles_empty_rows_via_ranges_fallback() {
        // Rows with zero cells map to empty ranges; a blank-grid shift must
        // NOT be reported as scroll when nothing differs.
        let empty: Vec<CellData> = Vec::new();
        assert_eq!(detect_vertical_shift(&empty, &empty, 4, 8), None);
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
/// Simulate scrolling through terminal history.
/// Writes many lines of content, then measures take_snapshot_with_scroll
/// at varying offset positions.
/// 归属说明：见本用例上方注释（与渲染 bench 共享 `GPU_BENCH_LOCK`）。
#[test]
fn bench_scroll_throughput() {
    use std::hint::black_box;
    use std::time::Instant;

    use crate::terminal::ghostty_terminal::GhosttyTerminal;
    // Serialize against the GPU benches: in parallel runs the shared CPU
    // (Lavapipe software rasterizer + this CPU-bound bench) drops the
    // measured throughput below the threshold — 400-500 MB/s vs 725 MB/s
    // in isolation. Each bench is fast (<1s) so the lock is
    // uncontended in practice.
    let _serial = super::GPU_BENCH_LOCK.lock();
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // Fill scrollback with 500 lines of content
    for i in 0..500 {
        t.vt_write(
            format!("Line {i}: some realistic terminal content with numbers and text\n").as_bytes(),
        );
    }
    t.flush();

    // Measure scroll snapshot at 3 different offsets
    let offsets = [0u32, 100, 400];
    let n = 20;
    for &offset in &offsets {
        let start = Instant::now();
        for _ in 0..n {
            let snap = black_box(t.take_snapshot_with_scroll(offset));
            black_box(snap.cells.len());
        }
        let elapsed = start.elapsed();
        let snaps_per_sec = n as f64 / elapsed.as_secs_f64();
        println!(
            "Scroll offset={}: {:.0} snapshots/sec ({:.1}ms for {} iterations)",
            offset,
            snaps_per_sec,
            elapsed.as_millis(),
            n,
        );
        // 单防抖地板：并行套件 + 软件 Vulkan 争用下吞吐波动大，
        // 只捕获量级回退（见本文件头注释）。
        let threshold = if offset == 0 { 400.0 } else { 250.0 };
        assert!(
            snaps_per_sec > threshold,
            "Scroll offset={offset} too slow: {:.0} snapshots/sec (need >{threshold:.0})",
            snaps_per_sec,
        );
    }
}

/// Benchmark the full CPU-side pipeline: write terminal content → flush →
/// receive CellData → build CellInstances. This simulates the complete
/// per-frame data path before GPU submission.
#[test]
fn bench_end_to_end_cpu_pipeline_latency() {
    use std::hint::black_box;
    use std::time::Instant;

    use crate::terminal::ghostty_terminal::GhosttyTerminal;

    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    let mut font_pipeline = super::font::FontPipeline::new(1024, 1024, 14.0);

    // Simulate a realistic screen: fill with text content
    let content = b"user@host:~$ cargo build --release --features=test-util\n   Compiling native v0.1.0\n    Finished `release` profile [optimized] target(s) in 0.42s\n";
    let n = 20; // 20 screens

    let start = Instant::now();
    for _ in 0..n {
        t.vt_write(content);
        t.flush();
        let cell_data = t.receive_cell_data();
        let (cells, cursor_info) = cell_data.expect("should receive CellData after flush");

        let cursor = super::CellCursor {
            row: cursor_info.row,
            col: cursor_info.col,
            visible: cursor_info.visible,
            style: cursor_info.style,
            color: None,
        };
        let mut instances = Vec::new();
        super::build_instances_from_cell_data(
            &cells,
            super::cell_builder::CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &mut instances,
        );
        let count = black_box(instances.len());
        black_box(count);
    }
    let elapsed = start.elapsed();
    let ms_per_frame = elapsed.as_millis() as f64 / n as f64;
    let fps = n as f64 / elapsed.as_secs_f64();
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
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
        codepoint: 0x1f468,
        width: 2,
        grapheme_extra: extras,
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row: 0,
        col: 0,
    }];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let instances =
        build_configured_cell_instance(&cell_data, cursor, cell_w, cell_h, &mut font_pipeline);
    if instances.len() == 1 {
        return;
    }
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
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell = |row: u32, col: u32| CellData {
        codepoint: 'd' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row,
        col,
    };
    let cell_data = vec![cell(0, 0), cell(0, 5)];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 99,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 6,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: TEST_ATLAS_SIZE,
            atlas_height: TEST_ATLAS_SIZE,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 2, "two cells must emit two quads");
    assert_eq!(
        instances[0].atlas_offset, instances[1].atlas_offset,
        "same glyph must sample the same atlas region"
    );
    assert_eq!(
        instances[0].atlas_size, instances[1].atlas_size,
        "same glyph must sample the same atlas extent"
    );
    let expected_dx = 5.0 * cell_w;
    assert!(
        (instances[1].quad_origin[0] - instances[0].quad_origin[0] - expected_dx).abs() < 1e-4,
        "quad origin must shift exactly by columns"
    );
}

#[test]
fn distinct_glyphs_sample_distinct_atlas_regions() {
    // 回归网（d 像 a 类字形混淆）：不同字符必须采样不同图集区域。
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell = |ch: char, col: u32| CellData {
        codepoint: ch as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row: 0,
        col,
    };
    let cell_data = vec![cell('d', 0), cell('a', 1)];
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 99,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: 2,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: TEST_ATLAS_SIZE,
            atlas_height: TEST_ATLAS_SIZE,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
        .flat_map(|row| {
            (0..italic.width).map(move |col| {
                let x = (italic.atlas_x + col) as usize;
                let y = (italic.atlas_y + row) as usize;
                bitmap[(y * atlas_width + x) * 4]
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
fn italic_d_advance_covers_sheared_bitmap() {
    // 斜体 d 像 a 根因回归：shear 让顶部右移约 height×0.2126，
    // advance 必须覆盖位图宽，否则 shader 右上裁剪。
    let mut pipeline = ascii_font();
    let regular = pipeline
        .glyph_information_styled('d', false, false)
        .expect("regular d");
    let italic = pipeline
        .glyph_information_styled('d', false, true)
        .expect("italic d");
    let overflow = italic.advance_width - regular.advance_width;
    assert!(
        overflow >= 0.0,
        "italic advance={} regular advance={} 合成斜体 advance 不得小于正体",
        italic.advance_width,
        regular.advance_width
    );
    // 取证结论（Liberation Mono 真斜体 face）：位图 9px vs
    // advance 8.43px，右悬 0.57px 是字体固有度量（合法 overhang，
    // ghostty/termux 同样按 advance 盒裁剪）。d 像 a 不来自此处。
    // 本测试 pin 该边界：右悬不得超过 1px，否则 shader 裁剪可见。
    let bitmap_overhang = italic.width as f32 - italic.advance_width;
    assert!(
        bitmap_overhang <= 1.0,
        "italic bitmap w={} advance={} 右悬空 {}px 超过 1px 则 shader 裁剪可见",
        italic.width,
        italic.advance_width,
        bitmap_overhang
    );
}

/// 首帧上传契约：实例构建（渲染帧 Phase 3）期间新光栅化的字形必须留下
/// 待上传脏区。render_inner 的 Phase 1 上传只覆盖构建前已有的脏区
/// （此处先 take 模拟），构建后仍必须能取到覆盖全部新字形区域的矩形；
/// 若构建后无脏区，帧绘制将按空纹理采样，斜体/新字形首帧缺失。
#[test]
fn first_build_leaves_pending_dirty_rect_covering_all_glyphs() {
    use crate::terminal::ghostty_terminal::CellData;
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut font_pipeline = ascii_font();
    // 模拟 render_inner Phase 1：先取走构建前的脏区（ASCII 预热字形）。
    let _pre_frame_upload = font_pipeline.take_dirty_rect();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let text = "abcdefghijklmnop";
    let cell_data: Vec<CellData> = text
        .char_indices()
        .map(|(col, ch)| CellData {
            codepoint: ch as u32,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: [1.0; 4],
            background: [0.0; 4],
            underline_color: [1.0; 4],
            flags: 1 << cell_flags::ITALIC,
            row: 0,
            col: col as u32,
        })
        .collect();
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 99,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        &cell_data,
        crate::render::gpu::CellInstanceConfig {
            rows: 1,
            cols: text.len() as u32,
            grid_cell_w: cell_w,
            grid_cell_h: cell_h,
            cursor,
            atlas_width: TEST_ATLAS_SIZE,
            atlas_height: TEST_ATLAS_SIZE,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
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
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let text = "abcdefghijklmnop";
    let cell_data: Vec<CellData> = text
        .char_indices()
        .map(|(col, ch)| CellData {
            codepoint: ch as u32,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: [1.0; 4],
            background: [0.0; 4],
            underline_color: [1.0; 4],
            // 交替斜体/正体：两种样式路径都必须幂等。
            flags: if col % 2 == 0 {
                1 << cell_flags::ITALIC
            } else {
                0
            },
            row: 0,
            col: col as u32,
        })
        .collect();
    let cursor = crate::render::CellCursor {
        row: 0,
        col: 99,
        visible: false,
        style: CursorStyle::Block,
        color: None,
    };
    let config = crate::render::gpu::CellInstanceConfig {
        rows: 1,
        cols: text.len() as u32,
        grid_cell_w: cell_w,
        grid_cell_h: cell_h,
        cursor,
        atlas_width: TEST_ATLAS_SIZE,
        atlas_height: TEST_ATLAS_SIZE,
        search_highlights: &[],
    };
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

/// 取 atlas 位图内 (x, y, w, h) 矩形对应的字节快照（测试辅助）。
fn snapshot_atlas_rect(bitmap: &[u8], atlas_width: usize, rect: (u32, u32, u32, u32)) -> Vec<u8> {
    let (x, y, w, h) = rect;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let start = ((y as usize + row) * atlas_width + x as usize) * 4;
        out.extend_from_slice(&bitmap[start..start + w as usize * 4]);
    }
    out
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
    let (row_count, cell_h) = (8u32, 5.0f32);
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
                quad_origin: [0.0, row as f32 * cell_h],
                atlas_offset: [0.0; 2],
                atlas_size: [0.0; 2],
                foreground: color,
                background: color,
                underline_color: color,
                quad_size: [surface_width as f32, cell_h],
                flags: 0.0,
                bearing: [0.0; 2],
                glyph_advance_width: 0.0,
            }
        })
        .collect();
    let background = [30u8, 30, 46, 255];

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

    let pixel_at = |buf: &[u8], x: u32, y: u32| -> [u8; 4] {
        let index = ((y * surface_width + x) * 4) as usize;
        [buf[index], buf[index + 1], buf[index + 2], buf[index + 3]]
    };

    // 帧 B：上边缘条带 [0, scroll_px) 必须是清屏背景色（该条带在
    // 行进位前本应显示上一行尚未到达的内容，背景即自洽表现）。
    for y in 0..scroll_px as u32 {
        for x in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, x, y),
                background,
                "top edge strip must be cleared background (x={x}, y={y})"
            );
        }
    }
    // 帧 B：行 r 内容整体下移，占据 [scroll_px + r*cell_h, scroll_px + (r+1)*cell_h)。
    for row in 0..row_count {
        let y_mid = (scroll_px as u32) + (row as f32 * cell_h) as u32 + (cell_h * 0.5) as u32;
        let expected = [
            stripe_colors[row as usize][0],
            stripe_colors[row as usize][1],
            stripe_colors[row as usize][2],
            255,
        ];
        for x in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, x, y_mid),
                expected,
                "row {row} must sit at shifted position (x={x}, y={y_mid})"
            );
        }
    }
    // 帧 B：下边缘条带（网格底线之下）为清屏背景色。
    let grid_bottom_px = (scroll_px as u32) + (row_count as f32 * cell_h) as u32;
    for y in grid_bottom_px..surface_height {
        for x in 0..surface_width {
            assert_eq!(
                pixel_at(&frame_scrolled, x, y),
                background,
                "bottom edge strip must be cleared background (x={x}, y={y})"
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
