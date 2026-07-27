//! GPU context — wgpu instance, adapter, device, and pipeline management.
//!
//! Centralizes all wgpu resources into a single `Renderer` struct.
use std::sync::{Arc, OnceLock};
use wgpu::util::DeviceExt;

use crate::render::pipeline::{DEFAULT_BG_ALPHA, QUAD_CORNERS};
use crate::render::{CATPPUCCIN_MOCHA_BG, GpuError};

pub(crate) fn log_gpu_error(error: &wgpu::Error) {
    log::error!("GPU_UNCAPTURED_ERROR: {error:#?}");
}

struct GlobalGpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn global_gpu() -> &'static GlobalGpu {
    static INSTANCE: OnceLock<GlobalGpu> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        match futures::executor::block_on(Renderer::initialize_instance_adapter_device()) {
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

/// Central GPU context — owns the wgpu device, queues, pipelines, and all GPU resources.
pub struct Renderer {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<std::sync::Arc<wgpu::Surface<'static>>>,
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
    pub(crate) projection_width: u32,
    pub(crate) projection_height: u32,
    pub(crate) readback_texture: Option<wgpu::Texture>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
    pub(crate) bg_color: wgpu::Color,
    pub bg_image_texture: Option<wgpu::Texture>,
    pub bg_image_view: Option<wgpu::TextureView>,
    pub(crate) bg_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) bg_bind_group_layout: Option<wgpu::BindGroupLayout>,
    pub(crate) bg_bind_group: Option<wgpu::BindGroup>,
    pub(crate) bg_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) bg_sampler: Option<wgpu::Sampler>,
    pub(crate) bg_blur_radius: f32,
    pub(crate) bg_alpha: f32,
    pub(crate) kgp_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) kgp_bind_group_layout: Option<wgpu::BindGroupLayout>,
    pub(crate) kgp_bind_group: Option<wgpu::BindGroup>,
    pub(crate) kgp_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_sampler: Option<wgpu::Sampler>,
    pub(crate) kgp_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_texture: Option<wgpu::Texture>,
    pub(crate) kgp_atlas_data: Vec<u8>,
    pub(crate) kgp_atlas_width: u32,
    pub(crate) kgp_atlas_height: u32,
    pub(crate) raster_scale: f32,
    pub(crate) blur_h_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) blur_v_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) render_paused: bool,
    pub(crate) pending_gpu_drain: bool,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.cell_bind_group = None;
        self.instance_buffer = None;
        self.cell_pipeline = None;
        self.cell_uniform_buffer = None;
        self.bg_bind_group_layout = None;
        self.bg_uniform_buffer = None;
        self.bg_bind_group = None;
        self.bg_sampler = None;
        self.blur_h_pipeline = None;
        self.blur_v_pipeline = None;
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

impl Renderer {
    /// Initialize wgpu instance, adapter, and device asynchronously.
    pub async fn initialize_instance_adapter_device()
    -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
        #[cfg(target_os = "android")]
        let backends = wgpu::Backends::VULKAN;
        #[cfg(not(target_os = "android"))]
        let backends = wgpu::Backends::PRIMARY;
        #[cfg(debug_assertions)]
        let instance_flags = wgpu::InstanceFlags::VALIDATION
            | wgpu::InstanceFlags::DEBUG
            | wgpu::InstanceFlags::DISCARD_HAL_LABELS;
        #[cfg(not(debug_assertions))]
        let instance_flags = wgpu::InstanceFlags::DISCARD_HAL_LABELS;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags: instance_flags,
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        crate::render::renderdoc_capture::initialize();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
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
            label: Some("Device"),
            #[cfg(debug_assertions)]
            required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            #[cfg(not(debug_assertions))]
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

    /// Create a new Renderer with full async initialization.
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
            raster_scale: 1.0,
            blur_h_pipeline: None,
            blur_v_pipeline: None,
            render_paused: false,
            pending_gpu_drain: false,
        })
    }

    /// Create a Renderer sharing the global wgpu instance/adapter/device.
    /// Useful for headless contexts (e.g., tests, screenshot capture).
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
            raster_scale: 1.0,
            blur_h_pipeline: None,
            blur_v_pipeline: None,
            render_paused: false,
            pending_gpu_drain: false,
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

    pub fn set_render_paused(&mut self, paused: bool) {
        self.render_paused = paused;
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        if scale > 0.0 && scale.is_finite() {
            self.raster_scale = scale;
        }
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
}

/// Create an orthographic projection matrix for the given viewport dimensions.
pub fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}
