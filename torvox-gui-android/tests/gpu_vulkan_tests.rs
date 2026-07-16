//! Real Vulkan (Mesa Lavapipe) tests for the renderer.
//!
//! These complement `gpu_noop_tests.rs` (which uses the NOOP backend and never
//! actually compiles/runs shaders). Here we drive a REAL Vulkan device provided
//! by Mesa's Lavapipe software implementation (`VK_ICD_FILENAMES=lvp_icd.x86_64.json`,
//! set by the nix develop shell). This exercises the same wgpu/Vulkan path the
//! Android app uses on a real GPU, and proves shaders actually execute (the NOOP
//! backend silently returns zeros on readback).
//!
//! Run under nix develop:
//! `nix develop --command cargo test -p torvox-gui-android --test gpu_vulkan_tests`

use core::future::Future as _;
use core::pin::pin;
use core::task;

const COMPUTE_SHADER: &str = r"
@group(0) @binding(0) var<storage, read_write> data: array<u32>;
@compute @workgroup_size(4)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * 2u;
}
";

/// Request a real Vulkan adapter+device. Panics if Lavapipe/Mesa Vulkan is not
/// available (i.e. `VK_ICD_FILENAMES` is not pointing at the Lavapipe ICD).
fn vulkan_device() -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        backend_options: wgpu::BackendOptions::default(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let ctx = &mut task::Context::from_waker(task::Waker::noop());
    let task::Poll::Ready(Ok(adapter)) =
        pin!(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).poll(ctx)
    else {
        panic!(
            "Vulkan request_adapter failed: is Mesa Lavapipe available (VK_ICD_FILENAMES=lvp_icd.x86_64.json)?"
        );
    };
    assert_eq!(
        adapter.get_info().backend,
        wgpu::Backend::Vulkan,
        "expected a real Vulkan backend (Lavapipe); got {:?}",
        adapter.get_info().backend
    );
    let task::Poll::Ready(Ok((device, queue))) =
        pin!(adapter.request_device(&wgpu::DeviceDescriptor::default())).poll(ctx)
    else {
        panic!("Vulkan request_device failed");
    };
    (instance, adapter, device, queue)
}

/// Torvox's real `GpuContext` must initialize against Lavapipe Vulkan (the same
/// wgpu/Vulkan path used on a real Android GPU).
#[test]
fn torvox_gpu_context_inits_on_vulkan() {
    let _gpu = torvox_renderer::gpu::GpuContext::new_with_no_surface();
}

#[test]
fn gpu_vulkan_adapter_is_vulkan() {
    let (_instance, _adapter, _device, _queue) = vulkan_device();
}

/// A real compute kernel must execute under Vulkan and the readback must reflect
/// its output. The NOOP backend returns zeros on readback, so a non-zero result
/// proves real Vulkan execution (Lavapipe).
#[test]
fn gpu_vulkan_compute_readback_real() {
    let (_instance, _adapter, device, queue) = vulkan_device();

    let input: [u32; 4] = [10, 20, 30, 40];
    let input_bytes: Vec<u8> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    let storage = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vulkan-storage"),
        size: 16,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&storage, 0, &input_bytes);

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vulkan-staging"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vulkan-compute"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(COMPUTE_SHADER)),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vulkan-bgl"),
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
        label: Some("vulkan-layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vulkan-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vulkan-bg"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage.as_entire_binding(),
        }],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vulkan-enc"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vulkan-cpass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&storage, 0, &staging, 0, 16);
    queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = slice.get_mapped_range();
    let bytes: Vec<u8> = mapped.to_vec();
    drop(mapped);

    // Expected: input * 2 -> [20, 40, 60, 80] as little-endian u32.
    let expected: Vec<u8> = [20u32, 40, 60, 80]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        bytes, expected,
        "Lavapipe must execute the compute kernel (NOOP backend would return zeros)"
    );
}
