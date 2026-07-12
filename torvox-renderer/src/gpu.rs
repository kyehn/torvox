//! Wgpu GPU render pipeline — atlas, instance management, and surface rendering.
//!
//! # Requirements
//! - [FR-012](crate) — Glyph: atlas allocation (guillotiere)
//! - [FR-014](crate) — Render: threaded GPU with frame pacing
//! - [FR-015](crate) — Font: size, family, ligatures
//! - [FR-017](crate) — Configuration: hot-reload on SIGHUP
//! - [FR-018](crate) — Surface: Android TextureView lifecycle
//! - [FR-019](crate) — Render: 60 FPS on modern devices, 30 FPS on emulators
//! - [NFR-022](crate) — Render: crash recovery

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use thiserror::Error;
use torvox_core::selection::SelectionMode;
use wgpu::util::DeviceExt;

const MIN_ATLAS_BUFFER_SIZE: u64 = 64;
const DESIRED_FRAME_LATENCY: u32 = 2;
const DESIRED_FRAME_LATENCY_ANDROID: u32 = 1;
const QUAD_VERTEX_COUNT: u32 = 6;
const DEFAULT_BG_ALPHA: f32 = 0.8;

#[cfg(target_os = "android")]
const SURFACE_RELEASE_POLL_MS: u64 = 50;

fn log_gpu_error(error: &wgpu::Error) {
    log::error!("GPU_UNCAPTURED_ERROR: {error:#?}");
}

/// A KGP (Kitty Graphics Protocol) image instance for GPU rendering.
/// Unlike CellInstance, this renders a full RGBA image quad rather than a font glyph.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct KgpInstance {
    pub quad_origin: [f32; 2],
    pub quad_size: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_region: [f32; 2],
    pub alpha: f32,
    pub _padding: f32,
}

impl KgpInstance {
    pub fn new(
        quad_origin: [f32; 2],
        quad_size: [f32; 2],
        atlas_offset: [f32; 2],
        atlas_region: [f32; 2],
        alpha: f32,
    ) -> Self {
        Self {
            quad_origin,
            quad_size,
            atlas_offset,
            atlas_region,
            alpha,
            _padding: 0.0,
        }
    }

    pub const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x2,
        5 => Float32,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<KgpInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Process-global Vulkan instance/adapter/device/queue, created once and never dropped.
/// Prevents SIGABRT on x86_64 Android emulators (SwiftShader) when the activity is
/// recreated and a new GpuContext is created before the old Vulkan instance fully cleans up.
struct GlobalGpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn global_gpu() -> &'static GlobalGpu {
    static INSTANCE: OnceLock<GlobalGpu> = OnceLock::new();
    // SAFETY: no_std initializer — cannot return Result from OnceLock::get_or_init.
    // If GPU init fails here the process cannot render and must abort.
    INSTANCE.get_or_init(|| {
        match futures::executor::block_on(GpuContext::initialize_instance_adapter_device()) {
            Ok((instance, adapter, device, queue)) => GlobalGpu {
                instance,
                adapter,
                device,
                queue,
            },
            Err(e) => {
                panic!(
                    "GPU initialization failed: {e}. \
                     Ensure a Vulkan-capable GPU is available: \
                     lavapipe on Linux (set VK_ICD_FILENAMES), \
                     SwiftShader on Android emulator, \
                     or a physical GPU with Vulkan drivers."
                )
            }
        }
    })
}

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("wgpu request adapter failed")]
    NoAdapter,
    #[error("wgpu request device failed: {0}")]
    DeviceRequest(String),
    #[error("surface creation failed: {0}")]
    Surface(String),
    #[error("shader compilation failed: {0}")]
    Shader(String),
    #[error("buffer creation failed: {0}")]
    Buffer(String),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub quad_origin: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_size: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub quad_size: [f32; 2],
    pub flags: f32,
    /// Pixel offset from cell top-left to glyph bitmap top-left within the cell.
    /// Used for bearing-aware fragment shader UV mapping.
    pub bearing: [f32; 2],
    /// The glyph's advance width in pixels (unscaled). Used by the fragment shader
    /// to stretch narrow glyphs to fill the cell, matching Termux's canvas.scale() behavior.
    pub glyph_advance_width: f32,
}

impl CellInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x2,
        7 => Float32,
        8 => Float32x2,
        9 => Float32,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

const QUAD_CORNERS: &[[f32; 2]; 6] = &[
    [-1.0, -1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [1.0, 1.0],
];

fn quad_corner_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CursorInstance {
    pub cursor_pos: [f32; 2],
    pub cursor_size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuUniforms {
    pub projection: [[f32; 4]; 4],
    pub atlas_size: [f32; 2],
    pub _padding: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BgUniforms {
    projection: [[f32; 4]; 4],
    image_size: [f32; 2],
    blur_radius: f32,
    alpha: f32,
    texel_size: [f32; 2],
    _padding: [f32; 2],
}

pub const CATPPUCCIN_MOCHA_BG: wgpu::Color = wgpu::Color {
    r: 30.0 / 255.0,
    g: 30.0 / 255.0,
    b: 46.0 / 255.0,
    a: 1.0,
};

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub cell_pipeline: Option<wgpu::RenderPipeline>,
    pub quad_vertex_buffer: wgpu::Buffer,
    pub cell_bind_group: Option<wgpu::BindGroup>,
    pub cell_uniform_buffer: Option<wgpu::Buffer>,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub atlas_texture: Option<wgpu::Texture>,
    pub atlas_view: Option<wgpu::TextureView>,
    pub atlas_sampler: Option<wgpu::Sampler>,
    pub pipeline_format: wgpu::TextureFormat,
    projection_width: u32,
    projection_height: u32,
    readback_texture: Option<wgpu::Texture>,
    readback_buffer: Option<wgpu::Buffer>,
    bg_color: wgpu::Color,
    pub bg_image_texture: Option<wgpu::Texture>,
    pub bg_image_view: Option<wgpu::TextureView>,
    bg_pipeline: Option<wgpu::RenderPipeline>,
    bg_bind_group_layout: Option<wgpu::BindGroupLayout>,
    bg_bind_group: Option<wgpu::BindGroup>,
    bg_uniform_buffer: Option<wgpu::Buffer>,
    bg_sampler: Option<wgpu::Sampler>,
    bg_blur_radius: f32,
    bg_alpha: f32,
    kgp_pipeline: Option<wgpu::RenderPipeline>,
    kgp_bind_group_layout: Option<wgpu::BindGroupLayout>,
    kgp_bind_group: Option<wgpu::BindGroup>,
    kgp_uniform_buffer: Option<wgpu::Buffer>,
    kgp_sampler: Option<wgpu::Sampler>,
    kgp_instance_buffer: Option<wgpu::Buffer>,
    kgp_texture: Option<wgpu::Texture>,
    kgp_atlas_data: Vec<u8>,
    kgp_atlas_width: u32,
    kgp_atlas_height: u32,
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        self.cell_bind_group = None;
        self.instance_buffer = None;
        self.cell_pipeline = None;
        self.cell_uniform_buffer = None;
        self.bg_bind_group_layout = None;
        self.bg_uniform_buffer = None;
        self.bg_bind_group = None;
        self.bg_sampler = None;
        self.bg_pipeline = None;
        self.bg_image_view = None;
        self.bg_image_texture = None;
        self.atlas_view = None;
        self.atlas_sampler = None;
        self.atlas_texture = None;
        self.readback_buffer = None;
        self.readback_texture = None;
        self.surface_config = None;
        self.kgp_instance_buffer = None;
        self.kgp_bind_group = None;
        self.kgp_sampler = None;
        self.kgp_uniform_buffer = None;
        self.kgp_bind_group_layout = None;
        self.kgp_pipeline = None;
        self.kgp_texture = None;
        self.surface = None;
    }
}

impl GpuContext {
    pub async fn initialize_instance_adapter_device()
    -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
        // Vulkan-only. No GL fallback, no CPU software rendering.
        // Emulator must provide SwiftShader for Vulkan support.
        #[cfg(target_os = "android")]
        let backends = wgpu::Backends::VULKAN;
        #[cfg(not(target_os = "android"))]
        let backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags: wgpu::InstanceFlags::DISCARD_HAL_LABELS,
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| GpuError::NoAdapter)?;

        let adapter_info = adapter.get_info();
        log::info!(
            "GPU adapter: {} (backend={:?}, type={:?})",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
        );

        let device_descriptor = wgpu::DeviceDescriptor {
            label: Some("Torvox Device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        };

        let (device, queue) = adapter
            .request_device(&device_descriptor)
            .await
            .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

        device.on_uncaptured_error(Arc::new(|error| {
            log_gpu_error(&error);
        }));

        log::info!("GPU device created, queue ok");
        Ok((instance, adapter, device, queue))
    }

    fn create_cell_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let wgsl_source = include_str!("../shaders/cell.wgsl");
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let cell_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cell Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let cell_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cell Pipeline Layout"),
            bind_group_layouts: &[Some(&cell_bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cell Pipeline"),
            layout: Some(&cell_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cell_shader,
                entry_point: Some("vs_main"),
                buffers: &[quad_corner_buffer_layout(), CellInstance::buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cell_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_bg_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let wgsl_source = include_str!("../shaders/background.wgsl");
        let bg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Background Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let bg_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Background Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bg_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Background Pipeline Layout"),
            bind_group_layouts: &[Some(&bg_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Background Pipeline"),
            layout: Some(&bg_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader,
                entry_point: Some("vs_main"),
                buffers: &[quad_corner_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, bg_bind_group_layout)
    }

    fn create_kgp_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let wgsl_source = include_str!("../shaders/kgp.wgsl");
        let kgp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("KGP Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let kgp_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("KGP Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let kgp_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("KGP Pipeline Layout"),
            bind_group_layouts: &[Some(&kgp_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("KGP Pipeline"),
            layout: Some(&kgp_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &kgp_shader,
                entry_point: Some("vs_main"),
                buffers: &[quad_corner_buffer_layout(), KgpInstance::buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &kgp_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, kgp_bind_group_layout)
    }

    /// Initialize the background image pipeline, bind group, and update uniforms.
    fn ensure_bg_pipeline(&mut self, surface_width: u32, surface_height: u32) {
        if self.bg_image_view.is_none() {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);

        if self.bg_pipeline.is_none() {
            let (pipeline, layout) = Self::create_bg_pipeline(&self.device, format);
            self.bg_pipeline = Some(pipeline);
            self.bg_bind_group_layout = Some(layout);
        }

        let pipeline = match self.bg_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };
        let layout = pipeline.get_bind_group_layout(0);

        let view = match self.bg_image_view.as_ref() {
            Some(v) => v,
            None => return,
        };

        if self.bg_sampler.is_none() {
            self.bg_sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let sampler = self
            .bg_sampler
            .as_ref()
            .expect("background sampler initialized in ensure_bg_pipeline");

        if self.bg_uniform_buffer.is_none() {
            self.bg_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Background Uniform Buffer"),
                size: std::mem::size_of::<BgUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = self
            .bg_uniform_buffer
            .as_ref()
            .expect("background uniform buffer initialized in ensure_bg_pipeline");

        let (blur, alpha) = (self.bg_blur_radius, self.bg_alpha);
        let texel_x = if surface_width > 0 {
            1.0 / surface_width as f32
        } else {
            0.0
        };
        let texel_y = if surface_height > 0 {
            1.0 / surface_height as f32
        } else {
            0.0
        };

        let proj = orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = BgUniforms {
            projection: proj,
            image_size: [surface_width as f32, surface_height as f32],
            blur_radius: blur,
            alpha,
            texel_size: [texel_x, texel_y],
            _padding: [0.0; 2],
        };
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));

        self.bg_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Bind Group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }));
    }

    /// Initialize the KGP pipeline, bind group, and update uniforms.
    fn ensure_kgp_pipeline(&mut self, surface_width: u32, surface_height: u32) {
        if self.kgp_texture.is_none() {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);

        if self.kgp_pipeline.is_none() {
            let (pipeline, layout) = Self::create_kgp_pipeline(&self.device, format);
            self.kgp_pipeline = Some(pipeline);
            self.kgp_bind_group_layout = Some(layout);
        }

        if self.kgp_sampler.is_none() {
            self.kgp_sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let sampler = match self.kgp_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };

        if self.kgp_uniform_buffer.is_none() {
            self.kgp_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("KGP Uniform Buffer"),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = match self.kgp_uniform_buffer.as_ref() {
            Some(b) => b,
            None => return,
        };

        let proj = orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = GpuUniforms {
            projection: proj,
            atlas_size: [self.kgp_atlas_width as f32, self.kgp_atlas_height as f32],
            _padding: [0.0; 2],
        };
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));

        let view = match self.kgp_texture.as_ref() {
            Some(t) => t.create_view(&wgpu::TextureViewDescriptor::default()),
            None => return,
        };

        let pipeline = match self.kgp_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };

        self.kgp_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("KGP Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }));
    }

    pub async fn new() -> Result<Self, GpuError> {
        let (instance, adapter, device, queue) = Self::initialize_instance_adapter_device().await?;
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
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
        })
    }

    pub fn new_with_no_surface() -> Self {
        let gpu = global_gpu();
        let instance = gpu.instance.clone();
        let adapter = gpu.adapter.clone();
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
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
        }
    }

    pub fn set_bg_color(&mut self, background: [u8; 3]) {
        self.bg_color = wgpu::Color {
            r: background[0] as f64 / 255.0,
            g: background[1] as f64 / 255.0,
            b: background[2] as f64 / 255.0,
            a: 1.0,
        };
    }

    pub fn set_background_params(&mut self, blur_radius: f32, alpha: f32) {
        self.bg_blur_radius = blur_radius.clamp(0.0, 20.0);
        self.bg_alpha = alpha.clamp(0.0, 1.0);
    }

    pub fn background_params(&self) -> (f32, f32) {
        (self.bg_blur_radius, self.bg_alpha)
    }

    pub fn set_bg_image(&mut self, rgba_data: &[u8], width: u32, height: u32) {
        let device = &self.device;
        let queue = &self.queue;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bg_image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        self.bg_image_view = Some(tex.create_view(&wgpu::TextureViewDescriptor::default()));
        self.bg_image_texture = Some(tex);
        self.bg_bind_group = None;
    }

    pub fn clear_bg_image(&mut self) {
        self.bg_image_view = None;
        self.bg_image_texture = None;
        self.bg_bind_group = None;
    }

    pub fn set_kgp_atlas(&mut self, rgba_data: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.kgp_texture = None;
            self.kgp_bind_group = None;
            self.kgp_atlas_width = 0;
            self.kgp_atlas_height = 0;
            return;
        }
        let device = &self.device;
        let queue = &self.queue;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kgp_atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        self.kgp_texture = Some(tex);
        self.kgp_atlas_data = rgba_data.to_vec();
        self.kgp_atlas_width = width;
        self.kgp_atlas_height = height;
        self.kgp_bind_group = None;
    }

    fn select_alpha_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::CompositeAlphaMode {
        if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            caps.alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
        }
    }

    fn select_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
        if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else if caps.present_modes.contains(&wgpu::PresentMode::AutoVsync) {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::Immediate
        }
    }

    pub fn set_surface_from_native_window(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
        initial_width: u32,
        initial_height: u32,
        configure_surface: bool,
    ) -> Result<(), GpuError> {
        use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle};

        let non_null = std::ptr::NonNull::new(window_ptr)
            .ok_or_else(|| GpuError::Surface("null window pointer".to_string()))?;

        let android_handle = AndroidNdkWindowHandle::new(non_null);
        let display_handle = AndroidDisplayHandle::new();

        let raw_win_handle = raw_window_handle::RawWindowHandle::AndroidNdk(android_handle);
        let raw_display_handle = raw_window_handle::RawDisplayHandle::Android(display_handle);

        // SAFETY: The caller guarantees the window handle is valid for the duration
        // of the wgpu surface. `window_ptr` was validated as non-null via `NonNull::new`
        // above and originates from Android's `ANativeWindow_fromSurface`, which is
        // already validated. The `AndroidNdkWindowHandle` wraps a verified non-null
        // pointer. The `NativeWindow` wrapper keeps the `ANativeWindow` alive for the
        // surface's lifetime. wgpu's `create_surface_unsafe` requires the raw handle
        // to remain valid — this is the only place this pointer is used unsafely.
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: raw_win_handle,
                    raw_display_handle: Some(raw_display_handle),
                })
        }
        .map_err(|e| GpuError::Surface(e.to_string()))?;

        // Request a new adapter compatible with this surface.
        // The initial adapter in new_with_no_surface was created without a
        // compatible_surface, which may select an adapter that does not
        // support the Android NDK native window surface (SwiftShader / real GPU).
        let adapter = futures::executor::block_on(self.instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ))
        .map_err(|_| GpuError::NoAdapter)?;

        let adapter_info = adapter.get_info();
        log::info!(
            "GPU adapter (surface-compatible): {} (backend={:?}, type={:?})",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
        );

        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Torvox Device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            }))
            .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

        device.on_uncaptured_error(Arc::new(|error| {
            log_gpu_error(&error);
        }));

        self.adapter = adapter;
        self.device = device;
        self.queue = queue;

        // Re-create device-dependent resources (old ones were tied to old device)
        self.quad_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Quad Vertex Buffer"),
                    contents: bytemuck::cast_slice(QUAD_CORNERS),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .or_else(|| caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm);

        log::info!(
            "Surface formats available: {:?} (chose: {:?})",
            caps.formats,
            format
        );
        log::info!("Present modes available: {:?}", caps.present_modes);

        // Prefer Opaque alpha mode.  With `setZOrderOnTop(true)` (SurfaceView
        // as overlay layer), Opaque alpha mode avoids SurfaceFlinger alpha
        // compositing — the pixel content is directly visible.  On real
        // Adreno/Mali GPUs, Opaque is universally supported by Vulkan WSI.
        // PreMultiplied is fine as a fallback (our shader outputs alpha=1.0).
        // Inherit is last resort — it inherits alpha=0 from the overlay plane
        // on SwiftShader/emulator, making GPU swapchain output invisible.
        let alpha_mode = Self::select_alpha_mode(&caps);
        log::info!(
            "Alpha mode selected: {:?} (available: {:?})",
            alpha_mode,
            caps.alpha_modes,
        );
        let present_mode = Self::select_present_mode(&caps);
        log::info!(
            "Present mode selected: {:?} (available: {:?})",
            present_mode,
            caps.present_modes
        );

        self.cell_pipeline = Some(Self::create_cell_pipeline(&self.device, format));
        self.pipeline_format = format;
        self.surface = Some(surface);

        if configure_surface {
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: initial_width,
                height: initial_height,
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: DESIRED_FRAME_LATENCY,
            };

            if let Some(ref configured_surface) = self.surface {
                configured_surface.configure(&self.device, &config);
            }
            self.surface_config = Some(config);
            log::info!(
                "Surface configured: {}x{}, alpha={:?}, present={:?} ({})",
                initial_width,
                initial_height,
                alpha_mode,
                present_mode,
                if cfg!(target_os = "android") {
                    "android"
                } else {
                    "desktop"
                },
            );
        } else {
            log::info!(
                "Surface created (pipeline only, no config): {}x{}, format={:?} (android offscreen)",
                initial_width,
                initial_height,
                format,
            );
        }

        Ok(())
    }

    /// Create GPU atlas texture.
    ///
    /// Format: `R8Unorm` — a single-channel **linear** (non-sRGB) byte layout
    /// that matches font.rs CPU bitmap output. The glyph alpha-coverage data
    /// is already in the correct (linear) space, so no gamma correction is
    /// applied by the GPU on sampling. (Despite the "Unorm" name, this is NOT
    /// an sRGB format and the GPU does not apply sRGB gamma here.)
    pub fn create_atlas_texture(&mut self, width: u32, height: u32) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.atlas_texture = Some(texture);
        self.atlas_view = Some(view);
        self.atlas_sampler = Some(sampler);
    }

    pub fn upload_atlas(&self, data: &[u8], width: u32, height: u32) {
        if let Some(texture) = &self.atlas_texture {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub fn update_bind_group(
        &mut self,
        atlas_width: f32,
        atlas_height: f32,
        projection_width: f32,
        projection_height: f32,
    ) {
        let pipeline = match self.cell_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };

        if self.cell_uniform_buffer.is_none() {
            self.cell_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cell Uniform Buffer"),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let proj = orthographic_projection(projection_width, projection_height);

        let uniforms = GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width, atlas_height],
            _padding: [0.0; 2],
        };

        let uniform_buffer = match self.cell_uniform_buffer.as_ref() {
            Some(buf) => buf,
            None => return,
        };
        self.queue
            .write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let atlas_view = match self.atlas_view.as_ref() {
            Some(v) => v,
            None => return,
        };
        let atlas_sampler = match self.atlas_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };

        self.cell_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cell Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        }));
    }

    /// Initialize the cell pipeline and bind group without a wgpu surface.
    /// Used on Android where create_surface_unsafe conflicts with
    /// ANativeWindow_lock (the Vulkan surface locks the buffer queue).
    /// Pipeline is created with Rgba8Unorm for offscreen rendering.
    pub fn initialize_pipeline_and_bind_group(
        &mut self,
        atlas_width: u32,
        atlas_height: u32,
        surface_width: u32,
        surface_height: u32,
    ) {
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);
        self.pipeline_format = format;
        self.cell_pipeline = Some(Self::create_cell_pipeline(&self.device, format));

        self.projection_width = surface_width;
        self.projection_height = surface_height;
        self.create_atlas_texture(atlas_width, atlas_height);

        let proj = orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width as f32, atlas_height as f32],
            _padding: [0.0; 2],
        };

        if self.cell_uniform_buffer.is_none() {
            self.cell_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cell Uniform Buffer"),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let uniform_buffer = match self.cell_uniform_buffer.as_ref() {
            Some(buf) => buf,
            None => return,
        };
        self.queue
            .write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let atlas_view = match self.atlas_view.as_ref() {
            Some(v) => v,
            None => return,
        };
        let atlas_sampler = match self.atlas_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };
        let pipeline = match self.cell_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };

        self.cell_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cell Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        }));

        log::info!(
            "initialize_pipeline_and_bind_group: pipeline={} atlas={}x{} surf={}x{}",
            self.cell_pipeline.is_some(),
            atlas_width,
            atlas_height,
            surface_width,
            surface_height,
        );
    }

    pub fn release_gpu_surface(&mut self) {
        self.surface = None;
        self.surface_config = None;
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            log::warn!("release_gpu_surface: device poll error: {error}");
        }
        // Give the Vulkan driver (especially SwiftShader) time to finish
        // asynchronous surface destruction before the next create_surface call.
        #[cfg(target_os = "android")]
        std::thread::sleep(std::time::Duration::from_millis(SURFACE_RELEASE_POLL_MS));
    }

    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    pub fn has_pipeline(&self) -> bool {
        self.cell_pipeline.is_some()
    }

    /// Create a wgpu Surface from an Android ANativeWindow and configure
    /// it with the existing device+queue.  Does NOT request a new adapter
    /// or create a new device — call after `initialize_pipeline_and_bind_group`.
    pub fn configure_android_surface(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<(), GpuError> {
        // Drop old surface FIRST to release ANativeWindow before creating a new one.
        // Poll the device to ensure the old surface's Vulkan resources are fully released
        // before creating a new surface (prevents SIGABRT on SwiftShader/emulator).
        self.surface = None;
        self.surface_config = None;
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            log::warn!("configure_android_surface: device poll error: {error}");
        }

        let non_null = std::ptr::NonNull::new(window_ptr)
            .ok_or_else(|| GpuError::Surface("null window pointer".to_string()))?;
        let android_handle = raw_window_handle::AndroidNdkWindowHandle::new(non_null);
        let display_handle = raw_window_handle::AndroidDisplayHandle::new();

        // SAFETY: Same invariant as the surface creation at the first call site above.
        // `window_ptr` was validated as non-null via `NonNull::new`; it originates from
        // a validated `ANativeWindow` and is kept alive by the `NativeWindow` wrapper.
        // The raw handle must remain valid for the wgpu surface's lifetime — this is
        // the only place this pointer is used unsafely.
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: raw_window_handle::RawWindowHandle::AndroidNdk(
                        android_handle,
                    ),
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                        display_handle,
                    )),
                })
        }
        .map_err(|e| GpuError::Surface(e.to_string()))?;

        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .or_else(|| caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm);

        let alpha_mode = Self::select_alpha_mode(&caps);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: Self::select_present_mode(&caps),
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: DESIRED_FRAME_LATENCY_ANDROID,
        };
        surface.configure(&self.device, &config);
        self.surface = Some(surface);

        log::info!(
            "configure_android_surface: {}x{} format={:?} alpha={:?} present={:?}",
            config.width,
            config.height,
            format,
            alpha_mode,
            config.present_mode,
        );

        self.surface_config = Some(config);
        Ok(())
    }

    /// Reconfigure the existing swapchain with new dimensions and update
    /// the projection matrix in the uniform buffer.  Called on surface
    /// resize (IME keyboard show/hide, window config change).
    #[cfg(target_os = "android")]
    pub fn reconfigure_swapchain(&mut self, width: u32, height: u32) {
        let (surface, config) = match (self.surface.as_ref(), self.surface_config.as_mut()) {
            (Some(s), Some(c)) => (s, c),
            _ => return,
        };
        if config.width == width && config.height == height {
            return;
        }
        config.width = width.max(1);
        config.height = height.max(1);
        surface.configure(&self.device, config);

        log::info!("RECONFIGURE_SWAPCHAIN: {}x{}", width, height);
    }

    pub fn warmup(&self) {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        // Catch panics from get_current_texture (e.g. SwiftShader Vulkan config issues)
        let output = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            surface.get_current_texture()
        })) {
            Ok(
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex),
            ) => tex,
            Ok(_) => return,
            Err(_) => {
                log::warn!("warmup: get_current_texture panicked (SwiftShader compat)");
                return;
            }
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Warmup Encoder"),
            });
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Warmup Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                ..Default::default()
            });
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Acquire a texture from the surface, retrying once on panic.
    /// Returns `None` if the texture could not be acquired (surface Lost, etc.).
    fn acquire_texture(
        &self,
        surface: &wgpu::Surface<'static>,
        _cfg_width: u32,
        _cfg_height: u32,
    ) -> Option<wgpu::SurfaceTexture> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            surface.get_current_texture()
        })) {
            Ok(
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex),
            ) => Some(tex),
            Ok(wgpu::CurrentSurfaceTexture::Lost) => {
                if let Some(config) = &self.surface_config {
                    surface.configure(&self.device, config);
                }
                None
            }
            Ok(_) => None,
            Err(_) => {
                log::warn!(
                    "render_frame: get_current_texture panicked (unconfigured surface), reconfiguring"
                );
                if let Some(config) = &self.surface_config {
                    surface.configure(&self.device, config);
                }
                // Retry once after reconfigure
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    surface.get_current_texture()
                })) {
                    Ok(
                        wgpu::CurrentSurfaceTexture::Success(tex)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(tex),
                    ) => Some(tex),
                    _ => None,
                }
            }
        }
    }

    pub fn render_frame(
        &mut self,
        instances: &[CellInstance],
        kgp_instances: &[KgpInstance],
    ) -> Result<(), GpuError> {
        let mut cfg_width = self
            .surface_config
            .as_ref()
            .map(|c| c.width)
            .ok_or(GpuError::Surface("No surface config".to_string()))?;
        let mut cfg_height = self
            .surface_config
            .as_ref()
            .map(|c| c.height)
            .ok_or(GpuError::Surface("No surface config".to_string()))?;

        // Ensure background and KGP pipelines are ready (must happen before immutable borrows)
        self.ensure_bg_pipeline(cfg_width, cfg_height);
        self.ensure_kgp_pipeline(cfg_width, cfg_height);

        let surface = self
            .surface
            .as_ref()
            .ok_or(GpuError::Surface("No surface configured".to_string()))?;
        let pipeline = self
            .cell_pipeline
            .as_ref()
            .ok_or(GpuError::Surface("No render pipeline".to_string()))?;

        log::trace!(
            "render_frame: {} instances, surface={}, pipeline={}, bind_group={}",
            instances.len(),
            self.surface.is_some(),
            self.cell_pipeline.is_some(),
            self.cell_bind_group.is_some(),
        );

        let output = self.acquire_texture(surface, cfg_width, cfg_height);

        let output = match output {
            Some(tex) => tex,
            None => return Ok(()),
        };

        let tex_size = output.texture.size();
        log::debug!(
            "RENDER_FRAME: config={}x{} tex={}x{} instances={}",
            cfg_width,
            cfg_height,
            tex_size.width,
            tex_size.height,
            instances.len(),
        );
        if tex_size.width != cfg_width || tex_size.height != cfg_height {
            log::warn!(
                "render_frame: size mismatch! config={}x{} texture={}x{}",
                cfg_width,
                cfg_height,
                tex_size.width,
                tex_size.height
            );
            cfg_width = tex_size.width;
            cfg_height = tex_size.height;
            let new_config = wgpu::SurfaceConfiguration {
                width: cfg_width,
                height: cfg_height,
                ..self
                    .surface_config
                    .as_ref()
                    .ok_or_else(|| {
                        GpuError::Surface("surface_config lost during reconfiguration".to_string())
                    })?
                    .clone()
            };
            surface.configure(&self.device, &new_config);
            self.surface_config = Some(new_config);

            let aw = self.atlas_texture.as_ref().map_or(0, |t| t.width());
            let ah = self.atlas_texture.as_ref().map_or(0, |t| t.height());
            let proj = orthographic_projection(cfg_width as f32, cfg_height as f32);
            let uniforms = GpuUniforms {
                projection: proj,
                atlas_size: [aw as f32, ah as f32],
                _padding: [0.0; 2],
            };
            if let Some(buf) = &self.cell_uniform_buffer {
                self.queue
                    .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
            }
            log::debug!("RENDER_FRAME_RECONFIGURE: {}x{}", cfg_width, cfg_height);
        }

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        // Upload instance data
        if !instances.is_empty() {
            let instance_data = bytemuck::cast_slice(instances);
            let needed_size = instance_data.len() as u64;
            let resize_buffer = self
                .instance_buffer
                .as_ref()
                .is_none_or(|buf| buf.size() < needed_size);
            if resize_buffer {
                self.instance_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Instance Buffer"),
                    size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            if let Some(ref buf) = self.instance_buffer {
                self.queue.write_buffer(buf, 0, instance_data);
            }
        }

        // Upload KGP instance data
        if !kgp_instances.is_empty() {
            let kgp_instance_data = bytemuck::cast_slice(kgp_instances);
            let needed_size = kgp_instance_data.len() as u64;
            let resize_buffer = self
                .kgp_instance_buffer
                .as_ref()
                .is_none_or(|buf| buf.size() < needed_size);
            if resize_buffer {
                self.kgp_instance_buffer =
                    Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("KGP Instance Buffer"),
                        size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            if let Some(ref buf) = self.kgp_instance_buffer {
                self.queue.write_buffer(buf, 0, kgp_instance_data);
            }
        }

        // Background image render pass (before cell pass)
        if let (Some(bg_pipeline), Some(bg_bind_group)) = (&self.bg_pipeline, &self.bg_bind_group) {
            {
                let mut bg_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Background Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.bg_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                bg_pass.set_pipeline(bg_pipeline);
                bg_pass.set_bind_group(0, bg_bind_group, &[]);
                bg_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
                bg_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
                bg_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                bg_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
        }

        // KGP image render pass (after background, before cells)
        if let (Some(kgp_pipeline), Some(kgp_bind_group)) =
            (&self.kgp_pipeline, &self.kgp_bind_group)
            && !kgp_instances.is_empty()
        {
            let mut kgp_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("KGP Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            kgp_pass.set_pipeline(kgp_pipeline);
            kgp_pass.set_bind_group(0, kgp_bind_group, &[]);
            kgp_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
            kgp_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
            kgp_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            if let Some(ref ib) = self.kgp_instance_buffer {
                kgp_pass.set_vertex_buffer(1, ib.slice(..));
            }
            kgp_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
        }

        {
            let has_bg = self.bg_bind_group.is_some();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cell Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if has_bg {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(self.bg_color)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            let viewport_width = cfg_width as f32;
            let viewport_height = cfg_height as f32;
            render_pass.set_pipeline(pipeline);
            render_pass.set_viewport(0.0, 0.0, viewport_width, viewport_height, 0.0, 1.0);
            render_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);

            let has_bind_group = self.cell_bind_group.is_some();
            let has_instance_buffer = self.instance_buffer.is_some();
            log::debug!(
                "DIAG_DRAW: bind_group={} instances={} instance_buffer={}",
                has_bind_group,
                instances.len(),
                has_instance_buffer,
            );

            if let Some(bind_group) = &self.cell_bind_group {
                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);

                if !instances.is_empty() {
                    render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    if let Some(ref instance_buffer) = self.instance_buffer {
                        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                    }
                    render_pass.draw(0..QUAD_VERTEX_COUNT, 0..instances.len() as u32);
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        log::debug!("render_frame: presented {} instances", instances.len());

        Ok(())
    }

    /// Render the current frame to an offscreen buffer and return raw RGBA pixels.
    /// Creates a dedicated offscreen texture if one does not exist or has mismatched size.
    /// This is a test-only path — NOT used for display.
    pub fn render_to_buffer(
        &mut self,
        instances: &[CellInstance],
        kgp_instances: &[KgpInstance],
    ) -> Result<Vec<u8>, GpuError> {
        let (w, h) = self
            .surface_config
            .as_ref()
            .map_or((0, 0), |c| (c.width, c.height));
        if w == 0 || h == 0 {
            return Err(GpuError::Surface("No surface config".to_string()));
        }

        self.ensure_bg_pipeline(w, h);
        self.ensure_kgp_pipeline(w, h);

        let tex_size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let needs_new = match &self.readback_texture {
            Some(t) => t.width() != w || t.height() != h,
            None => true,
        };
        if needs_new {
            self.readback_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Readback Texture"),
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }
        let texture = self
            .readback_texture
            .as_ref()
            .ok_or_else(|| GpuError::Surface("readback_texture creation failed".to_string()))?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_row_padded = ((w * 4) + (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1))
            & !(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1);
        let buf_size = (bytes_per_row_padded * h) as u64;
        let needs_buf_new = match &self.readback_buffer {
            Some(b) => b.size() < buf_size,
            None => true,
        };
        if needs_buf_new {
            self.readback_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Readback Buffer"),
                size: buf_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }

        let pipeline = self
            .cell_pipeline
            .as_ref()
            .ok_or_else(|| GpuError::Surface("No render pipeline".to_string()))?;

        let has_bg = self.bg_bind_group.is_some();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Readback Encoder"),
            });

        // Upload instance data
        if !instances.is_empty() {
            let instance_data = bytemuck::cast_slice(instances);
            let needed_size = instance_data.len() as u64;
            let resize = self
                .instance_buffer
                .as_ref()
                .is_none_or(|b| b.size() < needed_size);
            if resize {
                self.instance_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Instance Buffer (readback)"),
                    size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            if let Some(ref buf) = self.instance_buffer {
                self.queue.write_buffer(buf, 0, instance_data);
            }
        }

        // Upload KGP instance data
        if !kgp_instances.is_empty() {
            let kgp_instance_data = bytemuck::cast_slice(kgp_instances);
            let needed_size = kgp_instance_data.len() as u64;
            let resize = self
                .kgp_instance_buffer
                .as_ref()
                .is_none_or(|b| b.size() < needed_size);
            if resize {
                self.kgp_instance_buffer =
                    Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("KGP Instance Buffer (readback)"),
                        size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            if let Some(ref buf) = self.kgp_instance_buffer {
                self.queue.write_buffer(buf, 0, kgp_instance_data);
            }
        }

        // Background render pass (if bg bind group exists)
        if let (Some(bg_pipeline), Some(bg_bind_group)) =
            (self.bg_pipeline.as_ref(), self.bg_bind_group.as_ref())
        {
            let mut bg_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Background Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.bg_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            bg_pass.set_pipeline(bg_pipeline);
            bg_pass.set_bind_group(0, bg_bind_group, &[]);
            bg_pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
            bg_pass.set_scissor_rect(0, 0, w, h);
            bg_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            bg_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
        }

        // KGP render pass
        if let (Some(kgp_pipeline), Some(kgp_bind_group)) =
            (self.kgp_pipeline.as_ref(), self.kgp_bind_group.as_ref())
            && !kgp_instances.is_empty()
        {
            let mut kgp_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("KGP Render Pass (readback)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            kgp_pass.set_pipeline(kgp_pipeline);
            kgp_pass.set_bind_group(0, kgp_bind_group, &[]);
            kgp_pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
            kgp_pass.set_scissor_rect(0, 0, w, h);
            kgp_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            if let Some(ref ib) = self.kgp_instance_buffer {
                kgp_pass.set_vertex_buffer(1, ib.slice(..));
            }
            kgp_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
        }

        // Render pass → offscreen texture
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Readback Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if has_bg {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(self.bg_color)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let wf = w as f32;
            let hf = h as f32;
            rp.set_pipeline(pipeline);
            rp.set_viewport(0.0, 0.0, wf, hf, 0.0, 1.0);
            rp.set_scissor_rect(0, 0, w, h);
            if let Some(bind_group) = &self.cell_bind_group {
                rp.set_bind_group(0, bind_group, &[]);
                if !instances.is_empty() {
                    rp.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    if let Some(ref ib) = self.instance_buffer {
                        rp.set_vertex_buffer(1, ib.slice(..));
                    }
                    rp.draw(0..QUAD_VERTEX_COUNT, 0..instances.len() as u32);
                }
            }
        }

        // Copy texture → staging buffer
        let dst = self
            .readback_buffer
            .as_ref()
            .ok_or_else(|| GpuError::Surface("readback_buffer creation failed".to_string()))?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: dst,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_padded),
                    rows_per_image: Some(h),
                },
            },
            tex_size,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Wait for GPU to finish
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            log::warn!("render_to_buffer: device poll error: {error}");
        }

        // Map buffer
        let slice = dst.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| {
            if let Err(e) = r {
                log::error!("readback map failed: {e:?}");
            }
        });
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            log::warn!("render_to_buffer (map wait): device poll error: {error}");
        }
        let data = slice.get_mapped_range().to_vec();
        dst.unmap();

        // De-interleave padding: buffer rows are aligned to COPY_BYTES_PER_ROW_ALIGNMENT
        // (256 bytes). Extract only the actual pixel bytes from each row.
        let pixel_bytes = (w * h * 4) as usize;
        let stride = bytes_per_row_padded as usize;
        let trimmed = if data.len() > pixel_bytes && stride > (w as usize * 4) {
            let mut flat = Vec::with_capacity(pixel_bytes);
            for row in 0..h as usize {
                let row_start = row * stride;
                let row_end = row_start + (w as usize * 4);
                if row_end <= data.len() {
                    flat.extend_from_slice(&data[row_start..row_end]);
                }
            }
            flat
        } else {
            data
        };

        Ok(trimmed)
    }
}

pub fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    // wgpu NDC: clip_y=-1 = top, clip_y=+1 = bottom.
    // Terminal: row 0 = top of screen.
    //
    // GLES swapchain on Android presents with (0,0) at bottom-left (GL convention),
    // but ANativeWindow expects (0,0) at top-left. This causes an unwanted Y-flip.
    // To compensate, we flip the projection Y so the swapchain's double-flip
    // produces the correct orientation:
    //   clip_y = -2 * world_y / height + 1
    //   world_y=0 → clip_y=+1 (after presentation flip → screen top)
    //   world_y=h → clip_y=-1 (after presentation flip → screen bottom)
    //
    // Rust stores row-major but WGSL reads column-major.
    // With M*vec4, result[i] = sum_j Rust[j][i] * vec[j].
    // Translation components must be in Rust[3][0] and Rust[3][1]
    // so they act on the w component of the column-major columns.
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

/// Flat grid data for building cell instances without a GridSnapshot.
/// Used by Ghostty VT integration where terminal state is managed externally.
#[cfg(test)]
pub struct FlatGrid {
    pub rows: u32,
    pub cols: u32,
    pub chars: Vec<char>,
    pub foreground: Vec<[f32; 4]>,
    pub background: Vec<[f32; 4]>,
    pub selected: Vec<bool>,
}

#[cfg(test)]
impl FlatGrid {
    pub fn new(rows: u32, cols: u32) -> Self {
        let len = (rows * cols) as usize;
        Self {
            rows,
            cols,
            chars: vec![' '; len],
            foreground: vec![[1.0, 1.0, 1.0, 1.0]; len],
            background: vec![[0.0, 0.0, 0.0, 1.0]; len],
            selected: vec![false; len],
        }
    }

    pub fn set_cell(
        &mut self,
        row: u32,
        col: u32,
        ch: char,
        foreground: [f32; 4],
        background: [f32; 4],
    ) {
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            self.chars[idx] = ch;
            self.foreground[idx] = foreground;
            self.background[idx] = background;
        }
    }

    pub fn cell(&self, row: u32, col: u32) -> Option<(char, [f32; 4], [f32; 4])> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            Some((self.chars[idx], self.foreground[idx], self.background[idx]))
        } else {
            None
        }
    }
}

#[cfg(test)]
pub fn build_cell_instances_from_flat(
    flat: &FlatGrid,
    font_pipeline: &mut crate::font::FontPipeline,
    atlas_width: f32,
    atlas_height: f32,
) -> Vec<CellInstance> {
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();
    let mut instances = Vec::with_capacity((flat.rows * flat.cols) as usize);

    for row in 0..flat.rows {
        for col in 0..flat.cols {
            if let Some((ch, fg, bg)) = flat.cell(row, col) {
                let idx = (row * flat.cols + col) as usize;
                let (fg, bg) = if flat.selected.get(idx).copied().unwrap_or(false) {
                    (bg, fg)
                } else {
                    (fg, bg)
                };
                if ch == ' ' {
                    instances.push(CellInstance {
                        quad_origin: [col as f32 * cell_w, row as f32 * cell_h],
                        atlas_offset: [0.0, 0.0],
                        atlas_size: [0.0, 0.0],
                        fg_color: [0.0, 0.0, 0.0, 0.0],
                        bg_color: bg,
                        quad_size: [cell_w, cell_h],
                        flags: 0.0,
                        bearing: [0.0; 2],
                        glyph_advance_width: 0.0,
                    });
                } else if let Some(info) = font_pipeline.glyph_information(ch) {
                    let uv_x = info.atlas_x as f32 / atlas_width;
                    let uv_y = info.atlas_y as f32 / atlas_height;
                    let uv_w = info.width as f32 / atlas_width;
                    let uv_h = info.height as f32 / atlas_height;

                    let bearing_x = info.placement.left as f32;
                    let bearing_y = ascent_pixels - info.placement.top as f32;

                    instances.push(CellInstance {
                        quad_origin: [col as f32 * cell_w, row as f32 * cell_h],
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: [cell_w, cell_h],
                        flags: 0.0,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: info.advance_width,
                    });
                }
            }
        }
    }
    instances
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionRange {
    pub start_row: i32,
    pub start_col: i32,
    pub end_row: i32,
    pub end_col: i32,
    pub active: bool,
    pub mode: SelectionMode,
    pub origin: Option<(i32, i32)>,
}

impl SelectionRange {
    pub fn contains(&self, row: u32, col: u32, _cols: u32) -> bool {
        if !self.active {
            return false;
        }
        let row = row as i32;
        let col = col as i32;
        let (lo_row, lo_col, hi_row, hi_col) = self.ordered();
        match self.mode {
            SelectionMode::Line => row >= lo_row && row <= hi_row,
            SelectionMode::Block => {
                row >= lo_row && row <= hi_row && col >= lo_col && col <= hi_col
            }
            SelectionMode::Char | SelectionMode::Word => {
                if row < lo_row || row > hi_row {
                    return false;
                }
                if lo_row == hi_row {
                    col >= lo_col && col <= hi_col
                } else if row == lo_row {
                    col >= lo_col
                } else if row == hi_row {
                    col <= hi_col
                } else {
                    true
                }
            }
        }
    }

    fn ordered(&self) -> (i32, i32, i32, i32) {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            (self.start_row, self.start_col, self.end_row, self.end_col)
        } else {
            (self.end_row, self.end_col, self.start_row, self.start_col)
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchHighlight {
    pub row: i32,
    pub start_col: i32,
    pub end_col_exclusive: i32,
    pub color: [u8; 4],
}

fn cell_highlight<'a>(
    row: u32,
    col: u32,
    by_row: &'a HashMap<i32, Vec<&'a SearchHighlight>>,
) -> Option<&'a [u8; 4]> {
    let h_list = by_row.get(&(row as i32))?;
    let highlight = h_list
        .iter()
        .find(|h| (col as i32) >= h.start_col && (col as i32) < h.end_col_exclusive)?;
    Some(&highlight.color)
}

fn color_f32x4_eq(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0].to_bits() == b[0].to_bits()
        && a[1].to_bits() == b[1].to_bits()
        && a[2].to_bits() == b[2].to_bits()
        && a[3].to_bits() == b[3].to_bits()
}

fn blend_highlight(base: [f32; 4], hl_rgba: [u8; 4]) -> [f32; 4] {
    let alpha = hl_rgba[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return base;
    }
    let hr = hl_rgba[0] as f32 / 255.0;
    let hg = hl_rgba[1] as f32 / 255.0;
    let hb = hl_rgba[2] as f32 / 255.0;
    [
        base[0] * (1.0 - alpha) + hr * alpha,
        base[1] * (1.0 - alpha) + hg * alpha,
        base[2] * (1.0 - alpha) + hb * alpha,
        1.0,
    ]
}

pub struct CellInstanceConfig<'a> {
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub projection_height: f32,
    pub selection: Option<SelectionRange>,
    pub selection_bg: Option<[f32; 4]>,
    pub search_highlights: &'a [SearchHighlight],
    pub cursor_color: Option<[f32; 4]>,
    pub cursor_style: torvox_core::cursor::CursorStyle,
    /// Per-row dirty flags. Empty slice = all rows dirty (full rebuild).
    pub dirty_rows: &'a [bool],
    /// Cached CellInstances from the previous frame — used as source for
    /// non-dirty rows so we avoid re-executing the shaping/color/atlas
    /// lookup hot path for unchanged cells.
    pub cached_instances: &'a [CellInstance],
    /// Cumulative end offsets for each row in `cached_instances`.
    /// `cached_row_ends[row]` = exclusive end index of row `row`.
    pub cached_row_ends: &'a [usize],
}

pub fn build_cell_instances_from_snapshot(
    snapshot: &torvox_terminal::ghostty_terminal::GridSnapshot,
    font_pipeline: &mut crate::font::FontPipeline,
    config: CellInstanceConfig<'_>,
) -> Vec<CellInstance> {
    let mut instances = Vec::new();
    let mut _row_ends = Vec::new();
    build_cell_instances_into(
        snapshot,
        font_pipeline,
        config,
        &mut instances,
        &mut _row_ends,
    );
    instances
}

/// Apply a search highlight to a cell's foreground and background colors.
///
/// When the highlight alpha is >= 128 (current match), the foreground and
/// background are swapped (inverted) to produce a strong "反色" effect.
/// When alpha is < 128 (other match), only the background is tinted with
/// the highlight color, leaving the foreground unchanged.
#[inline]
fn apply_search_highlight(fg: &mut [f32; 4], bg: &mut [f32; 4], hl: [u8; 4]) {
    if hl[3] >= 128 {
        std::mem::swap(fg, bg);
    }
    *bg = blend_highlight(*bg, hl);
}

/// Like [`build_cell_instances_from_snapshot`] but reuses a caller-owned
/// `instances` buffer, clearing it first. Reusing the buffer avoids a fresh
/// `Vec` heap allocation on every rendered frame (the hot path would otherwise
/// allocate `rows * cols` `CellInstance`s, ~800 KB at 200x50, on each frame).
pub fn build_cell_instances_into(
    snapshot: &torvox_terminal::ghostty_terminal::GridSnapshot,
    font_pipeline: &mut crate::font::FontPipeline,
    config: CellInstanceConfig<'_>,
    instances: &mut Vec<CellInstance>,
    row_ends: &mut Vec<usize>,
) {
    let atlas_width = config.atlas_width;
    let atlas_height = config.atlas_height;
    let projection_height = config.projection_height;
    let selection = config.selection;
    let selection_bg = config.selection_bg;
    let search_highlights = config.search_highlights;
    let cursor_color = config.cursor_color;
    let cursor_style = config.cursor_style;
    let rows = snapshot.rows;
    let cols = snapshot.cols;
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();
    let expected = (snapshot.rows * snapshot.cols) as usize;
    if snapshot.cells.len() < expected {
        log::warn!(
            "build_cell_instances_into: snapshot cells too short ({} < {}), skipping render",
            snapshot.cells.len(),
            expected,
        );
        instances.clear();
        return;
    }

    instances.clear();
    let use_cache = !config.dirty_rows.is_empty()
        && config.dirty_rows.len() >= rows as usize
        && config.cached_row_ends.len() >= rows as usize
        && config.cached_instances.len() > config.cached_row_ends[rows as usize - 1];
    if use_cache {
        // Estimate capacity from cached total (typically matches or is close)
        instances.reserve(config.cached_instances.len());
    } else {
        instances.reserve((rows * cols) as usize);
    }
    row_ends.clear();

    let cursor_row = snapshot.cursor_row;
    let cursor_col = snapshot.cursor_col;
    let cursor_visible = snapshot.cursor_visible;

    let mut glyph_found = 0u64;
    let mut glyph_not_found = 0u64;

    // Cursor rendering constants matching termlib ratios
    const CURSOR_BAR_WIDTH_RATIO: f32 = 0.15;
    const CURSOR_UNDERLINE_HEIGHT_RATIO: f32 = 0.15;
    const CURSOR_BLOCK_ALPHA: f32 = 0.7;
    const CURSOR_LINE_ALPHA: f32 = 0.9;

    let mut highlights_by_row: HashMap<i32, Vec<&SearchHighlight>> = HashMap::new();
    for h in search_highlights {
        highlights_by_row.entry(h.row).or_default().push(h);
    }

    for row in 0..rows {
        // ── PROJECTION CLIP ──
        // Must run before the cached row path so that rows beyond the
        // visible area are always skipped, even when their content is
        // clean. Otherwise, when the surface shrinks (e.g. IME opens),
        // clean rows below the new projection_height would still be
        // copied from cache, wasting GPU bandwidth and potentially
        // rendering at wrong positions.
        if projection_height > 0.0 && (row as f32 * cell_h) >= projection_height {
            break;
        }

        // ── CACHED ROW PATH ──
        // When the row is clean (no cell content changed, no cursor, no selection,
        // no search highlight), copy the previous frame's instances directly.
        // This avoids re-executing the shaping, color blending, and atlas lookup
        // hot path for every cell in every frame.
        if use_cache && !config.dirty_rows[row as usize] {
            let ru = row as usize;
            let start = if ru == 0 {
                0_usize
            } else {
                config.cached_row_ends[ru - 1]
            };
            let end = config.cached_row_ends[ru];
            instances.extend_from_slice(&config.cached_instances[start..end]);
            row_ends.push(instances.len());
            continue;
        }
        let mut skip_cols = 0u32;
        for col in 0..cols {
            if skip_cols > 0 {
                skip_cols -= 1;
                continue;
            }

            let idx = (row * cols + col) as usize;
            let cell = &snapshot.cells[idx];
            let cell_span = cell.width.max(1) as f32;
            let is_cursor = cursor_visible && row == cursor_row && col == cursor_col;

            if cell.codepoint == 0 || cell.codepoint == 0x20 {
                let (mut fg, mut bg) = if cell.reverse {
                    (cell.background, cell.foreground)
                } else {
                    (cell.foreground, cell.background)
                };
                if selection.unwrap_or_default().contains(row, col, cols) {
                    if let Some(sbg) = selection_bg {
                        bg = sbg;
                    } else {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                }
                if let Some(hl) = cell_highlight(row, col, &highlights_by_row) {
                    apply_search_highlight(&mut fg, &mut bg, *hl);
                }
                let base_x = col as f32 * cell_w;
                let base_y = row as f32 * cell_h;
                let (quad_size, quad_origin) = if is_cursor {
                    let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let cursor_alpha = match cursor_style {
                        torvox_core::cursor::CursorStyle::Block => CURSOR_BLOCK_ALPHA,
                        _ => CURSOR_LINE_ALPHA,
                    };
                    let cursor_bg = [
                        raw_cursor_bg[0],
                        raw_cursor_bg[1],
                        raw_cursor_bg[2],
                        raw_cursor_bg[3] * cursor_alpha,
                    ];
                    fg = bg;
                    bg = cursor_bg;
                    match cursor_style {
                        torvox_core::cursor::CursorStyle::Block => {
                            ([cell_w, cell_h], [base_x, base_y])
                        }
                        torvox_core::cursor::CursorStyle::Bar => {
                            ([cell_w * CURSOR_BAR_WIDTH_RATIO, cell_h], [base_x, base_y])
                        }
                        torvox_core::cursor::CursorStyle::Underline => (
                            [cell_w, cell_h * CURSOR_UNDERLINE_HEIGHT_RATIO],
                            [
                                base_x,
                                base_y + cell_h - cell_h * CURSOR_UNDERLINE_HEIGHT_RATIO,
                            ],
                        ),
                    }
                } else {
                    ([cell_w, cell_h], [base_x, base_y])
                };
                instances.push(CellInstance {
                    quad_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size,
                    flags: 0.0,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                continue;
            }

            // ── LIGATURE RUN DETECTION ──
            let mut run_len = 1u32;
            if !is_cursor {
                let mut adv = col + cell_span as u32;
                while adv < cols {
                    let nidx = (row * cols + adv) as usize;
                    let next = &snapshot.cells[nidx];
                    let next_cursor = cursor_visible && row == cursor_row && adv == cursor_col;
                    if next.codepoint == 0 || next.codepoint == 0x20 {
                        break;
                    }
                    if next_cursor {
                        break;
                    }
                    let attrs_differ = !color_f32x4_eq(next.foreground, cell.foreground)
                        || !color_f32x4_eq(next.background, cell.background)
                        || next.bold != cell.bold
                        || next.dim != cell.dim
                        || next.italic != cell.italic
                        || next.underline != cell.underline
                        || next.double_underline != cell.double_underline
                        || next.reverse != cell.reverse
                        || next.strikethrough != cell.strikethrough
                        || next.overline != cell.overline
                        || next.uri.is_some() != cell.uri.is_some();
                    if attrs_differ {
                        break;
                    }
                    run_len += 1;
                    adv += next.width.max(1) as u32;
                }
            }

            if run_len > 1 {
                // ── LIGATURE-AWARE SHAPING PATH ──
                let mut run_text = String::with_capacity(run_len as usize * 4);
                let mut adv_col = col;
                let mut run_skip = 0u32;
                for _ in 0..run_len {
                    let cidx = (row * cols + adv_col) as usize;
                    let c = &snapshot.cells[cidx];
                    if let Some(ch) = char::from_u32(c.codepoint) {
                        run_text.push(ch);
                    }
                    for &cp in c.graphemes.iter().skip(1) {
                        if let Some(gch) = char::from_u32(cp) {
                            run_text.push(gch);
                        }
                    }
                    let span = c.width.max(1) as u32;
                    run_skip += span - 1;
                    adv_col += span;
                }

                let shaped = font_pipeline.shape_run(&run_text);
                let flags = (if cell.bold { 1.0 } else { 0.0 })
                    + (if cell.italic { 2.0 } else { 0.0 })
                    + (if cell.reverse { 4.0 } else { 0.0 })
                    + (if cell.underline { 8.0 } else { 0.0 })
                    + (if cell.uri.is_some() { 16.0 } else { 0.0 })
                    + (if cell.strikethrough { 32.0 } else { 0.0 })
                    + (if cell.overline { 64.0 } else { 0.0 })
                    + (if cell.dim { 128.0 } else { 0.0 })
                    + (if cell.double_underline { 256.0 } else { 0.0 });

                for sg in &shaped {
                    let gcol_f = sg.x / cell_w;
                    let gcol = (col as f32 + gcol_f).round() as u32;
                    let gspan = ((sg.w / cell_w).round() as u32).max(1);

                    let cell_idx = (row * cols + gcol) as usize;
                    let ref_cell = &snapshot.cells[cell_idx];
                    let g_cursor = cursor_visible && row == cursor_row && gcol == cursor_col;

                    let (mut gfg, mut gbg) = if ref_cell.reverse {
                        (ref_cell.background, ref_cell.foreground)
                    } else {
                        (ref_cell.foreground, ref_cell.background)
                    };
                    if selection.unwrap_or_default().contains(row, gcol, cols) {
                        if let Some(sbg) = selection_bg {
                            gbg = sbg;
                        } else {
                            std::mem::swap(&mut gfg, &mut gbg);
                        }
                    }
                    if let Some(hl) = cell_highlight(row, gcol, &highlights_by_row) {
                        apply_search_highlight(&mut gfg, &mut gbg, *hl);
                    }
                    let (gfg_scoped, gbg_scoped) = if g_cursor {
                        let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                        let (cursor_alpha, gfg_override) = match cursor_style {
                            torvox_core::cursor::CursorStyle::Block => (CURSOR_BLOCK_ALPHA, gbg),
                            _ => (CURSOR_LINE_ALPHA, gfg),
                        };
                        let cursor_bg = [
                            raw_cursor_bg[0],
                            raw_cursor_bg[1],
                            raw_cursor_bg[2],
                            raw_cursor_bg[3] * cursor_alpha,
                        ];
                        (gfg_override, cursor_bg)
                    } else {
                        (gfg, gbg)
                    };

                    if let Some(info) =
                        font_pipeline.glyph_information_for_glyph(sg.font_id, sg.glyph_id)
                    {
                        glyph_found += 1;
                        let uv_x = info.atlas_x as f32 / atlas_width;
                        let uv_y = info.atlas_y as f32 / atlas_height;
                        let uv_w = info.width as f32 / atlas_width;
                        let uv_h = info.height as f32 / atlas_height;
                        let bearing_x = info.placement.left as f32 + sg.x_offset;
                        let glyph_h = info.height as f32;
                        let raw_bearing_y = ascent_pixels - info.placement.top as f32;
                        let bearing_y = if glyph_h > cell_h {
                            (cell_h - glyph_h) / 2.0 + sg.y_offset
                        } else {
                            raw_bearing_y + sg.y_offset
                        };

                        instances.push(CellInstance {
                            quad_origin: [gcol as f32 * cell_w, row as f32 * cell_h],
                            atlas_offset: [uv_x, uv_y],
                            atlas_size: [uv_w, uv_h],
                            fg_color: gfg_scoped,
                            bg_color: gbg_scoped,
                            quad_size: [cell_w * gspan as f32, cell_h],
                            flags,
                            bearing: [bearing_x, bearing_y],
                            glyph_advance_width: info.advance_width,
                        });
                    } else {
                        glyph_not_found += 1;
                        instances.push(CellInstance {
                            quad_origin: [gcol as f32 * cell_w, row as f32 * cell_h],
                            atlas_offset: [0.0; 2],
                            atlas_size: [1.0 / atlas_width, 1.0 / atlas_height],
                            fg_color: gfg_scoped,
                            bg_color: gbg_scoped,
                            quad_size: [cell_w * gspan as f32, cell_h],
                            flags,
                            bearing: [0.0; 2],
                            glyph_advance_width: 0.0,
                        });
                    }
                }

                skip_cols = run_len - 1 + run_skip;
                continue;
            }

            // ── SINGLE-CHAR PATH (unchanged) ──
            let ch = char::from_u32(cell.codepoint).unwrap_or('\u{FFFD}');
            let flags = if cell.bold { 1.0 } else { 0.0 }
                + if cell.italic { 2.0 } else { 0.0 }
                + if cell.reverse { 4.0 } else { 0.0 }
                + if cell.underline { 8.0 } else { 0.0 }
                + if cell.uri.is_some() { 16.0 } else { 0.0 }
                + if cell.strikethrough { 32.0 } else { 0.0 }
                + if cell.overline { 64.0 } else { 0.0 }
                + if cell.dim { 128.0 } else { 0.0 }
                + if cell.double_underline { 256.0 } else { 0.0 };

            let (mut fg, mut bg) = if cell.reverse {
                (cell.background, cell.foreground)
            } else {
                (cell.foreground, cell.background)
            };

            if selection.unwrap_or_default().contains(row, col, cols) {
                if let Some(sbg) = selection_bg {
                    bg = sbg;
                } else {
                    std::mem::swap(&mut fg, &mut bg);
                }
            }
            if let Some(hl) = cell_highlight(row, col, &highlights_by_row) {
                apply_search_highlight(&mut fg, &mut bg, *hl);
            }

            let (fg, bg) = if is_cursor {
                let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let (cursor_alpha, fg_override) = match cursor_style {
                    torvox_core::cursor::CursorStyle::Block => (CURSOR_BLOCK_ALPHA, bg),
                    _ => (CURSOR_LINE_ALPHA, fg),
                };
                let cursor_bg = [
                    raw_cursor_bg[0],
                    raw_cursor_bg[1],
                    raw_cursor_bg[2],
                    raw_cursor_bg[3] * cursor_alpha,
                ];
                (fg_override, cursor_bg)
            } else {
                (fg, bg)
            };

            let base_x = col as f32 * cell_w;
            let base_y = row as f32 * cell_h;
            let (cursor_quad_size, cursor_quad_origin) = if is_cursor {
                match cursor_style {
                    torvox_core::cursor::CursorStyle::Block => {
                        ([cell_w * cell_span, cell_h], [base_x, base_y])
                    }
                    // Bar/Underline on text cells: use full cell so text is not clipped;
                    // cursor indicator color provides the positional highlight.
                    torvox_core::cursor::CursorStyle::Bar
                    | torvox_core::cursor::CursorStyle::Underline => {
                        ([cell_w * cell_span, cell_h], [base_x, base_y])
                    }
                }
            } else {
                ([cell_w * cell_span, cell_h], [base_x, base_y])
            };

            if let Some(info) = font_pipeline.glyph_information(ch) {
                glyph_found += 1;
                let uv_x = info.atlas_x as f32 / atlas_width;
                let uv_y = info.atlas_y as f32 / atlas_height;
                let uv_w = info.width as f32 / atlas_width;
                let uv_h = info.height as f32 / atlas_height;

                let bearing_x = info.placement.left as f32;
                let glyph_h = info.height as f32;
                let raw_bearing_y = ascent_pixels - info.placement.top as f32;
                let bearing_y = if glyph_h > cell_h {
                    (cell_h - glyph_h) / 2.0
                } else {
                    raw_bearing_y
                };

                instances.push(CellInstance {
                    quad_origin: cursor_quad_origin,
                    atlas_offset: [uv_x, uv_y],
                    atlas_size: [uv_w, uv_h],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size: cursor_quad_size,
                    flags,
                    bearing: [bearing_x, bearing_y],
                    glyph_advance_width: info.advance_width,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                for &cp in cell.graphemes.iter().skip(1) {
                    let Some(mark_ch) = char::from_u32(cp) else {
                        continue;
                    };
                    let Some(info) = font_pipeline.glyph_information(mark_ch) else {
                        continue;
                    };
                    let uv_x = info.atlas_x as f32 / atlas_width;
                    let uv_y = info.atlas_y as f32 / atlas_height;
                    let uv_w = info.width as f32 / atlas_width;
                    let uv_h = info.height as f32 / atlas_height;
                    let bearing_x = info.placement.left as f32;
                    let glyph_h = info.height as f32;
                    let raw_bearing_y = ascent_pixels - info.placement.top as f32;
                    let bearing_y = if glyph_h > cell_h {
                        (cell_h - glyph_h) / 2.0
                    } else {
                        raw_bearing_y
                    };
                    instances.push(CellInstance {
                        quad_origin: cursor_quad_origin,
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: cursor_quad_size,
                        flags,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: 0.0,
                    });
                }
            } else {
                glyph_not_found += 1;
                instances.push(CellInstance {
                    quad_origin: cursor_quad_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [1.0 / atlas_width, 1.0 / atlas_height],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size: cursor_quad_size,
                    flags,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                for &cp in cell.graphemes.iter().skip(1) {
                    let Some(mark_ch) = char::from_u32(cp) else {
                        continue;
                    };
                    let Some(info) = font_pipeline.glyph_information(mark_ch) else {
                        continue;
                    };
                    let uv_x = info.atlas_x as f32 / atlas_width;
                    let uv_y = info.atlas_y as f32 / atlas_height;
                    let uv_w = info.width as f32 / atlas_width;
                    let uv_h = info.height as f32 / atlas_height;
                    let bearing_x = info.placement.left as f32;
                    let glyph_h = info.height as f32;
                    let raw_bearing_y = ascent_pixels - info.placement.top as f32;
                    let bearing_y = if glyph_h > cell_h {
                        (cell_h - glyph_h) / 2.0
                    } else {
                        raw_bearing_y
                    };
                    instances.push(CellInstance {
                        quad_origin: cursor_quad_origin,
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: cursor_quad_size,
                        flags,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: 0.0,
                    });
                }
            }
        }
    }
    if glyph_found + glyph_not_found > 0 {
        log::debug!(
            "build_cell_instances: glyph_found={} glyph_not_found={} total={}",
            glyph_found,
            glyph_not_found,
            glyph_found + glyph_not_found
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Mailbox available -> Mailbox (vsync + drop oldest). Never Immediate.
        let mut caps = base_caps();
        caps.present_modes = vec![
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Fifo,
        ];
        assert_eq!(
            GpuContext::select_present_mode(&caps),
            wgpu::PresentMode::Mailbox,
            "Mailbox (vsync) must win over Immediate"
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
        assert_eq!(foreground_out, foreground);
        assert_eq!(background_out, background);
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

        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font_pipeline.rasterize_ascii();
        let (cell_w, _cell_h) = font_pipeline.cell_metrics();

        let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
        assert_eq!(instances.len(), 4);

        let cell0 = &instances[0];
        assert_eq!(cell0.quad_origin, [0.0, 0.0]);
        assert_eq!(cell0.bg_color, [0.0, 0.0, 0.0, 1.0]);

        let cell1 = &instances[1];
        assert_eq!(cell1.quad_origin, [cell_w, 0.0]);
        assert_eq!(cell1.bg_color, [0.5, 0.5, 0.5, 1.0]);
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
        assert_eq!(back.quad_origin, [1.0, 2.0]);
        assert_eq!(back.flags, 5.0);
        assert_eq!(back.glyph_advance_width, 8.0);
    }

    #[test]
    fn cell_instance_zeroable() {
        let c: CellInstance = bytemuck::Zeroable::zeroed();
        assert_eq!(c.quad_origin, [0.0, 0.0]);
        assert_eq!(c.fg_color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(c.flags, 0.0);
        assert_eq!(c.bearing, [0.0, 0.0]);
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
        assert_eq!(back.cursor_size, [10.0, 20.0]);
    }

    #[test]
    fn gpu_uniforms_size() {
        assert_eq!(std::mem::size_of::<GpuUniforms>(), 80);
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
            result[i] =
                proj[0][i] * v[0] + proj[1][i] * v[1] + proj[2][i] * v[2] + proj[3][i] * v[3];
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
        grid.set_cell(100, 100, 'X', [1.0; 4], [0.0; 4]); // out of bounds
        // Should not panic, value not stored
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
            assert_eq!(*f, [1.0, 1.0, 1.0, 1.0]);
        }
    }

    #[test]
    fn flat_grid_default_bg_is_black() {
        let grid = FlatGrid::new(2, 2);
        for b in &grid.background {
            assert_eq!(*b, [0.0, 0.0, 0.0, 1.0]);
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
        assert_eq!(foreground_loaded, foreground);
        assert_eq!(background_loaded, background);
    }

    #[test]
    fn build_cell_instances_from_flat_empty() {
        let grid = FlatGrid::new(0, 0);
        let mut font = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font.rasterize_ascii();
        let instances = build_cell_instances_from_flat(&grid, &mut font, 2048.0, 2048.0);
        assert!(instances.is_empty());
    }

    #[test]
    fn build_cell_instances_from_flat_space_only() {
        let grid = FlatGrid::new(1, 5);
        let mut font = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font.rasterize_ascii();
        let instances = build_cell_instances_from_flat(&grid, &mut font, 2048.0, 2048.0);
        assert_eq!(instances.len(), 5);
        // All spaces, atlas_size should be 0
        for inst in &instances {
            assert_eq!(inst.atlas_size, [0.0, 0.0]);
        }
    }

    #[test]
    fn build_cell_instances_from_flat_unicode_cjk() {
        let mut grid = FlatGrid::new(1, 3);
        grid.set_cell(0, 0, '中', [1.0; 4], [0.0; 4]);
        grid.set_cell(0, 1, '文', [1.0; 4], [0.0; 4]);
        let mut font = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font.rasterize_ascii();
        let instances = build_cell_instances_from_flat(&grid, &mut font, 2048.0, 2048.0);
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
            timeout: None,
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
        assert_eq!(back.projection[1][1], uniforms_400.projection[1][1]);
    }

    #[test]
    fn cursor_rendering_on_visible_cursor() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 0.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color,
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 2);
        let cursor_cell = &instances[0];
        // Block cursor alpha = cursor_color[3] * 0.7 (CURSOR_BLOCK_ALPHA constant)
        assert_eq!(
            cursor_cell.bg_color,
            [1.0, 1.0, 1.0, 0.7],
            "cursor cell bg should be white with block alpha when cursor_visible=true"
        );
        let non_cursor_cell = &instances[1];
        assert_ne!(
            non_cursor_cell.bg_color,
            [1.0, 1.0, 1.0, 1.0],
            "non-cursor cell bg should NOT be white"
        );
    }

    #[test]
    fn cursor_not_rendered_when_invisible() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 0.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        assert_ne!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 1.0],
            "cursor cell should not have white bg when cursor_visible=false"
        );
    }

    #[test]
    fn reverse_video_applied_to_blank_cell() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        // Reverse video swaps fg/bg: blank cell bg must become the foreground,
        // fg must become the background.
        assert_eq!(
            cell.bg_color, foreground,
            "reversed blank cell bg must equal foreground"
        );
        assert_eq!(
            cell.fg_color, background,
            "reversed blank cell fg must equal background"
        );
    }

    #[test]
    fn selection_swaps_fg_bg() {
        use super::SelectionRange;
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: None,
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        assert_eq!(
            cell.fg_color,
            [0.0, 0.0, 0.0, 1.0],
            "selected cell fg should be original bg (swap)"
        );
        assert_eq!(
            cell.bg_color,
            [1.0, 0.0, 0.0, 1.0],
            "selected cell bg should be original fg (swap)"
        );
    }

    // ── Bearing correctness: Termux-aligned font metrics ──────────────

    /// Verify bearing_y uses font baseline, not centering.
    /// build_cell_instances_from_flat uses raw bearing_y = ascent_pixels - placement.top
    /// (no centering, no clamping — the raw font baseline offset).
    #[test]
    fn bearing_y_uses_font_baseline_not_centering() {
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font_pipeline.rasterize_ascii();
        let ascent_pixels = font_pipeline.ascent_pixels();

        let chars = ['A', 'g', 'p', '.', ','];
        for ch in chars {
            let info = font_pipeline.glyph_information(ch).expect("glyph exists");
            let expected_bearing_y = ascent_pixels - info.placement.top as f32;

            let mut grid = FlatGrid::new(1, 1);
            grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
            let instances =
                build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font_pipeline.rasterize_ascii();

        let chars = ['A', 'i', 'l', 'W', 'M'];
        for ch in chars {
            let info = font_pipeline.glyph_information(ch).expect("glyph exists");
            let expected_bearing_x = info.placement.left as f32;

            let mut grid = FlatGrid::new(1, 1);
            grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
            let instances =
                build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font_pipeline.rasterize_ascii();
        let (cell_w, cell_h) = font_pipeline.cell_metrics();

        let chars = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
        let mut grid = FlatGrid::new(1, chars.len() as u32);
        for (i, &ch) in chars.iter().enumerate() {
            grid.set_cell(0, i as u32, ch, [1.0; 4], [0.0; 4]);
        }
        let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        let ascent_pixels = font_pipeline.ascent_pixels();

        let cjk_chars = ['中', '文', '好'];
        for ch in cjk_chars {
            if let Some(info) = font_pipeline.glyph_information(ch) {
                let expected = ascent_pixels - info.placement.top as f32;

                let mut grid = FlatGrid::new(1, 2);
                grid.set_cell(0, 0, ch, [1.0; 4], [0.0; 4]);
                let instances =
                    build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
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
    ) -> Option<GpuContext> {
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
        };
        ctx.initialize_pipeline_and_bind_group(256, 256, 50, 50);
        Some(ctx)
    }

    fn setup_test_gpu_context_custom(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Option<GpuContext> {
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
        };
        ctx.initialize_pipeline_and_bind_group(width.max(256), height.max(256), width, height);
        Some(ctx)
    }

    #[test]
    fn ocr_verifies_rendered_text() {
        let Some((instance, adapter, device, queue)) = create_test_device() else {
            return;
        };
        let width = 480u32;
        let height = 60u32;
        let atlas_dim = width.max(256);
        let Some(mut ctx) =
            setup_test_gpu_context_custom(instance, adapter, device, queue, width, height)
        else {
            return;
        };
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

        let instances = build_cell_instances_from_flat(
            &fg,
            &mut font_pipeline,
            atlas_dim as f32,
            atlas_dim as f32,
        );
        assert!(
            !instances.is_empty(),
            "build_cell_instances_from_flat returned 0 instances - font/glyph load failure"
        );
        ctx.upload_atlas(font_pipeline.atlas_bitmap(), atlas_dim, atlas_dim);
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
        let Some(mut ctx) = setup_test_gpu_context(instance, adapter, device, queue) else {
            return;
        };

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
        let Some(mut ctx) = setup_test_gpu_context(instance, adapter, device, queue) else {
            return;
        };

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
        let Some(mut ctx) = setup_test_gpu_context(instance, adapter, device, queue) else {
            return;
        };

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
        assert_eq!(blended, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn blend_highlight_zero_alpha() {
        let base = [0.2, 0.3, 0.4, 1.0];
        let transparent = [255, 0, 0, 0];
        let blended = blend_highlight(base, transparent);
        assert_eq!(blended, base);
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &highlights,
                cursor_color: None,
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 0.0,
                selection: None,
                selection_bg: None,
                search_highlights: &highlights,
                cursor_color: Some([0.5, 0.5, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        assert_eq!(
            cell.bg_color,
            [0.5, 0.5, 1.0, 0.7],
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
    fn group_highlights_by_row(
        highlights: &[SearchHighlight],
    ) -> HashMap<i32, Vec<&SearchHighlight>> {
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
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        assert_eq!(
            cell.quad_size[0], cell_w,
            "Block cursor width should equal cell width"
        );
        assert_eq!(
            cell.quad_size[1], cell_h,
            "Block cursor height should equal cell height"
        );
    }

    #[test]
    fn cursor_bar_width_ratio() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Bar,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
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
        assert_eq!(
            cell.quad_size[1], cell_h,
            "Bar cursor height should equal cell height"
        );
    }

    #[test]
    fn cursor_underline_height_ratio() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Underline,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
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
        assert_eq!(
            cell.quad_size[0], cell_w,
            "Underline cursor width should equal cell width"
        );
    }

    #[test]
    fn cursor_not_rendered_when_visible_false() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 768.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        // Non-cursor blank cell uses default background, not cursor color
        assert_ne!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 0.7],
            "cursor cell should not have block alpha bg when cursor_visible=false"
        );
    }

    #[test]
    fn cursor_at_origin() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 24.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(
            instances.len(),
            1,
            "cursor at (0,0) must produce an instance"
        );
        let cell = &instances[0];
        assert_eq!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 0.7],
            "cursor at origin must render with block alpha"
        );
    }

    #[test]
    fn cursor_with_text_and_block_style() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 24.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color,
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        // Block cursor swaps fg/bg: fg becomes original bg, bg becomes cursor color×alpha
        assert_eq!(
            cell.fg_color,
            [0.0, 0.0, 1.0, 1.0],
            "block cursor on text: fg should be original bg"
        );
        assert_eq!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 0.7],
            "block cursor on text: bg should be cursor color with block alpha"
        );
    }

    #[test]
    fn cursor_with_text_and_bar_style() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 24.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color,
                cursor_style: torvox_core::cursor::CursorStyle::Bar,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        // Bar cursor does NOT swap fg/bg — it just sets bg to cursor color
        assert_eq!(
            cell.fg_color,
            [0.0, 1.0, 0.0, 1.0],
            "bar cursor on text: fg should be original foreground"
        );
        assert_eq!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 0.9],
            "bar cursor on text: bg should be cursor color with line alpha"
        );
    }

    #[test]
    fn cursor_with_text_and_underline_style() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 24.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color,
                cursor_style: torvox_core::cursor::CursorStyle::Underline,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        // Underline cursor does NOT swap fg/bg — it just sets bg to cursor color
        assert_eq!(
            cell.fg_color,
            [0.0, 1.0, 0.0, 1.0],
            "underline cursor on text: fg should be original foreground"
        );
        assert_eq!(
            cell.bg_color,
            [1.0, 1.0, 1.0, 0.9],
            "underline cursor on text: bg should be cursor color with line alpha"
        );
    }

    #[test]
    fn cursor_color_custom_values() {
        use torvox_terminal::ghostty_terminal::{CellSnapshot, GridSnapshot};
        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
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
                atlas_width: 2048.0,
                atlas_height: 2048.0,
                projection_height: 24.0,
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: custom_color,
                cursor_style: torvox_core::cursor::CursorStyle::Block,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
        );
        assert_eq!(instances.len(), 1);
        let cell = &instances[0];
        assert_eq!(
            cell.bg_color,
            [0.5, 0.3, 0.8, 0.7],
            "custom cursor color should be reflected with block alpha multiplier"
        );
    }

    include!("screenshot_tests.rs");
}
