//! 像素回读：仅测试脚手架，生产构建不含此模块。
use super::frame::store_attachment;
use crate::render::GpuError;
use crate::render::Renderer;
use crate::render::pipeline::QUAD_VERTEX_COUNT;

/// 设备轮询超时：仅测试回读脚手架使用（`render_to_buffer` 内），生产零引用。
const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// 回读 map 轮询步长：每次 poll 等待的分片，避免忙等。
/// 仅测试使用（`render_to_buffer` 是无 surface 的回读测试脚手架，生产零调用）。
const MAP_POLL_STEP: std::time::Duration = std::time::Duration::from_millis(10);
/// 回读 map 总超时：超时即报 Readback 错误，不无限等待。
/// 仅测试使用（同上）。
const MAP_READBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

impl Renderer {
    /// 无 surface 的像素回读：仅测试脚手架（背景填充/滚动帧像素断言），生产零调用。
    /// 本模块整体受 `cfg(test)` 约束，不进生产二进制；删测试脚手架时随之删除。
    pub fn render_to_buffer(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<Vec<u8>, GpuError> {
        let (frame_width, frame_height) = self
            .surface_config
            .as_ref()
            .map_or((0, 0), |surface_config| {
                (surface_config.width, surface_config.height)
            });
        if frame_width == 0 || frame_height == 0 {
            return Err(GpuError::Surface("No surface config".to_string()));
        }

        self.ensure_kgp_pipeline(frame_width, frame_height);

        let tex_size = wgpu::Extent3d {
            width: frame_width,
            height: frame_height,
            depth_or_array_layers: 1,
        };
        // 回读纹理格式必须与管线格式一致（cell/kgp 管线按 surface 格式创建，
        // Android 上通常是 Bgra8Unorm）——硬编码 Rgba8Unorm 会让这些管线
        // 以它作渲染附件时触发 wgpu 校验错误。
        let pipeline_format = self.pipeline_format;
        let needs_new = match &self.readback_texture {
            Some(texture) => {
                texture.width() != frame_width
                    || texture.height() != frame_height
                    || texture.format() != pipeline_format
            }
            None => true,
        };
        if needs_new {
            self.readback_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Readback Texture"),
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: pipeline_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }
        let texture = self
            .readback_texture
            .as_ref()
            .ok_or_else(|| GpuError::Surface("readback_texture creation failed".to_string()))?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_row_padded = ((frame_width * 4) + (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1))
            & !(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1);
        let buf_size = (bytes_per_row_padded * frame_height) as u64;
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Readback Encoder"),
            });

        // 回读路径无脏带（强制全量重绘），整段从偏移 0 上传即可。
        Self::upload_instances(
            &self.device,
            &self.queue,
            &mut self.instance_buffer,
            bytemuck::cast_slice(instances),
            0,
            "Instance Buffer (readback)",
        );
        Self::upload_kgp_instances(
            &self.device,
            &self.queue,
            &mut self.kgp_instance_buffer,
            kgp_instances,
            "KGP Instance Buffer (readback)",
        );

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Readback Render Pass"),
                color_attachments: &store_attachment(&view, wgpu::LoadOp::Clear(self.background)),
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let width_float = frame_width as f32;
            let height_float = frame_height as f32;
            rp.set_pipeline(pipeline);
            rp.set_viewport(0.0, 0.0, width_float, height_float, 0.0, 1.0);
            rp.set_scissor_rect(0, 0, frame_width, frame_height);
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
            // Kitty 叠加层：与生产路径同一套管线与绑定组，回读才能断言图像像素。
            if !kgp_instances.is_empty()
                && let (Some(kgp_pipeline), Some(kgp_bind_group)) =
                    (&self.kgp_pipeline, &self.kgp_bind_group)
            {
                rp.set_pipeline(kgp_pipeline);
                rp.set_bind_group(0, kgp_bind_group, &[]);
                rp.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                if let Some(ref ib) = self.kgp_instance_buffer {
                    rp.set_vertex_buffer(1, ib.slice(..));
                    rp.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
                }
            }
        }

        let destination = self
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
                buffer: destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_padded),
                    rows_per_image: Some(frame_height),
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

        let slice = destination.slice(..);
        // 用 oneshot 通道可靠地判定 map 完成。
        let (map_tx, map_rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |map_result| {
            let _ = map_tx.send(map_result);
        });
        // 反复轮询直到 map 完成或超时。
        let poll_start = std::time::Instant::now();
        let mapped: Result<(), GpuError> = loop {
            if let Err(error) = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(MAP_POLL_STEP),
            }) {
                log::warn!("render_to_buffer (map wait): device poll error: {error}");
            }
            match map_rx.try_recv() {
                // 向上传递 map_async 错误（如缓冲过大、设备丢失）。
                Ok(result) => {
                    break result.map_err(|map_error| {
                        GpuError::Readback(format!("map_async failed: {map_error:?}"))
                    });
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break Err(GpuError::Readback("map channel disconnected".into()));
                }
            }
            if poll_start.elapsed() > MAP_READBACK_TIMEOUT {
                break Err(GpuError::Readback("map_async timed out".into()));
            }
        };
        // 成功、超时、通道断开、map 失败都要 unmap：留在 mapped 态的缓冲不是合法的
        // 拷贝目标，下次 copy_texture_to_buffer 会被 wgpu 校验拒绝（乃至设备丢失）。
        let data = mapped.and_then(|()| {
            slice
                .get_mapped_range()
                .map(|range| range.to_vec())
                .map_err(|map_error| GpuError::Readback(map_error.to_string()))
        });
        destination.unmap();
        let data = data?;

        let pixel_bytes = (frame_width * frame_height * 4) as usize;
        let stride = bytes_per_row_padded as usize;
        let trimmed = if data.len() > pixel_bytes && stride > (frame_width as usize * 4) {
            let mut flat = Vec::with_capacity(pixel_bytes);
            for row in 0..frame_height as usize {
                let row_start = row * stride;
                let row_end = row_start + (frame_width as usize * 4);
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
