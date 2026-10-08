//! GPU 上下文：把 wgpu 的 instance、adapter、device 与管线集中到单个 `Renderer` 结构。
use std::sync::OnceLock;
use wgpu::util::DeviceExt;

use crate::render::pass::{ACQUIRE_TIMEOUT, AcquireOutcome};
use crate::render::pipeline::QUAD_CORNERS;
use crate::render::{CATPPUCCIN_MOCHA_BACKGROUND, GpuError};

pub(crate) fn log_gpu_error(error: &wgpu::Error) {
    log::error!("GPU_UNCAPTURED_ERROR: {error:#?}");
}

/// 连续跳帧达此数即把 worker 死亡从 warn 升级为 error（60fps 下约 5 秒）。
const SKIP_FATAL_FRAME_LIMIT: u32 = 300;

/// 连续多少次 surface 级取纹理失败即判死窗口（见 [`Renderer::note_surface_acquire`]）。
///
/// 取 2 而非 1：单次 `Outdated` 可由 SurfaceFlinger 缩放竞态引起并在下一次
/// `reconfigure` 后自愈，死窗口则每帧必失败，连续两次足以区分且恢复延迟只有一帧。
const SURFACE_LOSS_STREAK_LIMIT: u8 = 2;

/// surface 失效判定的纯决策：给定（当前连续失败数、是否已失效、本帧是否 surface 级
/// 失败），返回（新的连续失败数、新的失效标志）。抽成纯函数以便宿主单测覆盖，
/// 不必构造 `Renderer`/GPU 设备。
///
/// 非 surface 级失败（拿到纹理或本帧跳过）一律清零计数——慢机器的连续超时不应被判死。
/// 已失效时保持失效（幂等），计数仍饱和累加以便日志可读。
pub(crate) fn surface_loss_transition(
    streak: u8,
    invalidated: bool,
    surface_lost: bool,
) -> (u8, bool) {
    if !surface_lost {
        return (0, invalidated);
    }
    let streak = streak.saturating_add(1);
    (streak, invalidated || streak >= SURFACE_LOSS_STREAK_LIMIT)
}

/// 逐帧渲染上下文：打包 encoder、surface 纹理与 view，把短生命周期资源与长生命周期的
/// `Renderer` 状态分开。由 `Renderer::begin_frame()` 创建。
pub struct FrameContext {
    pub(crate) encoder: wgpu::CommandEncoder,
    pub(crate) view: wgpu::TextureView,
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) config_width: u32,
    pub(crate) config_height: u32,
}

impl FrameContext {
    pub fn submit(self, queue: &wgpu::Queue) {
        queue.submit(std::iter::once(self.encoder.finish()));
        queue.present(self.texture);
    }
}

pub(crate) struct GlobalGpu {
    #[cfg(target_os = "android")]
    pub(crate) instance: wgpu::Instance,
    #[cfg(target_os = "android")]
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
}

#[cfg(test)]
pub(crate) fn global_gpu_for_tests() -> &'static GlobalGpu {
    global_gpu()
}

/// 初始化 Vulkan 全局对象；失败时按可操作的指引记账（原因只有这一处需要讲清）。
fn init_gpu() -> Result<GlobalGpu, GpuError> {
    match futures::executor::block_on(crate::render::wgpu_backend::initialize_wgpu()) {
        Ok((_inst, _adapt, device, queue)) => Ok(GlobalGpu {
            #[cfg(target_os = "android")]
            instance: _inst,
            #[cfg(target_os = "android")]
            adapter: _adapt,
            device,
            queue,
        }),
        Err(initialization_error) => {
            log::error!("GPU initialization failed: {initialization_error}");
            log::error!("Solution: ensure a Vulkan-capable GPU is available.");
            log::error!("  - Linux desktop: set VK_ICD_FILENAMES to a lavapipe or Mesa driver");
            log::error!("  - Android emulator: use SwiftShader (default with GPU emulation)");
            log::error!("  - Physical device: install Vulkan drivers for your hardware");
            Err(initialization_error)
        }
    }
}

fn gpu_instance() -> &'static OnceLock<GlobalGpu> {
    static INSTANCE: OnceLock<GlobalGpu> = OnceLock::new();
    &INSTANCE
}

/// 渲染路径取全局 GPU：不可用即致命（无法渲染的终端是坏的，不应跚行运转）。
fn global_gpu() -> &'static GlobalGpu {
    gpu_instance().get_or_init(|| match init_gpu() {
        Ok(global) => global,
        Err(initialization_error) => panic!(
            "GPU initialization failed: {initialization_error}. \
             See log for details."
        ),
    })
}

/// 可失败取全局 GPU：失败不致命，供启动预热等非渲染路径使用（见
/// `Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState`）。
///
/// 失败**不写入** `OnceLock`，故后续渲染路径仍会重试初始化：设备上的 Vulkan 初始化
/// 在进程刚起来的数秒内可能失败（GPU 尚被别的应用占用），而同一设备稍后即可成功，
/// 预热不该把一次瞬时失败固化成启动即崩。
pub(crate) fn try_global_gpu() -> Result<&'static GlobalGpu, GpuError> {
    if let Some(initialized) = gpu_instance().get() {
        return Ok(initialized);
    }
    let global = init_gpu()?;
    // 并发下可能已被另一个线程写入：取已缓存者，丢弃本次多余结果。
    Ok(gpu_instance().get_or_init(|| global))
}

/// GPU 渲染器：持有 wgpu 资源与管线。字段按空行分组：核心资源、单元管线资源、图集资源、
/// Kitty 图形协议（kgp_*）、逐帧瞬时状态。
///
/// 线程安全：`Renderer` 为 `Send + Sync`，位于全局 `RENDER_STATE` 互斥锁后，
/// `begin_frame()` 与 `render_frame()` 由单一渲染线程以 `&mut self` 调用。
pub struct Renderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: Option<std::sync::Arc<wgpu::Surface<'static>>>,
    pub(crate) surface_config: Option<wgpu::SurfaceConfiguration>,
    /// 连续 surface 级取纹理失败的计数（见 [Self::note_surface_acquire]）。
    surface_loss_streak: u8,
    /// 连续「本帧跳过」计数：工作线程死亡或取纹理持续超时时，帧循环每帧
    /// 只记 warn 不升级——累计到上限必须以 error 报出「屏幕冻结」这一事实。
    consecutive_skip_frames: u32,
    /// surface 已判死：缓存的 `surface` 指向已废弃的 BufferQueue，reconfigure 永不
    /// 复活（实测 abandoned BufferQueue 下每帧 `Lost` + `ERROR_SURFACE_LOST_KHR`，
    /// 22 分钟零帧上屏）。置位后 `attach_surface` 改走重建慢路径，且由
    /// `renderWithNewOutput` 的状态位上报宿主换新的 `ANativeWindow`。
    surface_invalidated: bool,
    /// 测试钩子：置位后每帧取纹理都按 surface 级失败处理。
    ///
    /// 真机上被遗弃的 BufferQueue 无法从进程内制造（那是 SurfaceFlinger 的回收决策），
    /// 而自愈路径若只能等系统回收来验证，就永远拿不到回归护栏。持续注入复刻的正是
    /// 实测到的真实形态（`abandoned=6148`、`begin_frame failed=3074`：每帧都失败），
    /// 供仪器化用例断言「宿主换新 `SurfaceView` 后画面重新有墨迹」。
    ///
    /// 只经进程内 JNI 暴露，无生产调用方；由调用方显式关闭。
    surface_loss_injected: bool,
    pub(crate) cell_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) quad_vertex_buffer: wgpu::Buffer,
    pub(crate) cell_bind_group: Option<wgpu::BindGroup>,
    pub(crate) cell_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) instance_buffer: Option<wgpu::Buffer>,
    /// 跨帧复用的 CPU 侧实例缓冲（避免每帧约 100KB 分配）。
    pub(crate) cpu_instances: Vec<crate::render::CellInstance>,
    /// 行级脏区实例缓存：保留各行的实例切片，使干净行只复制而不重建。
    pub(crate) cell_cache: Option<crate::render::cell_builder::CachedInstances>,
    /// 可复用的全 true 脏标记（长度为当前网格行数），用于 `cell_cache` 刚从零重建后
    /// 的那一帧（resize）：此时把“干净”行当作无缓存会导致丢行。
    pub(crate) cell_full_mask_cache: Vec<bool>,
    pub(crate) viewport_scroll_px: f32,
    /// 上一呈现帧实际使用的视口像素偏移。当前偏移非零或与此值不同
    /// 时，pass.rs 禁用脏带部分路径：纹理拷贝无法亚像素平移累加器，
    /// 部分路径会把旧像素与平移后的新几何混叠（滚动残留/撕裂）。
    pub(crate) last_drawn_viewport_scroll_px: f32,
    pub(crate) atlas_texture: Option<wgpu::Texture>,
    pub(crate) atlas_view: Option<wgpu::TextureView>,
    pub(crate) atlas_sampler: Option<wgpu::Sampler>,
    pub(crate) pipeline_format: wgpu::TextureFormat,
    pub(crate) projection_width: u32,
    pub(crate) projection_height: u32,
    pub(crate) readback_texture: Option<wgpu::Texture>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
    pub(crate) background: wgpu::Color,
    pub(crate) kgp_pipeline: Option<wgpu::RenderPipeline>,
    /// 创建 kgp_pipeline 时所用的表面格式；表面格式变更后必须重建管线。
    pub(crate) kgp_pipeline_format: Option<wgpu::TextureFormat>,
    pub(crate) kgp_bind_group_layout: Option<wgpu::BindGroupLayout>,
    pub(crate) kgp_bind_group: Option<wgpu::BindGroup>,
    pub(crate) kgp_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_sampler: Option<wgpu::Sampler>,
    pub(crate) kgp_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_texture: Option<wgpu::Texture>,
    pub(crate) kgp_atlas_width: u32,
    pub(crate) kgp_atlas_height: u32,
    pub(crate) raster_scale: f32,
    pub(crate) render_paused: bool,
    /// 持久离屏帧累加器：权威帧内容跨帧存活，使部分（脏带）帧可合成到上次输出上，
    /// 每帧一次 `copy_texture_to_texture` 呈现到交换链。
    pub(crate) frame_texture: Option<wgpu::Texture>,
    /// 累加器内容陈旧/未知，下一帧必须全量重绘（首帧、resize、格式变更、surface
    /// 重挂载）；成功全量绘制后清除。
    pub(crate) frame_invalidated: bool,
    /// 交换链是否支持 `COPY_DST`（挂载时依据 `usages` 检查）；为 false 时走旧的
    /// 直接渲染路径并忽略脏带。
    pub(crate) swapchain_copy_supported: bool,
}

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
            // 新纹理内容未定义，下一帧必须全量重绘。
            self.frame_invalidated = true;
        }
        self.frame_texture
            .as_ref()
            .map(|frame_texture| frame_texture.create_view(&wgpu::TextureViewDescriptor::default()))
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
            log::warn!(
                "begin_frame: size mismatch! config={}x{} texture={}x{}; reconfiguring and skipping frame",
                config_width,
                config_height,
                tex_size.width,
                tex_size.height
            );
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

        // uniform 缓冲内容每帧重写；绑定组按对象标识绑定，仍然有效。
        self.refresh_cell_uniforms(config_width as f32, config_height as f32);

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
        self.frame_texture = None;
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
        self.kgp_bind_group_layout = None;
        self.kgp_pipeline = None;
        self.kgp_pipeline_format = None;
        self.kgp_texture = None;
        self.surface = None;
    }
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
            background: CATPPUCCIN_MOCHA_BACKGROUND,
            kgp_pipeline: None,
            kgp_pipeline_format: None,
            kgp_bind_group_layout: None,
            kgp_bind_group: None,
            kgp_uniform_buffer: None,
            kgp_sampler: None,
            kgp_instance_buffer: None,
            kgp_texture: None,
            kgp_atlas_width: 0,
            kgp_atlas_height: 0,
            raster_scale: 1.0,
            render_paused: false,
            frame_texture: None,
            frame_invalidated: true,
            swapchain_copy_supported: false,
        }
    }

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
        if self.surface.is_some() && self.surface_config.is_some() && !self.surface_invalidated {
            self.reconfigure_swapchain(width, height);
            log::info!("attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)");
            return Ok(());
        }
        if self.surface_invalidated {
            log::warn!(
                "attach_surface: previous surface invalidated; rebuilding from ANativeWindow"
            );
        }
        let handle = AndroidNdkWindowHandle::new(non_null.cast());
        // 先释放旧 surface 再建新 surface：Android 上两个 wgpu surface 包裹同一个
        // ANativeWindow，旧 surface 仍存活时无法为该窗口创建 Vulkan swapchain，
        // 随后的 get_current_texture 永远报 “Surface is not configured for presentation”
        // （模拟器实测：首次之后的每个会话都黑屏）。调用方保证此刻无渲染线程处于帧中
        // （switchSession 会先停旧线程），故可安全丢弃。
        self.surface = None;
        self.surface_config = None;
        // SAFETY:
        // - `global_gpu().instance` 是有效的 wgpu Instance；
        // - 句柄包裹调用方保证存活的 ANativeWindow（JNI attachWindow 契约），
        //   存活至 detachWindow 丢弃由其产生的 surface——wgpu 在创建时自行取得引用，
        //   调用方在本调用返回后即释放自己的引用；
        // - Android 上无 X11/Wayland display。
        let surface = unsafe {
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
        }
        .map_err(|error| {
            GpuError::Surface(format!(
                "attach_surface: wgpu create_surface failed: {error}"
            ))
        })?;
        // 重建即视为恢复：失效位与连续失败计数随之清零，新 surface 重新接受判定。
        self.surface_invalidated = false;
        self.surface_loss_streak = 0;
        // 累加器内容属于旧 surface，重挂载后强制全量重绘。
        self.frame_texture = None;
        self.frame_invalidated = true;
        let surface = std::sync::Arc::new(surface);
        // 默认配置挑选驱动支持的呈现模式与格式；`RENDER_SCALE` 与 `reconfigure_swapchain`
        // 一致（跨渲染器固定的缩放，保证单元格度量一致）。
        let scaled_width = ((width as f32 * crate::render::RENDER_SCALE) as u32).max(1);
        let scaled_height = ((height as f32 * crate::render::RENDER_SCALE) as u32).max(1);
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
        // 优先非 sRGB 变体：Android SurfaceFlinger 默认 RGBA_8888（非 sRGB）；模拟器上
        // SwiftShader 交换链格式与 Surface 原生格式不符时缓冲区队列无法分配。
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|candidate| !candidate.is_srgb())
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
            // 三个缓冲（max_frame_latency=3）：FIFO vsync 下单缓冲时渲染线程会阻塞在
            // acquire 直到显示端消费上一帧，任何超过一个 vsync 周期的帧都会拖垮整条
            // 管线，帧时间还会混叠到 vsync 倍数（Mali 设备实测 20-30fps）。三缓冲使
            // acquire 立即返回、渲染管线与扫描输出解耦；Mailbox（驱动支持时已在上方
            // 选中）丢弃最旧的排队帧而不回压渲染线程——滚动终端中新帧永远胜出。
            desired_maximum_frame_latency: 3,
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
            self.kgp_bind_group_layout = None;
            log::info!(
                "attach_surface: surface format changed {previous_format:?} -> {:?}, cell/kitty pipelines scheduled for rebuild",
                self.pipeline_format,
            );
        }
        self.projection_width = scaled_width;
        self.projection_height = scaled_height;
        self.surface = Some(surface);
        // ── 启动黑屏防护 ─────────────────────────────
        // 首个内容帧要等 shell 输出 + 冷启动（SwiftShader 上实测数百毫秒：字形整形、
        // 整幅 atlas 上传、纹理分配）；期间交换链一帧未提交，屏幕保持空黑，静默 shell
        // （无任何新数据）甚至永远黑屏。故在 attach 线程上（渲染线程未启动，无竞争）
        // 预先创建帧累加器，把一次性纹理分配移出首个渲染帧，并立即 warmup() 呈现一帧
        // 背景色，让表面立刻可见。
        if let Some(config) = &self.surface_config {
            let _ = self.ensure_frame_texture(config.width, config.height, config.format);
        }
        self.warmup();
        log::info!("attach_surface: configured {scaled_width}x{scaled_height}");
        Ok(())
    }

    /// 丢弃已挂载的 surface（Android detach 路径）。
    #[cfg(target_os = "android")]
    pub fn release_surface(&mut self) {
        self.surface = None;
        self.surface_config = None;
        self.surface_invalidated = false;
        self.surface_loss_streak = 0;
        self.frame_texture = None;
        self.frame_invalidated = true;
        log::info!("release_surface: surface dropped");
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

    pub fn set_background_color(&mut self, background: [u8; 3]) {
        self.background = wgpu::Color {
            r: background[0] as f64 / 255.0,
            g: background[1] as f64 / 255.0,
            b: background[2] as f64 / 255.0,
            a: 1.0,
        };
        // 清除色只在全量帧上生效（部分帧从不再绘网格四边形之外的边距），
        // 故强制下一帧全量，使主题切换能重绘到每一处。
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

    /// 非有限输入被忽略；Kotlin 侧把该值限制在一行高度内（整行走行通道）。
    pub fn set_viewport_scroll_px(&mut self, px: f32) {
        if px.is_finite() {
            self.viewport_scroll_px = px;
        }
    }

    pub fn set_kgp_atlas(&mut self, rgba_data: &[u8], width: u32, height: u32) {
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
            return;
        };
        if rgba_data.len() < expected_len {
            log::warn!(
                "set_kgp_atlas: {}x{} needs {expected_len} bytes, got {}; atlas rejected",
                width,
                height,
                rgba_data.len()
            );
            return;
        }
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
    }
}

/// 单层 RGBA8 采样纹理：字形图集与 KGP 图集共用同一采样约定。
fn create_rgba_texture(device: &wgpu::Device, label: &str, size: wgpu::Extent3d) -> wgpu::Texture {
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

/// 为给定视口尺寸创建正交投影矩阵。
pub fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

/// 按视口 Y 像素偏移平移正交投影（正值 = 内容下移）。屏幕 Y 向下增长而 NDC Y
/// 向上增长，故偏移从平移行中减去；高度或偏移为 0 时矩阵不变。
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

/// 顶点缓冲的下限字节数。wgpu 拒绝 0 字节的缓冲，而调用方的空列表提前返回
/// 并不构成该下限的来源——留一个最小值让契约与 wgpu 的要求对齐。
pub const MIN_VERTEX_BUFFER_SIZE: u64 = 64;

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
            let needed = offset as usize
                + upload_height as usize
                    * upload_width as usize
                    * crate::render::font::ATLAS_BYTES_PER_PIXEL;
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
            raster_scale: self.raster_scale,
            _padding: 0.0,
        }
    }

    /// 写入 uniforms 并重建单元绑定组。
    ///
    /// 由 [`Renderer::update_bind_group`] 与 [`Renderer::initialize_pipeline_and_bind_group`] 共用，
    /// 避免约 40 行重复的 uniform 构建 + 缓冲写入 + 绑定组创建。
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

impl Renderer {
    /// 宿主构建排除了仅 Android 的 `attach_surface` 调用方，本函数在宿主上会成为死代码，
    /// 故限定为 Android 生产构建 + 测试，而非加 `allow(dead_code)`。
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn select_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
        // 渲染稳定性 spec §3「滚动不得有可见撕裂」：Immediate 无 vsync 直通
        // 扫描线，滚动时必然撕裂，且让渲染线程无界冲刷（真机实测 166fps，
        // 多数帧被显示端丢弃）。优先 Mailbox（vsync 节拍 + 新帧覆盖旧帧、
        // 无背压——滚动终端里「最新帧永远胜出」正是所需取舍，见下方
        // attach_surface 对 desired_maximum_frame_latency=3 的注释），其次
        // Fifo（标准 vsync），再 AutoVsync；Immediate 仅作驱动只支持它时的
        // 最后兜底。
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

        // 累加器内容属于旧尺寸，reconfigure 后强制全量重绘（同 attach 慢路径）。
        // 否则空闲 shell 会永远持有重建后为空的累加器——空闲门控只在
        // `frame_invalidated` 时重绘，导致输入法 resize / 切后台且 surface 保留时黑屏或闪烁。
        self.frame_texture = None;
        self.frame_invalidated = true;

        self.projection_width = scaled_width;
        self.projection_height = scaled_height;

        if let Some(buf) = &self.cell_uniform_buffer {
            let atlas_width = self
                .atlas_texture
                .as_ref()
                .map_or(0, |atlas_texture| atlas_texture.width());
            let atlas_height = self
                .atlas_texture
                .as_ref()
                .map_or(0, |atlas_texture| atlas_texture.height());
            let uniforms = self.cell_uniforms(
                scaled_width as f32,
                scaled_height as f32,
                atlas_width as f32,
                atlas_height as f32,
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
