use std::sync::Arc;

use crate::render::GpuError;
use crate::terminal::ghostty_terminal::GhosttyTerminal;

// ── GPU 初始化与错误分类 ────────────────────────────────────────────
/// 初始清除色：与终端初态同源（默认主题背景），避免首帧黑闪。
/// 运行时主题经 `set_background_color` 更新，此处只覆盖到达前的窗口。
pub(super) fn initial_background_color() -> wgpu::Color {
    background_clear_color(crate::terminal::theme::default_dark().palette.background)
}

/// 主题背景转清除色：与终端侧字节转浮点同源，避免两处手写除法。
pub(super) fn background_clear_color(background: libghostty_vt::color::Rgb) -> wgpu::Color {
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
/// （`BufferQueue` 被遗弃），surface 不再报告任何可用队列族。
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
/// 格式不支持、`view_formats` 不合法、尺寸为零等自家配置错误。
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
        return;
    }
    let detail = format!("{error:#?}");
    // 设备丢失本身只在第一次记：余波阶段的每条错误都是同一次丢失。
    if is_device_loss_text(&detail) {
        mark_device_lost(&one_line(&detail));
    } else {
        log::error!("GPU_UNCAPTURED_ERROR: {detail}");
    }
}

/// 连续跳帧达此数即把 worker 死亡从 warn 升级为 error（60fps 下约 5 秒）。
pub(super) const SKIP_FATAL_FRAME_LIMIT: u32 = 300;

/// 连续多少次 surface 级取纹理失败即判死窗口（见 [`Renderer::note_surface_acquire`](super::Renderer::note_surface_acquire)）。
///
/// 取 2 而非 1：单次 `Outdated` 可由 `SurfaceFlinger` 缩放竞态引起并在下一次
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

/// 原生窗口判死的日志级别：正常退化记 debug。
///
/// 平台收走原生窗口后取纹理必然失败，宿主换新窗口重挂载即恢复——这是平台事件的
/// 正常后果，按 `logcat-hygiene` 规范「平台收走原生窗口后的瞬时渲染错误」记
/// debug。新窗口迟迟不来的异常由宿主侧的重建预算裁决并升级为 error，原生侧
/// 不先一步报成告警。
pub(crate) const SURFACE_INVALIDATION_LOG_LEVEL: log::Level = log::Level::Debug;

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
    /// 本设备是否已判死。
    ///
    /// 标志与设备**同生共死**：丢失回调注册时拿到同一份（见
    /// [`register_device_lost_callback`]），故旧设备的丢失回调迟到时不会把重建后的
    /// 新设备误判成判死。重建路径据此判定「槽位里那个还能不能用」
    /// （见 [`gpu_slot_reusable`]）。
    lost: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
pub(crate) fn global_gpu_for_tests() -> Arc<GlobalGpu> {
    global_gpu()
}

/// 进程内的 GPU 设备是否已丢失。
///
/// 真机实证（2026-11-14，Mali）：切后台时 Vulkan 把设备判死，此后每次取纹理或
/// 轮询都在 wgpu 内部 `panic`（该次日志 207 条 `panicked ... Parent device is lost`），
/// 宿主换的新窗口也挂在**同一个死设备**上，画面永久黑屏。
///
/// wgpu 在致命 HAL 错误上**先**调设备丢失回调、**再** panic（`wgpu-core` 的
/// `Device::lose`），故回调置位后所有 GPU 入口都能在 panic 之前停下。
///
/// 只有「新设备 + 新 `Renderer` 都已就位」才清除（见 `attachWindow` 的重建路径）。
static DEVICE_LOST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn device_lost() -> bool {
    DEVICE_LOST.load(std::sync::atomic::Ordering::Acquire)
}

/// 新设备与新 `Renderer` 都装好后放行 GPU 入口（见 `attachWindow` 的重建路径）。
///
/// 连同槽位里那个设备的判死位一起清：判死位是「这个设备配不到新 surface」的
/// 事实，仪器化用例的注入撤销（`setDeviceLostInjected(false)`）也要把它清掉，
/// 否则正常路径会以为槽位里的设备已判死而反复重建。
pub(crate) fn clear_device_lost() {
    if let Some(current) = current_gpu() {
        current
            .lost
            .store(false, std::sync::atomic::Ordering::Release);
    }
    DEVICE_LOST.store(false, std::sync::atomic::Ordering::Release);
}

/// 置位丢失位，并且只在本进程**第一次**记账时记一条 error。
///
/// 丢失之后每个 GPU 调用都会失败，逐条记只是同一次丢失的余波；调用方因此只需把
/// 「发现丢失」这件事交给这里，不必各自决定日志级别。置位点是丢失回调
/// （[`register_device_lost_callback`]），以及回调覆盖不到的文本判据路径
/// （[`is_device_loss_text`]）；仪器化用例也经这里注入。
///
/// 文本判据路径拿不到设备身份，只能按「当前设备」置位：此刻正在用的就是出错的
/// 那个。回调路径由 [`register_device_lost_callback`] 精确置位（置的是它自己那
/// 一份标志），不在此处。
pub(crate) fn mark_device_lost(detail: &str) {
    if let Some(current) = current_gpu() {
        current
            .lost
            .store(true, std::sync::atomic::Ordering::Release);
    }
    record_device_loss(detail);
}

/// 判死记账：置全局丢失位，并且只在本进程**第一次**记账时记一条 error。
///
/// 与 [`mark_device_lost`] 分开：丢失回调置的是**它那一个**设备的标志，若在此
/// 再按「当前设备」置一次，旧设备的丢失回调迟到时会把重建后的新设备误判成判死。
fn record_device_loss(detail: &str) {
    if !DEVICE_LOST.swap(true, std::sync::atomic::Ordering::Release) {
        log::error!(
            "GPU device lost ({detail}); rendering stops until a new surface attach rebuilds the device"
        );
    }
}

/// 把原因压成单行：wgpu 的错误文本自带缩进与换行树，压平后仍能一眼读出
/// 「哪一步、什么错」，也不会在按行采集的日志里断成半截。
pub(crate) fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// 设备丢失的文本判据。
///
/// wgpu 有若干路径不经过丢失回调、只在错误文字里给出设备已判死：`DeviceError::Lost`
/// 的 Display 是 "Parent device is lost"，`SurfaceError::DeviceLost` 是 "Device Lost"。
/// panic 载荷与轮询错误都是这种文字（真机日志：`Error in Device::poll: Validation Error
/// | Caused by: | Parent device is lost`），故在**第一次**认出时同样置位——否则逐帧
/// 调用会逐条复现同一条 panic（该次日志 207 条）。
pub(crate) fn is_device_loss_text(text: &str) -> bool {
    ["Parent device is lost", "Device Lost", "device is lost"]
        .iter()
        .any(|marker| text.contains(marker))
}

/// 设备丢失回调：wgpu 判决设备不可用时调用（Vulkan `VK_ERROR_DEVICE_LOST`、
/// 驱动 OOM 等），在 wgpu 以 panic 结束当前调用**之前**执行。
///
/// 这是丢失的唯一生产置位点。原因只在这里记一次：逐帧的取帧/轮询路径此后看到
/// 标志即停，不再逐条重复报同一件事，也不再碰到死设备。
///
/// 返回与设备同生共死的判死标志（见 [`GlobalGpu::lost`]）：回调置的是**这一个**
/// 设备的标志，旧设备的丢失回调迟到时不会把重建后的新设备误判成判死。
fn register_device_lost_callback(device: &wgpu::Device) -> Arc<std::sync::atomic::AtomicBool> {
    let lost = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&lost);
    device.set_device_lost_callback(move |reason, message| {
        flag.store(true, std::sync::atomic::Ordering::Release);
        record_device_loss(&format!("{reason:?}: {message}"));
    });
    lost
}

/// 初始化 Vulkan 全局对象；失败时按可操作的指引记账（原因只有这一处需要讲清）。
fn init_gpu() -> Result<GlobalGpu, GpuError> {
    match pollster::block_on(crate::render::wgpu_backend::initialize_wgpu()) {
        Ok((_inst, _adapt, device, queue)) => {
            // 每个新设备（首次初始化与丢失后重建）都必须注册：wgpu 的回调随设备
            // 生灭，旧设备上的回调不会跟到新设备来。
            let lost = register_device_lost_callback(&device);
            Ok(GlobalGpu {
                #[cfg(target_os = "android")]
                instance: _inst,
                #[cfg(target_os = "android")]
                adapter: _adapt,
                device,
                queue,
                lost,
            })
        }
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

/// 全局 GPU 的当前槽位。
///
/// 用可替换的槽位而不是 `OnceLock`：设备丢失后必须换成**新 Instance 与新 Device**
/// （Vulkan surface 是 instance 级对象，旧实例上的 surface 配不到新设备上）。
/// 读路径克隆 `Arc`，故重建与渲染并发时各自拿到一致的一份，不会出现半个旧设备。
fn gpu_slot() -> &'static parking_lot::RwLock<Option<Arc<GlobalGpu>>> {
    static GPU: parking_lot::RwLock<Option<Arc<GlobalGpu>>> = parking_lot::RwLock::new(None);
    &GPU
}

fn current_gpu() -> Option<Arc<GlobalGpu>> {
    gpu_slot().read().clone()
}

/// 全局 GPU 槽位的写入互斥：首次初始化与丢失后重建都经它。
///
/// 两处共用一把锁，故不会出现「两个线程各建一个 GPU、后写者顶掉前者的」——
/// 那会让渲染器绑定的设备与 `global_gpu()` 返回的 instance 不是同一个，surface
/// 随即配不上（Vulkan surface 是 instance 级对象）。
fn gpu_lock() -> &'static parking_lot::Mutex<()> {
    static GPU_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    &GPU_LOCK
}

/// 重建决策：槽位里的全局 GPU 能否直接复用。
///
/// 参数是「槽位里那个是否已判死」；`None` 表示槽位为空（还没有 GPU）。
/// 判死的那个不能复用：判死后必须换**新**设备与**新** instance（Vulkan surface
/// 是 instance 级对象，旧 instance 上的 surface 配不到新设备上）。而「另一线程
/// 已经重建好」时可以复用——重建出来的设备必须与渲染器绑定的那个是同一个，
/// 不能各建一个。
///
/// 纯函数：便于单测覆盖三种组合，不必构造 GPU 设备。
#[cfg(any(target_os = "android", test))]
pub(crate) fn gpu_slot_reusable(slot_lost: Option<bool>) -> bool {
    slot_lost.is_some_and(|lost| !lost)
}

/// 渲染路径取全局 GPU：不可用即致命（无法渲染的终端是坏的，不应跚行运转）。
///
/// 设备已判死时同样致命：调用方只该在 `attachWindow` 的重建路径（先
/// [`rebuild_global_gpu`]）或 `render_inner` 的丢失检查之后取 GPU。
pub(super) fn global_gpu() -> Arc<GlobalGpu> {
    assert!(
        !device_lost(),
        "GPU device is lost; the next surface attach must rebuild it first"
    );
    global_gpu_or_init().unwrap_or_else(|initialization_error| {
        panic!("GPU initialization failed: {initialization_error}. See log for details.")
    })
}

/// 可失败取全局 GPU：失败不致命，供启动预热等非渲染路径使用（见
/// `Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState`）。
pub(crate) fn try_global_gpu() -> Result<Arc<GlobalGpu>, GpuError> {
    if device_lost() {
        return Err(GpuError::Surface(
            "GPU device lost; the next surface attach rebuilds it".into(),
        ));
    }
    global_gpu_or_init()
}

fn global_gpu_or_init() -> Result<Arc<GlobalGpu>, GpuError> {
    if let Some(initialized) = current_gpu() {
        return Ok(initialized);
    }
    initialize_global_gpu()
}

/// 设备丢失后重建全局 GPU，返回新句柄（仅 `attachWindow` 的恢复路径调用）。
///
/// 新 surface 必须由新 instance 重建：调用方随后照常走 `attach_surface`。
/// **不清**丢失标志：标志由调用方在「新 `Renderer` 也装好」之后清除，装不上时
/// 保持置位，下一次 attach 才会再试（见 [`clear_device_lost`]）。
///
/// 槽位里若还是**判死的那个**设备，必须换新的：判死的设备配不到新 surface，
/// 复用它等于把黑屏原样留着（这正是本函数存在的理由）。
#[cfg(target_os = "android")]
pub(crate) fn rebuild_global_gpu() -> Result<Arc<GlobalGpu>, GpuError> {
    let _rebuild = gpu_lock().lock();
    let slot = current_gpu();
    let slot_lost = slot
        .as_ref()
        .map(|gpu| gpu.lost.load(std::sync::atomic::Ordering::Acquire));
    if gpu_slot_reusable(slot_lost) {
        // 另一线程已重建好（同一个宿主窗口可能并发重挂）：直接复用，别再多建一个
        // VkInstance。`init_gpu` 成功但 `Renderer` 还没装上时也走这里，由调用方继续装。
        return Ok(slot.expect("可复用即槽位非空"));
    }
    let global = Arc::new(init_gpu()?);
    *gpu_slot().write() = Some(Arc::clone(&global));
    log::warn!("GPU rebuilt after device loss; the caller must attach a fresh surface");
    Ok(global)
}

/// GPU 初始化的唯一实现（致命渲染路径与可失败的预热路径共用，故只有这一处）。
///
/// 判空与写入之间持一把初始化锁，使 `init_gpu`（建 `Instance`、`request_adapter`、
/// `request_device`）在进程内**只发生一次**。先前的三段式
/// `get` 判空 → `init_gpu` → `get_or_init` 在判空与写入之间并不原子：预热线程与
/// 渲染线程在启动瞬间并发时，两者各自跑完一次完整初始化，第二次的
/// device/queue/adapter 被 `get_or_init` 直接丢弃——白耗一次显存与驱动资源
/// （真机 logcat 实证：`GPU adapter` 与 `GPU device created` 各打印两遍，分属两个线程）。
///
/// 失败**不写入**槽位（锁在返回 Err 前释放），故后续渲染路径仍会重试初始化：
/// 设备上的 Vulkan 初始化在进程刚起来的数秒内可能失败（GPU 尚被别的应用占用），
/// 而同一设备稍后即可成功，预热不该把一次瞬时失败固化成启动即崩。
fn initialize_global_gpu() -> Result<Arc<GlobalGpu>, GpuError> {
    if let Some(initialized) = current_gpu() {
        return Ok(initialized);
    }
    let _initialization = gpu_lock().lock();
    // 持锁后重查：等锁期间可能已由另一线程初始化完毕。
    if let Some(initialized) = current_gpu() {
        return Ok(initialized);
    }
    let global = Arc::new(init_gpu()?);
    #[cfg(test)]
    INITIALIZATION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *gpu_slot().write() = Some(Arc::clone(&global));
    Ok(global)
}

/// `init_gpu` 成功完成的次数，仅测试读取：生产构建不声明。
///
/// 计数使「初始化只发生一次」可断言：仅比对各线程拿到的引用做不到这一点——
/// 一旦填满，修复前（并发下各自初始化、后者被丢弃）与修复后的代码
/// 都让每个线程拿到同一引用，故该比对对竞态零判别力。
#[cfg(test)]
pub(crate) static INITIALIZATION_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
