//! 开孔遮罩：把保留的上一帧回读成 CPU 像素。
//!
//! 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
//! SurfaceView 的 surface 随之被拆；回前台时交换链必须重建，那几十毫秒里
//! 窗口的透明开孔没有缓冲——不遮就是「切后台返回闪一下」。
//!
//! 遮罩的取法：运行期周期性（宿主每处理一帧就唤醒一次，见
//! `TerminalRuntime.coverCaptureLoop`）把帧累加器的当前内容回读成 CPU 像素并缓存；
//! 切后台时宿主先发布缓存，再尽力回读一次最新帧替下它。累加器是纯设备纹理（不引用
//! surface），故回读与窗口是否已死无关；但设备本身会丢失——那时回读必然失败，
//! 所以 MUST 在设备还活着时周期备好一帧，而不是只在切后台那一刻才回读。
//!
//! 回读分两步，且**GPU 等待必须在全局渲染锁之外**：
//! 累加器句柄的快照（廉价克隆）在锁内取，回读本身（一次 GPU 同步等待，
//! 实测可达数十毫秒）在锁外做。锁内等待会把**所有**会话的渲染线程一起
//! 卡住——真机日志 2026-11-12：遮罩回读期间另一会话的渲染采样多花 38.98ms，
//! 触发 SLOW_FRAME/SLOW_LOOP 告警。
//!
//! 等待用非阻塞轮询（[`poll_device_now`]）而不是带超时的阻塞等待：Mali-G57 上
//! 带有限超时的 `vkWaitSemaphores` 在首帧提交后数毫秒内返回 `VK_ERROR_DEVICE_LOST`
//! （2026-11-24 三份真机日志），每次重建后同一条等待再次触发丢失。非阻塞轮询
//! 只读围栏值、回收已完成提交，不进入驱动等待。

use crate::render::context::Renderer;
use crate::render::context::{PollOutcome, poll_device_now};
use std::sync::atomic::Ordering;

/// 回读超时：遮罩是一次性补救，超时即放弃（终端照常恢复）。
const READBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// 轮询间隔：非阻塞轮询之间让出 CPU，避免忙等。
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2);

/// 一帧的 CPU 像素（RGBA8）与尺寸。
pub(crate) struct CoverFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

/// 回读源：累加器纹理的句柄快照。
///
/// wgpu 的 `Device`/`Queue`/`Texture` 都是引用计数的句柄，克隆廉价且独立于
/// 渲染器的所有权：纹理被换掉（resize）时快照仍指向旧纹理，内容照旧可读。
pub(crate) struct CoverReadbackSource {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    /// 该设备世代的判死位（与 `device` 同生共死）：回读期间的轮询只照看这一个设备。
    lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
    width: u32,
    height: u32,
}

impl CoverReadbackSource {
    /// 设备是否已判死：判死后不再触碰 GPU。
    ///
    /// 死设备上的任何 wgpu 调用都只会换来 wgpu 内部的一道 panic（`Parent device is
    /// lost`），那道 panic 穿出 JNI 边界就是整进程退出（2026-11-12 真机日志）。
    /// 判死位按设备而非进程：新设备活着不代表这个旧快照能继续轮询。
    fn is_lost(&self) -> bool {
        crate::render::context::device_lost() || self.lost.load(Ordering::Acquire)
    }

    /// 把快照里的累加器内容回读成 CPU 像素。
    ///
    /// 返回 `None` 表示回读失败或超时：调用方沿用已发布的缓存帧（没有缓存时开孔
    /// 由平底色兜住），不做任何补偿。
    /// 本方法**不持有任何全局锁**（见模块头）：GPU 等待期间别的会话照常渲染。
    pub(crate) fn read(&self) -> Option<CoverFrame> {
        // 快照在锁内取、回读在锁外做，两者之间设备可能恰好丢失：死设备上的 poll
        // 同样在 wgpu 内部 panic。放弃遮罩即可（恢复全程走 attach 重建）。
        if self.is_lost() {
            return None;
        }
        let width = self.width;
        let height = self.height;
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
                texture: &self.texture,
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
        // 提交前再判一次死：上面建缓冲与编码的间隙里设备可能已丢失，那时的提交
        // 会撞上 wgpu 的 `Parent device is lost` panic（穿出 JNI 即整进程退出）。
        if self.is_lost() {
            return None;
        }
        // 判死位到 panic 之间仍有一瞬：提交本身也在归一范围内。
        let submitted = super::poll::catch_device_loss_panic("read_cover_frame: 提交回读拷贝", || {
            self.queue.submit(std::iter::once(encoder.finish()));
        });
        submitted?;
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
            //
            // 非阻塞：带超时的阻塞等待在 Mali-G57 上本身会触发设备丢失
            // （见 `poll_device_now` 的文档）。这里只等自己的映射回调，不需要
            // 驱动层的阻塞等待。
            match poll_device_now(&self.device, &self.lost) {
                PollOutcome::Proceed => {}
                PollOutcome::Failed(reason) => {
                    // 设备丢失是**一次**事件：由标记点记一条并停止渲染，余波不再逐帧记 error。
                    if crate::render::context::is_device_loss_text(&reason) {
                        crate::render::context::mark_device_lost(&self.lost, &reason);
                    } else {
                        log::error!("read_cover_frame: 设备轮询失败，放弃遮罩: {reason}");
                    }
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
            std::thread::sleep(POLL_INTERVAL);
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
        // 周期回读：正常路径每次出帧后都会走到这里，info 会变成每帧一行噪声。
        log::trace!(
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

impl Renderer {
    /// 取累加器纹理的回读快照（锁内只做句柄克隆）。
    ///
    /// 返回 `None` 表示没有可回读的内容（累加器不存在、格式不是 RGBA8 或尺寸为零）：
    /// 调用方沿用已发布的缓存帧（没有缓存时开孔由平底色兜住），不做任何补偿。
    pub(crate) fn cover_readback_source(&self) -> Option<CoverReadbackSource> {
        // 设备丢失后遮罩回读是第一个撞上 `Parent device is lost` 的调用（2026-11-14
        // 真机日志：切后台时先报这条，随后才是渲染循环的同类 panic）。遮罩只是一次性
        // 补救：放弃它，等宿主换新窗口、attach 重建后再恢复，绝不在死设备上创建
        // 缓冲或提交命令。
        if crate::render::context::device_lost()
            || self.lost.load(std::sync::atomic::Ordering::Acquire)
        {
            return None;
        }
        // 累加器内容未定义时不得回读：纹理刚建好（尺寸或格式变化后重建）或已随
        // surface 重建丢弃时，它的内容是未初始化的显存——回读上来是雪花，画进开孔
        // 比没有遮罩更糟。`frame_content_valid` 正是「累加器里有一帧**已渲染**的
        // 内容」这一事实（warmup 呈现保留帧用的是同一个判据，见
        // `Renderer::retained_frame_presentable`）。
        if !self.frame_content_valid {
            return None;
        }
        // 新设备世代还没呈现过任何一帧时不得回读：那时累加器里的内容是旧设备画成
        // 的（重建保留了它供 warmup 呈现），而回读要在新设备上等第一次拷贝完成
        // ——Mali-G57 上正是这条等待触发设备丢失（2026-11-24 真机日志：每次重建
        // 后 1-4ms 内 `readCoverFrame` 的轮询报设备丢失）。等新设备真正出过一帧，
        // 回读才有意义，也才安全。
        if !self.has_presented_frame {
            return None;
        }
        let texture = self.frame_texture.as_ref()?;
        let format = texture.format();
        if format != wgpu::TextureFormat::Rgba8Unorm {
            // 通道序不同则宿主的 RGBA 假设不成立：宁可不遮，也不要画错颜色。
            // 格式是设备属性，整进程只记一次：回读至多每 600ms 一次，逐次记会把
            // logcat 刷成同一条告警。
            static NON_RGBA_FORMAT_REPORTED: std::sync::Once = std::sync::Once::new();
            NON_RGBA_FORMAT_REPORTED.call_once(|| {
                log::warn!("read_cover_frame: 累加器格式 {format:?} 不是 Rgba8Unorm，放弃遮罩");
            });
            return None;
        }
        let width = texture.width();
        let height = texture.height();
        if width == 0 || height == 0 {
            return None;
        }
        Some(CoverReadbackSource {
            device: self.device.clone(),
            queue: self.queue.clone(),
            texture: texture.clone(),
            lost: self.lost.clone(),
            width,
            height,
        })
    }
}
