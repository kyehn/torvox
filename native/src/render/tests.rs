//! Render unit tests + GPU benchmarks.
//!
//! # Requirements
//! - FR-050 — surface lifecycle and render pipeline covered by unit tests

use std::collections::HashMap;

use crate::terminal::{CursorStyle, SelectionMode};
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
    assert_eq!(std::mem::size_of::<CellInstance>(), 80);
}

#[test]
fn orthographic_projection_identity() {
    let proj = orthographic_projection(800.0, 600.0);
    assert!((proj[0][0] - 2.0 / 800.0).abs() < f32::EPSILON);
    assert!((proj[1][1] + 2.0 / 600.0).abs() < f32::EPSILON);
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

/// 120fps+ requires decoupling fps from Hz: prefer Immediate (no vsync)
/// when available so the pipeline can sustain 120+ presents per second even
/// on 60Hz panels. Hz and fps are unrelated — tearing is preferred over
/// throttling the terminal. See context.rs select_present_mode.
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

    // Immediate preferred when available for 120fps+ (Hz/fps decoupled).
    let mut caps = base_caps();
    caps.present_modes = vec![
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
        wgpu::PresentMode::Fifo,
    ];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::Immediate,
        "Immediate preferred for 120fps+ (Hz/fps decoupled)"
    );

    // No Immediate -> Mailbox (vsync) as fallback.
    let mut caps = base_caps();
    caps.present_modes = vec![wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::Mailbox
    );

    // No Immediate/Mailbox -> Fifo.
    let mut caps = base_caps();
    caps.present_modes = vec![wgpu::PresentMode::Fifo, wgpu::PresentMode::AutoVsync];
    assert_eq!(
        Renderer::select_present_mode(&caps),
        wgpu::PresentMode::Fifo
    );

    // Only Immediate -> Immediate.
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
        fg_color: [1.0, 1.0, 1.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
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
    assert!(f32_arrays_equal(&c.fg_color, &[0.0, 0.0, 0.0, 0.0]));
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
    assert_eq!(attribs.len(), 9);
    assert_eq!(attribs[0].shader_location, 1);
    assert_eq!(attribs[1].shader_location, 2);
    assert_eq!(attribs[7].shader_location, 8);
    assert_eq!(attribs[8].shader_location, 9);
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
                    let bg = vec4<f32>(40.0/255.0, 42.0/255.0, 54.0/255.0, 1.0);
                    let fg = vec4<f32>(1.0, 1.0, 1.0, 1.0);
                    let alpha = 0.5;
                    output[gid.x] = mix(bg, fg, vec4<f32>(alpha, alpha, alpha, alpha));
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
            fg_color: [1.0, 1.0, 1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 1.0],
            flags: 0,
            row: 0,
            col: 0,
        },
        CellData {
            codepoint: 0,
            width: 1,
            grapheme_extra: [0; 7],
            fg_color: [1.0, 1.0, 1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 1.0],
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
            selection: None,
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
        f32_arrays_equal(&cursor_cell.bg_color, &[1.0, 1.0, 1.0, 0.7]),
        "cursor cell bg should be white with block alpha when cursor_visible=true"
    );
    let non_cursor_cell = &instances[1];
    assert!(
        !f32_arrays_equal(&non_cursor_cell.bg_color, &[1.0, 1.0, 1.0, 1.0]),
        "non-cursor cell bg should NOT be white"
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
        !f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 1.0]),
        "cursor cell should not have white bg when cursor_visible=false"
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
    // Reverse video swaps fg/bg: blank cell bg must become the foreground,
    // fg must become the background.
    assert!(
        f32_arrays_equal(&cell.bg_color, &foreground),
        "reversed blank cell bg must equal foreground"
    );
    assert!(
        f32_arrays_equal(&cell.fg_color, &background),
        "reversed blank cell fg must equal background"
    );
}

#[test]
fn selection_swaps_fg_bg() {
    use super::SelectionRange;
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        codepoint: 'X' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        fg_color: [1.0, 0.0, 0.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
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
    let selection = Some(SelectionRange {
        start_row: 0,
        end_row: 0,
        start_col: 0,
        end_col: 0,
        active: true,
        mode: SelectionMode::Char,
        origin: None,
        is_empty: false,
    });
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
            selection,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.fg_color, &[0.0, 0.0, 0.0, 1.0]),
        "selected cell fg should be original bg (swap)"
    );
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 0.0, 0.0, 1.0]),
        "selected cell bg should be original fg (swap)"
    );
}

const TEST_ATLAS_SIZE: f32 = 1024.0;

/// Build production instances for one configured cell (shared by cursor,
/// reverse-video and selection tests): caller supplies the cell contents,
/// cursor state and optional selection, unit grid otherwise.
fn build_configured_cell_instance(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    cursor: crate::render::CellCursor,
    selection: Option<super::SelectionRange>,
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
            selection,
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
        fg_color: foreground,
        bg_color: background,
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
    build_configured_cell_instance(
        &cell_data,
        cursor,
        None,
        cell_width,
        cell_height,
        font_pipeline,
    )
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
        fg_color: [1.0; 4],
        bg_color: [0.0; 4],
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
            selection: None,
            search_highlights: &[],
        },
        font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    instances
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
            fg_color: [1.0; 4],
            bg_color: [0.0; 4],
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
            selection: None,
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
                fg_color: [1.0; 4],
                bg_color: [0.0; 4],
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
                    selection: None,
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
        "center R should be 30 (Catppuccin Mocha bg)"
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
        fg_color: [1.0, 1.0, 1.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
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
            selection: None,
            search_highlights: &highlights,
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        cell.bg_color[0] > 0.4,
        "highlighted cell bg should have red tint from blending: {:?}",
        cell.bg_color
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
        fg_color: [0.0, 1.0, 0.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
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
            selection: None,
            search_highlights: &highlights,
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.bg_color, &[0.5, 0.5, 1.0, 0.7]),
        "cursor cell bg should be cursor color (with block alpha), not highlight color"
    );
}

#[test]
fn selection_range_line_mode() {
    let sel = SelectionRange {
        start_row: 2,
        start_col: 0,
        end_row: 4,
        end_col: 0,
        active: true,
        mode: SelectionMode::Line,
        origin: None,
        is_empty: false,
    };
    assert!(sel.contains(3, 50, 80));
    assert!(!sel.contains(1, 0, 80));
}

#[test]
fn selection_range_block_mode() {
    let sel = SelectionRange {
        start_row: 1,
        start_col: 5,
        end_row: 3,
        end_col: 10,
        active: true,
        mode: SelectionMode::Block,
        origin: None,
        is_empty: false,
    };
    assert!(sel.contains(2, 7, 80));
    assert!(!sel.contains(2, 3, 80));
    assert!(!sel.contains(4, 7, 80));
}

#[test]
fn selection_range_char_mode() {
    let sel = SelectionRange {
        start_row: 1,
        start_col: 5,
        end_row: 3,
        end_col: 10,
        active: true,
        mode: SelectionMode::Char,
        origin: None,
        is_empty: false,
    };
    assert!(sel.contains(2, 0, 80));
    assert!(!sel.contains(1, 4, 80)); // before start_col on start row
    assert!(!sel.contains(0, 5, 80)); // before first row
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
fn search_highlight_current_match_inverts_fg_bg() {
    // Current match: alpha >= 128 triggers fg/bg swap
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
    // Other match: alpha < 128 should NOT swap fg and bg
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

    let original_fg: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // white text
    let original_bg: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // black background
    let mut fg = original_fg;
    let mut bg = original_bg;

    apply_search_highlight(&mut fg, &mut bg, hl);

    // alpha >= 128 must swap fg/bg (inverse video): fg becomes the
    // ORIGINAL background...
    assert!(
        f32_arrays_equal(&fg, &original_bg),
        "alpha=255 must swap fg/bg; fg should become the original bg: {:?}",
        fg
    );
    // ...and the swapped background is fully covered by the opaque
    // highlight color.
    let expected_bg = blend_highlight(original_bg, hl);
    assert!(
        f32_arrays_equal(&bg, &expected_bg),
        "bg should be highlight blended over the swapped bg: {:?}",
        bg
    );
    assert!(
        f32_arrays_equal(&bg, &[1.0, 200.0 / 255.0, 0.0, 1.0]),
        "opaque highlight must fully replace the background: {:?}",
        bg
    );
}

#[test]
fn search_highlight_other_match_alpha_matches_production() {
    use crate::render::cell_builder::apply_search_highlight;

    // Production anchor: SearchHighlightColors.OTHER_MATCH_ALPHA = 160,
    // at or above the 128 swap threshold (: ALL matches invert).
    let hl: [u8; 4] = [100, 150, 200, 160];

    let original_fg: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let original_bg: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let mut fg = original_fg;
    let mut bg = original_bg;

    apply_search_highlight(&mut fg, &mut bg, hl);

    // alpha >= 128 must swap (inverse video): fg becomes the original bg.
    assert!(
        f32_arrays_equal(&fg, &original_bg),
        "alpha=160 must swap fg/bg like every match: {:?}",
        fg
    );
    // Background gets the highlight blended over the swapped bg — which now
    // holds the ORIGINAL foreground — visibly different from BOTH the
    // untouched background and the fully opaque current-match treatment.
    let expected_bg = blend_highlight(original_fg, hl);
    assert!(
        f32_arrays_equal(&bg, &expected_bg),
        "bg should be highlight blended over the swapped bg: {:?}",
        bg
    );
    assert!(
        !f32_arrays_equal(&bg, &original_bg),
        "the alpha=160 blend must visibly change the background"
    );
}

#[test]
fn selection_intersect_current_match_double_swap() {
    // Covers the build path in cell_builder.rs where selection swaps fg/bg
    // first and apply_search_highlight runs on top: a current-match
    // highlight (production alpha 255 >= 128 from
    // SearchHighlightColors.CURRENT_MATCH_ALPHA) swaps AGAIN, so the two
    // swaps cancel — the cell keeps its original foreground while the
    // background becomes the fully-opaque highlight color.
    use crate::terminal::ghostty_terminal::CellData;
    let mut font_pipeline = ascii_font();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        codepoint: 'X' as u32,
        width: 1,
        grapheme_extra: [0; 7],
        fg_color: [1.0, 1.0, 1.0, 1.0], // white text
        bg_color: [0.0, 0.0, 0.0, 1.0], // black background
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
    let selection = SelectionRange {
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 0,
        active: true,
        mode: SelectionMode::Char,
        origin: None,
        is_empty: false,
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
            selection: Some(selection),
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
        f32_arrays_equal(&cell.fg_color, &[1.0, 1.0, 1.0, 1.0]),
        "selection swap + highlight swap must cancel; fg keeps original: {:?}",
        cell.fg_color
    );
    // ...and the background is fully covered by the opaque highlight.
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 200.0 / 255.0, 0.0, 1.0]),
        "bg should be the fully-opaque highlight color: {:?}",
        cell.bg_color
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
        !f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.7]),
        "cursor cell should not have block alpha bg when cursor_visible=false"
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
        f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.7]),
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
    // Block cursor keeps the original fg readable, bg becomes cursor color×alpha.
    assert!(
        f32_arrays_equal(&cell.fg_color, &[0.0, 1.0, 0.0, 1.0]),
        "block cursor on text: fg should stay the original foreground"
    );
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.7]),
        "block cursor on text: bg should be cursor color with block alpha"
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
        fg_color: [1.0, 1.0, 1.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
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
            selection: None,
            search_highlights: &[],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.bg_color, &[0.5, 0.3, 0.8, 0.7]),
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
    // When paused, render should succeed immediately (skips surface check)
    context.set_render_paused(true);
    assert!(
        context.render_frame(&[], &[]).is_ok(),
        "expected ok when paused regardless of surface"
    );
}

#[test]
fn render_paused_toggle_resumes_rendering() {
    let mut context = Renderer::new_with_no_surface();
    // Pause then unpause
    context.set_render_paused(true);
    assert!(
        context.render_frame(&[], &[]).is_ok(),
        "paused skips surface check"
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
            context.render_frame(&[], &[]).is_ok(),
            "paused render must stay ok across multiple frames"
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
        context.render_frame(&[], &[]).is_ok(),
        "double-pause still ok"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Render Pipeline Benchmarks — realistic content
// ══════════════════════════════════════════════════════════════════════════

/// Benchmark `build_instances_from_cell_data` with realistic mixed content:
/// varied colors, bold, italic, CJK, wide chars. This simulates a real
/// terminal screen with syntax highlighting, git output, and Unicode.
///
/// Thresholds are two-tiered (see docs/standards/TESTING.md,
/// "Benchmarks & Performance Thresholds"): local runs assert the
/// strict 200 fps floor; CI runs (software Vulkan/llvmpipe + parallel test
/// contention) keep a ~2.5x anti-flake floor that still catches
/// order-of-magnitude regressions.
fn render_benchmarks_strict() -> bool {
    std::env::var("CI").is_err() && std::env::var("GITHUB_ACTIONS").is_err()
}

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
        ('e', 1, [0.9, 0.9, 0.9, 1.0], [0.2, 0.0, 0.0, 1.0], 4), // red bg (diff)
        ('█', 1, [0.6, 0.6, 0.6, 1.0], [0.15, 0.15, 0.15, 1.0], 0), // block char
        ('~', 1, [0.4, 0.4, 0.4, 1.0], [0.1, 0.1, 0.1, 1.0], 8), // dim gray
    ];

    let cell_data: Vec<crate::terminal::ghostty_terminal::CellData> = (0..count)
        .map(|i| {
            let (ch, w, fg, bg, fl) = mixed_data[i % mixed_data.len()];
            crate::terminal::ghostty_terminal::CellData {
                codepoint: ch as u32,
                width: w,
                grapheme_extra: [0; 7],
                fg_color: fg,
                bg_color: bg,
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
                selection: None,
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
    let threshold_fps = if render_benchmarks_strict() {
        200.0
    } else {
        80.0
    };
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
            fg_color: [0.9, 0.9, 0.9, 1.0],
            bg_color: [0.1, 0.1, 0.1, 1.0],
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
            fg_color: [0.9, 0.9, 0.9, 1.0],
            bg_color: [0.1, 0.1, 0.1, 1.0],
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
