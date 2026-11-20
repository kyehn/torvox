//! 取纹理：在途记账、工作线程与单帧结局。
use crate::render::Renderer;
use crate::slow_log::SlowLogThrottle;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, SyncSender};

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
/// 生命周期切换时付一次，要让「慢但会返回」的驱动走完（模拟器实测 `SwiftShader` 的
/// `dequeueBuffer` 超时约 0.9s）。取 2s：既覆盖这类慢返回，又不至于在真卡死时把
/// 宿主线程按太久——超时即由调用方拒绝同窗口重建（见 `Renderer::attach_surface`）。
pub(crate) const ACQUIRE_DRAIN_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(2000);
type AcquireResult = Result<wgpu::CurrentSurfaceTexture, Box<dyn std::any::Any + Send>>;

/// 把已取到但本帧不再使用的交换链纹理清成背景色并呈现，让它回到交换链的缓冲区池。
///
/// **为什么必须呈现而不是丢弃**：wgpu-hal 的 Vulkan 后端在纹理未呈现即被丢弃时，
/// `discard_texture` 是空实现（上游 TODO）。那张交换链图像于是既不呈现、也不归还：
/// 累积几帧后 `vkAcquireNextImageKHR` 永远等不到可用图像，终端就此永久停帧
/// （模拟器实测：取纹理超时后持续 `dequeueBuffer timed out`，画面停在旧帧，
/// 帧循环每 600ms 报一次瞬时渲染错误，几十秒内毫无恢复）。
pub(crate) fn release_acquired_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: wgpu::SurfaceTexture,
    background: wgpu::Color,
) {
    let command_buffer = clear_to_background(device, &texture.texture, background);
    queue.submit(std::iter::once(command_buffer));
    queue.present(texture);
}

/// 归还取纹理结果里的交换链纹理；结果里没有纹理（错误、Lost/Outdated）时无事可做。
pub(super) fn release_acquired_result(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    result: AcquireResult,
    background: wgpu::Color,
) {
    if let Ok(
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture),
    ) = result
    {
        release_acquired_texture(device, queue, texture, background);
    }
}

/// 把纹理清成背景色：归还前必须清一次，否则屏幕会闪过未定义内容。
pub(super) fn clear_to_background(
    device: &wgpu::Device,
    texture: &wgpu::Texture,
    background: wgpu::Color,
) -> wgpu::CommandBuffer {
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Release Texture Encoder"),
    });
    {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Release Texture Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    encoder.finish()
}

/// 取纹理结果的交接状态。
#[derive(Clone, Copy, PartialEq, Eq)]
enum HandoffState {
    /// 工作线程尚未回送结果。
    Pending,
    /// 工作线程已把结果送进通道（持有锁期间完成）。
    Sent,
    /// 渲染线程已超时放弃：工作线程须自行归还结果里的交换链纹理。
    Abandoned,
}

/// 取纹理结果的交接协议：渲染线程超时放弃后，迟到结果必须有人归还。
///
/// **竞态**：渲染线程的 `recv_timeout` 超时返回后，接收端要活到函数返回；工作线程
/// 若恰在这个窗口里把纹理送进通道，纹理会随通道一起被丢弃——wgpu 的 Vulkan 后端
/// 丢弃未呈现纹理是空实现（见 [`release_acquired_texture`]），那张交换链图像永不
/// 归还，累积几帧后终端永久停帧。故「放弃」与「回送」必须互斥：
///
/// - 渲染线程先放弃：工作线程拿到结果后自行归还，不再回送；
/// - 工作线程先回送：渲染线程在锁内看到已回送，从通道取回结果并自行归还。
///
/// 两种顺序都恰好有一方归还纹理：既不漏（无人归还 → 图像永不归还），也不重复
/// （同一张纹理被归还两次）。
pub(super) struct ResultHandoff {
    state: std::sync::Mutex<HandoffState>,
}

impl ResultHandoff {
    pub(super) fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(HandoffState::Pending),
        }
    }

    /// 工作线程回送结果。
    ///
    /// `Err(result)` = 渲染线程已放弃本帧，结果（含交换链纹理）须由调用方自行归还。
    pub(super) fn deliver<T>(&self, response: &SyncSender<T>, result: T) -> Result<(), T> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *state == HandoffState::Abandoned {
            return Err(result);
        }
        // 回送必须在锁内完成：「已回送」与渲染线程的「取回」之间不得有空隙，
        // 否则渲染线程会看到「尚未回送」而让工作线程归还，两边都不归还纹理。
        // 通道容量为 1 且每请求只回送一次，`try_send` 不会阻塞。
        match response.try_send(result) {
            Ok(()) => {
                *state = HandoffState::Sent;
                Ok(())
            }
            Err(
                std::sync::mpsc::TrySendError::Full(result)
                | std::sync::mpsc::TrySendError::Disconnected(result),
            ) => Err(result),
        }
    }

    /// 渲染线程超时放弃本帧。
    ///
    /// `Some(result)` = 工作线程已回送但本帧已超时，结果须由调用方归还。
    pub(super) fn abandon<T>(&self, response: &Receiver<T>) -> Option<T> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *state {
            HandoffState::Pending => {
                *state = HandoffState::Abandoned;
                None
            }
            HandoffState::Sent => {
                // 结果已进通道：取回它，由渲染线程归还（工作线程已不再持有）。
                drop(state);
                response.try_recv().ok()
            }
            HandoffState::Abandoned => None,
        }
    }
}

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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *in_flight > 0 {
            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }
            let (guard, wait) = self
                .idle
                .wait_timeout(in_flight, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    /// 回送与放弃的互斥协议：渲染线程超时后，工作线程不得把纹理送进无人接收的通道。
    handoff: std::sync::Arc<ResultHandoff>,
    /// 归还迟到纹理所需：工作线程替已放弃本帧的渲染线程把纹理清空后呈现。
    device: wgpu::Device,
    queue: wgpu::Queue,
    background: wgpu::Color,
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
                    handoff,
                    device,
                    queue,
                    background,
                } = request;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    surface.get_current_texture()
                }));
                // 顺序不可交换：先放下克隆（其下的 Vulkan surface 随最后一个克隆销毁），
                // 再 finish。反过来会让排干方在克隆仍在途时就被唤醒，同窗口重建照旧
                // 撞上 ERROR_NATIVE_WINDOW_IN_USE_KHR。
                drop(surface);
                // 回送结果。渲染线程可能已超时放弃：此时由工作线程亲自把纹理清空后
                // 呈现——丢弃会让那张交换链图像永不归还，交换链很快便再也拿不到可用
                // 图像（终端永久停帧，见 `release_acquired_texture`）。
                // 交接协议保证「放弃」与「回送」互斥：渲染线程超时后纹理绝不会既
                // 无人归还、又躺在通道里随通道析构。
                //
                // 销账放在回送之后：在途记账覆盖「纹理尚未归还」的整个窗口，
                // 排干方（释放 surface 前）据此保证交换链销毁前纹理已被呈现或丢弃。
                if let Err(result) = handoff.deliver(&response, result) {
                    // 归还本身也须防 panic：工作线程死掉会让后续每帧都只能报
                    // 「worker busy or dead」，且该线程是进程级单例、无法重建。
                    // 设备丢失时 wgpu 会先调丢失回调用再 panic，这里的失败不影响
                    // 既有的设备重建链。
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        release_acquired_result(&device, &queue, result, background);
                    }));
                }
                lane.finish();
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
/// 死窗口，见 [`Renderer::note_surface_acquire`]），后者只是工作线程忙或超时，
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
        surface_config: Option<&wgpu::SurfaceConfiguration>,
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
        let handoff = std::sync::Arc::new(ResultHandoff::new());
        let request = AcquireRequest {
            surface: std::sync::Arc::clone(surface),
            response: response_sender,
            lane: std::sync::Arc::clone(lane),
            handoff: std::sync::Arc::clone(&handoff),
            device: self.device.clone(),
            queue: self.queue.clone(),
            background: self.background,
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
                    Self::reconfigure_surface(surface, self.surface_config.as_ref(), &self.device);
                    AcquireOutcome::SurfaceLost
                }
                _ => AcquireOutcome::Skipped,
            },
            Ok(Err(payload)) => {
                let text = crate::panic_text(payload.as_ref());
                if crate::render::context::is_device_loss_text(&text) {
                    // 丢失不必由这里报第二遍：标记点只记一条，渲染随即停在下一帧入口。
                    crate::render::context::mark_device_lost(
                        &self.lost,
                        &crate::render::context::one_line(&text),
                    );
                } else if text.contains("not configured for presentation") {
                    // 上一次 `Surface::configure` 失败（真机/模拟器日志：`GpuWaitTimeout`）
                    // 会让 surface 停在未配置态，此后每次取纹理都按 validation 错误 panic。
                    // 与 Lost/Outdated 同源：重配一次，下一帧即按正常 surface 取纹理。
                    // 配置本身仍可能失败——那时 reconfigure 只是 no-op 级的无害重试，
                    // wgpu 会再报一条 validation，不会 panic。
                    Self::reconfigure_surface(surface, self.surface_config.as_ref(), &self.device);
                    if ACQUIRE_PANIC_THROTTLE.should_log() {
                        log::warn!(
                            "acquire_texture: get_current_texture panicked (surface not configured); surface reconfigured: {}",
                            crate::render::context::one_line(&text)
                        );
                    }
                } else if ACQUIRE_PANIC_THROTTLE.should_log() {
                    log::warn!("acquire_texture: get_current_texture panicked");
                }
                AcquireOutcome::Skipped
            }
            Err(_) => {
                // 超时放弃本帧。迟到结果必须仍有人归还：交接协议把「放弃」与「回送」
                // 互斥——已回送的结果在此取回并归还（本线程持有渲染状态锁，surface
                // 不会在此期间被销毁）；尚未回送的则由工作线程自行归还。
                // 两边都不归还的后果：那张交换链图像永不归还，终端永久停帧。
                if let Some(result) = handoff.abandon(&response_receiver) {
                    release_acquired_result(&self.device, &self.queue, result, self.background);
                }
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
