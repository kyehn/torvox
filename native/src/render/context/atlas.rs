use super::Renderer;
use super::renderer::create_rgba_texture;

// ── 字形图集与 uniform ──────────────────────────────────────────────
impl Renderer {
    pub fn create_atlas_texture(&mut self, width: u32, height: u32) {
        // 注意：图集尺寸必须钳制到 adapter 的 `max_texture_dimension_2d` 上限
        // （部分 GPU 只报 2048）。当前调用方固定传 1024x1024，安全；
        // 若日后调大，须在此处钳制。
        let texture = create_rgba_texture(
            &self.device,
            "Atlas Texture",
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // 文本图集按 1:1 采样（字形光栅像素 = 屏幕像素，`raster_scale = 密度 * fontScale`）。
        // Nearest 保持字形锐利；Linear 会在纹素边界插值使文字模糊（真机“字体模糊”反馈），
        // 且在软件 Vulkan（Lavapipe/SwiftShader）下省去逐纹素双线性开销。
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
            let (origin_x, origin_y, upload_width, upload_height) = match dirty_rect {
                Some((dirty_x, dirty_y, dirty_width, dirty_height)) => {
                    let clamped_width = dirty_width.min(width);
                    let clamped_height = dirty_height.min(height);
                    (
                        dirty_x.min(width - clamped_width),
                        dirty_y.min(height - clamped_height),
                        clamped_width,
                        clamped_height,
                    )
                }
                None => (0, 0, width, height),
            };
            let offset = (origin_y as u64 * width as u64 + origin_x as u64)
                * crate::render::font::ATLAS_BYTES_PER_PIXEL as u64;
            // 长度下界必须按 wgpu 的**跨步**规则算，而不是「宽 × 高 × BPP」：
            // `bytes_per_row` 恒为整幅宽度 `width`，故末行之前每行都要走满一个跨步。
            // 正确式：offset + (upload_height - 1) * width * BPP + upload_width * BPP。
            // 旧式在 `upload_width < width`（脏矩形比幅面窄）时会低估，于是守卫放过
            // 一段实际过短的数据，最终由 wgpu  panic 收场而不是这里的干净报错。
            let bpp = crate::render::font::ATLAS_BYTES_PER_PIXEL;
            let needed = offset as usize
                + (upload_height as usize).saturating_sub(1) * width as usize * bpp
                + upload_width as usize * bpp;
            if data.len() < needed {
                log::error!(
                    "upload_atlas: data too short ({} < {}), upload_w={upload_width} upload_h={upload_height}",
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
                    bytes_per_row: Some(width * crate::render::font::ATLAS_BYTES_PER_PIXEL as u32),
                    rows_per_image: Some(upload_height),
                },
                wgpu::Extent3d {
                    width: upload_width,
                    height: upload_height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// 构建单元管线的 uniform 块。单点构造，使投影/图集字段在写入、刷新与交换链
    /// 重配置三条路径上始终同步。
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
            _padding: [0.0; 2],
        }
    }

    /// 写入 uniforms 并重建单元绑定组。
    ///
    /// 与 [`Renderer::refresh_cell_uniforms`] 不同，它重建绑定组，供
    /// [`Renderer::initialize_pipeline_and_bind_group`] 独用，避免约 40 行重复的
    /// uniform 构建 + 缓冲写入 + 绑定组创建。
    pub(super) fn write_uniforms(
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

    /// 轻量的逐帧同步：只刷新单元 uniform 缓冲内容。
    /// 与 `write_uniforms` 不同，它绝不重建绑定组——单元绑定组按缓冲对象标识绑定，
    /// 而 wgpu 在绘制时读取缓冲内容，故重写字节即可更新投影。
    pub(crate) fn refresh_cell_uniforms(&mut self, projection_width: f32, projection_height: f32) {
        let Some(buf) = self.cell_uniform_buffer.as_ref() else {
            return;
        };
        let (atlas_width, atlas_height) = self
            .atlas_texture
            .as_ref()
            .map_or((0.0, 0.0), |atlas_texture| {
                (atlas_texture.width() as f32, atlas_texture.height() as f32)
            });
        let uniforms = self.cell_uniforms(
            projection_width,
            projection_height,
            atlas_width,
            atlas_height,
        );
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
    }
}
