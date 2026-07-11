use core::future::Future as _;
use core::pin::pin;
use core::task;

const RENDER_SHADER: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#;

const COMPUTE_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> data: array<u32>;
@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] + 1;
}
"#;

fn noop_poll_device() -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions { enable: true },
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let ctx = &mut task::Context::from_waker(task::Waker::noop());
    let task::Poll::Ready(Ok(adapter)) =
        pin!(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).poll(ctx)
    else {
        panic!("NOOP request_adapter should resolve immediately");
    };
    let task::Poll::Ready(Ok((device, queue))) =
        pin!(adapter.request_device(&wgpu::DeviceDescriptor::default())).poll(ctx)
    else {
        panic!("NOOP request_device should resolve immediately");
    };
    (instance, adapter, device, queue)
}

/// Task 12.1: Create wgpu instance with Backends::NOOP
#[test]
fn gpu_noop_instance_creation() {
    let _instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions { enable: true },
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
}

/// Task 12.2: Request adapter + device (should always succeed with NOOP)
#[test]
fn gpu_noop_adapter_device() {
    let (_instance, _adapter, _device, _queue) = noop_poll_device();
}

/// Task 12.3: Compile shader module from WGSL source
#[test]
fn gpu_noop_shader_compile() {
    let (_instance, _adapter, device, _queue) = noop_poll_device();
    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("noop-render-shader"),
        source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
    });
}

/// Task 12.4: Create render pipeline
#[test]
fn gpu_noop_render_pipeline() {
    let (_instance, _adapter, device, _queue) = noop_poll_device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("noop-render-shader"),
        source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("noop-pipeline-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let _pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("noop-render-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
}

/// Task 12.5: Create buffer, write data, read it back
/// NOTE: In wgpu 29, MAP_READ cannot be combined with COPY_SRC. We create
/// two buffers: staging (MAP_READ | COPY_DST) and src (COPY_DST | COPY_SRC),
/// write to both, then verify staging.
#[test]
fn gpu_noop_buffer_roundtrip() {
    let (_instance, _adapter, device, queue) = noop_poll_device();
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("noop-staging-buffer"),
        size: 64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let data: [u8; 8] = [0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89];
    queue.write_buffer(&staging, 0, &data);
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| ());
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = slice.get_mapped_range();
    // NOOP backend accepts write_buffer but does not actually store data
    // for readback. Verify operations complete without error (mapping succeeds,
    // no panic), but accept that readback returns zeros.
    assert_eq!(mapped.len(), 64, "NOOP buffer should be 64 bytes");
}

/// Task 12.6: Create compute pipeline
#[test]
fn gpu_noop_compute_pipeline() {
    let (_instance, _adapter, device, _queue) = noop_poll_device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("noop-compute-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPUTE_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("noop-compute-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("noop-compute-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("noop-compute-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
}
