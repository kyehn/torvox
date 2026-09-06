//! GPU context — wgpu instance, adapter, device, and pipeline management.
//!
//! Centralizes all wgpu resources into a single `Renderer` struct.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: attach/detach recreates the wgpu surface and render pipeline
use parking_lot::Mutex;
use std::sync::OnceLock;
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
    /// wgpu instance — only stored for `attach_surface` (Android).
    #[cfg(target_os = "android")]
    pub(crate) instance: wgpu::Instance,
    /// Adapter — only stored for surface capability queries (Android).
    #[cfg(target_os = "android")]
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
}

/// Test-only access to the shared wgpu instance/adapter (used by
/// render/tests.rs helpers to construct Renderers).
#[cfg(test)]
pub(crate) fn global_gpu_for_tests() -> &'static GlobalGpu {
    global_gpu()
}

fn global_gpu() -> &'static GlobalGpu {
    static INSTANCE: OnceLock<GlobalGpu> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        match futures::executor::block_on(crate::render::wgpu_backend::initialize_wgpu()) {
            Ok((_inst, _adapt, device, queue)) => GlobalGpu {
                #[cfg(target_os = "android")]
                instance: _inst,
                #[cfg(target_os = "android")]
                adapter: _adapt,
                device,
                queue,
            },
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
    /// CPU-side instance buffer reused across frames (avoids a ~100KB
    /// allocation per frame; see build_instances_from_cell_data).
    pub(crate) cpu_instances: Vec<crate::render::CellInstance>,
    /// Row-dirty instance cache (FR-013 / NFR-010): retains per-row
    /// instance slices so clean rows are copied instead of rebuilt.
    pub(crate) cell_cache: Option<crate::render::cell_builder::CachedInstances>,
    /// Reusable all-true dirty mask (length = current grid rows) for the
    /// frame where `cell_cache` was just rebuilt from scratch (resize):
    /// serving "clean" rows from an empty cache would drop them
    /// regression fix).
    pub(crate) cell_full_mask_cache: Vec<bool>,
    pub(crate) flash_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) flash_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) flash_bind_group: Option<wgpu::BindGroup>,
    /// Bell-flash overlay phase in 0..=1 (0 = off). The Kotlin side
    /// drives the decay animation; the renderer only composites a white
    /// full-screen quad at `BELL_FLASH_ALPHA_255/255 * phase`.
    pub(crate) flash_phase: f32,
    /// Viewport Y pixel offset for per-pixel smooth scrolling (positive =
    /// content moves down). Driven by the Kotlin gesture remainder via
    /// `set_viewport_scroll_px`; consumed by `cell_uniforms` as a
    /// projection translation. Always within one row height in practice
    /// (the row channel carries whole rows); 0 = aligned.
    pub(crate) viewport_scroll_px: f32,
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
    /// Source background image size in pixels (set by `set_bg_image`);
    /// consumed by the bg uniforms for the cover (center-crop) mapping.
    pub(crate) bg_image_size: [f32; 2],
    pub(crate) bg_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) bg_bind_group_layout: Option<wgpu::BindGroupLayout>,
    pub(crate) bg_bind_group: Option<wgpu::BindGroup>,
    pub(crate) bg_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) bg_sampler: Option<wgpu::Sampler>,
    pub(crate) bg_blur_radius: f32,
    pub(crate) bg_alpha: f32,
    /// Intermediate texture for the two-pass blur: the H pass renders into
    /// it, the V pass samples it: previously both passes wrote
    /// the surface and the V pass sampled the original image, so the H
    /// pass was overwritten — blur was effectively vertical-only).
    pub(crate) bg_blur_texture: Option<wgpu::Texture>,
    pub(crate) bg_blur_texture_view: Option<wgpu::TextureView>,
    /// Bind group for the V pass: same uniforms/sampler, but binding 1
    /// points at the H-pass intermediate texture instead of the source.
    pub(crate) bg_blur_bind_group: Option<wgpu::BindGroup>,
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
    /// Persistent offscreen frame accumulator (render-vulkan-performance):
    /// authoritative frame content that survives across frames so partial
    /// (dirty-band) frames can composite onto previous output. Presented
    /// to the swapchain with one `copy_texture_to_texture` per frame.
    pub(crate) frame_texture: Option<wgpu::Texture>,
    /// True when the accumulator's content is stale/unknown and the next
    /// frame MUST be a full redraw (first frame, resize, format change,
    /// surface re-attach). Cleared after a successful full frame.
    pub(crate) frame_invalidated: bool,
    /// Whether the swapchain supports `COPY_DST` (checked against
    /// `supported_usage_flags` at attach time). When false, the legacy
    /// direct-to-swapchain path is used and dirty bands are ignored.
    pub(crate) swapchain_copy_supported: bool,
}

impl Renderer {
    /// Get (or recreate) the persistent frame accumulator view. Returns
    /// `None` when accumulation is unsupported or texture creation fails.
    /// A size/format mismatch recreates the texture AND flags the next
    /// frame as a full redraw (`frame_invalidated`).
    pub(crate) fn ensure_frame_texture(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Option<wgpu::TextureView> {
        if !self.swapchain_copy_supported {
            return None;
        }
        let needs_new = match self.frame_texture.as_ref() {
            Some(t) => t.width() != width || t.height() != height || t.format() != format,
            None => true,
        };
        if needs_new {
            log::info!("ensure_frame_texture: creating accumulator {width}x{height} ({format:?})");
            self.frame_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Frame Accumulator"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
            // Fresh texture has undefined content: the next frame must be
            // a full redraw.
            self.frame_invalidated = true;
        }
        self.frame_texture
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

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
                (tex_size.width, tex_size.height)
            } else {
                (cfg_width, cfg_height)
            };

        // Every frame, sync the cell uniforms' `image_active` flag with the
        // current bg bind group: `ensure_bg_pipeline` above may have just
        // (re)built it after setBackgroundImage/clearBackgroundImage, and
        // the cell shader must know whether default-background cells should
        // be transparent so the wallpaper shows through. Only the uniform
        // buffer content is rewritten — the bind group is bound by object
        // identity and stays valid, emulator-verified: wallpaper
        // was drawn every frame but opaque cell backgrounds covered it).
        self.refresh_cell_uniforms(cfg_width as f32, cfg_height as f32);

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
        self.frame_texture = None;
        self.cell_bind_group = None;
        self.instance_buffer = None;
        self.cell_pipeline = None;
        self.cell_uniform_buffer = None;
        self.bg_bind_group_layout = None;
        self.bg_uniform_buffer = None;
        self.bg_bind_group = None;
        self.bg_sampler = None;
        self.bg_blur_texture = None;
        self.bg_blur_texture_view = None;
        self.bg_blur_bind_group = None;
        self.blur_h_pipeline = None;
        self.blur_v_pipeline = None;
        self.bg_pipeline = None;
        self.bg_image_view = None;
        self.bg_image_texture = None;
        self.flash_pipeline = None;
        self.flash_uniform_buffer = None;
        self.flash_bind_group = None;
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
            cpu_instances: Vec::new(),
            cell_cache: None,
            cell_full_mask_cache: Vec::new(),
            flash_pipeline: None,
            flash_uniform_buffer: None,
            flash_bind_group: None,
            flash_phase: 0.0,
            viewport_scroll_px: 0.0,
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
            bg_image_size: [0.0; 2],
            bg_pipeline: None,
            bg_bind_group_layout: None,
            bg_bind_group: None,
            bg_uniform_buffer: None,
            bg_sampler: None,
            bg_blur_radius: 0.0,
            bg_alpha: DEFAULT_BG_ALPHA,
            bg_blur_texture: None,
            bg_blur_texture_view: None,
            bg_blur_bind_group: None,
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
            frame_texture: None,
            frame_invalidated: true,
            swapchain_copy_supported: false,
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

    /// Attach an Android `ANativeWindow` as the render surface (ADR-0007).
    ///
    /// Builds a wgpu surface from the native window handle and (re)creates
    /// the surface configuration at the given dimensions. The caller
    /// guarantees `ptr` is a valid `ANativeWindow*` that stays alive until
    /// [`Renderer::release_surface`] (or drop) — ownership is transferred
    /// to wgpu, which holds its own reference via `Surface::from_...`.
    ///
    /// Reference (zelland + wgpu-in-app): the surface lifecycle is driven by
    /// the Android SurfaceHolder.Callback (attach/release here), never by the
    /// Activity lifecycle; sizes are re-queried every frame; after attach the
    /// renderer must render immediately (zelland WGPU_FIXES.md Fix 2), and
    /// acquire failures (Timeout/Outdated/Lost) must reconfigure + retry.
    #[cfg(target_os = "android")]
    pub fn attach_surface(
        &mut self,
        ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<(), GpuError> {
        use raw_window_handle::{AndroidNdkWindowHandle, RawWindowHandle};
        let non_null = std::ptr::NonNull::new(ptr).ok_or_else(|| {
            GpuError::Surface("attach_surface: null ANativeWindow pointer".into())
        })?;
        // Fast path: if a surface is already attached (IME settle, HOME→recents with retained
        // Surface), reconfigure the live swapchain in place instead of dropping + recreating
        // (spec ime-smooth + app-switch-continuity). Recreation races the render thread and
        // fails with ERROR_NATIVE_WINDOW_IN_USE_KHR on SwiftShader. Reconfigure is zero-copy.
        if self.surface.is_some() && self.surface_config.is_some() {
            self.reconfigure_swapchain(width, height);
            log::info!("attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)");
            return Ok(());
        }
        let handle = AndroidNdkWindowHandle::new(non_null.cast());
        // SAFETY:
        // - `global_gpu().instance` is a valid wgpu Instance;
        // - the handle wraps a caller-guaranteed live ANativeWindow (JNI
        //   attachWindow contract) that stays valid until detachWindow
        //   drops the resulting surface — wgpu takes its own reference at
        //   creation, and the caller releases theirs right after this call;
        // - display handle is None on Android (no X11/Wayland display).
        let surface = unsafe {
            global_gpu()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    // Must match the instance display (wgpu_backend sets
                    // AndroidDisplayHandle at instance creation); wgpu-core rejects
                    // a None display when the instance carries one.
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                        raw_window_handle::AndroidDisplayHandle::new(),
                    )),
                    raw_window_handle: RawWindowHandle::AndroidNdk(handle),
                })
        }
        .map_err(|error| {
            GpuError::Surface(format!(
                "attach_surface: wgpu create_surface failed: {error}"
            ))
        })?;
        // Release the previous surface BEFORE creating the new one: on
        // Android both wgpu surfaces wrap the same ANativeWindow, and the
        // GL backend (SwiftShader-on-emulator, ADR-0007) cannot create a
        // second EGLSurface on a window whose previous surface is still
        // live — get_current_texture then fails with "Surface is not
        // configured for presentation" forever, emulator-
        // verified: every session after the first rendered black; the
        // first attach worked only because no surface existed yet).
        // Callers guarantee no render thread is mid-frame (switchSession
        // stops the old thread before/around this), so dropping here is
        // safe.
        self.surface = None;
        self.surface_config = None;
        // Accumulator content belongs to the old surface; force a full
        // redraw after re-attach.
        self.frame_texture = None;
        self.frame_invalidated = true;
        let surface = std::sync::Arc::new(surface);
        // The default config picks a present mode + format the driver
        // supports; RENDER_SCALE mirrors reconfigure_swapchain (a fixed
        // scale used across the renderer for consistent cell metrics).
        let scaled_width = ((width as f32 * crate::render::RENDER_SCALE) as u32).max(1);
        let scaled_height = ((height as f32 * crate::render::RENDER_SCALE) as u32).max(1);
        let caps = surface.get_capabilities(&global_gpu().adapter);
        // Dirty-band compositing needs the swapchain texture to accept a
        // copy from the frame accumulator. Universally supported on
        // Vulkan; if an exotic driver omits it we fall back to legacy
        // direct-to-swapchain rendering (full redraws only).
        // (wgpu 30 renamed `supported_usage_flags` → `usages`.)
        let swapchain_copy_supported = caps.usages.contains(wgpu::TextureUsages::COPY_DST);
        self.swapchain_copy_supported = swapchain_copy_supported;
        let usage = if swapchain_copy_supported {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        // Prefer the non-sRGB variant: Android SurfaceFlinger defaults to
        // RGBA_8888 (non-sRGB); SwiftShader-on-emulator buffer queues fail
        // to allocate (dequeueBuffer timeout) when the swapchain format
        // does not match the Surface's native format.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| {
                GpuError::Surface("attach_surface: no supported surface formats".into())
            })?;
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            width: scaled_width,
            height: scaled_height,
            present_mode: Self::select_present_mode(&caps),
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            // Three buffers (max_frame_latency=3): with FIFO vsync and a
            // single buffer the render thread blocks in acquire until the
            // display consumes the previous frame, so any frame that takes
            // longer than one vsync period stalls the whole pipeline and
            // frame times alias to vsync multiples (measured 20-30 fps on a
            // Mali device). With three buffers acquire returns immediately,
            // the render pipeline decouples from the display scanout, and
            // Mailbox (when the driver advertises it, selected above) drops
            // the oldest queued frame instead of backpressuring the render
            // thread — the right trade-off for a scrolling terminal where
            // the newest frame always wins.
            desired_maximum_frame_latency: 3,
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        // view_formats is deliberately empty on Android: the platform lacks
        // the SURFACE_VIEW_FORMATS downlevel flag, so any non-empty list
        // fails configure (wgpu-in-app app-surface/src/lib.rs:315-350 has the
        // full platform matrix — webgl/Android empty, desktop srgb±; format ==
        // view_formats is also ignored by configure per the spec). Verified on
        // the API-35 emulator: Rgba8Unorm + empty view_formats renders.
        // wgpu 30's configure returns (errors surface asynchronously via
        // get_current_texture's Lost state), so there is no Result to
        // propagate; the acquire path already reconfigure+retries on Lost.
        surface.configure(&self.device, &config);
        self.surface_config = Some(config);
        // Compare against the PREVIOUS pipeline format (the field is
        // updated below): a format change on re-attach must drop the
        // lazily-created cell pipeline so the next render rebuilds it with
        // the new surface's format (the lazy path in ffi.rs render_inner
        // recreates pipeline + bind group together).
        let previous_format = self.pipeline_format;
        self.pipeline_format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);
        if previous_format != self.pipeline_format {
            self.cell_pipeline = None;
            self.cell_bind_group = None;
            log::info!(
                "attach_surface: surface format changed {previous_format:?} -> {:?}, cell pipeline scheduled for rebuild",
                self.pipeline_format,
            );
        }
        self.projection_width = scaled_width;
        self.projection_height = scaled_height;
        self.surface = Some(surface);
        log::info!("attach_surface: configured {scaled_width}x{scaled_height}");
        Ok(())
    }

    /// Drop the attached surface (Android detach path).
    #[cfg(target_os = "android")]
    pub fn release_surface(&mut self) {
        self.surface = None;
        self.surface_config = None;
        self.frame_texture = None;
        self.frame_invalidated = true;
        log::info!("release_surface: surface dropped");
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
        // The clear color only lands on full frames (margins outside the
        // grid quads are never repainted by partial frames) — force the
        // next frame to be full so theme switches repaint everywhere.
        self.frame_invalidated = true;
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
        // Cap at 10: kernel taps = 2*ceil(r)+1 per pass;
        // 20 → 82 taps/frame is not interactive on Mali-class GPUs.
        self.bg_blur_radius = blur_radius.clamp(0.0, 10.0);
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
            // Textures sampled in shaders are independent of the render-target
            // (swapchain) format, so a fixed Rgba8Unorm is fine for both the
            // background image and the glyph atlas. Reference: zelland
            // WGPU_FIXES.md Fix 1 documents that glyphon's TextAtlas must
            // match the surface format ONLY because glyphon renders into its
            // atlas with the pipeline's render-pass format; direct shader
            // sampling (our path) has no such constraint.
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
        self.bg_image_size = [width as f32, height as f32];
        self.bg_bind_group = None;
    }

    pub fn clear_bg_image(&mut self) {
        self.bg_image_view = None;
        self.bg_image_texture = None;
        self.bg_image_size = [0.0; 2];
        self.bg_bind_group = None;
    }

    /// Set the bell-flash overlay phase (0.0 = off, 1.0 = peak flash).
    /// The Kotlin side drives the decay animation; the renderer only
    /// composites a white full-screen quad at an alpha proportional to
    /// `phase` (see `BELL_FLASH_ALPHA_255`). Any-thread entry point; the
    /// value is stored and consumed on the render thread.
    pub fn set_flash_phase(&mut self, phase: f32) {
        self.flash_phase = phase.max(0.0);
    }

    /// Set the viewport Y pixel offset for per-pixel smooth scrolling.
    /// Non-finite input is ignored; the Kotlin side keeps the value
    /// within one row height (whole rows travel the row channel).
    pub fn set_viewport_scroll_px(&mut self, px: f32) {
        if px.is_finite() {
            self.viewport_scroll_px = px;
        }
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

/// Translate an orthographic projection by a viewport Y pixel offset
/// (positive = content moves down). Screen Y grows downward while NDC Y
/// grows upward, so the offset subtracts from the translation row.
/// Zero height or zero offset leaves the matrix untouched.
pub fn apply_scroll_px_offset(
    mut proj: [[f32; 4]; 4],
    scroll_px: f32,
    height: f32,
) -> [[f32; 4]; 4] {
    if height > 0.0 && scroll_px != 0.0 {
        proj[3][1] -= scroll_px * 2.0 / height;
    }
    proj
}

// ── Inlined from atlas.rs ─────────────────────────────────────────
pub const MIN_ATLAS_BUFFER_SIZE: u64 = 64;

impl Renderer {
    pub fn create_atlas_texture(&mut self, width: u32, height: u32) {
        // NOTE: atlas size must be clamped to the adapter's
        // max_texture_dimension_2d limit (some GPUs report only 2048;
        // zelland WGPU_FIXES.md pitfall #9). Callers currently pass a fixed
        // 1024x1024 atlas, which is safe; if this ever grows, clamp here.
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
        // Text atlas is sampled 1:1 (glyph raster pixels == screen pixels via
        // raster_scale = density * fontScale). Nearest keeps glyphs sharp;
        // Linear interpolates across texel borders and renders text blurry —
        // the "font blur" reports on real devices. This also skips per-texel
        // bilinear filtering cost in software Vulkan (Lavapipe/SwiftShader).
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
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

    /// Build the cell-pipeline uniform block for the given projection and
    /// atlas dimensions. Single construction site so projection/atlas/
    /// image-active/background fields stay in lockstep across the write,
    /// refresh, and swapchain-reconfigure paths.
    pub(crate) fn cell_uniforms(
        &self,
        projection_width: f32,
        projection_height: f32,
        atlas_width: f32,
        atlas_height: f32,
    ) -> crate::render::pipeline::GpuUniforms {
        let proj = crate::render::apply_scroll_px_offset(
            crate::render::orthographic_projection(projection_width, projection_height),
            self.viewport_scroll_px,
            projection_height,
        );
        crate::render::pipeline::GpuUniforms {
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

        let uniforms = self.cell_uniforms(
            projection_width,
            projection_height,
            atlas_width,
            atlas_height,
        );

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

    /// Lightweight per-frame sync of the cell uniform buffer contents only.
    /// Unlike `write_uniforms`, it never recreates the bind group: the cell
    /// bind group is bound by buffer object identity, and wgpu reads the
    /// buffer contents at draw time, so rewriting the bytes is sufficient
    /// to flip `image_active` (wallpaper visibility) and the projection.
    pub(crate) fn refresh_cell_uniforms(&mut self, projection_width: f32, projection_height: f32) {
        let Some(buf) = self.cell_uniform_buffer.as_ref() else {
            return;
        };
        let (aw, ah) = self
            .atlas_texture
            .as_ref()
            .map_or((0.0, 0.0), |t| (t.width() as f32, t.height() as f32));
        let uniforms = self.cell_uniforms(projection_width, projection_height, aw, ah);
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
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

pub(crate) static GLOBAL_SURFACE: OnceLock<parking_lot::Mutex<Option<CachedSurface>>> =
    std::sync::OnceLock::new();

impl Renderer {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn select_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
        // 120fps+ requires decoupling from display vsync: fps and Hz are
        // unrelated. Prefer Immediate (no vsync, no backpressure) when
        // available so the pipeline can sustain 120+ present calls per
        // second even on 60Hz panels (tearing preferred over stalling).
        // Mailbox/Fifo would cap at display Hz and throttle the terminal.
        if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
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
                && let mut guard = GLOBAL_SURFACE.get_or_init(|| Mutex::new(None)).lock()
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
        let mut guard = GLOBAL_SURFACE.get_or_init(|| Mutex::new(None)).lock();
        {
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
            let uniforms = self.cell_uniforms(
                scaled_width as f32,
                scaled_height as f32,
                aw as f32,
                ah as f32,
            );
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

#[cfg(test)]
mod send_check {
    #[test]
    fn renderer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<super::Renderer>();
        fn assert_sync<T: Sync>() {}
        assert_sync::<super::Renderer>();
    }
}
