use super::super::query::{session_query_channel, u64_vec_to_long_array};
use super::super::*;

// ── 会话枚举与标题 ──────────────────────────────────────────────────
/// 返回活跃会话 id 的数组（长整型，非 JSON）。
///
/// 注：由 `NativeBridgeSmokeTest` 覆盖（会话枚举的 JVM 边界）；生产 Kotlin 侧
/// 在 `TerminalRuntime.sessionIds` 中跟踪 id。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listSessions<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jlongArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        list_sessions_inner(env, _class)
    })
}

/// 按传入顺序返回各会话的终端标题；未知会话对应元素为 null。
///
/// 会话列表刷新原本逐会话各发一次 `getTitle`，N 个会话就是 N 次同步往返。标题查询
/// 与列表刷新同批发生，故一次取回。未知会话不抛：调用方此时正在按自己的会话表回填，
/// 竞态下少一个会话是正常状态，空标题即可表达。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionTitles<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        get_session_titles_inner(env, session_ids)
    })
}

pub(super) fn get_session_titles_inner<'local>(
    env: &mut Env<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    collect_session_strings(env, session_ids, "getSessionTitles", session_title)
}

/// 按传入顺序返回各会话的 OSC 7 工作目录；未知会话或尚未上报的会话为 null。
///
/// 与 `getSessionTitles` 同批发生（抽屉打开时一次刷新），理由相同：逐会话各发一次
/// 就是 N 次同步 JNI，而列表刷新是按帧节流的路径。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionPwds<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        collect_session_strings(env, session_ids, "getSessionPwds", session_pwd)
    })
}

/// 批量取字符串的公共实现：按传入顺序为每个会话取一项，取不到的留 `null`。
///
/// 标题与工作目录的批量查询只差一个取数函数。各写一遍意味着两处都要维护
/// 「读数组 → 建 `String[]` → 逐元素填充 → 任一步失败返回 null」这套 JNI 序列，
/// 而其中任何一步写错都表现为「某些项为空」，极难定位。
fn collect_session_strings<'local>(
    env: &mut Env<'local>,
    session_ids: jlongArray,
    what: &str,
    fetch: impl Fn(u64) -> Option<String>,
) -> jobjectArray {
    // SAFETY：`session_ids` 是本次 JNI 调用的实参，生命周期覆盖本函数。
    let ids = unsafe { JLongArray::from_raw(env, session_ids) };
    let Ok(length) = ids.len(env) else {
        log::error!("{what}: cannot read session id array length");
        return std::ptr::null_mut();
    };
    let mut session_id_values = vec![0 as jlong; length];
    if ids.get_region(env, 0, &mut session_id_values).is_err() {
        log::error!("{what}: cannot read session ids");
        return std::ptr::null_mut();
    }
    let Ok(string_class) = env.find_class(jni_str!("java/lang/String")) else {
        log::error!("{what}: find_class java/lang/String failed");
        return std::ptr::null_mut();
    };
    let Ok(array) = env.new_object_array(
        session_id_values.len() as jsize,
        string_class,
        JObject::null(),
    ) else {
        log::error!("{what}: new_object_array failed");
        return std::ptr::null_mut();
    };
    for (index, id) in session_id_values.iter().enumerate() {
        // 未知会话留 null：调用方按「无此项」处理，不抛异常——批量查询发生在
        // 列表刷新途中，竞态下少一个会话是正常状态。
        let element: JObject = match fetch(*id as u64) {
            Some(text) => match env.new_string(text) {
                Ok(text) => text.into(),
                Err(error) => {
                    log::error!("{what}: cannot encode string for {id}: {error}");
                    return std::ptr::null_mut();
                }
            },
            None => JObject::null(),
        };
        if let Err(error) = array.set_element(env, index, &element) {
            log::error!("{what}: cannot store element {index}: {error}");
            return std::ptr::null_mut();
        }
    }
    array.into_raw()
}

/// Rust 字符串 → JNI `jstring`：编码失败返回 null，不抛异常、不吞错（调用方按 null 处理）。
pub(crate) fn string_to_jstring(env: &mut Env, text: &str) -> jstring {
    match env.new_string(text) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 会话标识列表 → JNI `LongArray`；注册表为空时返回空数组。
pub(super) fn list_sessions_inner<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
) -> jlongArray {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();
    u64_vec_to_long_array(env, &ids)
}

/// 字体管线默认的图集单元尺寸（像素）。
pub(crate) const DEFAULT_FONT_CELL_SIZE: f32 = 14.0;

/// 字形管线初始化用的图集尺寸。UV 归一化的尺寸由管线自身持有并对外提供，
/// 渲染侧不再并传第二份，故此处是唯一声明点。
///
/// 取 2048²（原为 1024²）：真机约 3x 显示密度下 14sp 字形光栅化后约 40px，
/// 原图集只能容纳约 700 个字形——几百个不同字符的滚动日志屏会持续冲刷 LRU，
/// 逐帧淘汰并重新光栅化同一批字形（CPU 密集）。2048² 使容量翻两番（约 2800 个字形），
/// 且内存代价有界（设备 16MB + 暂存位图 16MB）。
pub(crate) const ATLAS_SIZE: u32 = 2048;

/// 返回某会话的终端标题（OSC 0/2），无则 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTitle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 单会话查询保持严格契约：会话未知即抛，不返回空标题——调用方要能区分
        // 「没有标题」与「会话没了」。
        let Some(title) = session_title(session_id as u64) else {
            throw_session_missing(env, "getTitle");
            return Ok(std::ptr::null_mut());
        };
        string_to_jstring(env, &title)
    })
}

/// 会话标题的查询通道外置（理由同 `wideCharTailCols`：标题随每次状态更新拉取）。
/// 会话未知返回 `None`，与 `getTitle` 的抛错契约区分开。
pub(super) fn session_title(session_id: u64) -> Option<String> {
    let query_tx = session_query_channel(session_id)?;
    Some(crate::terminal::ghostty_terminal::GhosttyTerminal::title_on(&query_tx))
}

/// 会话的 OSC 7 工作目录；会话未知返回 `None`，尚未上报时是 `Some("")`。
///
/// 两者必须分开：空串是「shell 还没报」，调用方据此保留上一次已知目录；
/// `None` 是「会话没了」，调用方据此清掉该行。
fn session_pwd(session_id: u64) -> Option<String> {
    let query_tx = session_query_channel(session_id)?;
    Some(crate::terminal::ghostty_terminal::GhosttyTerminal::pwd_on(
        &query_tx,
    ))
}

/// 返回某会话从 OSC 7 记录的工作目录（原始 URI），shell 尚未上报时为空串。
///
/// 与 `getTitle` 同一契约：会话未知即抛，不返回空串冒充「未知目录」。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getPwd<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getPwd");
            return Ok(std::ptr::null_mut());
        };
        let pwd = crate::terminal::ghostty_terminal::GhosttyTerminal::pwd_on(&query_tx);
        string_to_jstring(env, &pwd)
    })
}
