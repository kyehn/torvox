#[cfg(target_os = "android")]
use super::super::*;

// ── 表面挂载与卸载 ──────────────────────────────────────────────────
/// 挂载 Android Surface（仅 Android）。surface 挂载时由 Bridge.kt 调用：
/// `TerminalRuntime` 把 Android Surface 越过 JNI 边界交给渲染线程，后者经原生窗口消费；
/// surface 随后由 `detachWindow` 卸载。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`surface` 须为本次调用期间有效的 Surface 对象。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_attachWindow(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        // SAFETY: 外层已由 JVM 保证参数有效，此处透传同一调用期的引用。
        unsafe { attach_window_inner(env, _class, _session_id, surface, width, height) }
    })
}

#[cfg(target_os = "android")]
unsafe fn attach_window_inner(
    env: &mut Env,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    if surface.is_null() {
        log::error!("FFI: attachWindow called with null surface");
        return;
    }

    // 从 Surface 对象取得 ANativeWindow 裸指针。
    let raw_env = env.get_raw();
    // SAFETY: `raw_env` 来自 `get_raw()`，返回有效的 JNIEnv 指针；`surface` 是 JNI
    // 方法参数，JVM 运行时保证其有效。`ANativeWindow_fromSurface` 是 `libandroid.so`
    // 中有文档的 NDK 函数。
    let ptr = unsafe { ANativeWindow_fromSurface(raw_env as *mut _, surface as *mut _) };

    if ptr.is_null() {
        log::error!("FFI: attachWindow — ANativeWindow_fromSurface returned NULL");
        return;
    }

    log::info!("FFI: attachWindow ptr={:p} {}x{}", ptr, width, height);

    // 设备丢失（Mali 在切后台时实测发生）后，旧 instance 上的 surface 配不到新设备：
    // 必须先建新的 VkInstance/Device，再把 Renderer 换到新设备上，之后才挂载本窗口。
    // 重建失败时保持丢失标志、释放窗口并返回：宿主看到失效位后换窗口再试，绝不在
    // 死设备上继续建 surface。重建放在 `ANativeWindow_setBuffersGeometry` 之前，
    // 使下方几何失败路径取渲染状态时也不会落在死设备上。
    if crate::render::context::device_lost() {
        match crate::render::context::rebuild_global_gpu() {
            Ok(gpu) => {
                let mut state = render_state_mut();
                if let Some(render_state) = state.as_mut() {
                    render_state.renderer.rebuild_after_device_loss(&gpu);
                }
                // 状态被 detach 拆除时无需搬运：下一次 `render_state_mut` 会直接在新
                // 设备上新建。放下锁之前新渲染状态已可用。
                drop(state);
                // 新设备与使用它的渲染状态都已就位，此后 GPU 入口才放行。
                crate::render::context::clear_device_lost();
            }
            Err(error) => {
                log::error!("FFI: attachWindow — GPU device rebuild failed: {error}");
                ATTACHED_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
                // SAFETY: `ptr` 是上面取得、尚无 surface 接管的 `ANativeWindow*`。
                unsafe { ANativeWindow_release(ptr) };
                return;
            }
        }
    }

    // 把 ANativeWindow 交给渲染器：wgpu 在创建 surface 时自行取得引用，故此处释放
    // `ANativeWindow_fromSurface` 返回的调用方自有引用（wgpu surface 会保持窗口存活
    // 直至 `detachWindow` 丢弃它）。
    //
    // 显式把交换链配置对齐到 BufferQueue：以 TextureView 为后端的 surface 否则可能
    // 停留在陈旧/默认几何，而模拟器上的 SwiftShader 会拒绝出队格式/几何与队列不符的
    // 缓冲（dequeueBuffer 超时）。格式取 `WINDOW_FORMAT_RGBA_8888`——它与交换链
    // 格式 `Rgba8Unorm` 一一对应（见 `render::context::WINDOW_FORMAT_RGBA_8888`）。
    //
    // 尺寸先钳位再两处共用：NDK 调用与 `attach_surface` 必须看到同一组尺寸，
    // 否则队列几何与交换链几何各说各话，SwiftShader 仍会拒绝出队。
    let surface_width = width.max(0);
    let surface_height = height.max(0);
    // SAFETY: `ptr` 是有效的 `ANativeWindow*`（上方已检查非空）；
    // `ANativeWindow_setBuffersGeometry` 是有文档的 NDK 函数。
    let geometry_status = unsafe {
        ANativeWindow_setBuffersGeometry(
            ptr,
            surface_width,
            surface_height,
            crate::render::context::WINDOW_FORMAT_RGBA_8888,
        )
    };
    if geometry_status != 0 {
        // 非 0 即失败。丢弃它会让几何错配在 dequeueBuffer 超时后才暴露，且无从归因，
        // 故不挂载 surface 并在此出声。宿主不观察挂载结果（返回 Unit），因此
        // 「未挂载」不会被自动恢复：渲染线程照常跑，surface 失效位与看门狗照常记账。
        log::error!(
            "FFI: attachWindow — ANativeWindow_setBuffersGeometry failed: {geometry_status}"
        );
        // 与下方 `attach_surface` 失败同一条自愈路径：不得让任何会话继续「拥有」一个
        // 已不存在的 surface，否则旧属主的 detachWindow 仍能 CAS 成功并清掉状态，
        // 而新窗口无人挂载——渲染对「无 surface」返回 0 与空闲帧不可区分，终端永久黑屏。
        // SAFETY: 同下方的 `ANativeWindow_release`，此处无 surface 接管该引用。
        // 先释放再碰渲染状态：`render_state_mut()` 在渲染器尚未建好时会创建它并可能
        // panic，引用在那之后泄漏就再也归还不回来了。
        unsafe { ANativeWindow_release(ptr) };
        {
            let mut state = render_state_mut();
            state
                .as_mut()
                .expect("render_state_mut always initializes")
                .renderer
                .mark_surface_invalidated();
        }
        ATTACHED_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
        return;
    }
    let mut state = render_state_mut();
    let renderer = &mut state
        .as_mut()
        .expect("render_state_mut always initializes")
        .renderer;
    let session_id = _session_id as u64;
    // 同会话复用判定：surface 归属仍是本会话（销毁重建路径无人 detach，归属保持；
    // 会话切换/关闭先 detach 把归属清零）才允许把上一帧呈现到新 surface，否则丢弃
    // ——绝不把上一会话的画面闪给新会话。判据必须在取锁后、attach 前读取：
    // `attach_surface` 成功后会改写归属。
    let resume_last_frame =
        ATTACHED_SESSION_ID.load(std::sync::atomic::Ordering::Acquire) == session_id;
    match renderer.attach_surface(
        ptr.cast(),
        surface_width as u32,
        surface_height as u32,
        resume_last_frame,
    ) {
        Ok(()) => {
            ATTACHED_SESSION_ID.store(session_id, std::sync::atomic::Ordering::Release);
            // 立刻建好单元管线：着色器编译与 2048×2048 图集分配是**一次性**开销，
            // 放在挂载路径（IO 调度器线程，不占 UI 线程）而不是渲染循环里，首帧就
            // 不必背着它——否则慢帧诊断每次启动都会记一条假的「渲染回归」。
            // 尺寸未知时推迟到尺寸就绪后由渲染路径补建（见 `ensure_cell_pipeline`）。
            match state.as_mut() {
                Some(render_state) => {
                    if !super::frame::ensure_cell_pipeline(render_state) {
                        log::warn!(
                            "FFI: attachWindow — surface 尺寸未知，单元管线推迟到尺寸就绪后建立"
                        );
                    }
                }
                None => {
                    log::error!("FFI: attachWindow — render state missing，单元管线无法建立");
                }
            }
            log::info!("FFI: attachWindow surface attached (session {session_id})");
        }
        Err(error) => {
            log::error!("FFI: attachWindow surface attach failed: {error}");
            // 不得让任何会话继续「拥有」一个已不存在的 surface：旧属主的 detachWindow
            // 仍能 CAS 成功并清掉状态，而新窗口无人挂载，渲染对「无 surface」返回 0
            // 与空闲帧不可区分——终端永久黑屏。改为判死并上报，宿主据此换一个新窗口
            // 重新挂载（与 surface 丢失同一条自愈路径）。
            renderer.mark_surface_invalidated();
            ATTACHED_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
        }
    }
    // SAFETY: `ptr` 是来自 `ANativeWindow_fromSurface` 的有效 `ANativeWindow*`。
    // 无论挂载成功或失败，调用方自有的引用都必须释放：成功路径下 wgpu 在创建 surface
    // 时已取得自己的引用，失败路径下则无人接管。`ANativeWindow_release` 是有文档的
    // NDK 函数，会递减窗口引用计数。
    unsafe { ANativeWindow_release(ptr) };
}

/// 卸载当前 surface（仅 Android）。
///
/// 实现：带属主检查（`ATTACHED_SESSION_ID` CAS）地释放 wgpu surface，并拆除该会话的
/// `RenderState`，使下次挂载时重建。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_detachWindow(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        detach_window_inner(env, _class, _session_id)
    })
}

#[cfg(target_os = "android")]
pub(super) fn detach_window_inner(_env: &mut Env, _class: JClass, _session_id: jlong) {
    let session_id = _session_id as u64;
    // 只有拥有已挂载 surface 的会话才能丢弃它：`switchSession` 先挂载新会话的 surface
    // 再卸载旧的，无条件卸载会抹掉刚挂载的 surface（首个之后的每个会话都黑屏）。
    //
    // 属主检查与 surface 丢弃在**同一个** `RENDER_STATE` 锁内完成
    // （`attach_window_inner` 也取该锁）：若拆成“先检查后丢弃”，并发的挂载
    // （已存入新属主）可能插入其间而其新建的 surface 被丢弃。
    //
    // 中毒后照常取内层值（与 `render_state_mut` 同口径）：渲染期 panic 已被
    // `jni_export_guard` 转成 Java 异常，锁随之中毒；此处置为放弃就永久漏掉
    // `release_surface` 与 `ATTACHED_SESSION_ID` 清零，surface 自愈路径随之失效。
    let mut state = RENDER_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if ATTACHED_SESSION_ID
        .compare_exchange(
            session_id,
            0,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        log::debug!(
            "detachWindow: session {session_id} does not own the attached surface, ignoring",
        );
        return;
    }
    log::info!("FFI: detachWindow (session {session_id})");
    // 丢弃 wgpu surface；下次 `attachWindow` 重建它。wgpu 持有的 ANativeWindow
    // 引用在 Surface 丢弃时释放。
    if let Some(render_state) = state.as_mut() {
        render_state.renderer.release_surface();
    }
}

/// 回读保留的上一帧（RGBA8 像素），供宿主画开孔遮罩。
///
/// 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
/// SurfaceView 的 surface 随之被拆；回前台时交换链必须重建，那几十毫秒里
/// 窗口的透明开孔没有缓冲——宿主把离开时的画面留在开孔里，用户看到的就不是闪一下。
///
/// `width`/`height` 是宿主期望的尺寸（surface 尺寸）：与回读结果不符即放弃遮罩，
/// 宁可让开孔是平底色，也不要画错内容（尺寸不符的位图铺进去是错位的画面）。
///
/// 返回 null 表示没有可遮罩的内容（没有累加器、格式不是 RGBA8、回读失败或超时）。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_readCoverFrame(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    width: jint,
    height: jint,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 快照在锁内取，**回读在锁外做**：回读是一次 GPU 同步等待（实测数十毫秒），
        // 锁内等待会把所有会话的渲染线程一起卡住（2026-11-12 真机日志：
        // 另一会话的渲染采样多花 38.98ms，触发 SLOW_FRAME/SLOW_LOOP 告警）。
        let source = {
            let state = render_state_mut();
            state
                .as_ref()
                .expect("render_state_mut always initializes")
                .renderer
                .cover_readback_source()
        };
        match source.and_then(|source| source.read()) {
            None => std::ptr::null_mut(),
            Some(frame) => {
                if frame.width != width as u32 || frame.height != height as u32 {
                    log::warn!(
                        "FFI: readCoverFrame — 尺寸不符（原生 {}x{}，宿主 {width}x{height}），放弃遮罩",
                        frame.width,
                        frame.height
                    );
                    std::ptr::null_mut()
                } else {
                    match env.byte_array_from_slice(&frame.pixels) {
                        Ok(array) => array.into_raw(),
                        Err(error) => {
                            log::warn!("FFI: readCoverFrame — byte_array_from_slice 失败: {error}");
                            std::ptr::null_mut()
                        }
                    }
                }
            }
        }
    })
}
