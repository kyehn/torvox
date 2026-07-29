//! Surface management — Android ANativeWindow integration and wgpu surface lifecycle.
use std::sync::{Mutex, OnceLock};

use crate::render::Renderer;

type CachedSurface = (
    std::sync::Arc<wgpu::Surface<'static>>,
    wgpu::SurfaceConfiguration,
);

pub(crate) static GLOBAL_SURFACE: OnceLock<Mutex<Option<CachedSurface>>> = OnceLock::new();

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

        let proj =
            crate::render::orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = crate::render::pipeline::GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width as f32, atlas_height as f32],
            raster_scale: self.raster_scale,
            image_active: crate::render::pipeline::image_active_value(self.bg_bind_group.is_some()),
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
                size: std::mem::size_of::<crate::render::pipeline::GpuUniforms>() as u64,
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
