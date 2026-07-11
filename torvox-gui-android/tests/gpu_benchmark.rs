//! Performance benchmarks.
//! Run with: cargo test -p torvox-gui-android --test gpu_benchmark -- --nocapture
//! GPU tests use NOOP backend (no Vulkan needed). All platforms.

use std::pin::pin;
use std::task;
use std::task::{Context, Waker};
use std::time::{Duration, Instant};
use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn noop_device() -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions { enable: true },
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let ctx = &mut Context::from_waker(Waker::noop());
    let task::Poll::Ready(Ok(adapter)) =
        pin!(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).poll(ctx)
    else {
        panic!("NOOP adapter");
    };
    let task::Poll::Ready(Ok((device, queue))) =
        pin!(adapter.request_device(&wgpu::DeviceDescriptor::default())).poll(ctx)
    else {
        panic!("NOOP device");
    };
    (instance, adapter, device, queue)
}

#[test]
fn bench_noop_device_creation() {
    const N: u32 = 3;
    let s = Instant::now();
    for _ in 0..N {
        let _ = noop_device();
    }
    let e = s.elapsed();
    println!("BENCH: NOOP DEVICE CREATION x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} devices/s",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_shader_compile() {
    let (_, _, device, _) = noop_device();
    let src = "@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) { var x = id.x; }";
    const N: u32 = 5;
    let s = Instant::now();
    for _ in 0..N {
        let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(src)),
        });
    }
    let e = s.elapsed();
    println!("BENCH: SHADER COMPILE x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} compiles/s",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_buffer_creation() {
    let (_, _, device, _) = noop_device();
    const N: u32 = 10;
    let s = Instant::now();
    for _ in 0..N {
        let _ = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 65536,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
    }
    let e = s.elapsed();
    println!("BENCH: 64KB BUFFER CREATE x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} buffers/s",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_noop_render_frame() {
    let (_, _, device, queue) = noop_device();
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 720,
            height: 1560,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let clear = wgpu::Color {
        r: 0.1,
        g: 0.1,
        b: 0.3,
        a: 1.0,
    };
    const N: u32 = 5;
    let s = Instant::now();
    for _ in 0..N {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        drop(_rpass);
        queue.submit([encoder.finish()]);
    }
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_millis(100)),
    });
    let e = s.elapsed();
    println!("BENCH: NOOP RENDER 720x1560 x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} FPS (CPU-bound)",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_compute_dispatch() {
    let (_, _, device, queue) = noop_device();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
            "@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) { var x = id.x; }",
        )),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    const N: u32 = 10;
    let s = Instant::now();
    for _ in 0..N {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&pipeline);
        cpass.dispatch_workgroups(64, 1, 1);
        drop(cpass);
        queue.submit([encoder.finish()]);
    }
    let e = s.elapsed();
    println!("BENCH: COMPUTE DISPATCH 64wg x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} dispatches/s",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_texture_atlas() {
    let (_, _, device, _) = noop_device();
    const N: usize = 5;
    let s = Instant::now();
    for _ in 0..N {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default());
    }
    let e = s.elapsed();
    println!("BENCH: ATLAS 1024x1024 RGBA8 x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} textures/s",
        e.as_secs_f64() * 1000.0,
        (e / N as u32).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}

#[test]
fn bench_vt_parse() {
    let mut t = GhosttyTerminal::new(24, 80, 10000).expect("GhosttyTerminal");
    let data: Vec<u8> = (0..1_000)
        .flat_map(|i| format!("\x1b[{}mLine {}\x1b[0m\n", 30 + (i % 8), i).into_bytes())
        .collect();
    let len = data.len();
    let s = Instant::now();
    t.vt_write(&data);
    t.flush();
    let e = s.elapsed();
    println!("BENCH: VT PARSE 1k lines ({} bytes)", len);
    println!(
        "  total {:9.1} ms | {:8.0} lines/s | {:5.2} MB/s",
        e.as_secs_f64() * 1000.0,
        1_000_f64 / e.as_secs_f64(),
        len as f64 / 1_000_000.0 / e.as_secs_f64()
    );
}

#[test]
fn bench_snapshot_throughput() {
    let mut t = GhosttyTerminal::new(24, 80, 10000).expect("GhosttyTerminal");
    t.vt_write(
        &(0..50)
            .flat_map(|i| format!("Line {i} with text\n").into_bytes())
            .collect::<Vec<_>>(),
    );
    t.flush();
    const N: u32 = 20;
    let s = Instant::now();
    for _ in 0..N {
        let _ = t.take_snapshot();
    }
    let e = s.elapsed();
    println!("BENCH: GRID SNAPSHOT 24x80 x{N}");
    println!(
        "  total {:9.1} ms | avg {:6.3} ms | {:6.0} snapshots/s",
        e.as_secs_f64() * 1000.0,
        (e / N).as_secs_f64() * 1000.0,
        N as f64 / e.as_secs_f64()
    );
}
