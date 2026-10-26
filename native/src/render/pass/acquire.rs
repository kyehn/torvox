//! 取纹理：在途记账、工作线程与单帧结局。
use crate::render::Renderer;
use crate::render::slow_log::SlowLogThrottle;
use std::sync::OnceLock;
use std::sync::mpsc::SyncSender;

/// 取纹理请求被拒（工作线程忙或已死）的限频闸门。
///
/// 该路径每帧都可能命中（工作线程卡住时逐帧被拒），逐条记会把 logcat 刷成告警洪水；
/// 真正的升级信号是 [`crate::render::context`] 侧的连续跳过计数（到上限记 error）。
static ACQUIRE_BUSY_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();
/// 取纹理工作线程内 panic 的限频闸门（panic 可逐帧复现）。
static ACQUIRE_PANIC_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();
/// 取纹理超时的限频闸门（慢 SurfaceFlinger/SwiftShader 下可逐帧命中）。
static ACQUIRE_TIMEOUT_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();
/// 取纹理期限（约 6 个 vsync）。两条取纹理路径共用一个值，理由相同：都在持有全局
/// `RENDER_STATE` 的临界区内，而 UI 线程（`surfaceChanged`、`setViewportScrollPx`、
/// `setTheme`、`setFontSizeInPlace`、`applySearchHighlights`）会争这把锁——期限就是
/// 这些调用方在一次 GPU 卡死（Mali GPU 的 `vkAcquireNextImageKHR` 会永久卡住，见
/// `acquire_texture` 注释）时最多要等多久。
///
/// 此前渲染路径取 2s、warmup 取 100ms，口径不一致：一次卡死会把主线程冻住 2s。
/// 超时落在 `AcquireOutcome::Skipped` 上，它**不**判死 surface（只有
/// `SurfaceLost` 才推进 `surface_loss_streak`），下一帧照常重试；连续跳过满
/// `SKIP_FATAL_FRAME_LIMIT` 帧会记一条 error。因此短期限的代价只是「慢机器上
/// 多跳几帧」，换来的却是卡死时 UI 线程不被按在地上两秒。
pub(crate) const ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);
/// 排干在途取帧的期限，由 surface 销毁/重建方（`release_surface`、`attach_surface`
/// 慢路径、`Renderer::drop`）使用。
///
/// 与 [`ACQUIRE_TIMEOUT`] 的分工：那条是**每帧**预算，越短越好；这条只在 surface
/// 生命周期切换时付一次，要让「慢但会返回」的驱动走完（模拟器实测 SwiftShader 的
/// `dequeueBuffer` 超时约 0.9s）。取 2s：既覆盖这类慢返回，又不至于在真卡死时把
/// 宿主线程按太久——超时即由调用方拒绝同窗口重建（见 `Renderer::attach_surface`）。
pub(crate) const ACQUIRE_DRAIN_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(2000);
type AcquireResult = Result<wgpu::CurrentSurfaceTexture, Box<dyn std::any::Any + Send>>;
/// 在途取帧的记账：回答「工作线程现在是否仍持有某个 surface 的 `Arc` 克隆」。
///
/// 排干（drain）语义的必要性：克隆存活期间其下的 Vulkan surface 不销毁；此时在
/// **同一个** `ANativeWindow` 上再建一个 Vulkan surface，`vkCreateAndroidSurfaceKHR`
/// 返回 `ERROR_NATIVE_WINDOW_IN_USE_KHR`，而 wgpu-hal 的 vulkan 后端对这个错误直接
/// `expect()` panic。该 panic 出现在 JNI 调用（`attachWindow`）里会被导出守卫转成
/// Java 异常——宿主不捕获，进程随即 FATAL 退出，整轮仪器化运行作废（2026-10-18
/// 日志实测：`acquire 超时 → release+同址 attach → 860ms 后 panic`）。
///
/// 计数在请求入队前 +1、工作线程**放下克隆之后** -1（顺序不可交换，见工作线程处
/// 注释）；[`Self::drain`] 等到归零，供 surface 销毁/重建前调用。
pub(super) struct AcquireLane {
    pub(super) in_flight: std::sync::Mutex<usize>,
    pub(super) idle: std::sync::Condvar,
}

impl AcquireLane {
    pub(super) fn begin(&self) {
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *in_flight += 1;
    }

    /// 工作线程在放下 surface 克隆**之后**调用；归零时唤醒排干方。
    ///
    /// `saturating_sub` 而非 `-=`：单测以手工 begin/finish 驱动本类型，计数错乱时
    /// 宁可停在 0（排干方立刻放行）也不要在工作线程上 panic——那条线程上 panic
    /// 等于把整条取纹理通道打死。
    pub(super) fn finish(&self) {
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *in_flight = in_flight.saturating_sub(1);
        if *in_flight == 0 {
            self.idle.notify_all();
        }
    }

    /// 等到无在途取帧。`true` = 已排干（无克隆在途）；`false` = 超时（工作线程被
    /// 驱动卡住，Mali GPU 的已知形态）——调用方必须据此拒绝同窗口重建，
    /// 而不是继续建 surface。
    pub(super) fn drain(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *in_flight > 0 {
            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }
            let (guard, wait) = self
                .idle
                .wait_timeout(in_flight, deadline - now)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            in_flight = guard;
            if wait.timed_out() && *in_flight > 0 {
                return false;
            }
        }
        true
    }
}

fn acquire_lane() -> &'static std::sync::Arc<AcquireLane> {
    static LANE: OnceLock<std::sync::Arc<AcquireLane>> = OnceLock::new();
    LANE.get_or_init(|| {
        std::sync::Arc::new(AcquireLane {
            in_flight: std::sync::Mutex::new(0),
            idle: std::sync::Condvar::new(),
        })
    })
}

/// 排干在途取帧，供 surface 销毁/重建前调用（实现见 [`AcquireLane`]）。
pub(crate) fn drain_in_flight_acquires(timeout: std::time::Duration) -> bool {
    acquire_lane().drain(timeout)
}
pub(super) struct AcquireRequest {
    surface: std::sync::Arc<wgpu::Surface<'static>>,
    response: std::sync::mpsc::SyncSender<AcquireResult>,
    lane: std::sync::Arc<AcquireLane>,
}

fn spawn_acquire_worker() -> SyncSender<AcquireRequest> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<AcquireRequest>(1);
    std::thread::Builder::new()
        .name("gpu-acquire".into())
        .spawn(move || {
            for request in rx {
                let AcquireRequest {
                    surface,
                    response,
                    lane,
                } = request;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    surface.get_current_texture()
                }));
                // 顺序不可交换：先放下克隆（其下的 Vulkan surface 随最后一个克隆销毁），
                // 再 finish。反过来会让排干方在克隆仍在途时就被唤醒，同窗口重建照旧
                // 撞上 ERROR_NATIVE_WINDOW_IN_USE_KHR。
                drop(surface);
                lane.finish();
                // 销账之后才回送结果：`send` 可能因接收端已超时退出通道而被阻塞，
                // 若把它排在 finish 之前，排干方就要替通道背压买单（最多一帧
                // `ACQUIRE_TIMEOUT`），而它只该关心克隆有没有放下。
                let _ = response.send(result);
            }
        })
        .unwrap_or_else(|spawn_error| {
            panic!(
                "FATAL: cannot spawn gpu-acquire worker thread: {spawn_error}. \
                 This is required for safe surface texture acquisition. \
                 Check RLIMIT_NPROC / RLIMIT_THREAD."
            )
        });
    tx
}

pub(super) fn acquire_worker_tx() -> &'static SyncSender<AcquireRequest> {
    static WORKER_TX: OnceLock<SyncSender<AcquireRequest>> = OnceLock::new();
    WORKER_TX.get_or_init(spawn_acquire_worker)
}
/// 单帧取纹理的结局。
///
/// `Lost`/`Outdated` 与「本帧跳过」必须分开：前者是 surface 级信号（连续出现即判
/// 死窗口，见 [`Renderer::note_surface_acquire`]`），后者只是工作线程忙或超时，
/// surface 仍然可用。合并成 `Option` 会让慢机器每帧误伤 surface。
pub(crate) enum AcquireOutcome {
    /// 拿到可呈现纹理。
    Acquired(wgpu::SurfaceTexture),
    /// 本帧跳过：工作线程忙/已死、取纹理超时或呈现缺失，下一帧重试。
    Skipped,
    /// surface 级失败：已原地 `reconfigure`，本帧丢弃。
    SurfaceLost,
}

impl AcquireOutcome {
    pub(crate) fn into_texture(self) -> Option<wgpu::SurfaceTexture> {
        match self {
            Self::Acquired(texture) => Some(texture),
            Self::Skipped | Self::SurfaceLost => None,
        }
    }
}

impl Renderer {
    /// surface 失效时重配（Lost/Outdated 的统一恢复）。
    fn reconfigure_surface(
        surface: &wgpu::Surface<'static>,
        surface_config: &Option<wgpu::SurfaceConfiguration>,
        device: &wgpu::Device,
    ) {
        if let Some(config) = surface_config {
            surface.configure(device, config);
        }
    }

    pub(crate) fn acquire_texture(
        &self,
        surface: &std::sync::Arc<wgpu::Surface<'static>>,
        _config_width: u32,
        _config_height: u32,
        deadline: std::time::Duration,
    ) -> AcquireOutcome {
        // Mali GPU（联发科/展锐 SoC）在缺 SURFACE_VIEW_FORMATS 时会永久卡在
        // vkAcquireNextImageKHR，故用常驻工作线程 + 超时，绝不让渲染线程无限阻塞。
        // Lost/Outdated 在工作线程内就地处理；卡死视为永久故障（Mali GPU 专有），
        // Outdated 是瞬态（surface 被缩放或重建），reconfigure 后可恢复 —— 模拟器实测：
        // SwiftShader 的 dequeueBuffer 超时与 SurfaceFlinger 缩放竞态都会报 Outdated，
        // 不 reconfigure 的话切回应用后渲染线程会永久空转在 begin_frame 失败上。
        // 图集与 surface 格式各取所需：字形图集恒为 Rgba8Unorm（只作采样源，见
        // create_atlas_texture），不随 surface 格式变；surface 按 caps 取非 sRGB
        // 变体。Android 的 view_formats 故意置空（见 context/renderer.rs 的 surface.configure
        // 处说明），故此处无需格式降级协商。
        // 工作线程经 OnceLock 只建一次并跨帧复用，避免每进程重复建线程；
        // 每帧仅一次同步通道分配。
        let (response_sender, response_receiver) =
            std::sync::mpsc::sync_channel::<AcquireResult>(1);
        // 先记账再入队：入队成功即可能在工作线程里持有 surface 克隆，
        // 排干方（attach/release/drop）据此才不会把「在途克隆」当成「没有克隆」。
        let lane = acquire_lane();
        lane.begin();
        let request = AcquireRequest {
            surface: std::sync::Arc::clone(surface),
            response: response_sender,
            lane: std::sync::Arc::clone(lane),
        };
        // 通道满 = 上一帧的请求仍卡在工作线程里（Mali GPU 专有卡死），此时**绝不能**
        // 退回渲染线程就地取纹理：那正是本模块存在的理由所要避免的无超时阻塞，
        // 会让渲染线程永久挂死。本帧跳过，下一帧重试。
        if let Err(error) = acquire_worker_tx().try_send(request) {
            // 请求从未进入工作线程（退回的请求里没有本帧的在途克隆），
            // 立刻销账，避免排干方白等一个不存在的在途取帧。
            lane.finish();
            if ACQUIRE_BUSY_THROTTLE.should_log() {
                log::warn!("acquire_texture: worker busy or dead ({error}); frame skipped");
            }
            return AcquireOutcome::Skipped;
        }

        match response_receiver.recv_timeout(deadline) {
            Ok(Ok(result)) => match result {
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => AcquireOutcome::Acquired(tex),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    Self::reconfigure_surface(surface, &self.surface_config, &self.device);
                    AcquireOutcome::SurfaceLost
                }
                _ => AcquireOutcome::Skipped,
            },
            Ok(Err(_)) => {
                if ACQUIRE_PANIC_THROTTLE.should_log() {
                    log::warn!("acquire_texture: get_current_texture panicked");
                }
                AcquireOutcome::Skipped
            }
            Err(_) => {
                if ACQUIRE_TIMEOUT_THROTTLE.should_log() {
                    log::warn!(
                        "acquire_texture: get_current_texture timed out after {}ms (slow SurfaceFlinger/SwiftShader; frame skipped, retried next frame)",
                        deadline.as_millis()
                    );
                }
                AcquireOutcome::Skipped
            }
        }
    }
}
