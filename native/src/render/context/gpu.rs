use std::sync::OnceLock;

use crate::render::GpuError;
use crate::terminal::ghostty_terminal::GhosttyTerminal;

// ── GPU 初始化与错误分类 ────────────────────────────────────────────
/// 初始清除色：与终端初态同源（默认主题背景），避免首帧黑闪。
/// 运行时主题经 `set_background_color` 更新，此处只覆盖到达前的窗口。
pub(super) fn initial_background_color() -> wgpu::Color {
    background_clear_color(crate::terminal::theme::default_dark().palette.background)
}

/// 主题背景转清除色：与终端侧字节转浮点同源，避免两处手写除法。
pub(super) fn background_clear_color(background: libghostty_vt::style::RgbColor) -> wgpu::Color {
    let [red, green, blue, _] = GhosttyTerminal::color_to_float(background);
    wgpu::Color {
        r: f64::from(red),
        g: f64::from(green),
        b: f64::from(blue),
        a: 1.0,
    }
}

/// wgpu 的 surface 配置 API 名（出现在校验错误的描述里）。
///
/// 来自 wgpu-core 的 `handle_error_nolabel(..., "Surface::configure")`：那是
/// 上游自己给出的调用点标识，不是本仓硬编码的文案。
const SURFACE_CONFIGURE_API: &str = "Surface::configure";

/// wgpu-core 对 `ConfigureSurfaceError::UnsupportedQueueFamily` 的描述片段。
///
/// 该错误只在 `surface.get_capabilities()` 失败时产生（wgpu-core 的
/// `configure_surface`：`Err(_) => break 'error E::UnsupportedQueueFamily`），
/// 而 Android 上该调用失败的常态只有一个：原生窗口已被平台收回
/// （BufferQueue 被遗弃），surface 不再报告任何可用队列族。
const UNSUPPORTED_QUEUE_FAMILY: &str = "does not support the adapter's queue family";

/// 「配置尺寸与纹理尺寸不符」告警的限频闸门。
///
/// 重配本应让下一帧尺寸一致，但重配未生效时该告警会逐帧复现；逐条记会把 logcat
/// 刷成告警洪水。真正的状态变化（重配后的尺寸）在后续诊断里仍可见。
pub(super) static SIZE_MISMATCH_THROTTLE: crate::slow_log::SlowLogThrottle =
    crate::slow_log::SlowLogThrottle::new();

/// 未捕获 GPU 错误是否属于「平台收走原生窗口后 wgpu 重新配置 surface 必然失败」这一类。
///
/// 判据是错误**种类**而非文案泛泛：校验错误同时出现 wgpu 的配置 API 名
/// `Surface::configure` 与 `UnsupportedQueueFamily` 的描述。前者保证错误出在
/// 配置 surface 上，后者保证失败原因是「这个 surface 已经不能用了」——不是
/// 格式不支持、view_formats 不合法、尺寸为零等自家配置错误。
///
/// 这是平台事件的连带结果（切后台后系统收回窗口，自愈路径会换新窗口重挂载），
/// 不是应用缺陷，故按 debug 记；其余未捕获错误一律 error。
///
/// 上游措辞若变，判据只会失效成「一律 error」——更吵，不会漏报。
pub(crate) fn gpu_error_is_surface_lost(error: &wgpu::Error) -> bool {
    match error {
        wgpu::Error::Validation { description, .. } => {
            description.contains(SURFACE_CONFIGURE_API)
                && description.contains(UNSUPPORTED_QUEUE_FAMILY)
        }
        _ => false,
    }
}

pub(crate) fn log_gpu_error(error: &wgpu::Error) {
    if gpu_error_is_surface_lost(error) {
        log::debug!("GPU_UNCAPTURED_ERROR (surface lost after window reclaim): {error:#?}");
    } else {
        log::error!("GPU_UNCAPTURED_ERROR: {error:#?}");
    }
}

/// 连续跳帧达此数即把 worker 死亡从 warn 升级为 error（60fps 下约 5 秒）。
pub(super) const SKIP_FATAL_FRAME_LIMIT: u32 = 300;

/// 连续多少次 surface 级取纹理失败即判死窗口（见 [`Renderer::note_surface_acquire`](super::Renderer::note_surface_acquire)）。
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
pub(super) fn global_gpu() -> &'static GlobalGpu {
    initialize_global_gpu().unwrap_or_else(|initialization_error| {
        panic!("GPU initialization failed: {initialization_error}. See log for details.")
    })
}

/// 可失败取全局 GPU：失败不致命，供启动预热等非渲染路径使用（见
/// `Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState`）。
pub(crate) fn try_global_gpu() -> Result<&'static GlobalGpu, GpuError> {
    initialize_global_gpu()
}

/// GPU 初始化的唯一实现（致命渲染路径与可失败的预热路径共用，故只有这一处）。
///
/// 判空与写入之间持一把初始化锁，使 `init_gpu`（建 Instance、request_adapter、
/// request_device）在进程内**只发生一次**。先前的三段式
/// `get` 判空 → `init_gpu` → `get_or_init` 在判空与写入之间并不原子：预热线程与
/// 渲染线程在启动瞬间并发时，两者各自跑完一次完整初始化，第二次的
/// device/queue/adapter 被 `get_or_init` 直接丢弃——白耗一次显存与驱动资源
/// （真机 logcat 实证：`GPU adapter` 与 `GPU device created` 各打印两遍，分属两个线程）。
///
/// 失败**不写入** `OnceLock`（锁在返回 Err 前释放），故后续渲染路径仍会重试初始化：
/// 设备上的 Vulkan 初始化在进程刚起来的数秒内可能失败（GPU 尚被别的应用占用），
/// 而同一设备稍后即可成功，预热不该把一次瞬时失败固化成启动即崩。
fn initialize_global_gpu() -> Result<&'static GlobalGpu, GpuError> {
    static INITIALIZATION: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    if let Some(initialized) = gpu_instance().get() {
        return Ok(initialized);
    }
    let _initialization = INITIALIZATION.lock();
    // 持锁后重查：等锁期间可能已由另一线程初始化完毕。
    if let Some(initialized) = gpu_instance().get() {
        return Ok(initialized);
    }
    let global = init_gpu()?;
    #[cfg(test)]
    INITIALIZATION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(gpu_instance().get_or_init(|| global))
}

/// `init_gpu` 成功完成的次数，仅测试读取：生产构建不声明。
///
/// 计数使「初始化只发生一次」可断言：仅比对各线程拿到的引用做不到这一点——
/// `OnceLock` 一旦填满，修复前（并发下各自初始化、后者被丢弃）与修复后的代码
/// 都让每个线程拿到同一引用，故该比对对竞态零判别力。
#[cfg(test)]
pub(crate) static INITIALIZATION_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
