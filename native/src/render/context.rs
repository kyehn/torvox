//! GPU context — wgpu instance, adapter, device, and pipeline management.
//!
//! Centralizes all wgpu resources into a single `Renderer` struct.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: attach/detach recreates the wgpu surface and render pipeline
use std::sync::{Mutex, OnceLock};
use wgpu::util::DeviceExt;

use crate::render::pipeline::{DEFAULT_BG_ALPHA, QUAD_CORNERS};
use crate::render::{CATPPUCCIN_MOCHA_BG, GpuError};

pub(crate) fn log_gpu_error(error: &wgpu::Error) {
    log::error!("GPU_UNCAPTURED_ERROR: {error:#?}");
}

/// Per-frame rendering context — bundles encoder, surface texture, and view
/// so these short-lived resources are clearly separated from the long-lived
/// `Renderer` state. Created by `Renderer::begin_frame()`.
pub struct FrameContext {
    pub(crate) encoder: wgpu::CommandEncoder,
    pub(crate) view: wgpu::TextureView,
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) cfg_width: u32,
    pub(crate) cfg_height: u32,
}

impl FrameContext {
    /// Submit the encoded commands to the GPU queue and present the surface texture.
    pub fn submit(self, queue: &wgpu::Queue) {
        queue.submit(std::iter::once(self.encoder.finish()));
        queue.present(self.texture);
    }
}

pub(crate) struct GlobalGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn global_gpu() -> &'static GlobalGpu {
    static INSTANCE: OnceLock<GlobalGpu> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        match futures::executor::block_on(crate::render::wgpu_backend::initialize_wgpu()) {
            Ok((_instance, _adapter, device, queue)) => GlobalGpu { device, queue },
            Err(e) => {
                log::error!("GPU initialization failed: {e}");
                log::error!("Solution: ensure a Vulkan-capable GPU is available.");
                log::error!("  - Linux desktop: set VK_ICD_FILENAMES to a lavapipe or Mesa driver");
                log::error!("  - Android emulator: use SwiftShader (default with GPU emulation)");
                log::error!("  - Physical device: install Vulkan drivers for your hardware");
                log::error!("This is a fatal error — the terminal cannot render without a GPU.");
                panic!(
                    "GPU initialization failed: {e}. \
                     See log for details."
                )
            }
        }
    })
}

/// Central GPU context — owns the wgpu device, queues, pipelines, and all GPU resources.
/// GPU renderer — owns wgpu resources and pipelines.
///
/// Fields are grouped (via blank lines) into:
/// 1. Core wgpu resources (instance, adapter, device, queue, surface) — always
///    created, never `Option` except surface which is acquired from Android.
/// 2. Cell pipeline resources (pipeline, buffers, bind group) — created lazily
///    on first frame, hence `Option`.
/// 3. Atlas resources (texture, view, sampler) — created on first font upload.
/// 4. Background image pipeline (bg_*) — separate from cell pipeline because
///    the shader is different.
/// 5. Kitty graphics protocol (kgp_*) — KGP image display, separate pipeline.
/// 6. Blur pipelines (blur_h/blur_v) — gaussian blur for transparent backgrounds.
/// 7. Frame state (raster_scale, render_paused, pending_gpu_drain) — per-frame
///    transient state.
///
/// # Thread safety
///
/// `Renderer` is **not** `Send` or `Sync` (wgpu types carry that constraint).
/// It is created on the JNI/render thread and accessed from exactly one thread
/// its entire lifetime.  `begin_frame()` and `render_frame()` must be called
/// from the same thread, with `&mut self`.
pub struct Renderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: Option<std::sync::Arc<wgpu::Surface<'static>>>,
    pub(crate) surface_config: Option<wgpu::SurfaceConfiguration>,
    pub(crate) cell_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) quad_vertex_buffer: wgpu::Buffer,
    pub(crate) cell_bind_group: Option<wgpu::BindGroup>,
    pub(crate) cell_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) instance_buffer: Option<wgpu::Buffer>,
    pub(crate) atlas_texture: Option<wgpu::Texture>,
    pub(crate) atlas_view: Option<wgpu::TextureView>,
    pub(crate) atlas_sampler: Option<wgpu::Sampler>,
    pub(crate) pipeline_format: wgpu::TextureFormat,
    pub(crate) projection_width: u32,
    pub(crate) projection_height: u32,
    pub(crate) readback_texture: Option<wgpu::Texture>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
    pub(crate) bg_color: wgpu::Color,
    pub(crate) bg_image_texture: Option<wgpu::Texture>,
    pub(crate) bg_image_view: Option<wgpu::TextureView>,
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

impl Renderer {
    /// Acquire the surface texture and create a FrameContext for this frame.
    /// Returns `None` if acquire fails (lost surface, timeout, or hung GPU).
    pub(crate) fn begin_frame(&mut self) -> Option<FrameContext> {
        // Drain deferred GPU work before acquiring new texture.
        if self.pending_gpu_drain {
            let _ = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_millis(16)),
            });
            self.pending_gpu_drain = false;
        }
        let cfg_width = self.surface_config.as_ref().map(|c| c.width)?;
        let cfg_height = self.surface_config.as_ref().map(|c| c.height)?;

        self.ensure_bg_pipeline(cfg_width, cfg_height);
        self.ensure_kgp_pipeline(cfg_width, cfg_height);

        let surface = self.surface.as_ref()?;
        let output = self.acquire_texture(surface, cfg_width, cfg_height)?;

        let tex_size = output.texture.size();
        let (cfg_width, cfg_height) =
            if tex_size.width != cfg_width || tex_size.height != cfg_height {
                log::warn!(
                    "begin_frame: size mismatch! config={}x{} texture={}x{}",
                    cfg_width,
                    cfg_height,
                    tex_size.width,
                    tex_size.height
                );
                let existing_config = self.surface_config.take()?;
                let new_config = wgpu::SurfaceConfiguration {
                    width: tex_size.width,
                    height: tex_size.height,
                    ..existing_config
                };
                surface.configure(&self.device, &new_config);
                self.surface_config = Some(new_config);

                let aw = self.atlas_texture.as_ref().map_or(0, |t| t.width());
                let ah = self.atlas_texture.as_ref().map_or(0, |t| t.height());
                let proj = crate::render::orthographic_projection(
                    tex_size.width as f32,
                    tex_size.height as f32,
                );
                let uniforms = crate::render::pipeline::GpuUniforms {
                    projection: proj,
                    atlas_size: [aw as f32, ah as f32],
                    raster_scale: self.raster_scale,
                    image_active: crate::render::pipeline::image_active_value(
                        self.bg_bind_group.is_some(),
                    ),
                    default_bg: [
                        self.bg_color.r as f32,
                        self.bg_color.g as f32,
                        self.bg_color.b as f32,
                        1.0,
                    ],
                };
                if let Some(buf) = &self.cell_uniform_buffer {
                    self.queue
                        .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
                }
                (tex_size.width, tex_size.height)
            } else {
                (cfg_width, cfg_height)
            };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        Some(FrameContext {
            encoder,
            view,
            texture: output,
            cfg_width,
            cfg_height,
        })
    }
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
    /// Shared initialization for all constructors.
    pub(crate) fn new_inner(
        device: wgpu::Device,
        queue: wgpu::Queue,
        quad_vertex_buffer: wgpu::Buffer,
    ) -> Self {
        Self {
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

    /// Create a new Renderer with full async initialization.
    pub async fn new() -> Result<Self, GpuError> {
        let (_instance, _adapter, device, queue) =
            crate::render::wgpu_backend::initialize_wgpu().await?;
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self::new_inner(device, queue, quad_vertex_buffer))
    }

    /// Create a Renderer sharing the global wgpu instance/adapter/device.
    /// Useful for headless contexts (e.g., tests, screenshot capture).
    pub fn new_with_no_surface() -> Self {
        let gpu = global_gpu();
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self::new_inner(device, queue, quad_vertex_buffer)
    }

    /// Set the surface configuration used by headless/off-screen tests.
    ///
    /// The render pass uses `surface_config` as the frame dimensions and
    /// format when no real window surface is attached. Exposed publicly for
    /// integration tests that drive the renderer without an Android window.
    pub fn set_surface_config(&mut self, config: wgpu::SurfaceConfiguration) {
        self.surface_config = Some(config);
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

// ── Inlined from atlas.rs ─────────────────────────────────────────
pub const MIN_ATLAS_BUFFER_SIZE: u64 = 64;

impl Renderer {
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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

    pub fn upload_atlas(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        dirty_rect: Option<(u32, u32, u32, u32)>,
    ) {
        if let Some(texture) = &self.atlas_texture {
            let (origin_x, origin_y, upload_w, upload_h) = match dirty_rect {
                Some((x, y, w, h)) => {
                    let w = w.min(width);
                    let h = h.min(height);
                    (x.min(width - w), y.min(height - h), w, h)
                }
                None => (0, 0, width, height),
            };
            let offset = (origin_y as u64 * width as u64 + origin_x as u64) * 4;
            let needed = offset as usize + upload_h as usize * upload_w as usize * 4;
            if data.len() < needed {
                log::error!(
                    "upload_atlas: data too short ({} < {}), upload_w={upload_w} upload_h={upload_h}",
                    data.len(),
                    needed
                );
                return;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: origin_x,
                        y: origin_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(upload_h),
                },
                wgpu::Extent3d {
                    width: upload_w,
                    height: upload_h,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Write uniforms and rebuild the cell bind group.
    ///
    /// Shared by [`update_bind_group`] and [`initialize_pipeline_and_bind_group`]
    /// to eliminate ~40 lines of identical uniform construction + buffer write
    /// + bind group creation.
    fn write_uniforms(
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
                size: std::mem::size_of::<crate::render::pipeline::GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let proj = crate::render::orthographic_projection(projection_width, projection_height);
        let uniforms = crate::render::pipeline::GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width, atlas_height],
            raster_scale: self.raster_scale,
            image_active: crate::render::pipeline::image_active_value(self.bg_bind_group.is_some()),
            default_bg: [
                self.bg_color.r as f32,
                self.bg_color.g as f32,
                self.bg_color.b as f32,
                1.0,
            ],
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

    pub fn update_bind_group(
        &mut self,
        atlas_width: f32,
        atlas_height: f32,
        projection_width: f32,
        projection_height: f32,
    ) {
        self.write_uniforms(
            atlas_width,
            atlas_height,
            projection_width,
            projection_height,
        );
    }
}

// ── Inlined from surface.rs ───────────────────────────────────────
type CachedSurface = (
    std::sync::Arc<wgpu::Surface<'static>>,
    wgpu::SurfaceConfiguration,
);

pub(crate) static GLOBAL_SURFACE: std::sync::OnceLock<std::sync::Mutex<Option<CachedSurface>>> =
    std::sync::OnceLock::new();

impl Renderer {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn select_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
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

    /// Release the current GPU surface, caching it for potential reuse.
    pub fn release_gpu_surface(&mut self) {
        if self.surface.is_some() {
            let surface = self
                .surface
                .take()
                .expect("surface confirmed Some by is_some guard");
            let config = self.surface_config.take();
            if let Some(config) = config
                && let Ok(mut guard) = GLOBAL_SURFACE.get_or_init(|| Mutex::new(None)).lock()
            {
                *guard = Some((surface, config));
            }
        }
        self.surface_config = None;
        // Mark that we need to drain GPU work before the next frame.
        // The poll is deferred to avoid blocking session switches.
        self.pending_gpu_drain = true;
    }

    pub fn clear_global_surface() {
        if let Ok(mut guard) = GLOBAL_SURFACE.get_or_init(|| Mutex::new(None)).lock() {
            *guard = None;
        }
    }

    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    pub fn has_pipeline(&self) -> bool {
        self.cell_pipeline.is_some()
    }

    #[cfg(target_os = "android")]
    pub fn reconfigure_swapchain(&mut self, width: u32, height: u32) {
        let (surface, config) = match (self.surface.as_ref(), self.surface_config.as_mut()) {
            (Some(s), Some(c)) => (s, c),
            _ => return,
        };
        let scaled_width = ((width as f32 * crate::render::RENDER_SCALE) as u32).max(1);
        let scaled_height = ((height as f32 * crate::render::RENDER_SCALE) as u32).max(1);
        if config.width == scaled_width && config.height == scaled_height {
            return;
        }
        config.width = scaled_width;
        config.height = scaled_height;
        surface.configure(&self.device, config);

        self.projection_width = scaled_width;
        self.projection_height = scaled_height;

        if let Some(buf) = &self.cell_uniform_buffer {
            let aw = self.atlas_texture.as_ref().map_or(0, |t| t.width());
            let ah = self.atlas_texture.as_ref().map_or(0, |t| t.height());
            let proj =
                crate::render::orthographic_projection(scaled_width as f32, scaled_height as f32);
            let uniforms = crate::render::pipeline::GpuUniforms {
                projection: proj,
                atlas_size: [aw as f32, ah as f32],
                raster_scale: self.raster_scale,
                image_active: crate::render::pipeline::image_active_value(
                    self.bg_bind_group.is_some(),
                ),
                default_bg: [
                    self.bg_color.r as f32,
                    self.bg_color.g as f32,
                    self.bg_color.b as f32,
                    1.0,
                ],
            };
            self.queue
                .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
        }

        log::info!(
            "RECONFIGURE_SWAPCHAIN: {}x{} (projection updated)",
            width,
            height
        );
    }

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

        self.write_uniforms(
            atlas_width as f32,
            atlas_height as f32,
            surface_width as f32,
            surface_height as f32,
        );

        log::info!(
            "initialize_pipeline_and_bind_group: pipeline={} atlas={}x{} surf={}x{}",
            self.cell_pipeline.is_some(),
            atlas_width,
            atlas_height,
            surface_width,
            surface_height,
        );
    }
}
