use super::*;

// ── 会话生命周期 ──────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：initSession
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_initSession(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    rows: jint,
    cols: jint,
    shell: JString,
    home: JString,
    working_directory: JString,
    prefix: JString,
    mkshrc_path: JString,
) -> jlong {
    jni_export_guard!(&mut unowned_env, 0, |env| {
        init_session_inner(
            env,
            _class,
            rows,
            cols,
            shell,
            home,
            working_directory,
            prefix,
            mkshrc_path,
        )
    })
}

// JNI 导出函数体可以合法地拥有很多参数：参数表由 Kotlin 的 `NativeBridge` 声明
// 决定，而非设计选择。参数个数由 ABI 固定，不可能在不配套修改 Kotlin 的情况下减少。
/// 读取 initSession 的 JNI 字符串参数，失败时抛 Java 异常并返回 None。
/// 解码一个 JNI 字符串参数。解码失败即抛 `RuntimeException` 并返回 `None`，
/// 由调用方按各自的返回类型提前退出。
///
/// 绝不把解码失败折成「参数不合法」或「未找到」那类正常空结果：前者会让
/// `Bridge.onSession` 记一条警告后继续，后者则零日志——而解码失败
/// 是 Kotlin 传了非法字符串的编程错误，静默返回会让它伪装成配置正常。
pub(super) fn read_jni_string(
    env: &mut Env,
    value: &JString,
    caller: &str,
    name: &str,
) -> Option<String> {
    match value.try_to_string(env) {
        Ok(text) => Some(text),
        Err(error) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("{caller}: failed to read {name}: {error}")),
            );
            None
        }
    }
}

pub(super) fn init_session_inner(
    env: &mut Env,
    _class: JClass,
    rows: jint,
    cols: jint,
    shell: JString,
    home: JString,
    working_directory: JString,
    prefix: JString,
    mkshrc_path: JString,
) -> jlong {
    let rows = match u32::try_from(rows) {
        Ok(r) => r,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("initSession: rows must be non-negative"),
            );
            return 0;
        }
    };
    let cols = match u32::try_from(cols) {
        Ok(c) => c,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("initSession: cols must be non-negative"),
            );
            return 0;
        }
    };

    let Some(shell_path) = read_jni_string(env, &shell, "initSession", "shell path") else {
        return 0;
    };
    // 生效 shell 由 Kotlin 侧按 DESIGN 的顺序解析（Termux bash → login → 系统 sh）。
    // 空串在此直接报错：`pty.rs` 已经以 `EmptyShell` 拒绝空 shell，此处改写会让那个
    // 错误永不可达，并把配置错误伪装成「能启动」（DESIGN 禁止未声明的回退）。
    if shell_path.is_empty() {
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            jni_str!("initSession: shell path is empty"),
        );
        return 0;
    }

    // 读取 Kotlin 侧从 bootstrap 解析出的环境（home / 工作目录 / prefix / mkshrc 路径）。
    // 空串按“未指定”处理：prefix 空表示无 PREFIX（failsafe 会话与未装引导程序时
    // 正是如此，见 TerminalRuntime 的 buildConfig），工作目录空则取 home。
    // 注意这里**不读进程环境**——子进程环境由 `pty::build_env` 按白名单逐项重建。
    let Some(home) = read_jni_string(env, &home, "initSession", "home") else {
        return 0;
    };
    let Some(working_directory) =
        read_jni_string(env, &working_directory, "initSession", "workingDirectory")
    else {
        return 0;
    };
    let Some(prefix) = read_jni_string(env, &prefix, "initSession", "prefix") else {
        return 0;
    };
    let Some(mkshrc_path) = read_jni_string(env, &mkshrc_path, "initSession", "mkshrcPath") else {
        return 0;
    };

    let default = ShellEnv::default();
    let home = if home.is_empty() {
        default.home.clone()
    } else {
        home
    };
    let working_directory = if working_directory.is_empty() {
        home.clone()
    } else {
        working_directory
    };
    let shell_env = ShellEnv {
        home,
        working_directory,
        prefix: if prefix.is_empty() {
            None
        } else {
            Some(prefix.clone())
        },
        mkshrc_path: if mkshrc_path.is_empty() {
            None
        } else {
            Some(mkshrc_path)
        },
    };

    // 回滚行数固定：PROHIBITED 禁止「终端回滚行数」设置，故不提供任何入参通道。
    // 会话以默认主题起，真实主题随后经 `setTheme` 按标识下发。
    let render_wake = Arc::new(RenderWake::default());
    let output_wake = {
        let wake = Arc::clone(&render_wake);
        Some(Arc::new(move || wake.notify()) as crate::terminal::session::OutputWake)
    };
    match Session::spawn(&shell_path, rows, cols, &shell_env, None, output_wake) {
        Ok(session) => {
            let id = next_session_id();
            let entry = SessionEntry::new(Arc::new(Mutex::new(session)), render_wake);

            let mut registry = wlock_session_registry();
            registry.insert(id, entry);
            // 原子检查并写入：并发的 `initSession` 调用（`start()` 与在 Kotlin 锁外的
            // `createSession` spawn）不得重复写入活跃 id。
            let _ = ACTIVE_SESSION_ID.compare_exchange(
                0,
                id,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            );

            log::info!("FFI: initSession -> id={}", id);
            id as jlong
        }
        Err(error) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("initSession failed: {error}")),
            );
            0
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：destroySession
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_destroySession(
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut _unowned, JNI_FALSE, |env| {
        destroy_session_inner(env, _class, session_id)
    })
}

pub(super) fn destroy_session_inner(_env: &mut Env, _class: JClass, session_id: jlong) -> jboolean {
    let id = session_id as u64;
    // 先把条目从注册表取出并在**丢弃之前**释放写锁：`Session::drop` 会杀死子进程并 join
    // 其读取/等待线程（数十至数百毫秒），期间持有全局注册表锁会阻塞所有会话的
    // pollEvent/feedPty/encodeKeyText。活跃 id 的修正在写锁**临界区内**完成，
    // 使并发的 `switchSession` 不可能插入陈旧值。
    let removed_entry = {
        let mut guard = wlock_session_registry();
        let removed = guard.remove(&id);
        if removed.is_some() {
            // 移除的是活跃会话时，清空活跃 id。
            ACTIVE_SESSION_ID
                .compare_exchange(
                    id,
                    0,
                    std::sync::atomic::Ordering::Release,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .ok();
        }
        removed
    };
    let removed = removed_entry.is_some();
    // 立即向子进程投递结束信号：注册表移除后，剪贴板应答等异步克隆仍可能短期持
    // 有 Arc<Mutex<Session>>，延迟到 Drop 才杀子进程会让 shell 在 destroySession
    // 返回后仍存活（生命周期契约被破坏）。request_exit 幂等，Drop 重复调用无害。
    if let Some(entry) = &removed_entry {
        match entry.session.try_lock() {
            Some(mut session) => session.request_exit(),
            None => {
                log::warn!(
                    "FFI: destroySession id={id}: session lock contended, exit signal deferred to Drop"
                );
            }
        }
    }
    // `removed_entry` 在**此处**、写锁之外丢弃（`Session::drop` 会杀死并 join 子进程线程）。

    if removed {
        log::info!("FFI: destroySession id={}", id);
        JNI_TRUE
    } else {
        log::warn!("FFI: destroySession id={} not found", id);
        JNI_FALSE
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：switchSession
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_switchSession(
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut _unowned, JNI_FALSE, |env| {
        switch_session_inner(env, _class, session_id)
    })
}

pub(super) fn switch_session_inner(_env: &mut Env, _class: JClass, session_id: jlong) -> jboolean {
    let id = session_id as u64;
    // 在**写锁**下原子地检查并写入：先读锁检查再无锁写入会留下 TOCTOU 窗口——
    // 并发的 `destroySession` 可能在其间删除该 id，导致存入陈旧的活跃 id。
    // destroy/switch 都是低频操作，写锁争用不成问题。
    {
        let guard = wlock_session_registry();
        if !guard.contains_key(&id) {
            log::warn!("FFI: switchSession id={} not found", id);
            return JNI_FALSE;
        }
        ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Release);
    }
    JNI_TRUE
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：resetTerminal
// ══════════════════════════════════════════════════════════════════════════

/// RIS 全重置指定会话：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_resetTerminal(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let id = session_id as u64;
        // 注册表读锁：会话锁可被 PTY 写入按住数秒（见 `setScrollOffset` 的同款理由），
        // 写锁会把销毁/切换会话等全部操作一并堵住，而本路径只读注册表。
        let Some(session) = session_handle(id) else {
            throw_session_missing(env, "resetTerminal");
            return Ok(());
        };
        session.lock().reset_terminal();
    })
}

// ══════════════════════════════════════════════════════════════════════════
