const TEST_WIDTH: u32 = 640;
const TEST_HEIGHT: u32 = 480;

struct HeadlessEnv {
    _instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn try_create_headless_env() -> Option<HeadlessEnv> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::empty(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });

    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;

    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("headless test device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;

    Some(HeadlessEnv {
        _instance: instance,
        adapter,
        device,
        queue,
    })
}

fn create_headless_env() -> HeadlessEnv {
    try_create_headless_env()
        .expect("no Vulkan adapter found — run in nix develop or install SwiftShader/Lavapipe")
}

#[test]
fn gpu_adapter_available() {
    let env = create_headless_env();
    let info = env.adapter.get_info();
    assert!(!info.name.is_empty(), "adapter name must not be empty");
    assert!(
        info.vendor > 0 || info.device > 0,
        "adapter vendor/device must be non-zero"
    );
}

#[test]
fn gpu_headless_render_red_quad() {
    let env = create_headless_env();
    let device = &env.device;
    let queue = &env.queue;

    // Offscreen texture as render target
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless render target"),
        size: wgpu::Extent3d {
            width: TEST_WIDTH,
            height: TEST_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Red full-screen quad shader
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("red quad shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
            r#"
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let pos = array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(pos[vi], 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 0.0, 1.0);
}
"#,
        )),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("red quad layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("red quad pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless render encoder"),
    });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("headless render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rpass.set_pipeline(&render_pipeline);
        rpass.draw(0..3, 0..1);
    }

    // Readback buffer
    let stride = (TEST_WIDTH * 4) as u32;
    let buffer_size = (TEST_HEIGHT as u64) * (stride as u64);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(TEST_HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: TEST_WIDTH,
            height: TEST_HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    // Wait for GPU and map
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| {
        if let Err(e) = r {
            panic!("readback map failed: {e:?}");
        }
    });
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let data = slice.get_mapped_range().to_vec();
    readback.unmap();

    let stride_usize = (TEST_WIDTH * 4) as usize;
    assert!(data.len() >= 4, "buffer too small: {}", data.len());

    // Check center pixel is red
    let cx = (TEST_WIDTH / 2) as usize;
    let cy = (TEST_HEIGHT / 2) as usize;
    let center_offset = cy * stride_usize + cx * 4;
    let r = data[center_offset];
    let g = data[center_offset + 1];
    let b = data[center_offset + 2];
    let a = data[center_offset + 3];

    assert_eq!(r, 255, "expected red=255 at center, got {r}");
    assert_eq!(g, 0, "expected green=0 at center, got {g}");
    assert_eq!(b, 0, "expected blue=0 at center, got {b}");
    assert_eq!(a, 255, "expected alpha=255 at center, got {a}");
}
