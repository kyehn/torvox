use super::atlas::MIN_ATLAS_BUFFER_SIZE;
use super::pipeline::QUAD_VERTEX_COUNT;
use super::GpuContext;
use super::GpuError;

const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

impl GpuContext {
    pub fn warmup(&self) {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

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
        instances: &[super::CellInstance],
        kgp_instances: &[super::KgpInstance],
    ) -> Result<(), GpuError> {
        if self.render_paused {
            return Ok(());
        }
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
            let proj = super::orthographic_projection(cfg_width as f32, cfg_height as f32);
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

        if let (Some(bg_bind_group), Some(blur_h), Some(blur_v)) = (
            self.bg_bind_group.as_ref(),
            self.blur_h_pipeline.as_ref(),
            self.blur_v_pipeline.as_ref(),
        ) && self.bg_blur_radius >= 0.5
        {
            {
                let mut h_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blur H Pass"),
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
                h_pass.set_pipeline(blur_h);
                h_pass.set_bind_group(0, bg_bind_group, &[]);
                h_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
                h_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
                h_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                h_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
            {
                let mut v_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blur V Pass"),
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
                v_pass.set_pipeline(blur_v);
                v_pass.set_bind_group(0, bg_bind_group, &[]);
                v_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
                v_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
                v_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                v_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
        } else if let (Some(bg_pipeline), Some(bg_bind_group)) =
            (&self.bg_pipeline, &self.bg_bind_group)
        {
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

    pub fn render_to_buffer(
        &mut self,
        instances: &[super::CellInstance],
        kgp_instances: &[super::KgpInstance],
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

        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_POLL_TIMEOUT),
        }) {
            log::warn!("render_to_buffer: device poll error: {error}");
        }

        let slice = dst.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| {
            if let Err(e) = r {
                log::error!("readback map failed: {e:?}");
            }
        });
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_POLL_TIMEOUT),
        }) {
            log::warn!("render_to_buffer (map wait): device poll error: {error}");
        }
        let data = slice.get_mapped_range().to_vec();
        dst.unmap();

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
