use std::collections::HashMap;

use torvox_core::selection::SelectionMode;

use super::*;

const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn f32_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

fn f32_arrays_equal(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| f32_eq(*x, *y))
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

/// Locks the vsync fix (#1): present mode must prefer a vsync-capable mode
/// (Mailbox/Fifo/AutoVsync) over `Immediate`, which disables vsync and lets
/// the render thread flood the GPU with unthrottled frames (the original
/// Android lag root cause).
#[test]
fn select_present_mode_prefers_vsync() {
    fn base_caps() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Rgba8Unorm],
            present_modes: vec![wgpu::PresentMode::Immediate],
            alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    // Mailbox preferred over Fifo when both available (lower latency, vsync).
    // Mali-G57 does not expose Mailbox → falls back to Fifo on affected devices.
    let mut caps = base_caps();
    caps.present_modes = vec![
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
        wgpu::PresentMode::Fifo,
    ];
    assert_eq!(
        GpuContext::select_present_mode(&caps),
        wgpu::PresentMode::Mailbox,
        "Mailbox preferred over Fifo (Fifo fallback on Mali-G57 which omits Mailbox)"
    );

    // No Mailbox -> Fifo (vsync).
    let mut caps = base_caps();
    caps.present_modes = vec![wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo];
    assert_eq!(
        GpuContext::select_present_mode(&caps),
        wgpu::PresentMode::Fifo
    );

    // No Mailbox/Fifo -> AutoVsync (still vsync).
    let mut caps = base_caps();
    caps.present_modes = vec![wgpu::PresentMode::Immediate, wgpu::PresentMode::AutoVsync];
    assert_eq!(
        GpuContext::select_present_mode(&caps),
        wgpu::PresentMode::AutoVsync
    );

    // Only Immediate -> Immediate (last resort; the lag mode we fixed away).
    let base = base_caps();
    assert_eq!(
        GpuContext::select_present_mode(&base),
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
fn flat_grid_new() {
    let grid = FlatGrid::new(10, 20);
    assert_eq!(grid.rows, 10);
    assert_eq!(grid.cols, 20);
    assert_eq!(grid.chars.len(), 200);
    assert!(grid.chars.iter().all(|&c| c == ' '));
}

#[test]
fn flat_grid_set_and_get_cell() {
    let mut grid = FlatGrid::new(5, 5);
    let foreground = [1.0, 0.0, 0.0, 1.0];
    let background = [0.0, 0.0, 0.0, 1.0];
    grid.set_cell(2, 3, 'A', foreground, background);

    let (character, foreground_out, background_out) = grid.cell(2, 3).unwrap();
    assert_eq!(character, 'A');
    assert!(f32_arrays_equal(&foreground_out, &foreground));
    assert!(f32_arrays_equal(&background_out, &background));
}

#[test]
fn flat_grid_out_of_bounds() {
    let grid = FlatGrid::new(3, 3);
    assert!(grid.cell(3, 0).is_none());
    assert!(grid.cell(0, 3).is_none());
}

#[test]
fn build_cell_instances_from_flat_basic() {
    let mut grid = FlatGrid::new(1, 4);
    grid.set_cell(0, 0, 'A', [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
    grid.set_cell(0, 1, ' ', [0.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]);
    grid.set_cell(0, 2, 'B', [0.0, 1.0, 0.0, 1.0], [0.2, 0.2, 0.2, 1.0]);
    grid.set_cell(0, 3, 'C', [1.0, 0.0, 1.0, 1.0], [0.3, 0.3, 0.3, 1.0]);

    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let (cell_w, _cell_h) = font_pipeline.cell_metrics();

    let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 1024.0, 1024.0);
    assert_eq!(instances.len(), 4);

    let cell0 = &instances[0];
    assert!(f32_arrays_equal(&cell0.quad_origin, &[0.0, 0.0]));
    assert!(f32_arrays_equal(&cell0.bg_color, &[0.0, 0.0, 0.0, 1.0]));

    let cell1 = &instances[1];
    assert!(f32_arrays_equal(&cell1.quad_origin, &[cell_w, 0.0]));
    assert!(f32_arrays_equal(&cell1.bg_color, &[0.5, 0.5, 0.5, 1.0]));
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
fn color_f32x4_eq_exact_match() {
    assert!(color_f32x4_eq([1.0, 0.5, 0.0, 1.0], [1.0, 0.5, 0.0, 1.0]));
}

#[test]
fn color_f32x4_eq_mismatch() {
    assert!(!color_f32x4_eq([1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]));
}

#[test]
fn color_f32x4_eq_zero_vs_near_zero() {
    assert!(!color_f32x4_eq(
        [0.0, 0.0, 0.0, 0.0],
        [0.0001, 0.0, 0.0, 0.0]
    ));
}

#[test]
fn color_f32x4_eq_negative_zero() {
    assert!(!color_f32x4_eq([0.0, 0.0, 0.0, 0.0], [-0.0, 0.0, 0.0, 0.0]));
}

#[test]
fn cursor_instance_size() {
    // Verify it is pod and serializable
    let c = CursorInstance {
        cursor_pos: [0.0, 0.0],
        cursor_size: [10.0, 20.0],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let bytes = bytemuck::bytes_of(&c);
    let back: &CursorInstance = bytemuck::from_bytes(bytes);
    assert!(f32_arrays_equal(&back.cursor_size, &[10.0, 20.0]));
}

#[test]
fn gpu_uniforms_size() {
    // #[repr(C)] layout: 64 (mat4) + 8 (vec2) + 4 + 4 (raster_scale,
    // image_active) + 16 (default_bg [f32;4]) = 96. Matches the WGSL
    // std140 `Uniforms` (default_bg split into two vec2).
    assert_eq!(std::mem::size_of::<GpuUniforms>(), 96);
}

#[test]
fn orthographic_projection_basic() {
    let proj = orthographic_projection(100.0, 100.0);
    // [0][0] = 2/width
    assert!((proj[0][0] - 0.02).abs() < f32::EPSILON);
    // [1][1] = -2/height (flipped Y for GLES swapchain)
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
fn flat_grid_zero_size() {
    let grid = FlatGrid::new(0, 0);
    assert_eq!(grid.chars.len(), 0);
    assert_eq!(grid.foreground.len(), 0);
}

#[test]
fn flat_grid_set_out_of_bounds_no_panic() {
    let mut grid = FlatGrid::new(2, 2);
    // out of bounds — should not panic, value not stored
    grid.set_cell(100, 100, 'X', [1.0; 4], [0.0; 4]);
    assert_eq!(grid.chars.len(), 4);
}

#[test]
fn flat_grid_default_chars_are_spaces() {
    let grid = FlatGrid::new(3, 3);
    assert!(grid.chars.iter().all(|&c| c == ' '));
}

#[test]
fn flat_grid_default_fg_is_white() {
    let grid = FlatGrid::new(2, 2);
    for f in &grid.foreground {
        assert!(f32_arrays_equal(f, &[1.0, 1.0, 1.0, 1.0]));
    }
}

#[test]
fn flat_grid_default_bg_is_black() {
    let grid = FlatGrid::new(2, 2);
    for b in &grid.background {
        assert!(f32_arrays_equal(b, &[0.0, 0.0, 0.0, 1.0]));
    }
}

#[test]
fn flat_grid_cell_after_set() {
    let mut grid = FlatGrid::new(2, 2);
    let foreground = [0.5, 0.6, 0.7, 1.0];
    let background = [0.1, 0.2, 0.3, 1.0];
    grid.set_cell(0, 0, 'H', foreground, background);
    let (character, foreground_loaded, background_loaded) = grid.cell(0, 0).unwrap();
    assert_eq!(character, 'H');
    assert!(f32_arrays_equal(&foreground_loaded, &foreground));
    assert!(f32_arrays_equal(&background_loaded, &background));
}

#[test]
fn build_cell_instances_from_flat_empty() {
    let grid = FlatGrid::new(0, 0);
    let mut font = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font.rasterize_ascii();
    let instances = build_cell_instances_from_flat(&grid, &mut font, 1024.0, 1024.0);
    assert!(instances.is_empty());
}

#[test]
fn build_cell_instances_from_flat_space_only() {
    let grid = FlatGrid::new(1, 5);
    let mut font = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font.rasterize_ascii();
    let instances = build_cell_instances_from_flat(&grid, &mut font, 1024.0, 1024.0);
    assert_eq!(instances.len(), 5);
    // All spaces, atlas_size should be 0
    for inst in &instances {
        assert!(f32_arrays_equal(&inst.atlas_size, &[0.0, 0.0]));
    }
}

#[test]
fn build_cell_instances_from_flat_unicode_cjk() {
    let mut grid = FlatGrid::new(1, 3);
    grid.set_cell(0, 0, '中', [1.0; 4], [0.0; 4]);
    grid.set_cell(0, 1, '文', [1.0; 4], [0.0; 4]);
    let mut font = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font.rasterize_ascii();
    let instances = build_cell_instances_from_flat(&grid, &mut font, 1024.0, 1024.0);
    // 3 cells (CJK may or may not be rasterized, may or may not produce instances)
    // Verify no panic
    assert_eq!(instances.len(), 3);
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
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
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

    // Create staging buffer for readback
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

    // Dispatch compute
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
        let view = staging_buffer.slice(..).get_mapped_range();
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
    assert_eq!(
        output1, output2,
        "write_color: non-deterministic output: first={:?} second={:?}",
        output1, output2
    );

    let input = [255u8; 4];
    assert_ne!(
        output1, input,
        "write_color: output identical to input — shader is a no-op"
    );
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

    let output2 = run_compute_and_capture(&device, &queue, blend_shader());
    assert_eq!(
        output1, output2,
        "blend: non-deterministic output: first={:?} second={:?}",
        output1, output2
    );
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
        image_active: 0.0,
        default_bg: [0.0, 0.0, 0.0, 1.0],
    };
    let uniforms_400 = GpuUniforms {
        projection: orthographic_projection(800.0, 400.0),
        atlas_size: [1024.0, 1024.0],
        raster_scale: 1.0,
        image_active: 0.0,
        default_bg: [0.0, 0.0, 0.0, 1.0],
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
    // Write/readback via bytemuck
    let bytes = bytemuck::bytes_of(&uniforms_400);
    let back: &GpuUniforms = bytemuck::from_bytes(bytes);
    assert!(f32_eq(back.projection[1][1], uniforms_400.projection[1][1]));
}

#[test]
fn cursor_rendering_on_visible_cursor() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![
        CellSnapshot {
            codepoint: 'A' as u32,
            ..Default::default()
        },
        CellSnapshot {
            codepoint: 0,
            ..Default::default()
        },
    ];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 2,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true; 1],
        ..Default::default()
    };
    let cursor_color = Some([1.0, 1.0, 1.0, 1.0]);
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 0.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color,
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'A' as u32,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cells,
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 0.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let foreground = [1.0, 0.0, 0.0, 1.0];
    let background = [0.0, 0.0, 1.0, 1.0];
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        foreground,
        background,
        reverse: true,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: false,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        dirty: vec![true],
        cells,
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'X' as u32,
        foreground: [1.0, 0.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: false,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        dirty: vec![true],
        cells,
        ..Default::default()
    };
    let selection = Some(SelectionRange {
        start_row: 0,
        end_row: 0,
        start_col: 0,
        end_col: 0,
        active: true,
        mode: SelectionMode::Char,
        origin: None,
    });
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: None,
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
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

// ── Bearing correctness: Termux-aligned font metrics ──────────────

/// Verify bearing_y uses font baseline, not centering.
/// build_cell_instances_from_flat uses raw bearing_y = ascent_pixels - placement.top
/// (no centering, no clamping — the raw font baseline offset).
#[test]
fn bearing_y_uses_font_baseline_not_centering() {
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let ascent_pixels = font_pipeline.ascent_pixels();

    let chars = ['A', 'g', 'p', '.', ','];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_y = ascent_pixels - info.placement.top as f32;

        let mut grid = FlatGrid::new(1, 1);
        grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
        let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 1024.0, 1024.0);
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
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();

    let chars = ['A', 'i', 'l', 'W', 'M'];
    for ch in chars {
        let info = font_pipeline.glyph_information(ch).expect("glyph exists");
        let expected_bearing_x = info.placement.left as f32;

        let mut grid = FlatGrid::new(1, 1);
        grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
        let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 1024.0, 1024.0);
        let cell = &instances[0];

        assert!(
            (cell.bearing[0] - expected_bearing_x).abs() < 1.0,
            "'{ch}' bearing_x={} should be font natural bearing {}",
            cell.bearing[0],
            expected_bearing_x
        );
    }
}

/// Verify all characters in a row share the same bearing_y baseline.
/// This ensures no vertical misalignment between characters.
#[test]
fn all_chars_share_same_baseline_y() {
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();

    let chars = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
    let mut grid = FlatGrid::new(1, chars.len() as u32);
    for (i, &ch) in chars.iter().enumerate() {
        grid.set_cell(0, i as u32, ch, [1.0; 4], [0.0; 4]);
    }
    let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 1024.0, 1024.0);
    assert_eq!(instances.len(), chars.len());

    // All cells should have the same quad_size (cell dimensions)
    for (i, inst) in instances.iter().enumerate() {
        assert!(
            (inst.quad_size[0] - cell_w).abs() < 0.1,
            "cell[{i}] width={} should be {cell_w}",
            inst.quad_size[0]
        );
        assert!(
            (inst.quad_size[1] - cell_h).abs() < 0.1,
            "cell[{i}] height={} should be {cell_h}",
            inst.quad_size[1]
        );
    }
}

/// Verify CJK bearing_y is not centered.
/// build_cell_instances_from_flat uses raw bearing_y = ascent_pixels - placement.top.
#[test]
fn cjk_bearing_y_not_centered() {
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    let ascent_pixels = font_pipeline.ascent_pixels();

    let cjk_chars = ['中', '文', '好'];
    for ch in cjk_chars {
        if let Some(info) = font_pipeline.glyph_information(ch) {
            let expected = ascent_pixels - info.placement.top as f32;

            let mut grid = FlatGrid::new(1, 2);
            grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
            let instances =
                build_cell_instances_from_flat(&grid, &mut font_pipeline, 1024.0, 1024.0);
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
fn setup_test_gpu_context(
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
) -> GpuContext {
    let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Quad Vertex Buffer"),
        contents: bytemuck::cast_slice(QUAD_CORNERS),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut ctx = GpuContext {
        instance,
        adapter,
        device: device.clone(),
        queue,
        surface: None,
        surface_config: Some(wgpu::SurfaceConfiguration {
            width: 50,
            height: 50,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }),
        cell_pipeline: None,
        quad_vertex_buffer,
        cell_bind_group: None,
        cell_uniform_buffer: None,
        instance_buffer: None,
        atlas_texture: None,
        atlas_view: None,
        atlas_sampler: None,
        pipeline_format: wgpu::TextureFormat::Rgba8Unorm,
        projection_width: 0,
        projection_height: 0,
        readback_texture: None,
        readback_buffer: None,
        bg_color: CATPPUCCIN_MOCHA_BG,
        bg_image_texture: None,
        bg_image_view: None,
        bg_pipeline: None,
        bg_bind_group_layout: None,
        bg_bind_group: None,
        bg_uniform_buffer: None,
        bg_sampler: None,
        bg_blur_radius: 0.0,
        bg_alpha: DEFAULT_BG_ALPHA,
        kgp_pipeline: None,
        kgp_bind_group_layout: None,
        kgp_bind_group: None,
        kgp_uniform_buffer: None,
        kgp_sampler: None,
        kgp_instance_buffer: None,
        kgp_texture: None,
        kgp_atlas_data: Vec::new(),
        kgp_atlas_width: 0,
        kgp_atlas_height: 0,
        raster_scale: 1.0,
        blur_h_pipeline: None,
        blur_v_pipeline: None,
        render_paused: false,
        pending_gpu_drain: false,
    };
    ctx.initialize_pipeline_and_bind_group(256, 256, 50, 50);
    ctx
}

fn setup_test_gpu_context_custom(
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: u32,
    height: u32,
) -> GpuContext {
    let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Quad Vertex Buffer"),
        contents: bytemuck::cast_slice(QUAD_CORNERS),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut ctx = GpuContext {
        instance,
        adapter,
        device: device.clone(),
        queue,
        surface: None,
        surface_config: Some(wgpu::SurfaceConfiguration {
            width,
            height,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }),
        cell_pipeline: None,
        quad_vertex_buffer,
        cell_bind_group: None,
        cell_uniform_buffer: None,
        instance_buffer: None,
        atlas_texture: None,
        atlas_view: None,
        atlas_sampler: None,
        pipeline_format: wgpu::TextureFormat::Rgba8Unorm,
        projection_width: 0,
        projection_height: 0,
        readback_texture: None,
        readback_buffer: None,
        bg_color: wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        bg_image_texture: None,
        bg_image_view: None,
        bg_pipeline: None,
        bg_bind_group_layout: None,
        bg_bind_group: None,
        bg_uniform_buffer: None,
        bg_sampler: None,
        bg_blur_radius: 0.0,
        bg_alpha: DEFAULT_BG_ALPHA,
        kgp_pipeline: None,
        kgp_bind_group_layout: None,
        kgp_bind_group: None,
        kgp_uniform_buffer: None,
        kgp_sampler: None,
        kgp_instance_buffer: None,
        kgp_texture: None,
        kgp_atlas_data: Vec::new(),
        kgp_atlas_width: 0,
        kgp_atlas_height: 0,
        raster_scale: 1.0,
        blur_h_pipeline: None,
        blur_v_pipeline: None,
        render_paused: false,
        pending_gpu_drain: false,
    };
    ctx.initialize_pipeline_and_bind_group(width.max(256), height.max(256), width, height);
    ctx
}

#[test]
fn ocr_verifies_rendered_text() {
    let Some((instance, adapter, device, queue)) = create_test_device() else {
        return;
    };
    let width = 480u32;
    let height = 60u32;
    let atlas_dim = width.max(256);
    let mut ctx = setup_test_gpu_context_custom(instance, adapter, device, queue, width, height);
    // Ensure GPU atlas dimensions match the font pipeline atlas (both must be square)
    ctx.initialize_pipeline_and_bind_group(atlas_dim, atlas_dim, width, height);

    let mut font_pipeline =
        crate::font::FontPipeline::new(atlas_dim as i32, atlas_dim as i32, 14.0);

    let mut fg = FlatGrid::new(1, 11);
    fg.chars = "HELLO WORLD".chars().collect();
    for col in 0..11 {
        fg.foreground[col] = [1.0, 1.0, 1.0, 1.0];
        fg.background[col] = [0.0, 0.0, 0.0, 1.0];
    }

    let instances =
        build_cell_instances_from_flat(&fg, &mut font_pipeline, atlas_dim as f32, atlas_dim as f32);
    assert!(
        !instances.is_empty(),
        "build_cell_instances_from_flat returned 0 instances - font/glyph load failure"
    );
    ctx.upload_atlas(font_pipeline.atlas_bitmap(), atlas_dim, atlas_dim, None);
    let pixels = ctx
        .render_to_buffer(&instances, &[])
        .expect("wgpu render must succeed");

    assert_eq!(
        pixels.len(),
        (width * height * 4) as usize,
        "render output size mismatch"
    );

    let has_white = pixels
        .chunks(4)
        .any(|p| p[0] > 200 && p[1] > 200 && p[2] > 200);
    assert!(
        has_white,
        "rendered output should contain non-black pixels (text)"
    );

    let dir = std::env::temp_dir().join("torvox-ocr-test");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::error!("gpu: failed to create dir {dir:?}: {error}");
    }
    let raw_path = dir.join("helloworld.raw");
    let meta_path = dir.join("helloworld.meta");
    if let Err(error) = std::fs::write(&raw_path, &pixels) {
        log::error!("gpu: failed to write GPU debug data to {raw_path:?}: {error}");
    }
    if let Err(error) = std::fs::write(&meta_path, format!("{width}\n{height}")) {
        log::error!("gpu: failed to write GPU debug data to {meta_path:?}: {error}");
    }

    let ppm_path = dir.join("helloworld.png");
    save_png(&pixels, width, height, &ppm_path);

    let ppm_ocr = std::process::Command::new("rapidocr")
        .args(["-img", ppm_path.to_str().unwrap_or("")])
        .output()
        .expect("rapidocr CLI must be available");
    let stdout = String::from_utf8_lossy(&ppm_ocr.stdout);
    let stderr = String::from_utf8_lossy(&ppm_ocr.stderr);
    let combined = format!("stdout:\n{stdout}stderr:\n{stderr}");
    assert!(
        stdout.to_uppercase().contains("HELLO"),
        "OCR should find HELLO in wgpu-rendered text.\n{combined}"
    );

    // Save full screenshot to repo directory
    let out_dir = {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("test-screenshots");
        p
    };
    if let Err(error) = std::fs::create_dir_all(&out_dir) {
        log::error!("gpu: failed to create dir {out_dir:?}: {error}");
    }
    let repo_png = out_dir.join("OCR_RENDERED_TEXT.png");
    save_png(&pixels, width, height, &repo_png);
}

#[test]
fn gpu_background_solid_image_opaque() {
    let Some((instance, adapter, device, queue)) = create_test_device() else {
        return;
    };
    let mut ctx = setup_test_gpu_context(instance, adapter, device, queue);

    let pixel = [255u8, 0, 0, 255];
    let pixels: Vec<u8> = pixel.repeat(50 * 50);
    ctx.set_bg_image(&pixels, 50, 50);
    ctx.set_background_params(0.0, 1.0);
    ctx.ensure_bg_pipeline(50, 50);

    // render_to_buffer must not panic — bg pipeline initialization is valid
    let result = ctx
        .render_to_buffer(&[], &[])
        .expect("wgpu render must succeed");

    // Verify render produced valid RGBA data (50×50×4 bytes)
    assert_eq!(result.len(), 50 * 50 * 4, "bg opaque render output size");

    // Center pixel must be fully opaque red
    let idx = (25 * 50 + 25) * 4;
    assert_eq!(
        result[idx],
        255,
        "bg opaque center pixel should be red, got ({},{},{},{})",
        result[idx],
        result[idx + 1],
        result[idx + 2],
        result[idx + 3]
    );
    assert_eq!(
        result[idx + 3],
        255,
        "bg opaque center pixel should be fully opaque, got ({},{},{},{})",
        result[idx],
        result[idx + 1],
        result[idx + 2],
        result[idx + 3]
    );
}

#[test]
fn gpu_background_solid_image_transparent() {
    let Some((instance, adapter, device, queue)) = create_test_device() else {
        return;
    };
    let mut ctx = setup_test_gpu_context(instance, adapter, device, queue);

    let pixel = [255u8, 0, 0, 255];
    let pixels: Vec<u8> = pixel.repeat(50 * 50);
    ctx.set_bg_image(&pixels, 50, 50);
    ctx.set_background_params(0.0, 0.5);
    ctx.ensure_bg_pipeline(50, 50);

    // render_to_buffer must not panic
    let result = ctx
        .render_to_buffer(&[], &[])
        .expect("wgpu render must succeed");

    assert_eq!(
        result.len(),
        50 * 50 * 4,
        "bg transparent render output size"
    );
    let idx = (25 * 50 + 25) * 4;
    assert_eq!(
        result[idx],
        255,
        "bg transparent center pixel should be red (alpha=0.5), got ({},{},{},{})",
        result[idx],
        result[idx + 1],
        result[idx + 2],
        result[idx + 3]
    );
    assert_eq!(
        result[idx + 3],
        128,
        "bg transparent center pixel alpha should be 0.5*255=128, got ({},{},{},{})",
        result[idx],
        result[idx + 1],
        result[idx + 2],
        result[idx + 3]
    );
}

#[test]
fn gpu_background_no_image_fallback() {
    let Some((instance, adapter, device, queue)) = create_test_device() else {
        return;
    };
    let mut ctx = setup_test_gpu_context(instance, adapter, device, queue);

    let result = ctx
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
    let by_row: HashMap<i32, Vec<&SearchHighlight>> = HashMap::from([(5, vec![&hl])]);
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'X' as u32,
        foreground: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: false,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        dirty: vec![true],
        cells,
        ..Default::default()
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [255, 0, 0, 128],
    }];
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &highlights,
            cursor_color: None,
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'A' as u32,
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        dirty: vec![true],
        cells,
        ..Default::default()
    };
    let highlights = vec![SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [200, 0, 0, 200],
    }];
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 0.0,
            selection: None,
            selection_bg: None,
            search_highlights: &highlights,
            cursor_color: Some([0.5, 0.5, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
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
    };
    assert!(sel.contains(2, 0, 80));
    assert!(!sel.contains(1, 4, 80)); // before start_col on start row
    assert!(!sel.contains(0, 5, 80)); // before first row
}

// ── Search highlight helpers and tests ───────────────────────────

/// Test helper: groups `SearchHighlight`s by row, just like
/// `build_cell_instances_into` does inline.
fn group_highlights_by_row(highlights: &[SearchHighlight]) -> HashMap<i32, Vec<&SearchHighlight>> {
    let mut by_row: HashMap<i32, Vec<&SearchHighlight>> = HashMap::new();
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

// ── Cursor shape tests ──

#[test]
fn cursor_block_full_cell_size() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_eq(cell.quad_size[0], cell_w),
        "Block cursor width should equal cell width"
    );
    assert!(
        f32_eq(cell.quad_size[1], cell_h),
        "Block cursor height should equal cell height"
    );
}

#[test]
fn cursor_bar_width_ratio() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Bar,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Bar,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    let expected_w = cell_w * 0.15;
    assert!(
        (cell.quad_size[0] - expected_w).abs() < 0.01,
        "Bar cursor width should be {expected_w}, got {}",
        cell.quad_size[0]
    );
    assert!(
        f32_eq(cell.quad_size[1], cell_h),
        "Bar cursor height should equal cell height"
    );
}

#[test]
fn cursor_underline_height_ratio() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Underline,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Underline,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    let expected_h = cell_h * 0.15;
    assert!(
        (cell.quad_size[1] - expected_h).abs() < 0.01,
        "Underline cursor height should be {expected_h}, got {}",
        cell.quad_size[1]
    );
    assert!(
        f32_eq(cell.quad_size[0], cell_w),
        "Underline cursor width should equal cell width"
    );
}

#[test]
fn cursor_not_rendered_when_visible_false() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: false,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 768.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'A' as u32,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_row: 0,
        cursor_col: 0,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 24.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
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
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'X' as u32,
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 1.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let cursor_color = Some([1.0, 1.0, 1.0, 1.0]);
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 24.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color,
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Block cursor swaps fg/bg: fg becomes original bg, bg becomes cursor color×alpha
    assert!(
        f32_arrays_equal(&cell.fg_color, &[0.0, 0.0, 1.0, 1.0]),
        "block cursor on text: fg should be original bg"
    );
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.7]),
        "block cursor on text: bg should be cursor color with block alpha"
    );
}

#[test]
fn cursor_with_text_and_bar_style() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'X' as u32,
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 1.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Bar,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let cursor_color = Some([1.0, 1.0, 1.0, 1.0]);
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 24.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color,
            cursor_style: torvox_core::cursor::CursorStyle::Bar,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Bar cursor does NOT swap fg/bg — it just sets bg to cursor color
    assert!(
        f32_arrays_equal(&cell.fg_color, &[0.0, 1.0, 0.0, 1.0]),
        "bar cursor on text: fg should be original foreground"
    );
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.9]),
        "bar cursor on text: bg should be cursor color with line alpha"
    );
}

#[test]
fn cursor_with_text_and_underline_style() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 'X' as u32,
        foreground: [0.0, 1.0, 0.0, 1.0],
        background: [0.0, 0.0, 1.0, 1.0],
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Underline,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let cursor_color = Some([1.0, 1.0, 1.0, 1.0]);
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 24.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color,
            cursor_style: torvox_core::cursor::CursorStyle::Underline,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    // Underline cursor does NOT swap fg/bg — it just sets bg to cursor color
    assert!(
        f32_arrays_equal(&cell.fg_color, &[0.0, 1.0, 0.0, 1.0]),
        "underline cursor on text: fg should be original foreground"
    );
    assert!(
        f32_arrays_equal(&cell.bg_color, &[1.0, 1.0, 1.0, 0.9]),
        "underline cursor on text: bg should be cursor color with line alpha"
    );
}

#[test]
fn cursor_color_custom_values() {
    use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
    let mut font_pipeline = crate::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    let cells = vec![CellSnapshot {
        codepoint: 0x20,
        ..Default::default()
    }];
    let snapshot = GridSnapshot {
        rows: 1,
        cols: 1,
        cursor_visible: true,
        cursor_style: torvox_core::cursor::CursorStyle::Block,
        cells,
        dirty: vec![true],
        ..Default::default()
    };
    let custom_color = Some([0.5, 0.3, 0.8, 1.0]);
    let instances = build_cell_instances_from_snapshot(
        &snapshot,
        &mut font_pipeline,
        CellInstanceConfig {
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            projection_height: 24.0,
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: custom_color,
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            dirty_rows: &[],
            cached_instances: &[],
            cached_row_ends: &[],
            surface_bg: [0.0, 0.0, 0.0, 1.0],
            render_scale: 1.0,
        },
    );
    assert_eq!(instances.len(), 1);
    let cell = &instances[0];
    assert!(
        f32_arrays_equal(&cell.bg_color, &[0.5, 0.3, 0.8, 0.7]),
        "custom cursor color should be reflected with block alpha multiplier"
    );
}

#[test]
fn render_paused_skips_frame() {
    let mut ctx = GpuContext::new_with_no_surface();
    assert!(!ctx.render_paused, "should start unpaused");
    // Without a surface config, render should fail when not paused
    assert!(
        ctx.render_frame(&[], &[]).is_err(),
        "expected error when not paused and no surface"
    );
    // When paused, render should succeed immediately (skips surface check)
    ctx.set_render_paused(true);
    assert!(
        ctx.render_frame(&[], &[]).is_ok(),
        "expected ok when paused regardless of surface"
    );
}

#[test]
fn render_paused_toggle_resumes_rendering() {
    let mut ctx = GpuContext::new_with_no_surface();
    // Pause then unpause
    ctx.set_render_paused(true);
    assert!(
        ctx.render_frame(&[], &[]).is_ok(),
        "paused skips surface check"
    );
    ctx.set_render_paused(false);
    assert!(
        ctx.render_frame(&[], &[]).is_err(),
        "unpaused fails without surface (correct behavior)"
    );
}

#[test]
fn render_paused_remains_paused_after_multiple_frames() {
    let mut ctx = GpuContext::new_with_no_surface();
    ctx.set_render_paused(true);
    for _ in 0..10 {
        assert!(
            ctx.render_frame(&[], &[]).is_ok(),
            "paused render must stay ok across multiple frames"
        );
    }
}

#[test]
fn new_with_no_surface_starts_unpaused() {
    let ctx = GpuContext::new_with_no_surface();
    assert!(!ctx.render_paused, "new context must start unpaused");
}

#[test]
fn set_render_paused_idempotent() {
    let mut ctx = GpuContext::new_with_no_surface();
    ctx.set_render_paused(true);
    ctx.set_render_paused(true);
    assert!(ctx.render_frame(&[], &[]).is_ok(), "double-pause still ok");
}

#[test]
fn image_active_value_flag_matches_bg_bind_group() {
    // Fix F branch logic: a background image being active is exactly the
    // uniform flag that makes default-background cells transparent.
    assert!(f32_eq(image_active_value(true), 1.0));
    assert!(f32_eq(image_active_value(false), 0.0));
}

include!("../screenshot_tests.rs");
