use super::render::{pump_sessions, string_to_jstring};
use super::{
    Arc, Duration, EVENT_QUEUE, Env, EnvUnowned, JClass, NOTIFICATION_QUEUE,
    ThrowRuntimeExAndFailure, jlong, jstring, rlock_session_registry,
};

// ── 渲染唤醒 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：waitForOutput / wakeRender
// ══════════════════════════════════════════════════════════════════════════

/// 渲染循环的驻留点：等新输出或任意唤醒信号，返回新的唤醒代数。
///
/// `last_generation` 是调用方上次拿到的代数；期间若已发生 notify，立即返回，
/// 因此「检查完信号再进场」与 notify 竞争也不会丢唤醒。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_waitForOutput(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
    last_generation: jlong,
    timeout_millis: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, last_generation, |_env| {
        let timeout = Duration::from_millis(u64::try_from(timeout_millis).unwrap_or(0));
        let last_generation = u64::try_from(last_generation).unwrap_or(0);
        // 读锁只用于取一份 Arc：等待期间绝不能持注册表锁（销毁会话要写锁）。
        let wake = {
            let registry = rlock_session_registry();
            registry
                .get(&(session_id as u64))
                .map(|entry| Arc::clone(&entry.render_wake))
        };
        if let Some(wake) = wake {
            wake.wait(last_generation, timeout) as jlong
        } else {
            // 会话已销毁：按同一节奏睡一段再返回，调用方下一拍就会看到会话消失
            // 并退出循环。返回原代数而不是新值，避免伪造「有新信号」。
            std::thread::sleep(timeout);
            last_generation as jlong
        }
    })
}

/// 唤醒指定会话的渲染循环（vsync、输入、滚动、选区等所有 Kotlin 侧信号）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_wakeRender(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let wake = {
            let registry = rlock_session_registry();
            registry
                .get(&(session_id as u64))
                .map(|entry| Arc::clone(&entry.render_wake))
        };
        if let Some(wake) = wake {
            wake.notify();
        }
    });
}

// ── 事件轮询 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：pollEvent
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pollEvent<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
) -> jstring {
    // 此处的 panic（如 ghostty 的 VT 处理内部）会 abort 整个进程，故转为 Java 异常。
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        poll_event_inner(env, class)
    })
}

pub(super) fn poll_event_inner<'local>(env: &mut Env<'local>, _class: JClass<'local>) -> jstring {
    // 步骤 1：推进全部会话（输出灌入终端、收割 VT 副作用、事件入队）。
    pump_sessions();

    // 步骤 2：从队列排空一个事件。
    let event = EVENT_QUEUE.pop();

    match event {
        Some(e) => {
            let json = match serde_json::to_string(&e) {
                Ok(json) => json,
                Err(err) => {
                    log::error!("pollEvent: event serialization failed: {err}");
                    return std::ptr::null_mut();
                }
            };
            string_to_jstring(env, &json)
        }
        None => std::ptr::null_mut(),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：pollNotificationEvent
// ══════════════════════════════════════════════════════════════════════════

/// 取走一个待上报的桌面通知事件（JSON）；无通知时返回 null。
///
/// 与 `pollEvent` 分开：后台期间只有会话推进泵在跑，它要取得到通知（否则通知
/// 要等用户回到应用才出现），却不得取走退出事件。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pollNotificationEvent<
    'local,
>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        poll_notification_event_inner(env, class)
    })
}

pub(super) fn poll_notification_event_inner<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
) -> jstring {
    // 步骤 1：推进全部会话（把 PTY 输出灌入终端，收割上游通知回调）。
    // 与 `poll_event_inner` 同构：通知的产出依赖会话推进（OSC 9 序列要先被 VT
    // 解析，回调才会触发），故本入口必须自带推进，不能指望渲染循环或后台推进
    // 泵恰好先跑过一轮。
    pump_sessions();

    // 步骤 2：从通知队列排空一个通知。
    match NOTIFICATION_QUEUE.pop() {
        Some(event) => {
            let json = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(err) => {
                    log::error!("pollNotificationEvent: event serialization failed: {err}");
                    return std::ptr::null_mut();
                }
            };
            string_to_jstring(env, &json)
        }
        None => std::ptr::null_mut(),
    }
}
