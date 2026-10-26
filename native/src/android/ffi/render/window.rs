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

    // 把 ANativeWindow 交给渲染器：wgpu 在创建 surface 时自行取得引用，故此处释放
    // `ANativeWindow_fromSurface` 返回的调用方自有引用（wgpu surface 会保持窗口存活
    // 直至 `detachWindow` 丢弃它）。
    //
    // 显式把交换链配置对齐到 BufferQueue：以 TextureView 为后端的 surface 否则可能
    // 停留在陈旧/默认几何，而模拟器上的 SwiftShader 会拒绝出队格式/几何与队列不符的
    // 缓冲（dequeueBuffer 超时）。WINDOW_FORMAT_RGBA_8888 = 1。
    //
    // 尺寸先钳位再两处共用：NDK 调用与 `attach_surface` 必须看到同一组尺寸，
    // 否则队列几何与交换链几何各说各话，SwiftShader 仍会拒绝出队。
    let surface_width = width.max(0);
    let surface_height = height.max(0);
    // SAFETY: `ptr` 是有效的 `ANativeWindow*`（上方已检查非空）；
    // `ANativeWindow_setBuffersGeometry` 是有文档的 NDK 函数。
    let geometry_status =
        unsafe { ANativeWindow_setBuffersGeometry(ptr, surface_width, surface_height, 1) };
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
    match renderer.attach_surface(ptr.cast(), surface_width as u32, surface_height as u32) {
        Ok(()) => {
            ATTACHED_SESSION_ID.store(session_id, std::sync::atomic::Ordering::Release);
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
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut _unowned, (), |env| {
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
