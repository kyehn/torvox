//! 开孔遮罩：把保留的上一帧回读成 CPU 像素。
//!
//! 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
//! SurfaceView 的 surface 随之被拆；回前台时交换链必须重建，那几十毫秒里
//! 窗口的透明开孔没有缓冲——不遮就是「切后台返回闪一下」。
//!
//! 遮罩的取法：切后台时（渲染线程停下之前）把帧累加器的当前内容回读成
//! CPU 像素，宿主把它画在开孔里，等新帧到达再撤掉。累加器是纯设备纹理
//! （不引用 surface），故回读与窗口是否已死无关。

use crate::render::context::Renderer;
use crate::render::context::poll::{PollOutcome, poll_device_wait};

/// 回读超时：遮罩是一次性补救，超时即放弃（终端照常恢复）。
const READBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// 轮询步长：每次 `device.poll` 的等待时长，避免忙等。
const POLL_STEP: std::time::Duration = std::time::Duration::from_millis(5);

/// 一帧的 CPU 像素（RGBA8）与尺寸。
pub(crate) struct CoverFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

impl Renderer {
    /// 把帧累加器（保留的上一帧）回读成 CPU 像素。
    ///
    /// 返回 `None` 表示没有可回读的内容（累加器不存在、格式不是 RGBA8、
    /// 回读失败或超时）：调用方沿用平底色遮罩，不做任何补偿。
    pub(crate) fn read_cover_frame(&mut self) -> Option<CoverFrame> {
        let texture = self.frame_texture.as_ref()?;
        let format = texture.format();
        if format != wgpu::TextureFormat::Rgba8Unorm {
            // 通道序不同则宿主的 RGBA 假设不成立：宁可不遮，也不要画错颜色。
            log::warn!("read_cover_frame: 累加器格式 {format:?} 不是 Rgba8Unorm，放弃遮罩");
            return None;
        }
        let width = texture.width();
        let height = texture.height();
        if width == 0 || height == 0 {
            return None;
        }
        let bytes_per_row = width * 4;
        // 回读缓冲的行必须按 `COPY_BYTES_PER_ROW_ALIGNMENT` 对齐，故每行末尾有填充。
        let padded = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cover Readback Buffer"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Cover Readback Encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        let slice = buffer.slice(..);
        let (map_tx, map_rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |map_result| {
            let _ = map_tx.send(map_result);
        });
        let deadline = std::time::Instant::now() + READBACK_TIMEOUT;
        loop {
            // 设备轮询的失败形态不止 `Err`：wgpu 内部会在设备真出错时直接 panic，
            // 那道 panic 穿出 JNI 边界就是整进程退出（2026-11-12 真机日志）。
            // 故走统一封装：常态继续，真错误带原因退出，panic 也归成「失败」。
            match poll_device_wait(&self.device, POLL_STEP) {
                PollOutcome::Proceed => {}
                PollOutcome::Failed(reason) => {
                    log::error!("read_cover_frame: 设备轮询失败，放弃遮罩: {reason}");
                    return None;
                }
            }
            match map_rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    log::warn!("read_cover_frame: map_async failed: {error:?}");
                    return None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    log::warn!("read_cover_frame: map channel disconnected");
                    return None;
                }
            }
            if std::time::Instant::now() >= deadline {
                log::warn!(
                    "read_cover_frame: map_async 超时（{}ms），放弃遮罩",
                    READBACK_TIMEOUT.as_millis()
                );
                return None;
            }
        }
        let data = match slice.get_mapped_range() {
            Ok(data) => data,
            Err(error) => {
                log::warn!("read_cover_frame: get_mapped_range failed: {error:?}");
                return None;
            }
        };
        let mut pixels = Vec::with_capacity(bytes_per_row as usize * height as usize);
        for row in 0..height as usize {
            let start = row * padded as usize;
            pixels.extend_from_slice(&data[start..start + bytes_per_row as usize]);
        }
        drop(data);
        buffer.unmap();
        log::info!(
            "read_cover_frame: 已回读 {}x{} 的遮罩帧（{} 字节）",
            width,
            height,
            pixels.len()
        );
        Some(CoverFrame {
            width,
            height,
            pixels,
        })
    }
}
