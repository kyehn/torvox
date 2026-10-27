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
    a.iter()
        .zip(b)
        .all(|(first, second)| first.to_bits() == second.to_bits())
}

/// Font pipeline with the ASCII glyph atlas pre-rasterized.
fn ascii_font() -> crate::render::font::FontPipeline {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    font_pipeline.rasterize_ascii();
    font_pipeline
}

/// 测试单元格：白前景、透明背景、无字素续接码点、无样式位。
/// 需要变体时用结构体更新语法覆盖单个字段。
fn test_cell_data(
    codepoint: u32,
    row: u32,
    col: u32,
) -> crate::terminal::ghostty_terminal::CellData {
    crate::terminal::ghostty_terminal::CellData {
        codepoint,
        width: 1,
        grapheme_extra: [0; 7],
        foreground: [1.0; 4],
        background: [0.0; 4],
        underline_color: [1.0; 4],
        flags: 0,
        row,
        col,
    }
}

/// 测试光标：块状、白色、原点。
fn test_cell_cursor(visible: bool) -> CellCursor {
    CellCursor {
        row: 0,
        col: 0,
        visible,
        style: CursorStyle::Block,
        color: [1.0; 4],
    }
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

    // 把 storage 拷入 staging
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

const TEST_ATLAS_SIZE: f32 = 1024.0;

/// 单元实例构建的测试配置：图集尺寸固定为 TEST_ATLAS_SIZE（本文件的 FontPipeline
/// 一律 1024×1024），其余参数由调用方给出——调用点不再重复同一份字段清单。
fn test_config<'a>(
    rows: u32,
    cols: u32,
    grid_cell_width: f32,
    grid_cell_height: f32,
    cursor: CellCursor,
    search_highlights: &'a [cell_builder::SearchHighlight],
) -> cell_builder::CellInstanceConfig<'a> {
    cell_builder::CellInstanceConfig {
        rows,
        cols,
        grid_cell_width,
        grid_cell_height,
        cursor,
        atlas_width: TEST_ATLAS_SIZE,
        atlas_height: TEST_ATLAS_SIZE,
        search_highlights,
        default_background: [0.0; 4],
    }
}

/// 构建生产实例并断言成功：网格配置由调用方给出。
fn build_instances(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: cell_builder::CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    let mut instances = Vec::new();
    let built = crate::render::build_instances_from_cell_data(
        cell_data,
        config,
        font_pipeline,
        &mut instances,
    );
    assert!(built.is_some(), "production instance build failed");
    instances
}

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
    build_instances(
        cell_data,
        test_config(
            1,
            cell_data.len() as u32,
            cell_width,
            cell_height,
            cursor,
            &[],
        ),
        font_pipeline,
    )
}

/// 每格一个非默认背景的网格（每格都发实例，便于逐格计数）。
fn non_default_grid(rows: u32, cols: u32) -> Vec<crate::terminal::ghostty_terminal::CellData> {
    use crate::terminal::ghostty_terminal::CellData;
    (0..rows)
        .flat_map(|row| {
            (0..cols).map(move |col| CellData {
                foreground: [0.5, 0.5, 0.5, 1.0],
                background: [0.1, 0.1, 0.1, 1.0],
                underline_color: [0.5, 0.5, 0.5, 1.0],
                ..test_cell_data(b'a' as u32 + col, row, col)
            })
        })
        .collect()
}

/// 白字黑底、块状且隐藏的光标探针：光标关闭时该格不得染上光标色。
fn build_hidden_cursor_probe_instance(
    codepoint: u32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    build_cursor_probe_instance(
        codepoint,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
        false,
        font_pipeline,
        CursorStyle::Block,
    )
}

// Build one cursor-probe cell with white block cursor styling: covers the
// repeated single-cell cursor/reverse-video setup with varying codepoint,
// colors, flags and cursor visibility.
// 测试辅助构造器：单元尺寸取自字体流水线，故不作为参数传入。
fn build_cursor_probe_instance(
    codepoint: u32,
    foreground: [f32; 4],
    background: [f32; 4],
    flags: u32,
    cursor_visible: bool,
    font_pipeline: &mut crate::render::font::FontPipeline,
    style: CursorStyle,
) -> Vec<crate::render::CellInstance> {
    use crate::terminal::ghostty_terminal::CellData;
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let cell_data = vec![CellData {
        foreground,
        background,
        underline_color: foreground,
        flags,
        ..test_cell_data(codepoint, 0, 0)
    }];
    let cursor = CellCursor {
        visible: cursor_visible,
        style,
        ..test_cell_cursor(cursor_visible)
    };
    build_configured_cell_instance(&cell_data, cursor, cell_width, cell_height, font_pipeline)
}

/// Build production instances for a single cell (shared by the bearing tests
/// below): one cell, default cursor, unit grid.
fn build_single_cell_instance(
    ch: char,
    cell_width: f32,
    cell_height: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) -> Vec<crate::render::CellInstance> {
    let cell_data = vec![test_cell_data(ch as u32, 0, 0)];
    let cursor = test_cell_cursor(false);
    build_instances(
        &cell_data,
        test_config(1, 1, cell_width, cell_height, cursor, &[]),
        font_pipeline,
    )
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

/// Test helper: groups `SearchHighlight`s by row, just like
/// `build_row_instances_into` does inline.
fn group_highlights_by_row(
    highlights: &[SearchHighlight],
) -> HashMap<i32, Vec<&SearchHighlight>, foldhash::fast::RandomState> {
    let mut by_row: HashMap<i32, Vec<&SearchHighlight>, foldhash::fast::RandomState> =
        HashMap::with_hasher(foldhash::fast::RandomState::default());
    for highlight in highlights {
        by_row.entry(highlight.row).or_default().push(highlight);
    }
    by_row
}

/// 基准用的离屏渲染目标：800×600 Rgba8Unorm，仅作渲染附件。
fn bench_render_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
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
    (texture, view)
}

/// 编码并提交一个只清屏的渲染通道：基准只测命令编码与提交开销。
fn submit_clear_pass(device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) {
    use std::hint::black_box;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Bench Encoder"),
    });
    {
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Bench Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        black_box(&render_pass);
    }
    queue.submit(black_box([encoder.finish()]));
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

mod cell_pipeline;
mod frame_and_atlas;
mod gpu_core;
