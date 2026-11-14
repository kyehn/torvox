//! 会话外观类 JNI 导出：主题目录、主题应用与光标样式。
//!
//! 这几个导出此前寄居在字体模块里，于是 logcat 的 tag 是
//! `native::android::ffi::font`——把主题与光标记成字体事件，按 tag 过滤必然漏掉。
//! 日志 tag 取自 `log` 宏的模块路径（`native/src/android/logging.rs` 用
//! `record.target()` 作 tag），故归位到外观模块即可让 tag 与语义一致，无需任何
//! 过滤或降级。

use super::session::read_jni_string;
use super::{
    Env, EnvUnowned, JClass, JNIString, JString, Ordering, ThrowRuntimeExAndFailure, jbyteArray,
    jint, jintArray, jlong, jni_str, render_state_mut, rlock_session_registry, theme,
    throw_session_missing,
};

// ── 主题 ──────────────────────────────────────────────────────────
/// 主题目录与全部调色板的一次性打包（布局见 [`crate::terminal::theme::pack_catalog`]）。
///
/// 一次同步调用取代「逐个标识取调色板」的 `1 + N` 次往返：目录在构建期即固定，
/// 进程内不变，而设置页要一次显示全部主题。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getThemeCatalog(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let packed = crate::terminal::theme::pack_catalog();
        match env.byte_array_from_slice(&packed) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("getThemeCatalog: byte_array_from_slice failed: {error}");
                std::ptr::null_mut()
            }
        }
    })
}

/// 按标识应用主题：终端默认色与调色板进 VT 线程，光标色与搜索高亮色供渲染态取用。
/// 标识未知即抛异常——主题真源在 [`crate::terminal::theme`]，Kotlin 无从构造清单外的主题。
///
/// `overrides` 是 `colors.properties` 的覆盖通道：19 个槽位（背景、前景、光标、16 个
/// ANSI 色），负值表示该槽沿用主题原色。合并只在此处做一次，故终端默认色、ANSI 调色板、
/// 光标色与搜索高亮色全都取自同一份合并后的调色板。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setTheme(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
    theme_name: JString<'_>,
    overrides: jintArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let id = session_id as u64;
        let Some(name) = read_jni_string(env, &theme_name, "setTheme", "theme name") else {
            return Ok(());
        };
        let Some(entry) = theme::by_name(&name) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                JNIString::from(format!("setTheme: unknown theme {name}")),
            );
            return Ok(());
        };
        // SAFETY: `overrides` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
        let Some(slots) = (unsafe { read_int_array(env, overrides, "setTheme") }) else {
            return Ok(());
        };
        let palette_override = match theme::PaletteOverride::from_slots(&slots) {
            Ok(palette_override) => palette_override,
            Err(message) => {
                throw_illegal_argument(env, &format!("setTheme: {message}"));
                return Ok(());
            }
        };
        let palette = entry.palette.with_overrides(&palette_override);
        let background = palette.background;
        {
            let registry = rlock_session_registry();
            let Some(session_entry) = registry.get(&id) else {
                throw_session_missing(env, "setTheme");
                return Ok(());
            };
            let session = session_entry.session.lock();
            session
                .terminal()
                .set_theme_palette(entry.name, palette.clone());
            // 先让 VT 线程把换主题后的单元数据推出来，再标记渲染脏：
            // 否则渲染线程可能先用旧单元数据 + 新背景色重绘一帧，造成新旧底色混屏。
            session.terminal().flush();
        }
        // 全局锁序：注册表读锁 → 会话锁 → RENDER_STATE。`render_inner` 在阶段 3
        // 持 RENDER_STATE 反向取注册表与会话锁，若此处嵌套持有即构成环形等待
        // （parking_lot 互斥锁不可重入），渲染线程与设置应用线程将同时冻结。
        //
        // 单元格着色器的 Fix F 判定会把每个单元格的背景与 `uniforms.default_background`
        // 比对，后者取自 `Renderer::background`。不同步时终端主题与渲染器默认值不一致，
        // `is_default_background` 恒为 false，背景色判定失效。
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.palette = palette;
            render_state.renderer.set_background_color(background);
            // 主题色存于实例缓存，闲时无新 CellData 则颜色过期，一并重绘。
            render_state.renderer.cell_cache = None;
            // VT 线程已推送换主题后的单元数据：标记脏并整幅重绘。
            render_state.dirty.store(true, Ordering::Relaxed);
            render_state.full_repaint = true;
        }
        drop(state);
        log::info!("setTheme: session {id} theme={name}");
    });
}

/// 读取 JNI `int[]`（调色板覆盖通道）。
///
/// # Safety
/// `data` 须是本次 JNI 调用期间有效的 `jintArray`（JNI 方法参数即满足）。
unsafe fn read_int_array(env: &mut Env<'_>, data: jintArray, label: &str) -> Option<Vec<i32>> {
    // SAFETY: 调用方保证 `data` 在本次调用期间有效；`from_raw` 只包装指针而不
    // 取得所有权，局部引用由 JVM 在本 native 方法返回时释放。
    let array = unsafe { jni::objects::JIntArray::from_raw(env, data) };
    let length = match array.len(env) {
        Ok(length) => length,
        Err(error) => {
            let message = jni::strings::JNIString::from(format!(
                "{label}: failed to read the override array length: {error}"
            ));
            let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
            return None;
        }
    };
    let mut values = vec![0i32; length];
    if let Err(error) = array.get_region(env, 0, &mut values) {
        let message = jni::strings::JNIString::from(format!(
            "{label}: failed to read the override array: {error}"
        ));
        let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
        return None;
    }
    Some(values)
}

// ── 光标样式 ──────────────────────────────────────────────────────
/// 设置会话的默认光标样式（`termux.properties` 的 `terminal-cursor-style`）。
///
/// `style` 取值与 Kotlin `TerminalCursorStyle` 的 `nativeValue` 对应：0 方块、1 竖线、
/// 2 下划线。越界即抛——静默回落成方块会让用户在配置文件里写的值无声失效。
///
/// 只是默认值：程序用 DECSCUSR 指定过样式时以程序为准。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorStyle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    style: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let Some(cursor_style) = (match style {
            0 => Some(crate::terminal::ghostty_terminal::CursorStyle::Block),
            1 => Some(crate::terminal::ghostty_terminal::CursorStyle::Bar),
            2 => Some(crate::terminal::ghostty_terminal::CursorStyle::Underline),
            _ => None,
        }) else {
            throw_illegal_argument(
                env,
                &format!("setCursorStyle: unknown cursor style {style} (expected 0..2)"),
            );
            return Ok(());
        };
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            throw_session_missing(env, "setCursorStyle");
            return Ok(());
        };
        entry
            .session
            .lock()
            .terminal()
            .set_cursor_style(cursor_style);
        drop(registry);
        log::info!("setCursorStyle: session {id} style={cursor_style:?}");
    });
}

/// 抛 `IllegalArgumentException`。错误路径可能已在 JNI 上挂起异常，重复抛会
/// 覆盖首个原因；故仅在无挂起异常时使用。
fn throw_illegal_argument(env: &mut Env<'_>, message: &str) {
    let _ = env.throw_new(
        jni_str!("java/lang/IllegalArgumentException"),
        JNIString::from(message.to_string()),
    );
}
