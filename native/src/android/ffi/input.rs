use super::query::session_query_channel;
use super::session::read_jni_string;
use super::state::read_terminal_flag_jboolean;
use super::*;

// ── 输入、调整与键鼠编码 ────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：resize
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_resize(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    rows: jint,
    cols: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        resize_inner(env, _class, session_id, rows, cols)
    })
}

pub(super) fn resize_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    rows: jint,
    cols: jint,
) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        throw_session_missing(env, "resize");
        return;
    };
    let mut session = entry.session.lock();
    let rows = match u32::try_from(rows) {
        Ok(r) => r,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("resize: rows must be non-negative"),
            );
            return;
        }
    };
    let cols = match u32::try_from(cols) {
        Ok(c) => c,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("resize: cols must be non-negative"),
            );
            return;
        }
    };
    // 网格命令被丢弃时只记日志：PTY 已缩放而网格滞后，下次缩放事件重试，
    // 不向 Kotlin 抛异常（调用方按固定节拍重发，异常会刷爆日志）。
    match session.resize(rows, cols) {
        Ok(crate::terminal::session::ResizeOutcome::Dropped) => {
            log::warn!(
                "resize: grid command dropped for session {id} ({rows}x{cols}); PTY updated, grid retries on next resize"
            );
        }
        Err(error) => {
            if let Err(throw_error) = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("resize: failed: {error}")),
            ) {
                log::error!("resize: throw_new failed: {throw_error}");
            }
        }
        Ok(_) => {}
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：setPixelSize
// ══════════════════════════════════════════════════════════════════════════

/// 更新指定会话的 PTY winsize 像素字段（ws_xpixel/ws_ypixel）。会话不存在时抛
/// RuntimeException。
///
/// 像素感知的程序（`icat`、全屏 TUI）从 TIOCGWINSZ 读像素尺寸，像素字段为 0 会使其
/// 回退到错误的默认单元格尺寸。Kotlin 宿主在每次网格 resize 时随 surface 的像素
/// 尺寸一并调用本方法。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setPixelSize(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    width_px: jint,
    height_px: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        set_pixel_size_inner(env, _class, session_id, width_px, height_px)
    })
}

pub(super) fn set_pixel_size_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    width_px: jint,
    height_px: jint,
) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        throw_session_missing(env, "setPixelSize");
        return;
    };
    let (Ok(width), Ok(height)) = (u16::try_from(width_px), u16::try_from(height_px)) else {
        let _ = env.throw_new(
            jni_str!("java/lang/IllegalArgumentException"),
            jni_str!("setPixelSize: pixel dimensions must be in 0..=65535"),
        );
        return;
    };
    let session = entry.session.lock();
    if let Err(error) = session.set_pixel_size(width, height)
        && let Err(throw_error) = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            JNIString::from(format!("setPixelSize failed: {error}")),
        )
    {
        log::error!("setPixelSize: throw_new failed: {throw_error}");
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：focusEvent
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_focusEvent(
    mut unowned_env: EnvUnowned<'_>,
    class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
        focus_event_inner(env, class, session_id, focused)
    })
}

pub(super) fn focus_event_inner(
    _env: &mut Env,
    _class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    let id = session_id as u64;
    // 写入在两把锁之外：主端满时排空上限是 5s，而会话锁每帧被渲染与事件收割取得、
    // 注册表读锁被销毁/切换会话等写操作等待——持任一把写入都会把整台终端冻住
    // 同样长的时间（理由同 `feed_pty`）。故锁内只做 VT 查询与克隆写入面。
    let (payload, pty_master) = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return JNI_FALSE;
        };
        let mut session = entry.session.lock();
        let Some(payload) = session.focus_event_bytes(focused == JNI_TRUE) else {
            return JNI_TRUE;
        };
        (payload, session.pty_master())
    };
    // 焦点事件按用户操作触发、极稀疏，且丢出会让 vim 一类应用停在错误的焦点模式：
    // 故用完整的排空期限，不与逐帧收割共用短上限。
    if let Err(error) =
        pty_master.write_with_timeout(payload, crate::terminal::pty::WRITE_DRAIN_TIMEOUT)
    {
        log::warn!("session: focus_event write failed: {error}");
    }
    JNI_TRUE
}

// JNI 导出：feedPty
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_feedPty(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        feed_pty_inner(env, _class, session_id, data)
    })
}

pub(super) fn feed_pty_inner(env: &mut Env, _class: JClass, session_id: jlong, data: jbyteArray) {
    let id = session_id as u64;

    // 传原始字节而非 String：PTY 输入可能是任意二进制（粘贴的 GBK/ISO-8859-1 文本、
    // 协议数据）。经 Java String 会把非法 UTF-8 序列替换为 U+FFFD，静默破坏子进程
    // 收到的字节。
    let input: Vec<u8> = {
        // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
        // `from_raw` 只包装指针而不取得所有权；局部引用由 JVM 在本 native 方法返回时释放。
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
        match env.convert_byte_array(&byte_array) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = env.throw_new(
                    jni_str!("java/lang/RuntimeException"),
                    jni_str!("feedPty: failed to read input bytes"),
                );
                return;
            }
        }
    };

    // 写入在两把锁之外：主端满时 `write_all` 最多等 `pty::WRITE_DRAIN_TIMEOUT`，
    // 而会话锁每帧被渲染与事件收割取得，注册表读锁则被销毁/切换会话等写操作等待——
    // 持任一把写入都会把整台终端冻住同样长的时间。故先克隆写入面再放锁。
    let pty_master = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            throw_session_missing(env, "feedPty");
            return;
        };
        entry.session.lock().pty_master()
    };
    if let Err(write_error) = pty_master.write(&input) {
        // 主端 fd 是 O_NONBLOCK：`write_all` 已为缓冲排空等待过（见 pty::WRITE_DRAIN_TIMEOUT），
        // 到此仍失败说明子进程长时间不读 stdin。此处记日志——静默返回等于让用户
        // 丢输入却毫无痕迹，xterm 亦不会截断。
        if write_error.is_would_block() {
            log::error!(
                "feedPty: 子进程未在期限内消费输入，丢弃 {} 字节",
                input.len()
            );
            return;
        }
        // 会话已退出是正常生命周期终态，不是故障：抛异常会让 UI 线程上那一次按键
        // 把整个进程带走。`exited` 由读取线程或等待线程在察觉之后才置位，而从端
        // 已关闭时主端写得到的正是 `EIO`/`EPIPE`——只判 `Closed` 会漏掉「子进程已经
        // 死了但还没人知道」这段窗口，那正是击键最易命中的时刻。
        if let crate::terminal::session::SessionError::Io(io_error) = &write_error
            && matches!(
                io_error.raw_os_error().map(nix::errno::Errno::from_raw),
                Some(nix::errno::Errno::EIO) | Some(nix::errno::Errno::EPIPE)
            )
        {
            log::debug!(
                "feedPty: 子进程已退出（{write_error}），丢弃 {} 字节",
                input.len()
            );
            return;
        }
        if matches!(write_error, crate::terminal::session::SessionError::Closed) {
            log::debug!("feedPty: 会话已退出，丢弃 {} 字节", input.len());
            return;
        }
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            JNIString::from(format!("feedPty: write failed: {write_error}")),
        );
    }
}

// JNI 导出：feedTerminal
// ══════════════════════════════════════════════════════════════════════════
// 直接把字节送入 VT 解析器（`terminal.vt_write`）而非 PTY。供测试注入必须由终端
// 解析（而非 shell 回显）的转义序列（OSC 8 链接、DECSET）。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_feedTerminal(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        feed_terminal_inner(env, session_id, data)
    })
}

/// 读 JNI 字节数组：失败即抛 `RuntimeException` 并返回 `None`。
///
/// 喂给终端（`feedTerminal`）与编码粘贴（`encodePaste`）都要这一步：数组读取
/// 本身可能失败（数组已被释放），失败必须让调用方知道——静默当作空输入的表现
/// 是「按键没反应」，无从归因。
///
/// # Safety
/// `data` 须是本次 JNI 调用期间有效的 `jbyteArray`（JNI 方法参数即满足）。
unsafe fn read_byte_array(env: &mut Env, data: jbyteArray, label: &str) -> Option<Vec<u8>> {
    // SAFETY: 调用方保证 `data` 在本次调用期间有效；`from_raw` 只包装指针而不
    // 取得所有权，局部引用由 JVM 在本 native 方法返回时释放。
    let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
    match env.convert_byte_array(&byte_array) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            let message =
                jni::strings::JNIString::from(format!("{label}: failed to read input bytes"));
            let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
            None
        }
    }
}

pub(super) fn feed_terminal_inner(env: &mut Env, session_id: jlong, data: jbyteArray) {
    let id = session_id as u64;
    // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
    let Some(input) = (unsafe { read_byte_array(env, data, "feedTerminal") }) else {
        return;
    };
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        throw_session_missing(env, "feedTerminal");
        return;
    };
    let mut session = entry.session.lock();
    session.terminal_mut().vt_write(&input);
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：encodeKeyText
// ══════════════════════════════════════════════════════════════════════════
/// 把按键编码器未覆盖的键码所对应的码点转成终端字节，`meta_prefix` 为真时加 ESC 前缀。
///
/// 只在**按键编码器不覆盖该键码**时调用：上游 `key::Key` 没有对应键的键码
/// （如 `KEYCODE_AT` 的 `@` 实为 Shift+2）落到此处，取平台给出的码点直写。
/// Ctrl 折叠不在此处——调用方已保证 Ctrl 未激活，Ctrl 由上游编码器负责。
///
/// 纯编码，**不碰会话也不写 PTY**：原实现在此处同步写，主端满时最多等
/// `WRITE_DRAIN_TIMEOUT`（5s），而调用点在 UI 线程上（击键与鼠标事件）——子进程不读
/// stdin 时每次按键冻住主线程，正是 `ptyWriteQueue` 当初要避免的事；它还绕过
/// 写入队列与 `onPtyWrite` 唤醒钩子，回显要多等一个空闲闭锁。编码留在原生（JNI
/// 不传 Java 字符串做字节通道），写入交回调用方与其它按键同一条有序出口。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodeKeyText(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    key: JString,
    meta_prefix: jboolean,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(key_str) = read_jni_string(env, &key, "encodeKeyText", "key string") else {
            return Ok(empty_java_byte_array(env));
        };
        let bytes = encode_meta_prefix(key_str.as_bytes(), meta_prefix != JNI_FALSE);
        match env.byte_array_from_slice(&bytes) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("encodeKeyText: byte_array_from_slice failed: {error}");
                // 返回空数组而非 null：Kotlin 侧声明为非空 `ByteArray`，返回 null
                // 会在 UI 线程上抛 NPE——比「这次按键丢失」严重得多。
                empty_java_byte_array(env)
            }
        }
    })
}

// JNI 导出：encodeMouseEvent
// ══════════════════════════════════════════════════════════════════════════
// 用 Ghostty 鼠标编码器把鼠标事件编码为终端转义序列（按应用方的 DECSET 选择
// SGR/X10/UTF-8）。`position` 为 surface 像素，`cellWidth`/`cellHeight` 为渲染器的实时
// 单元格尺寸。鼠标上报关闭或编码失败时返回空字节数组（该事件被丢弃）。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodeMouseEvent(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    action: jint,
    button: jint,
    modifiers: jint,
    cell_width: jfloat,
    cell_height: jfloat,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        encode_mouse_event_inner(
            env,
            session_id,
            x_px,
            y_px,
            action,
            button,
            modifiers,
            cell_width,
            cell_height,
        )
    })
}

// JNI 导出函数体可以合法地拥有很多参数：参数表由 Kotlin 的 `NativeBridge` 声明
// 决定，而非设计选择。参数个数由 ABI 固定，不可能在不配套修改 Kotlin 的情况下减少。
/// 鼠标事件被丢弃时的空字节数组（上报关闭/编码失败/空编码三处复用）。
pub(super) fn empty_java_byte_array(env: &mut Env) -> jbyteArray {
    env.byte_array_from_slice(&[])
        .map(|array| array.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// 把编码器的字节结果交给 JVM：空结果交回空数组（Kotlin 侧按「本按键不产生序列」
/// 处理），非空则拷贝成 `byte[]`；拷贝失败交回 null，此时 JVM 已挂起异常。
///
/// 三条按键/粘贴编码路径（鼠标、键盘、粘贴）共用本函数，使「空即空数组、失败即
/// null」的口径只有一处定义——各自抄一份时，任何一处漏掉空判断都会让 Kotlin 收到
/// 长度为 0 的数组与「编码失败」无法区分的信号。
pub(super) fn java_bytes_or_empty(env: &mut Env, bytes: &[u8]) -> jbyteArray {
    if bytes.is_empty() {
        return empty_java_byte_array(env);
    }
    env.byte_array_from_slice(bytes)
        .map(|array| array.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

pub(super) fn encode_mouse_event_inner(
    env: &mut Env,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    action: jint,
    button: jint,
    modifiers: jint,
    cell_width: jfloat,
    cell_height: jfloat,
) -> jbyteArray {
    let id = session_id as u64;
    // 通道外置：查询在两把锁之外执行（每次鼠标移动都会走到这里，锁内等待 VT
    // 应答会连同按帧取锁的渲染一起卡住一个查询超时）。
    let Some(query_tx) = session_query_channel(id) else {
        return empty_java_byte_array(env);
    };
    // 逐个收窄、逐个早退：收窄失败已经抛了异常并把它挂起，此后**不得**再调任何
    // JNI 函数（JNI 规范只允许极少数调用，checkJNI 下直接 abort 进程）。故这里既
    // 不再收窄其余字段，也不去 `NewByteArray`——直接交回宏的失败值。
    let Some(action) = narrow_jint(env, "encodeMouseEvent", "action", action) else {
        return std::ptr::null_mut();
    };
    let Some(button) = narrow_jint(env, "encodeMouseEvent", "button", button) else {
        return std::ptr::null_mut();
    };
    let Some(modifiers) = narrow_jint(env, "encodeMouseEvent", "modifiers", modifiers) else {
        return std::ptr::null_mut();
    };
    let Some(bytes) = crate::terminal::ghostty_terminal::GhosttyTerminal::encode_mouse_event_on(
        &query_tx,
        (x_px, y_px),
        action,
        button,
        modifiers,
        cell_width,
        cell_height,
    ) else {
        return empty_java_byte_array(env);
    };
    java_bytes_or_empty(env, &bytes)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：encodeMouseWheel
// ══════════════════════════════════════════════════════════════════════════
// 一次往返编码多档滚轮（每档 press + release）。触摸拖动一个事件可能滚过数行，
// 逐档调用会把 UI 线程按在每个档位的一次 VT 应答上；合并后每次事件只有一次
// 往返，PTY 也只写一次。鼠标上报关闭时返回空数组（该事件被丢弃）。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodeMouseWheel(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    button: jint,
    modifiers: jint,
    cell_width: jfloat,
    cell_height: jfloat,
    repeat: jint,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        encode_mouse_wheel_inner(
            env,
            session_id,
            MouseWheelEvent {
                x_px,
                y_px,
                button,
                modifiers,
                cell_width,
                cell_height,
                repeat,
            },
        )
    })
}

/// 一次滚轮事件的原始参数（仍是 JNI 的原始标量，收窄在调用方完成）。
///
/// JNI 导出的参数表由 Kotlin 侧的 `NativeBridge` 声明决定，导出函数本身无法
/// 收敛参数个数；事件参数收进结构体后，内层函数就只剩「环境 + 会话 + 事件」
/// 三件事，而不是七个平铺的标量。
pub(super) struct MouseWheelEvent {
    x_px: jfloat,
    y_px: jfloat,
    button: jint,
    modifiers: jint,
    cell_width: jfloat,
    cell_height: jfloat,
    repeat: jint,
}

pub(super) fn encode_mouse_wheel_inner(
    env: &mut Env,
    session_id: jlong,
    event: MouseWheelEvent,
) -> jbyteArray {
    let id = session_id as u64;
    let Some(query_tx) = session_query_channel(id) else {
        return empty_java_byte_array(env);
    };
    // 逐个收窄、逐个早退：收窄失败已经抛了异常并把它挂起，此后不得再调 JNI。
    let Some(button) = narrow_jint(env, "encodeMouseWheel", "button", event.button) else {
        return std::ptr::null_mut();
    };
    let Some(modifiers) = narrow_jint(env, "encodeMouseWheel", "modifiers", event.modifiers) else {
        return std::ptr::null_mut();
    };
    let Some(repeat) = narrow_jint::<u32>(env, "encodeMouseWheel", "repeat", event.repeat) else {
        return std::ptr::null_mut();
    };
    let Some(bytes) = crate::terminal::ghostty_terminal::GhosttyTerminal::encode_mouse_wheel_on(
        &query_tx,
        (event.x_px, event.y_px),
        button,
        modifiers,
        event.cell_width,
        event.cell_height,
        repeat,
    ) else {
        return empty_java_byte_array(env);
    };
    java_bytes_or_empty(env, &bytes)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：mouseTrackingActive
// ══════════════════════════════════════════════════════════════════════════
// 远端是否启用了鼠标上报（DECSET 1000/1002/1003）。无锁读取 VT 线程每帧维护
// 的镜像——触摸拖动的每个事件都要判一次，走查询通道会阻塞 UI 线程。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_mouseTrackingActive(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |_env| {
        read_terminal_flag_jboolean(session_id as u64, |session| {
            session.terminal().mouse_tracking_active_atomic()
        })
    })
}

// ── 按键编码 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：encodeKeyEvent
// ══════════════════════════════════════════════════════════════════════════
// 用 Ghostty 键编码器把按键事件编码为终端转义序列。DECCKM、modifyOtherKeys、
// kitty 键盘协议等模式全部由编码器从终端状态读取，Kotlin 不再自行查询模式。
// `keyCode`/`unicodeChar`/`unshiftedChar` 为 Android 键事件字段，`modifiers`
// 为上游 `key.Mods` 原始位。未收录的键码、不产生序列的键（单独的修饰键）与
// 编码失败都返回空字节数组，由 Kotlin 决定是否回退到按键文本。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodeKeyEvent(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    key_code: jint,
    unicode_char: jint,
    unshifted_char: jint,
    modifiers: jint,
    action: jint,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        encode_key_event_inner(
            env,
            session_id,
            key_code,
            unicode_char,
            unshifted_char,
            modifiers,
            action,
        )
    })
}

pub(super) fn encode_key_event_inner(
    env: &mut Env,
    session_id: jlong,
    key_code: jint,
    unicode_char: jint,
    unshifted_char: jint,
    modifiers: jint,
    action: jint,
) -> jbyteArray {
    let Some(key) = crate::android::keymap::key_for_android_keycode(key_code) else {
        return empty_java_byte_array(env);
    };
    // 通道外置：查询在两把锁之外执行（理由同 `encode_mouse_event_inner`）。
    let Some(query_tx) = session_query_channel(session_id as u64) else {
        return empty_java_byte_array(env);
    };
    // 收窄失败已挂起 Java 异常：此后不得再调 JNI（同上），故直接交回失败值。
    let Some(modifiers) = narrow_jint(env, "encodeKeyEvent", "modifiers", modifiers) else {
        return std::ptr::null_mut();
    };
    let Some(bytes) = crate::terminal::ghostty_terminal::GhosttyTerminal::encode_key_event_on(
        &query_tx,
        key,
        match action {
            1 => key::Action::Release,
            2 => key::Action::Repeat,
            _ => key::Action::Press,
        },
        modifiers,
        key_event_text(unicode_char),
        key_event_text(unshifted_char),
    ) else {
        return empty_java_byte_array(env);
    };
    java_bytes_or_empty(env, &bytes)
}

// ── 粘贴编码 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：encodePaste
// ══════════════════════════════════════════════════════════════════════════
// 用上游 `paste::encode` 把剪贴板字节编码为 PTY 写入字节（bracketed 包裹与否
// 取自终端模式 2004 的活体状态）。传原始字节而非 String：经 Java String 会把
// 非法 UTF-8 序列替换为 U+FFFD，静默破坏编码器看到的内容。
// 会话未知或编码失败返回空字节数组（本次粘贴被丢弃）；JVM 拷贝失败抛异常。

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodePaste(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        encode_paste_inner(env, _class, session_id, data)
    })
}

pub(super) fn encode_paste_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) -> jbyteArray {
    // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
    let Some(input) = (unsafe { read_byte_array(env, data, "encodePaste") }) else {
        return std::ptr::null_mut();
    };
    // 通道外置：查询在两把锁之外执行（理由同 `encode_mouse_event_inner`）。
    let Some(query_tx) = session_query_channel(session_id as u64) else {
        return empty_java_byte_array(env);
    };
    let Some(bytes) =
        crate::terminal::ghostty_terminal::GhosttyTerminal::encode_paste_on(&query_tx, &input)
    else {
        return empty_java_byte_array(env);
    };
    java_bytes_or_empty(env, &bytes)
}

/// Android 的码点转上游键事件的文本。
///
/// `KeyEvent.getUnicodeChar` 对回车/方向键/退格一类非文本键给出 C0 控制字符
/// （Enter→`\n`、Backspace→DEL），对 Ctrl 组合也给出控制字节。上游文档明确要求
/// 这些一律以「无文本」表达——编码器据逻辑键推导字节，收到控制字符反而会把它
/// 当作该键的真实文本发出去。macOS 私有功能键区（U+F700..U+F8FF）同理：那是
/// 平台约定的功能键代号而非字面量。
pub(super) fn key_event_text(code_point: jint) -> Option<char> {
    let code_point = u32::try_from(code_point).ok()?;
    let character = char::from_u32(code_point)?;
    (!character.is_control() && !(0xF700..=0xF8FF).contains(&code_point)).then_some(character)
}
