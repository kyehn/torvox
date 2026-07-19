//! Surface management — Android ANativeWindow integration and wgpu surface lifecycle.
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use wgpu::util::DeviceExt;

use super::GpuContext;
use super::GpuError;
use super::pipeline::QUAD_CORNERS;

const DESIRED_FRAME_LATENCY: u32 = 2;
const DESIRED_FRAME_LATENCY_ANDROID: u32 = 2;
const GPU_POLL_TIMEOUT: Duration = Duration::from_secs(2);

type CachedSurface = (
    std::sync::Arc<wgpu::Surface<'static>>,
    wgpu::SurfaceConfiguration,
);

pub(crate) static GLOBAL_SURFACE: OnceLock<Mutex<Option<CachedSurface>>> = OnceLock::new();

impl GpuContext {
    pub(crate) fn select_alpha_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::CompositeAlphaMode {
        if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
            wgpu::CompositeAlphaMode::Auto
        } else {
            caps.alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
        }
    }

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

        device.on_uncaptured_error(std::sync::Arc::new(|error| {
            super::log_gpu_error(&error);
        }));

        self.adapter = adapter;
        self.device = device;
        self.queue = queue;

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
        self.surface = Some(std::sync::Arc::new(surface));

        if configure_surface {
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: initial_width,
                height: initial_height,
                present_mode,
                alpha_mode,
                view_formats: Vec::new(),
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

    pub fn release_gpu_surface(&mut self) {
        if self.surface.is_some() {
            let surface = self.surface.take().unwrap();
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

    pub fn configure_android_surface(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<(), GpuError> {
        if self.surface.is_none()
            && let Ok(mut guard) = GLOBAL_SURFACE.get_or_init(|| Mutex::new(None)).lock()
            && let Some((_cached_surface, mut cached_config)) = guard.take()
        {
            cached_config.width = ((width as f32 * super::RENDER_SCALE) as u32).max(1);
            cached_config.height = ((height as f32 * super::RENDER_SCALE) as u32).max(1);
            cached_config.view_formats = Vec::new();
            if let Some(buf) = &self.cell_uniform_buffer {
                let aw = self.atlas_texture.as_ref().map_or(0, |t| t.width());
                let ah = self.atlas_texture.as_ref().map_or(0, |t| t.height());
                let proj = super::orthographic_projection(
                    cached_config.width as f32,
                    cached_config.height as f32,
                );
                let uniforms = super::pipeline::GpuUniforms {
                    projection: proj,
                    atlas_size: [aw as f32, ah as f32],
                    raster_scale: self.raster_scale,
                    image_active: super::pipeline::image_active_value(self.bg_bind_group.is_some()),
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
                "configure_android_surface: {}x{} (reused cached surface, projection updated)",
                cached_config.width,
                cached_config.height,
            );
            return Ok(());
        }

        self.surface = None;
        self.surface_config = None;
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_POLL_TIMEOUT),
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
            width: ((width as f32 * super::RENDER_SCALE) as u32).max(1),
            height: ((height as f32 * super::RENDER_SCALE) as u32).max(1),
            present_mode: Self::select_present_mode(&caps),
            alpha_mode,
            view_formats: Vec::new(),
            desired_maximum_frame_latency: DESIRED_FRAME_LATENCY_ANDROID,
        };
        surface.configure(&self.device, &config);
        self.surface = Some(std::sync::Arc::new(surface));

        self.projection_width = config.width;
        self.projection_height = config.height;

        if let Some(buf) = &self.cell_uniform_buffer {
            let aw = self.atlas_texture.as_ref().map_or(0, |t| t.width());
            let ah = self.atlas_texture.as_ref().map_or(0, |t| t.height());
            let proj = super::orthographic_projection(config.width as f32, config.height as f32);
            let uniforms = super::pipeline::GpuUniforms {
                projection: proj,
                atlas_size: [aw as f32, ah as f32],
                raster_scale: self.raster_scale,
                image_active: super::pipeline::image_active_value(self.bg_bind_group.is_some()),
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
            "configure_android_surface: {}x{} format={:?} alpha={:?} present={:?} (projection updated)",
            config.width,
            config.height,
            format,
            alpha_mode,
            config.present_mode,
        );

        self.surface_config = Some(config);
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn reconfigure_swapchain(&mut self, width: u32, height: u32) {
        let (surface, config) = match (self.surface.as_ref(), self.surface_config.as_mut()) {
            (Some(s), Some(c)) => (s, c),
            _ => return,
        };
        let scaled_width = ((width as f32 * super::RENDER_SCALE) as u32).max(1);
        let scaled_height = ((height as f32 * super::RENDER_SCALE) as u32).max(1);
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
            let proj = super::orthographic_projection(scaled_width as f32, scaled_height as f32);
            let uniforms = super::pipeline::GpuUniforms {
                projection: proj,
                atlas_size: [aw as f32, ah as f32],
                raster_scale: self.raster_scale,
                image_active: super::pipeline::image_active_value(self.bg_bind_group.is_some()),
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

        let proj = super::orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = super::pipeline::GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width as f32, atlas_height as f32],
            raster_scale: self.raster_scale,
            image_active: super::pipeline::image_active_value(self.bg_bind_group.is_some()),
            default_bg: [
                self.bg_color.r as f32,
                self.bg_color.g as f32,
                self.bg_color.b as f32,
                1.0,
            ],
        };

        if self.cell_uniform_buffer.is_none() {
            self.cell_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cell Uniform Buffer"),
                size: std::mem::size_of::<super::pipeline::GpuUniforms>() as u64,
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
}
