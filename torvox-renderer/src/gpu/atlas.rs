//! Texture atlas management — glyph packing and GPU texture allocation.
use super::GpuContext;

pub const MIN_ATLAS_BUFFER_SIZE: u64 = 64;

impl GpuContext {
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
            let offset = (origin_y * width + origin_x) as u64 * 4;
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
                size: std::mem::size_of::<super::pipeline::GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let proj = super::orthographic_projection(projection_width, projection_height);

        let uniforms = super::pipeline::GpuUniforms {
            projection: proj,
            atlas_size: [atlas_width, atlas_height],
            raster_scale: self.raster_scale,
            image_active: super::pipeline::image_active_value(self.bg_bind_group.is_some()),
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
}
