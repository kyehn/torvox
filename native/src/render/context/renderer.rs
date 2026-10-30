use wgpu::util::DeviceExt;

use super::gpu::{
    SIZE_MISMATCH_THROTTLE, SKIP_FATAL_FRAME_LIMIT, background_clear_color, global_gpu,
    initial_background_color, surface_loss_transition,
};
use super::{FrameContext, Renderer};
#[cfg(target_os = "android")]
use crate::render::GpuError;
use crate::render::pass::{ACQUIRE_TIMEOUT, AcquireOutcome};
use crate::render::pipeline::QUAD_CORNERS;

// ── 渲染器生命周期与帧上下文 ─────────────────────────────────────────
impl Renderer {
    /// 取得（或重建）持久帧累加器的 view。不支持累加或纹理创建失败时返回 `None`；
    /// 尺寸/格式不匹配会重建纹理并把下一帧标记为全量重绘。
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
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
            });
            self.frame_view = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
            self.frame_texture = Some(texture);
            // 新纹理内容未定义，下一帧必须全量重绘。
            self.frame_invalidated = true;
        }
        self.frame_view.clone()
    }

    /// 获取 surface 纹理并为本帧创建 `FrameContext`；获取失败（surface 丢失、超时、
    /// GPU 卡死）时返回 `None`。
    ///
    /// 取纹理结局（拿到纹理 / 本帧跳过 / surface 级失败）在此汇总并驱动
    /// [Self::surface_invalidated]：连续 [SURFACE_LOSS_STREAK_LIMIT] 次 surface 级
    /// 失败即判死窗口，使下一次 `attach_surface` 走重建慢路径。
    pub(crate) fn begin_frame(&mut self) -> Option<FrameContext> {
        let config_width = self
            .surface_config
            .as_ref()
            .map(|surface_config| surface_config.width)?;
        let config_height = self
            .surface_config
            .as_ref()
            .map(|surface_config| surface_config.height)?;

        self.refresh_cell_uniforms(config_width as f32, config_height as f32);
        self.ensure_kgp_pipeline(config_width, config_height);

        let surface = std::sync::Arc::clone(self.surface.as_ref()?);
        let mut outcome =
            self.acquire_texture(&surface, config_width, config_height, ACQUIRE_TIMEOUT);
        if self.surface_loss_injected {
            log::warn!("surface loss injected for test; this frame reports a surface failure");
            outcome = AcquireOutcome::SurfaceLost;
        }
        self.note_surface_acquire(&outcome);
        match &outcome {
            AcquireOutcome::Skipped => {
                self.consecutive_skip_frames = self.consecutive_skip_frames.saturating_add(1);
                if self.consecutive_skip_frames == SKIP_FATAL_FRAME_LIMIT {
                    log::error!(
                        "begin_frame: {} consecutive frames skipped (acquire worker dead or repeatedly timing out); \
                         screen frozen until the worker recovers",
                        SKIP_FATAL_FRAME_LIMIT
                    );
                }
            }
            AcquireOutcome::Acquired(_) | AcquireOutcome::SurfaceLost => {
                self.consecutive_skip_frames = 0;
            }
        }
        let output = outcome.into_texture()?;

        let tex_size = output.texture.size();
        if tex_size.width != config_width || tex_size.height != config_height {
            // 尺寸不一致：surface 已被 SurfaceFlinger 缩放，而当前配置仍是旧尺寸。
            // 按纹理实际尺寸重配并丢弃本次纹理、跳过本帧——绝不拿旧尺寸纹理渲染，
            // 那会把帧画进尺寸已不合脚的 swapchain；跳过也保证刚取得的 SurfaceTexture
            // 不被丢弃而不 present，交换链不丢缓冲区。下一帧取得的纹理尺寸即与配置一致。
            if SIZE_MISMATCH_THROTTLE.should_log() {
                log::warn!(
                    "begin_frame: size mismatch! config={}x{} texture={}x{}; reconfiguring and skipping frame",
                    config_width,
                    config_height,
                    tex_size.width,
                    tex_size.height
                );
            }
            if let Some(mut new_config) = self.surface_config.take() {
                new_config.width = tex_size.width;
                new_config.height = tex_size.height;
                surface.configure(&self.device, &new_config);
                self.surface_config = Some(new_config);
            }
            // 丢弃 `output`（不 present）后返回：下一帧按新配置重新取纹理。
            self.frame_invalidated = true;
            return None;
        }

        // uniform 已在函数开头按本帧尺寸（`refresh_cell_uniforms`）写过：此路径到
        // 这里只有取纹理与尺寸核对，两者都不改图集尺寸、投影尺寸与滚动偏移，
        // 故不再重写同一份字节（每帧一次多余的 `write_buffer`）。
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
            config_width,
            config_height,
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // 同 `release_surface`：渲染器被丢弃时若有在途取帧持有 surface 克隆，
        // 销毁可能跑在克隆之后，而宿主可能已把同一窗口交给下一个渲染器
        // （会话重建）。排干后再丢，把「同窗口两个 Vulkan surface」的窗口期压到零。
        // 超时只记日志：丢弃路径没有可拒绝的调用方，且此处已是清理终点。
        if !crate::render::pass::drain_in_flight_acquires(
            crate::render::pass::ACQUIRE_DRAIN_TIMEOUT,
        ) {
            log::warn!(
                "Renderer::drop: in-flight acquire did not drain within {}ms; dropping anyway",
                crate::render::pass::ACQUIRE_DRAIN_TIMEOUT.as_millis(),
            );
        }
        self.frame_texture = None;
        self.frame_view = None;
        self.cell_bind_group = None;
        self.instance_buffer = None;
        self.cell_pipeline = None;
        self.cell_uniform_buffer = None;
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
        self.kgp_pipeline = None;
        self.kgp_pipeline_format = None;
        self.kgp_texture = None;
        self.surface = None;
        self.attached_window = None;
        self.retired_window = None;
    }
}

/// [Renderer::can_reuse_attached_surface] 的判定本体：四个条件缺一不可——已建 surface、
/// 已配置、未判死、且送来的正是当前挂载的那个 `ANativeWindow`。抽成自由函数以便单测
/// 穷举：设备上判错的代价是整屏停帧（见 `Renderer::attached_window`）。
#[cfg(any(target_os = "android", test))]
pub(crate) fn reuse_attached_surface(
    has_surface: bool,
    has_config: bool,
    invalidated: bool,
    attached_window: Option<usize>,
    incoming_window: usize,
) -> bool {
    has_surface && has_config && !invalidated && attached_window == Some(incoming_window)
}

/// 排干失败（工作线程仍持有旧 surface 的 `Arc` 克隆）时，是否必须拒绝在
/// `incoming_window` 上重建 surface。
///
/// 只有「重建目标正是那个仍有克隆在途的窗口」才不可做：同一 `ANativeWindow` 上并存
/// 两个 Vulkan surface 会被驱动拒绝（`ERROR_NATIVE_WINDOW_IN_USE_KHR`）并触发
/// wgpu-hal panic，把进程带走。换新窗口重建是安全的——陈旧克隆包着的是旧
/// BufferQueue，与新窗口无关（真机日志里回前台/切会话送的正是新窗口）。
///
/// `retired_window` 覆盖「释放与重建之间」的窗口期：`release_surface` 排干失败后
/// 仍会清掉 `attached_window`，只凭它无从判断刚释放的窗口是否就是送回来的这个
/// （2026-10-18 实测：同址 attach，相距 53ms）。抽成自由函数以便单测穷举。
#[cfg(any(target_os = "android", test))]
pub(crate) fn rebuild_blocked_by_in_flight_surface(
    in_flight_drained: bool,
    incoming_window: usize,
    attached_window: Option<usize>,
    retired_window: Option<usize>,
) -> bool {
    !in_flight_drained
        && (attached_window == Some(incoming_window) || retired_window == Some(incoming_window))
}

impl Renderer {
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
            attached_window: None,
            retired_window: None,
            surface_loss_streak: 0,
            consecutive_skip_frames: 0,
            surface_invalidated: false,
            surface_loss_injected: false,
            cell_pipeline: None,
            quad_vertex_buffer,
            cell_bind_group: None,
            cell_uniform_buffer: None,
            instance_buffer: None,
            cpu_instances: Vec::new(),
            cell_cache: None,
            cell_full_mask_cache: Vec::new(),
            viewport_scroll_px: 0.0,
            last_drawn_viewport_scroll_px: 0.0,
            atlas_texture: None,
            atlas_view: None,
            atlas_sampler: None,
            pipeline_format: wgpu::TextureFormat::Rgba8Unorm,
            projection_width: 0,
            projection_height: 0,
            readback_texture: None,
            readback_buffer: None,
            background: initial_background_color(),
            kgp_pipeline: None,
            kgp_pipeline_format: None,
            kgp_bind_group: None,
            kgp_uniform_buffer: None,
            kgp_sampler: None,
            kgp_instance_buffer: None,
            kgp_texture: None,
            kgp_atlas_width: 0,
            kgp_atlas_height: 0,
            render_paused: false,
            frame_texture: None,
            frame_view: None,
            frame_invalidated: true,
            swapchain_copy_supported: false,
            present_log: crate::slow_log::CountedLogThrottle::new(),
        }
    }

    /// 创建共享全局 wgpu instance/adapter/device 的 `Renderer`，适用于无窗口上下文。
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

    /// 把 Android `ANativeWindow` 挂载为渲染 surface：据原生窗口句柄建 wgpu surface
    /// 并按给定尺寸重建配置。调用方保证 `ptr` 是有效的 `ANativeWindow*`，存活至
    /// [`Renderer::release_surface`]（或 drop）——所有权转交 wgpu，其在创建时自行持有引用。
    ///
    /// surface 生命周期由 Android SurfaceHolder.Callback 驱动（此处挂载/释放）而非
    /// Activity 生命周期；尺寸每帧重新查询；挂载后须立即渲染一帧。Acquire 失败的
    /// 处理见 `render/pass.rs`：不对 Lost/Outdated 重试，而是重新 configure 后丢弃本帧。
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
        // 快速路径：已挂载 surface 时（输入法收起、HOME→recents 且 surface 保留）
        // 原地 reconfigure 而非丢弃重建。重建会与渲染线程竞争，在 SwiftShader 上
        // 报 ERROR_NATIVE_WINDOW_IN_USE_KHR；reconfigure 是零拷贝的。
        let window = ptr as usize;
        if self.can_reuse_attached_surface(window) {
            self.reconfigure_swapchain(width, height);
            log::info!("attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)");
            return Ok(());
        }
        if self.surface_invalidated {
            log::warn!(
                "attach_surface: previous surface invalidated; rebuilding from ANativeWindow"
            );
        } else if self.attached_window.is_some() {
            log::info!("attach_surface: ANativeWindow changed; rebuilding for the new window");
        }
        // 排干在途取帧：工作线程可能仍持有旧 surface 的 `Arc` 克隆（其下的 Vulkan
        // surface 直到克隆被放下才销毁）。同一窗口上并存两个 Vulkan surface 会被驱动
        // 拒绝（ERROR_NATIVE_WINDOW_IN_USE_KHR）并触发 wgpu-hal panic，故必须先排干。
        // 排干在正常路径上是瞬时的（工作线程一帧取纹理内就放克隆）；排不干只可能是
        // 驱动卡死（Mali GPU 已知形态），此时同窗口重建必须拒绝——见下方判定。
        let in_flight_drained = crate::render::pass::drain_in_flight_acquires(
            crate::render::pass::ACQUIRE_DRAIN_TIMEOUT,
        );
        if rebuild_blocked_by_in_flight_surface(
            in_flight_drained,
            window,
            self.attached_window,
            self.retired_window,
        ) {
            log::error!(
                "attach_surface: in-flight acquire on the SAME window {window:#x} did not drain within {}ms; \
                 refusing to rebuild (two Vulkan surfaces on one ANativeWindow panic wgpu-hal). \
                 The host must bring a fresh ANativeWindow.",
                crate::render::pass::ACQUIRE_DRAIN_TIMEOUT.as_millis(),
            );
            return Err(GpuError::Surface(format!(
                "attach_surface: same-window rebuild refused: an in-flight acquire still holds the \
                 previous surface on window {window:#x} after {}ms; retry with a fresh ANativeWindow",
                crate::render::pass::ACQUIRE_DRAIN_TIMEOUT.as_millis(),
            )));
        }
        if !in_flight_drained {
            log::warn!(
                "attach_surface: in-flight acquire did not drain within {}ms but targets a different window; \
                 proceeding with the rebuild",
                crate::render::pass::ACQUIRE_DRAIN_TIMEOUT.as_millis(),
            );
        }
        let handle = AndroidNdkWindowHandle::new(non_null.cast());
        // 先丢帧累加器再丢 surface：累加器持有旧 surface 的输出细节（present/discard
        // 都记在 `SurfaceId` 上），倒过来会让旧纹理去访问已注销的 surface id。
        // 顺序之外的理由与下一段注释相同：旧 surface 必须死透，才能为同一窗口再建
        // 一个 Vulkan surface。
        self.frame_texture = None;
        self.frame_view = None;
        self.frame_invalidated = true;
        // 先释放旧 surface 再建新 surface：Android 上两个 wgpu surface 包裹同一个
        // ANativeWindow，旧 surface 仍存活时无法为该窗口创建 Vulkan swapchain，
        // 随后的 get_current_texture 永远报 “Surface is not configured for presentation”
        // （模拟器实测：首次之后的每个会话都黑屏）。调用方保证此刻无渲染线程处于帧中
        // （switchSession 会先停旧线程），且在途取帧已在函数开头排干，故可安全丢弃。
        self.surface = None;
        self.surface_config = None;
        // SAFETY:
        // - `global_gpu().instance` 是有效的 wgpu Instance；
        // - 句柄包裹调用方保证存活的 ANativeWindow（JNI attachWindow 契约），
        //   存活至 detachWindow 丢弃由其产生的 surface——wgpu 在创建时自行取得引用，
        //   调用方在本调用返回后即释放自己的引用；
        // - Android 上无 X11/Wayland display。
        // `create_surface_unsafe` 包在 catch_unwind 里：wgpu-hal 的 vulkan 后端对
        // `vkCreateAndroidSurfaceKHR` 的错误直接 `expect()`（`instance.rs`），而
        // `ERROR_NATIVE_WINDOW_IN_USE_KHR`（同一 ANativeWindow 上已有 Vulkan surface）
        // 是它认得的错误之一。panic 会沿 JNI 边界变成未捕获的 Java 异常，把整个进程
        // 带走（2026-10-18 日志：宿主在 postDelayed 回调里 attach，进程 FATAL 退出，
        // 整轮仪器化运行作废）。这里转成显式错误上报，绝不静默：日志保留 panic 载荷，
        // 调用方（ffi）据此走既有的判死/换窗口自愈路径。
        let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            global_gpu()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    // 须与 instance 的 display 匹配（wgpu_backend 在建 instance 时设置
                    // AndroidDisplayHandle）；instance 携带 display 时 wgpu-core 拒绝 None。
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                        raw_window_handle::AndroidDisplayHandle::new(),
                    )),
                    raw_window_handle: RawWindowHandle::AndroidNdk(handle),
                })
        }));
        let surface = match created {
            Ok(Ok(surface)) => surface,
            Ok(Err(error)) => {
                return Err(GpuError::Surface(format!(
                    "attach_surface: wgpu create_surface failed: {error}"
                )));
            }
            Err(panic) => {
                let detail = crate::panic_text(&*panic);
                log::error!("attach_surface: wgpu create_surface panicked: {detail}");
                return Err(GpuError::Surface(format!(
                    "attach_surface: wgpu create_surface panicked: {detail}"
                )));
            }
        };
        let surface = std::sync::Arc::new(surface);
        // 默认配置挑选驱动支持的呈现模式与格式；`RENDER_SCALE` 与 `reconfigure_swapchain`
        // 一致（跨渲染器固定的缩放，保证单元格度量一致）。
        let (scaled_width, scaled_height) = super::surface::surface_extent(
            width,
            height,
            global_gpu().device.limits().max_texture_dimension_2d,
        );
        let caps = surface.get_capabilities(&global_gpu().adapter);
        // 脏带合成要求交换链纹理接受来自帧累加器的拷贝（Vulkan 上普遍支持）；
        // 特殊驱动缺失时回退到旧的直接渲染路径（仅全量重绘）。
        let swapchain_copy_supported = caps.usages.contains(wgpu::TextureUsages::COPY_DST);
        self.swapchain_copy_supported = swapchain_copy_supported;
        let usage = if swapchain_copy_supported {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        let format = Self::select_surface_format(&caps)?;
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            width: scaled_width,
            height: scaled_height,
            present_mode: Self::select_present_mode(&caps),
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            // 双缓冲：FIFO 下单缓冲会让 acquire 阻塞到显示端消费上一帧；三缓冲又把
            // 排队深度堆到 3 帧，慢帧时 acquire 实测等到 2-3 个 vsync（Mali 上单帧
            // 33-58ms）。取 2 保留流水线，同时把等待上限压到一帧。Mailbox（驱动支持
            // 时已在上方选中）丢弃最旧排队帧，滚动终端中新帧仍然胜出。
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        // Android 上 view_formats 故意置空：平台缺少 SURFACE_VIEW_FORMATS 降级标志，
        // 任何非空列表都会使 configure 失败（已在 API 35 模拟器上验证
        // Rgba8Unorm + 空 view_formats 可正常渲染）。
        // wgpu 30 的 configure 无返回值（错误经 get_current_texture 的 Lost 状态异步
        // 上报），故无可传播的 Result；render/pass.rs 的 acquire 路径在 Lost 时
        // 重新 configure 并丢弃该帧。
        surface.configure(&self.device, &config);
        self.surface_config = Some(config);
        // 与**上一个**管线格式比较（该字段在下方更新）：重挂载时格式变化必须丢弃
        // 惰性创建的单元管线，使下次渲染用新 surface 的格式重建。
        let previous_format = self.pipeline_format;
        self.pipeline_format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |surface_config| {
                surface_config.format
            });
        if previous_format != self.pipeline_format {
            self.cell_pipeline = None;
            self.cell_bind_group = None;
            // Kitty 管线同样以 surface 格式为 color target：格式变化后若保留旧管线，
            // 绘制时附件格式与管线声明不符，每帧 wgpu 校验失败，Kitty 图像全部不可见。
            self.kgp_pipeline = None;
            self.kgp_pipeline_format = None;
            self.kgp_bind_group = None;
            log::info!(
                "attach_surface: surface format changed {previous_format:?} -> {:?}, cell/kitty pipelines scheduled for rebuild",
                self.pipeline_format,
            );
        }
        self.projection_width = scaled_width;
        self.projection_height = scaled_height;
        self.surface = Some(surface);
        self.attached_window = Some(window);
        // 重建成立：本窗口上不再有「可能仍存活的旧 surface」需要记着（见 `retired_window`）。
        self.retired_window = None;
        // 重建**确实**成功才清失效位与连续失败计数。
        //
        // 放在这里而非 `create_surface_unsafe` 之后：本次此间的 `get_capabilities` /
        // `select_surface_format` / `configure` 任一失败都会提前 `?` 上抛，而那时
        // 重建尚未成立。先清零等于让失败路径替成功路径还愿——`get_capabilities` 在
        // 模拟器上失败时（Surface::configure 报 `UnsupportedQueueFamily` 的那条根因）
        // 会反复把 `surface_loss_streak` 打回 0，使「连续两次取帧失败即判失效」的
        // 自愈阈值永远攒不满，渲染就此停摆，连重试都没了。
        self.surface_invalidated = false;
        self.surface_loss_streak = 0;
        // ── 启动黑屏防护 ─────────────────────────────
        // 首个内容帧要等 shell 输出 + 冷启动（SwiftShader 上实测数百毫秒：字形整形、
        // 整幅 atlas 上传、纹理分配）；期间交换链一帧未提交，屏幕保持空黑，静默 shell
        // （无任何新数据）甚至永远黑屏。故在 attach 线程上（渲染线程未启动，无竞争）
        // 预先创建帧累加器，把一次性纹理分配移出首个渲染帧，并立即 warmup() 呈现一帧
        // 背景色，让表面立刻可见。
        if let Some(config) = &self.surface_config {
            let (width, height, format) = (config.width, config.height, config.format);
            let (present_mode, latency) =
                (config.present_mode, config.desired_maximum_frame_latency);
            let _ = self.ensure_frame_texture(width, height, format);
            log::info!(
                "attach_surface: configured {scaled_width}x{scaled_height} format={format:?} present={present_mode:?} latency={latency}"
            );
        }
        self.warmup();
        Ok(())
    }

    /// 丢弃已挂载的 surface（Android detach 路径）。
    #[cfg(target_os = "android")]
    pub fn release_surface(&mut self) {
        // 先排干在途取帧：工作线程持有 `Arc` 克隆时，其下的 Vulkan surface 尚未销毁，
        // 而宿主（`TerminalSurface.postDelayedSurfaceRecreate`）紧接着就可能把同一个
        // `ANativeWindow` 送回来重建——那时再发现克隆在途就已经在 `create_surface` 里了
        // （实测同址 attach，距离释放仅 53ms）。排不干**不**阻塞释放（驱动可能永久卡死），
        // 改为记下窗口句柄，让后续的同窗口重建自己拒绝（见 `retired_window`）。
        if !crate::render::pass::drain_in_flight_acquires(
            crate::render::pass::ACQUIRE_DRAIN_TIMEOUT,
        ) {
            log::warn!(
                "release_surface: in-flight acquire did not drain within {}ms; \
                 a later same-window rebuild will be refused until it completes",
                crate::render::pass::ACQUIRE_DRAIN_TIMEOUT.as_millis(),
            );
        }
        // 同 `attach_surface` 的慢路径：先丢帧累加器再丢 surface，让旧纹理的
        // present/discard 还能落在活着的 surface 上。
        self.frame_texture = None;
        self.frame_view = None;
        self.frame_invalidated = true;
        self.retired_window = self.attached_window;
        self.surface = None;
        self.surface_config = None;
        self.attached_window = None;
        self.surface_invalidated = false;
        self.surface_loss_streak = 0;
        log::info!("release_surface: surface dropped");
    }

    /// 能否在已挂载的 surface 上原地 reconfigure（而不是丢弃重建）。
    ///
    /// 同一 `ANativeWindow` 是必要条件：换窗口后缓存 surface 指向的 BufferQueue 已被
    /// 回收，reconfigure 不会复活它（见 `attached_window` 字段）。
    #[cfg(target_os = "android")]
    fn can_reuse_attached_surface(&self, window: usize) -> bool {
        reuse_attached_surface(
            self.surface.is_some(),
            self.surface_config.is_some(),
            self.surface_invalidated,
            self.attached_window,
            window,
        )
    }

    /// 挂载失败后记账：标记 surface 判死，使宿主经 [`Self::surface_invalidated`] 换新
    /// 窗口重试（既有自愈路径）。
    ///
    /// 只挂在失败路径上才有意义：`create_surface_unsafe` / `get_capabilities` /
    /// `select_surface_format` 任一失败都会提前返回，此时 `surface` 已是 `None`。
    /// 而 `render` 对「无 surface」返回 0——与空闲帧不可区分，宿主的换窗口看门狗
    /// 因此永远不触发，终端就此黑屏到 Activity 重建为止。
    pub fn mark_surface_invalidated(&mut self) {
        self.surface_invalidated = true;
    }

    /// 当前缓存的 surface 是否已判死（见 [Self::surface_invalidated]）。
    pub fn surface_invalidated(&self) -> bool {
        self.surface_invalidated
    }

    /// 测试钩子：让此后每帧取纹理都按 surface 级失败处理（见 `surface_loss_injected` 字段），
    /// 直到传入 false 关闭。
    pub fn set_surface_loss_injected_for_test(&mut self, enabled: bool) {
        self.surface_loss_injected = enabled;
    }

    /// 按单帧取纹理结局推进失效判定：拿到纹理或仅本帧跳过都清零计数，
    /// surface 级失败（`Lost`/`Outdated`）连续达 [SURFACE_LOSS_STREAK_LIMIT] 次即
    /// 置失效——单次可由 SurfaceFlinger 缩放竞态引起（实测 `Outdated` 可自愈），
    /// 而死窗口每帧必失败，故以连续次数区分二者。置位幂等：已失效时不再重复记日志。
    fn note_surface_acquire(&mut self, outcome: &AcquireOutcome) {
        let (streak, invalidated) = surface_loss_transition(
            self.surface_loss_streak,
            self.surface_invalidated,
            matches!(outcome, AcquireOutcome::SurfaceLost),
        );
        self.surface_loss_streak = streak;
        if invalidated && !self.surface_invalidated {
            self.surface_invalidated = true;
            log::error!(
                "surface invalidated after {} consecutive acquire failures ({}x{})",
                streak,
                self.surface_config
                    .as_ref()
                    .map_or(0, |config| config.width),
                self.surface_config
                    .as_ref()
                    .map_or(0, |config| config.height),
            );
        }
    }

    pub fn set_background_color(&mut self, background: libghostty_vt::style::RgbColor) {
        self.background = background_clear_color(background);
        // 清除色只在全量帧上生效（部分帧从不再绘网格四边形之外的边距），
        // 故强制下一帧全量，使主题切换能重绘到每一处。
        self.frame_invalidated = true;
    }

    pub fn set_render_paused(&mut self, paused: bool) {
        self.render_paused = paused;
    }

    /// 非有限输入被忽略；Kotlin 侧把该值限制在一行高度内（整行走行通道）。
    pub fn set_viewport_scroll_px(&mut self, px: f32) {
        if px.is_finite() {
            self.viewport_scroll_px = px;
        }
    }

    /// 装载 KGP 图集。返回是否装载成功——**拒绝也必须让调用方知道**：拒绝后纹理为空，
    /// 若调用方仍留着按旧图集算好的实例，非空实例会让部分帧增量渲染被永久关掉
    /// （`kgp_present` 恒真而绑定组恒 `None`），且每帧都因「尺寸漂移」重打包一次。
    pub fn set_kgp_atlas(&mut self, rgba_data: &[u8], width: u32, height: u32) -> bool {
        // 换图集 = 换掉累加器里那些像素的内容，故下一帧必须全量自包含重绘。
        // 漏掉的后果只在**清空**分支现形：图像消失后 `kgp_instances` 为空，
        // `should_render_partial_frame` 于是重新放行部分帧，而图像原先压住的那些行
        // 若并不脏（kitty 图常叠在空白单元格上），此后每帧只画脏带——图像像素在
        // 屏幕上永久残留，重启会话前消不掉。换图集分支同理：新图集的 UV 布局与
        // 旧图集无关，未重绘的行上留的是上一张图的残片。
        self.frame_invalidated = true;
        // `write_texture` 在数据不足 `bytes_per_row * height` 时 panic，而这份数据
        // 经 JNI 跨越 FFI：调用方算错一行字节数就会把渲染线程直接带走。
        // 长度不符是调用方的缺陷，故此处拒绝并留证据，而不是让 wgpu 抛。
        let Some(expected_len) = (width as usize)
            .checked_mul(4)
            .and_then(|row| row.checked_mul(height as usize))
        else {
            log::warn!(
                "set_kgp_atlas: {}x{} overflows usize; atlas rejected",
                width,
                height
            );
            self.reject_kgp_atlas();
            return false;
        };
        if rgba_data.len() < expected_len {
            log::warn!(
                "set_kgp_atlas: {}x{} needs {expected_len} bytes, got {}; atlas rejected",
                width,
                height,
                rgba_data.len()
            );
            self.reject_kgp_atlas();
            return false;
        }
        if width == 0 || height == 0 {
            self.reject_kgp_atlas();
            return false;
        }
        // 超出设备上限时 `create_texture` 返回无效句柄，装上去也不会有任何图像，
        // 只有逐帧的校验错误——与长度不符同款处理：拒绝并清空。
        let max_side = self.device.limits().max_texture_dimension_2d;
        if width > max_side || height > max_side {
            log::warn!(
                "set_kgp_atlas: {}x{} exceeds device limit {max_side}; atlas rejected",
                width,
                height
            );
            self.reject_kgp_atlas();
            return false;
        }
        let device = &self.device;
        let queue = &self.queue;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let tex = create_rgba_texture(device, "kgp_atlas", size);
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
        self.kgp_atlas_width = width;
        self.kgp_atlas_height = height;
        self.kgp_bind_group = None;
        true
    }

    /// 拒绝一次图集装载：清空纹理与尺寸，使上传侧不再每帧判定「尺寸漂移」而重打包。
    fn reject_kgp_atlas(&mut self) {
        self.kgp_texture = None;
        self.kgp_bind_group = None;
        self.kgp_atlas_width = 0;
        self.kgp_atlas_height = 0;
    }
}

/// 单层 RGBA8 采样纹理：字形图集与 KGP 图集共用同一采样约定。
pub(super) fn create_rgba_texture(
    device: &wgpu::Device,
    label: &str,
    size: wgpu::Extent3d,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

impl Renderer {
    pub fn initialize_pipeline_and_bind_group(
        &mut self,
        atlas_width: u32,
        atlas_height: u32,
        config_width: u32,
        config_height: u32,
    ) {
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |surface_config| {
                surface_config.format
            });
        self.pipeline_format = format;
        self.cell_pipeline = Some(Self::create_cell_pipeline(&self.device, format));

        self.projection_width = config_width;
        self.projection_height = config_height;
        self.create_atlas_texture(atlas_width, atlas_height);

        self.write_uniforms(
            atlas_width as f32,
            atlas_height as f32,
            config_width as f32,
            config_height as f32,
        );

        log::info!(
            "initialize_pipeline_and_bind_group: pipeline={} atlas={}x{} surface={}x{}",
            self.cell_pipeline.is_some(),
            atlas_width,
            atlas_height,
            config_width,
            config_height,
        );
    }
}
