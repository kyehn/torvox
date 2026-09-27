//! JNI FFI 桥接层：导出 `extern "system"` 函数供 Kotlin 调用，命名遵循
//! `Java_terminal_emulator_bridge_NativeBridge_<方法名>`。会话经 `initSession` 注册、
//! `destroySession` 注销、`switchSession` 切换；事件推入全局队列由 `pollEvent` 排空。
//!
//! 线程模型：生命周期调用（`initSession`/`destroySession`/`switchSession`/`resize`/
//! `feedPty`/`writeKey`）来自 `Dispatchers.IO`，绝不在主 UI 线程且不得无限阻塞
//! （VT 命令通道用 `try_send`，查询 RPC 用有界超时）。例外：`focusEvent` 在主线程运行，
//! 其模式查询限时 50ms 且仅在该窗口内持锁；`dialogResult`/`clipboardResult` 只短暂持
//! `REQUEST_REGISTRY` 锁。`pollEvent` 由每会话单个渲染线程按帧率调用。
//!
//! 并发：`SESSION_REGISTRY` 是 `RwLock`（读多于写），`EVENT_QUEUE` 是 `Mutex`，
//! `ACTIVE_SESSION_ID` 是 `AtomicU64`（`Acquire`/`Release`，0 = 无活跃会话）。
//! 锁顺序 `SESSION_REGISTRY` → `Session` → `exit_code`；`EVENT_QUEUE` 独立加锁，
//! 绝不在持有 `Session` 锁时获取。

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::event::Event;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::JObject;
use jni::objects::{JClass, JString};
use jni::strings::JNIString;
use jni::sys::{
    JNI_FALSE, JNI_TRUE, jboolean, jbyteArray, jfloat, jint, jintArray, jlong, jobjectArray, jsize,
    jstring,
};
use jni::{Env, EnvUnowned, jni_str};

use super::text_utils::encode_modifiers;
use crate::terminal::ShellEnv;
use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::session::Session;
use std::sync::Arc;

/// 捕获逸出 JNI 导出函数体的 panic。panic 越过 `extern "system"` 边界属未定义行为
/// （进程 abort）——所有会话瞬间死亡且崩溃处理器不运行。本守卫把 panic 转为 Java
/// RuntimeException 并向 Kotlin 返回默认值。
macro_rules! jni_export_guard {
    ($unowned:expr, $default:expr, |$env_param:ident| $call:expr) => {{
        $unowned
            .with_env(|$env_param| -> jni::errors::Result<_> { Ok($call) })
            .resolve::<ThrowRuntimeExAndDefault>()
    }};
}

// ══════════════════════════════════════════════════════════════════════════
// NDK FFI 声明
// ══════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "android")]
type JNIEnvPtr = *mut std::ffi::c_void;
#[cfg(target_os = "android")]
type JObjectPtr = *mut std::ffi::c_void;
#[cfg(target_os = "android")]
use jni::sys::jobject;

// SAFETY: 这些是来自 `libandroid.so` 的公开 Android NDK 函数。指针参数必须是有效的
// JNI 环境与 jobject 引用，调用方通过 JNI 入口点从 Kotlin/Java 运行时接收有效参数
// 从而保证这一点。
#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    pub(crate) fn ANativeWindow_fromSurface(
        env: JNIEnvPtr,
        surface: JObjectPtr,
    ) -> *mut std::ffi::c_void;
    pub(crate) fn ANativeWindow_release(window: *mut std::ffi::c_void);
    pub(crate) fn ANativeWindow_setBuffersGeometry(
        window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
}

// ══════════════════════════════════════════════════════════════════════════
// 会话注册表
// ══════════════════════════════════════════════════════════════════════════

struct SessionEntry {
    session: Arc<Mutex<Session>>,
    /// 本会话上次应用的滚动偏移。若按单一全局值计算增量，切换会话后旧会话的渲染线程
    /// 带着另一会话的偏移恢复时就会错误移动旧会话视口。
    last_scroll_offset: i64,
}

static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, SessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 全局渲染状态：Android 渲染线程使用的 wgpu 渲染器 + 字体管线。首次 `attachWindow`
/// 时惰性创建，由 JNI 渲染线程持有（Kotlin 渲染循环只从单一线程调用 `render`）。
/// `Renderer` 是 `Send + Sync`（render::context 的编译期测试保证），故互斥锁安全，
/// 且争用可忽略（每帧一次 `render`）。
static RENDER_STATE: std::sync::Mutex<Option<RenderState>> = std::sync::Mutex::new(None);

/// 当前 surface 挂载在（全局）渲染状态上的会话 id，0 = 无。`detachWindow` 仅在调用方
/// 是属主会话时才丢弃 surface——`switchSession` 先挂载新会话的 surface 再释放旧的，
/// 无条件 detach 会抹掉刚挂载的 surface（首个之后的每个会话都黑屏）。
#[cfg(target_os = "android")]
static ATTACHED_SESSION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct RenderState {
    renderer: crate::render::context::Renderer,
    font_pipeline: crate::render::font::FontPipeline,
    /// 下一帧的搜索高亮区间。由 `setSearchHighlights` 写入（按字节打包的 rows/cols），
    /// `render_inner` 消费；只有最新输入有意义（渲染循环约 60fps 且每次按键都重设）。
    /// 存为已解析结构便于按切片传入，由 `clearSearchHighlights` 清除。
    search_highlights: Vec<crate::render::cell_builder::SearchHighlight>,
    /// 上次渲染的帧（单元 + 光标 + 尺寸）。`render()` 只在有新输出时绘制，故空闲
    /// 终端复用此缓存帧而非逐帧重新合成。
    last_frame: Option<(
        Vec<crate::terminal::ghostty_terminal::CellData>,
        crate::terminal::ghostty_terminal::CursorInfo,
        u32,
        u32,
    )>,
    /// 上次绘制时的视口 Y 像素偏移——供空闲重绘门控检测逐像素滚动余数变化并强制重绘
    /// （该偏移移动像素但不改单元内容）。
    last_scroll_px: f32,
    /// 上次绘制时的搜索高亮（视口行空间）。脏带渲染必须在其变化或被清除时标记对应行，
    /// 否则陈旧的高亮像素会残留在累加器中。
    last_drawn_search_highlights: Vec<crate::render::cell_builder::SearchHighlight>,
    /// 应用层光标色覆盖（用户主题），叠加在终端自身光标色之上；`None` = 跟随终端。
    /// 由 `setCursorColor` 写入，渲染线程构建 `CellCursor` 时读取；主题应用总是伴随
    /// 重绘，故无需空闲重绘门控。
    cursor_color: Option<[f32; 4]>,
    /// 预分配的脏行掩码，跨帧复用以避免逐帧 `Vec<bool>` 分配（约 100-300 字节 × 120fps）。
    dirty_mask: Vec<bool>,
    /// 缓存的回滚长度——在 FrameData::New 时更新，空闲时复用，避免同步
    /// `scrollback_length()` RPC（VT 线程繁忙时会把渲染线程阻塞最多 50ms）。
    cached_scrollback: u32,
    /// Kitty 图像缓存（按会话键控）：VT 线程经生成戳推送变更通知，
    /// 渲染线程仅在生成戳/滚动/网格变化时查询放置，平时复用实例。
    kitty_session: u64,
    kitty_generation: u64,
    kitty_scroll_offset: i64,
    kitty_rows: u32,
    kitty_cols: u32,
    kitty_cell_width: f32,
    kitty_cell_height: f32,
    kitty_frames: Vec<crate::terminal::ghostty_terminal::KittyPlacementFrame>,
    kitty_instances: Vec<crate::render::KittyGraphicsInstance>,
    kitty_uploaded_generation: u64,
    /// 内容脏标志（见 docs/specification/REFERENCE.md）：由改动延迟渲染输入的 JNI 入口
    /// （`setSearchHighlights`/`clearSearchHighlights`、`setFontSizeInPlace`）置位，
    /// 渲染线程在 `render_inner` 中用一次 `getAndSet(false)` 交换消费。与每会话的
    /// `new_output` 标志（PTY 摄入）独立：高亮/字号变化必须重绘但绝不复位视口。
    ///
    /// 它**同时**是 Kotlin 渲染循环的唤醒信号与空闲门控的通过条件之一——绝不可作为
    /// 外层短路（空闲重绘决策在门控内部做出）。
    dirty: AtomicBool,
}

/// 在无 surface 的情况下预热渲染器与字体库：把 wgpu 设备初始化与 200+ 个系统字体加载
/// 移出 attach→首帧路径（软件 GL 上冷启动在此耗时约 3s）。spawn 后在后台线程调用一次。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        drop(render_state_mut());
        log::info!("render state prefetched");
    })
}
/// 确保渲染状态存在，首次使用时创建渲染器与字体管线。GPU 初始化失败时 panic
/// （致命——按项目策略不做优雅降级：无法渲染的终端是坏的，不应跚行运转）。
fn render_state_mut() -> std::sync::MutexGuard<'static, Option<RenderState>> {
    let mut guard = RENDER_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        let renderer = crate::render::context::Renderer::new_with_no_surface();
        let font_pipeline = crate::render::font::FontPipeline::new(
            ATLAS_SIZE as i32,
            ATLAS_SIZE as i32,
            DEFAULT_FONT_CELL_SIZE,
        );
        *guard = Some(RenderState {
            renderer,
            font_pipeline,
            search_highlights: Vec::new(),
            last_frame: None,
            last_scroll_px: 0.0,
            last_drawn_search_highlights: Vec::new(),
            cursor_color: None,
            dirty_mask: Vec::new(),
            cached_scrollback: 0,
            kitty_session: 0,
            kitty_generation: u64::MAX,
            kitty_scroll_offset: i64::MIN,
            kitty_rows: u32::MAX,
            kitty_cols: u32::MAX,
            kitty_cell_width: 0.0,
            kitty_cell_height: 0.0,
            kitty_frames: Vec::new(),
            kitty_instances: Vec::new(),
            kitty_uploaded_generation: u64::MAX,
            dirty: AtomicBool::new(false),
        });
        log::info!("render state initialized (renderer + font pipeline)");
    }
    guard
}

fn rlock_session_registry() -> parking_lot::RwLockReadGuard<'static, HashMap<u64, SessionEntry>> {
    // parking_lot 无中毒机制；panic 安全由 `jni_export_guard` 负责。
    SESSION_REGISTRY.read()
}

fn wlock_session_registry() -> parking_lot::RwLockWriteGuard<'static, HashMap<u64, SessionEntry>> {
    SESSION_REGISTRY.write()
}

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// pollEvent 扫描中每个后台会话每帧排空的最大 VT 块数。既防止后台会话的读取线程阻塞在
/// 已满的输出通道上（会填满 PTY 内核缓冲并冻结子进程），又不饿死活跃会话的帧预算。
const PTY_POLL_CHUNKS_PER_FRAME: u32 = 2;

static ACTIVE_SESSION_ID: AtomicU64 = AtomicU64::new(0);

fn next_session_id() -> u64 {
    NEXT_SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// 整包重传 KGP 图集：打包成功则替换图集与实例，失败则两者清空。
fn repack_kitty_atlas(
    render_state: &mut RenderState,
    cell_width: f32,
    cell_height: f32,
    uploaded_generation: u64,
) {
    match crate::render::kitty::pack_and_build(&render_state.kitty_frames, cell_width, cell_height)
    {
        Some((atlas, atlas_width, atlas_height, instances)) => {
            render_state
                .renderer
                .set_kgp_atlas(&atlas, atlas_width, atlas_height);
            render_state.kitty_instances = instances;
        }
        None => {
            render_state.renderer.set_kgp_atlas(&[], 0, 0);
            render_state.kitty_instances.clear();
        }
    }
    render_state.kitty_uploaded_generation = uploaded_generation;
}

// ══════════════════════════════════════════════════════════════════════════
// 事件队列
// ══════════════════════════════════════════════════════════════════════════

static EVENT_QUEUE: crate::event::EventQueue = crate::event::EventQueue::new();

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// 待宿主应用作答的剪贴板应答通道发送端，按 (会话 id, 请求 id) 存入
/// [`REQUEST_REGISTRY`]。
type ClipboardAnswerTx = std::sync::mpsc::Sender<String>;

static REQUEST_REGISTRY: LazyLock<Mutex<HashMap<(u64, u64), ClipboardAnswerTx>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 有界超时地等待宿主应用作答剪贴板。Kotlin 总会应答 `clipboardResult`（失败时也回空串），
/// 故只有进程死亡或客户端异常才会触及截止时间；空应答即兼容 xterm 的“空剪贴板”响应。
const CLIPBOARD_ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// 剪贴板应答的轮询节奏：在上述 2s 应答截止时间内，应答方以此频率检查一次性槽位。
const CLIPBOARD_POLL_INTERVAL_MS: u64 = 25;

fn wait_for_clipboard_answer(rx: std::sync::mpsc::Receiver<String>) -> String {
    let deadline = std::time::Instant::now() + CLIPBOARD_ANSWER_TIMEOUT;
    loop {
        match rx.try_recv() {
            Ok(text) => return text,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return String::new(),
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    return String::new();
                }
                std::thread::sleep(std::time::Duration::from_millis(CLIPBOARD_POLL_INTERVAL_MS));
            }
        }
    }
}

pub(crate) fn register_request(session_id: u64) -> (u64, std::sync::mpsc::Receiver<String>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    REQUEST_REGISTRY.lock().insert((session_id, request_id), tx);
    (request_id, rx)
}

/// 移除待处理的剪贴板请求而不作答。在应答线程启动前会话消失时调用，避免每次调用都在
/// `REQUEST_REGISTRY` 中泄漏一个永不应答的一次性 Sender。
pub(crate) fn cancel_request(session_id: u64, request_id: u64) {
    REQUEST_REGISTRY.lock().remove(&(session_id, request_id));
}

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
fn init_session_inner(
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

    let shell_path: String = match shell.try_to_string(env) {
        Ok(s) => s,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                jni_str!("initSession: failed to read shell path"),
            );
            return 0;
        }
    };
    // 生效 shell 由 Kotlin 侧解析（装了 bootstrap 时是 Termux bash，否则是系统 shell）。
    // 此处仍防御空值：`execve("")` 会失败并使会话立即退出。
    let shell_path = if shell_path.is_empty() {
        "/system/bin/sh".to_string()
    } else {
        shell_path
    };

    // 读取 Kotlin 侧从 bootstrap 解析出的环境（home / 工作目录 / prefix / mkshrc 路径）。
    // 空串表示“未知”，回退到进程环境。
    let read_env_string = |env: &mut Env, value: &JString, name: &str| -> Option<String> {
        match value.try_to_string(env) {
            Ok(text) => Some(text),
            Err(_) => {
                let _ = env.throw_new(
                    jni_str!("java/lang/RuntimeException"),
                    JNIString::from(format!("initSession: failed to read {name}")),
                );
                None
            }
        }
    };
    let home = match read_env_string(env, &home, "home") {
        Some(value) => value,
        None => return 0,
    };
    let working_directory = match read_env_string(env, &working_directory, "workingDirectory") {
        Some(value) => value,
        None => return 0,
    };
    let prefix = match read_env_string(env, &prefix, "prefix") {
        Some(value) => value,
        None => return 0,
    };
    let mkshrc_path = match read_env_string(env, &mkshrc_path, "mkshrcPath") {
        Some(value) => value,
        None => return 0,
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
    let theme = crate::terminal::session::ThemeConfig::default();

    match Session::spawn_with_theme(&shell_path, rows, cols, &shell_env, None, theme) {
        Ok(session) => {
            let id = next_session_id();
            let entry = SessionEntry {
                session: Arc::new(Mutex::new(session)),
                last_scroll_offset: 0,
            };

            let mut registry = wlock_session_registry();
            registry.insert(id, entry);
            // 原子检查并写入：并发的 `initSession` 调用（`start()` 与在 Kotlin 锁外的
            // `createSession` spawn）不得重复写入活跃 id。
            let _ = ACTIVE_SESSION_ID.compare_exchange(
                0,
                id,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            );

            log::info!("FFI: initSession -> id={}", id);
            id as jlong
        }
        Err(e) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("initSession failed: {e}")),
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

fn destroy_session_inner(_env: &mut Env, _class: JClass, session_id: jlong) -> jboolean {
    let id = session_id as u64;
    // 先把条目从注册表取出并在**丢弃之前**释放写锁：`Session::drop` 会杀死子进程并 join
    // 其读取/等待线程（数十至数百毫秒），期间持有全局注册表锁会阻塞所有会话的
    // pollEvent/feedPty/writeKey。活跃 id 的修正在写锁**临界区内**完成，
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
                    std::sync::atomic::Ordering::Acquire,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .ok();
        }
        removed
    };
    let removed = removed_entry.is_some();
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

fn switch_session_inner(_env: &mut Env, _class: JClass, session_id: jlong) -> jboolean {
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
        let mut registry = wlock_session_registry();
        let Some(entry) = registry.get_mut(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                jni_str!("resetTerminal: session not found"),
            );
            return Ok(());
        };
        let session = entry.session.lock();
        session.reset_terminal();
        // VT 视口已归零：同步清零本会话的滚动记账，否则下一次
        // setScrollOffset 会按 stale 值算出错误 delta 误滚视图。
        entry.last_scroll_offset = 0;
    })
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：getSessionCount
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionCount(
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
) -> jint {
    jni_export_guard!(&mut _unowned, 0, |env| get_session_count_inner(env, _class))
}

fn get_session_count_inner(_env: &mut Env, _class: JClass) -> jint {
    rlock_session_registry().len() as i32
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：getScrollbackRows
// ══════════════════════════════════════════════════════════════════════════

/// 返回某会话的回滚行数（未知或为空时为 0）。轻量读取：加会话锁 → 查询 Ghostty → 解锁。
/// 供 Kotlin 帧耗时内存仪表使用——跨窗口单调增长的行数意味着回滚无界。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getScrollbackRows(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(0);
        };
        let session = entry.session.lock();
        session.terminal().scrollback_length() as jint
    })
}

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

fn resize_inner(env: &mut Env, _class: JClass, session_id: jlong, rows: jint, cols: jint) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            jni_str!("resize: session not found"),
        );
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
    if let Err(e) = session.resize(rows, cols)
        && let Err(e) = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            JNIString::from(format!("resize: failed: {e}")),
        )
    {
        log::error!("resize: throw_new failed: {e}");
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

fn set_pixel_size_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    width_px: jint,
    height_px: jint,
) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            jni_str!("setPixelSize: session not found"),
        );
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
        && let Err(e) = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            JNIString::from(format!("setPixelSize failed: {error}")),
        )
    {
        log::error!("setPixelSize: throw_new failed: {e}");
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

fn focus_event_inner(
    _env: &mut Env,
    _class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        let mut session = entry.session.lock();
        session.focus_event(focused == JNI_TRUE);
        return JNI_TRUE;
    }
    JNI_FALSE
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

fn feed_pty_inner(env: &mut Env, _class: JClass, session_id: jlong, data: jbyteArray) {
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

    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            jni_str!("feedPty: session not found"),
        );
        return;
    };
    let mut session = entry.session.lock();
    if let Err(e) = session.write(&input) {
        // 主端 fd 是 O_NONBLOCK（`Session::spawn` 中设置）：PTY 缓冲区已满
        // （子进程未读取）时表现为 EAGAIN。丢弃输入与 xterm 行为一致；
        // 上报为错误会在大量粘贴的每次按键时刷爆日志。
        if e.is_would_block() {
            return;
        }
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            JNIString::from(format!("feedPty: write failed: {e}")),
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

fn feed_terminal_inner(env: &mut Env, session_id: jlong, data: jbyteArray) {
    let id = session_id as u64;
    // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
    let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
    let input: Vec<u8> = match env.convert_byte_array(&byte_array) {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                jni_str!("feedTerminal: failed to read input bytes"),
            );
            return;
        }
    };
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            jni_str!("java/lang/RuntimeException"),
            jni_str!("feedTerminal: session not found"),
        );
        return;
    };
    let mut session = entry.session.lock();
    session.terminal_mut().vt_write(&input);
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：writeKey
// ══════════════════════════════════════════════════════════════════════════
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_writeKey(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    key: JString,
    modifiers: jint,
    text: JString,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        write_key_inner(env, _class, session_id, key, modifiers, text)
    })
}

fn write_key_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    key: JString,
    modifiers: jint,
    text: JString,
) {
    let id = session_id as u64;

    let key_str: String = match key.try_to_string(env) {
        Ok(s) => s,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                jni_str!("writeKey: failed to read key string"),
            );
            return;
        }
    };
    let has_text = !text.is_null();

    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        let mut session = entry.session.lock();
        let result = if has_text {
            match text.try_to_string(env) {
                Ok(t) => session.write(t.as_bytes()),
                Err(_) => {
                    let _ = env.throw_new(
                        jni_str!("java/lang/RuntimeException"),
                        jni_str!("writeKey: failed to read text string"),
                    );
                    return;
                }
            }
        } else {
            // IME 可打印字符回退入口：Kotlin 侧已过滤 Ctrl（特殊键/组合键经
            // TerminalInputEncoder 编码后走 feedPty），到达此处的仅为无 Ctrl 的
            // 可打印字符（含 Alt 前缀处理）。完整 Kitty 键盘协议编码由上游
            // key::Encoder 经 Query::KeyEncode 承担（需数字 keyCode，本入口仅有
            // 字符故不适用），本函数不做 Kitty CSI-u 编码。
            let bytes = encode_modifiers(key_str.as_bytes(), modifiers);
            session.write(&bytes)
        };
        if let Err(e) = result {
            // EAGAIN（PTY 缓冲区已满）时静默丢弃输入，理由同 `feedPty`。
            if e.is_would_block() {
                return;
            }
            if let Err(e) = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("writeKey: write failed: {e}")),
            ) {
                log::error!("writeKey: throw_new failed: {e}");
            }
        }
        return;
    }
    let _ = env.throw_new(
        jni_str!("java/lang/RuntimeException"),
        jni_str!("writeKey: session not found"),
    );
}

// JNI 导出：encodeMouseEvent
// ══════════════════════════════════════════════════════════════════════════
// 用 Ghostty 鼠标编码器把鼠标事件编码为终端转义序列（按应用方的 DECSET 选择
// SGR/X10/UTF-8）。`position` 为 surface 像素，`cellW`/`cellH` 为渲染器的实时
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
    cell_w: jfloat,
    cell_h: jfloat,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        encode_mouse_event_inner(env, session_id, x_px, y_px, action, button, cell_w, cell_h)
    })
}

// JNI 导出函数体可以合法地拥有很多参数：参数表由 Kotlin 的 `NativeBridge` 声明
// 决定，而非设计选择。参数个数由 ABI 固定，不可能在不配套修改 Kotlin 的情况下减少。
fn encode_mouse_event_inner(
    env: &mut Env,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    action: jint,
    button: jint,
    cell_w: jfloat,
    cell_h: jfloat,
) -> jbyteArray {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        return env
            .byte_array_from_slice(&[])
            .map(|arr| arr.into_raw())
            .unwrap_or(std::ptr::null_mut());
    };
    let session = entry.session.lock();
    let Some(bytes) = session.terminal().encode_mouse_event(
        (x_px, y_px),
        action as u8,
        button as u8,
        cell_w,
        cell_h,
    ) else {
        return env
            .byte_array_from_slice(&[])
            .map(|arr| arr.into_raw())
            .unwrap_or(std::ptr::null_mut());
    };
    if bytes.is_empty() {
        return env
            .byte_array_from_slice(&[])
            .map(|arr| arr.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }
    env.byte_array_from_slice(&bytes)
        .map(|arr| arr.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── 事件轮询 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：pollEvent
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pollEvent<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
) -> jstring {
    // 此处的 panic（如 ghostty 的 VT 处理内部）会 abort 整个进程，故转为 Java 异常。
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        poll_event_inner(env, class)
    })
}

/// 等待（最多 EXIT_CODE_WAIT_TIMEOUT_MS）等待线程写入会话的退出码，以短暂持有会话锁的方式轮询，
/// 避免其他线程在整个等待期被阻塞。读取线程可能在等待线程写入 `exit_code` 前刚好
/// 置位 `exited`（EOF）；本函数关闭该窗口，使真实退出码（如 137）不会被报成 0。
/// **必须**在不持有会话锁时调用。
///
/// 超时时按 0 上报——与正常退出无法区分。这是已文档化的取舍：延长阻塞会拖慢
/// pollEvent 帧，而退出事件无法重发（`mark_exit_reported` 已置位），只有 warn 日志可作信号。
/// 退出码等待上限：轮询步数 × 步长即此值，超时按 0 上报（见函数文档）。
const EXIT_CODE_WAIT_TIMEOUT_MS: u64 = 100;
/// 退出码轮询步长：短暂持有会话锁后让出，避免阻塞其他线程。
const EXIT_CODE_POLL_STEP_MS: u64 = 10;

fn wait_exit_code(session: &Arc<Mutex<Session>>) -> i32 {
    for _ in 0..(EXIT_CODE_WAIT_TIMEOUT_MS / EXIT_CODE_POLL_STEP_MS) {
        {
            let guard = session.as_ref().lock();
            if let Some(code) = guard.exit_code_now() {
                return code;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(EXIT_CODE_POLL_STEP_MS));
    }
    log::warn!("ffi: exit code not written within {EXIT_CODE_WAIT_TIMEOUT_MS}ms of exit");
    0
}

/// 读取子进程记录的存活时长（fork → waitpid，毫秒）。由等待线程在退出时写入，
/// 故本调用立即返回；0 是字段尚未填充的异常会话的回退值。
fn wait_exit_alive_ms(session: &Arc<Mutex<Session>>) -> u64 {
    let guard = session.as_ref().lock();
    let alive = guard.exit_alive_ms.lock();
    (*alive).unwrap_or(0)
}

fn poll_event_inner<'local>(env: &mut Env<'local>, _class: JClass<'local>) -> jstring {
    // 步骤 1：轮询活跃会话的新事件。先收集事件，释放会话锁后再推入，
    // 以保持锁顺序 `SESSION_REGISTRY` → `Session` → `EVENT_QUEUE`。
    // 绝不在持有 `Session` 锁时锁 `EVENT_QUEUE`。
    let mut pending_events: Vec<Event> = Vec::new();
    // 本帧需上报 Exit 事件的会话，并克隆其 Arc，使退出码可在释放
    // `SESSION_REGISTRY` 读锁**之后**读取：`wait_exit_code` 最多忙等 100ms，
    // 长时间持有注册表读锁会阻塞 destroySession/initSession 的写锁（RwLock 写者饥饿）。
    let mut pending_exits: Vec<(u64, Arc<Mutex<Session>>)> = Vec::new();
    // 剪贴板/退出的轮询对活跃会话与所有后台会话完全相同，共用同一实现。
    let collect_session_events =
        |session_id: u64,
         session: &mut Session,
         handle: &Arc<Mutex<Session>>,
         events: &mut Vec<Event>,
         exits: &mut Vec<(u64, Arc<Mutex<Session>>)>| {
            if let Some(text) = session.poll_clipboard() {
                events.push(Event::Clipboard { session_id, text });
            }
            // BEL 振铃与剪贴板同一优先级：锁存取走即上报（单帧多响已合并为一）。
            if session.poll_bell() {
                events.push(Event::Bell { session_id });
            }
            // 进程退出后只有首次轮询会报告（`mark_exit_reported`）；后台扫描分支
            // 使用同一去重，使缓慢的消费方也不会看到同一会话的重复 Exit 事件。
            // 退出码在两个锁都释放之后才读取（见下方 `pending_exits`）。
            if session.is_exited() && session.mark_exit_reported() {
                exits.push((session_id, handle.clone()));
            }
        };
    let mut pending_clipboard_reads: Vec<(u64, String)> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    {
        let registry = rlock_session_registry();
        if active_id != 0
            && let Some(entry) = registry.get(&active_id)
        {
            let mut session = entry.session.lock();
            // 处理来自 PTY 读取线程的 VT 输出。这是驱动全部终端状态更新的关键路径：
            // 从 `output_rx` 读取数据送入 Ghostty 的 VT 解析器，并填充下方轮询的
            // 各类事件标志（剪贴板等）。缺少此调用则终端永远不处理输出，输出通道死锁。
            session.process_output();
            // 检查 OSC 52 剪贴板读取请求（`ESC ] 52 ; c ; ?`）。此处（会话锁内）
            // 收集 selection 名；一次性槽位与应答线程在注册表/会话锁释放后才建立
            // （见下方），保持锁顺序单一方向。
            if let Some(selection) = session.poll_clipboard_read() {
                pending_clipboard_reads.push((active_id, selection));
            }
            collect_session_events(
                active_id,
                &mut session,
                &entry.session,
                &mut pending_events,
                &mut pending_exits,
            );
            // 会话锁在此释放（if-let 块末尾）。
        }
        // 扫描后台会话：一次性上报退出（`exit_reported` 标志）**并**排空其 PTY 输出。
        // 后台会话的输出通道一旦填满（读取线程阻塞在有界发送 → PTY 内核缓冲填满
        // → 子进程写入阻塞）就会冻结后台作业；每帧排空若干块既保持管道流动，
        // 又不饿死活跃会话的帧预算。
        for (id, entry) in registry.iter() {
            if active_id != 0 && *id == active_id {
                continue;
            }
            let mut session = entry.session.lock();
            // 每个后台会话每帧 2 块：足以在持续输出下不让读取线程阻塞。
            session.poll_pty_output(PTY_POLL_CHUNKS_PER_FRAME);
            // 立即消费陈旧的事件标志并带上正确的 `session_id` 推入。若留着不管，
            // 它们会在数分钟后该会话重新活跃时被重放（陈旧重放）。
            collect_session_events(
                *id,
                &mut session,
                &entry.session,
                &mut pending_events,
                &mut pending_exits,
            );
        }
        // 注册表读锁在此释放。
    }
    // 处理待处理的 OSC 52 剪贴板读取请求（在注册表/会话锁之外）：登记一次性应答槽位、
    // 推入事件，并启动短生命周期的应答线程把宿主应用的答复写回 PTY。
    // VT 线程绝不能阻塞等待宿主应用，且 `clipboardResult` 可能在任意线程到达。
    for (session_id, selection) in pending_clipboard_reads {
        let (request_id, rx) = register_request(session_id);
        pending_events.push(Event::ClipboardRead {
            session_id,
            request_id,
            selection: selection.clone(),
        });
        let registry = wlock_session_registry();
        let session = registry.get(&session_id).map(|entry| entry.session.clone());
        drop(registry);
        if let Some(session) = session {
            std::thread::spawn(move || {
                let text = wait_for_clipboard_answer(rx);
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(&selection, &text) {
                    log::warn!("osc52: clipboard read answer write-back failed: {error}");
                }
            });
        } else {
            // 应答线程启动前会话已消失：丢弃一次性槽位，避免 `REQUEST_REGISTRY` 泄漏 Sender。
            cancel_request(session_id, request_id);
        }
    }

    // 退出码在注册表读锁释放**之后**才读取：等待最长 100ms，长时间持有读锁会让
    // destroySession/initSession 的写锁饥饿（RwLock 写者饥饿）。克隆的 Arc 保证会话
    // 不受注册表变动影响而保持存活。
    for (id, session) in pending_exits {
        pending_events.push(Event::Exit {
            session_id: id,
            code: wait_exit_code(&session),
            alive_ms: wait_exit_alive_ms(&session),
        });
    }
    // 推入已收集的事件——此时不持有 `Session` 或 `SESSION_REGISTRY` 锁。
    for event in pending_events {
        EVENT_QUEUE.push(event);
    }

    // 步骤 2：从队列排空一个事件。
    let event = EVENT_QUEUE.pop();

    match event {
        Some(e) => {
            let json = serde_json::to_string(&e).unwrap_or_else(|err| {
                log::error!("pollEvent: event serialization failed: {err}");
                String::new()
            });
            match env.new_string(&json) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        None => std::ptr::null_mut(),
    }
}

// ── `new_output` 旁路标志 ──────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：consumeNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// 读取并清除每会话的 `new_output` 标志（滚动复位信号，见 docs/specification/REFERENCE.md）。
///
/// 标志由 PTY 摄入路径（`Session::process_output` / `poll_pty_output` →
/// `OutputProcessor::process`）置位，渲染线程在此每帧作为**旁路**读清（与
/// `pollAll()` 循环并行）——刻意不做成排队的 `Event` 变体：持续输出（`tail -f`）下
/// 事件变体会抢占 `MAX_EVENTS_PER_POLL` 预算，饿死剪贴板/退出事件。
/// 与 `dirty` 标志相互独立（选区/高亮/字号变化必须重绘但绝不复位视口）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_consumeNewOutput(
    _unowned: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    // 内部无 panic 风险（无 JNI 调用、无 unwrap）；仍保持与相邻导出点一致的守卫写法。
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&(session_id as u64)) else {
        return JNI_FALSE;
    };
    let session = entry.session.lock();
    if session.take_new_output() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// ── 日志与渲染生命周期 ──────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：initLogger
// ══════════════════════════════════════════════════════════════════════════
/// 初始化 Rust 侧日志（logcat + 可选文件）；应用启动时由 Kotlin 调用一次。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_initLogger(
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut _unowned, (), |env| init_logger_inner(env, _class))
}

fn init_logger_inner(_env: &mut Env, _class: JClass) {
    // 本 crate 没有 `JNI_OnLoad` 钩子，故这是唯一能初始化日志的地方。缺了它，生产
    // 环境中的每次 `log::*` 调用（含 GPU 错误、锁中毒、VT 线程 panic）都会被静默
    // 丢弃，使崩溃诊断不可能。
    #[cfg(target_os = "android")]
    crate::android::logging::init();
    log::info!("NativeBridge::initLogger called");
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：attachWindow
// ══════════════════════════════════════════════════════════════════════════

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
    // SAFETY: `ptr` 是有效的 `ANativeWindow*`（上方已检查非空）；
    // `ANativeWindow_setBuffersGeometry` 是有文档的 NDK 函数。
    unsafe {
        ANativeWindow_setBuffersGeometry(ptr, width, height, 1);
    }
    let mut state = render_state_mut();
    let renderer = &mut state
        .as_mut()
        .expect("render_state_mut always initializes")
        .renderer;
    let session_id = _session_id as u64;
    match renderer.attach_surface(ptr.cast(), width.max(0) as u32, height.max(0) as u32) {
        Ok(()) => {
            ATTACHED_SESSION_ID.store(session_id, std::sync::atomic::Ordering::Release);
            log::info!("FFI: attachWindow surface attached (session {session_id})");
        }
        Err(error) => {
            log::error!("FFI: attachWindow surface attach failed: {error}");
        }
    }
    // SAFETY: `ptr` 是来自 `ANativeWindow_fromSurface` 的有效 `ANativeWindow*`。
    // 无论挂载成功或失败，调用方自有的引用都必须释放：成功路径下 wgpu 在创建 surface
    // 时已取得自己的引用，失败路径下则无人接管。`ANativeWindow_release` 是有文档的
    // NDK 函数，会递减窗口引用计数。
    unsafe { ANativeWindow_release(ptr) };
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：render——经 CellData 快路径渲染一帧
// ══════════════════════════════════════════════════════════════════════════

/// 为活跃会话渲染一帧。返回：1 = 有输出且已呈现帧；0 = 会话无待处理单元数据（空闲）；
/// -1 = 出错（surface 缺失、GPU 失败、会话未知）。
///
/// 由 Kotlin 的渲染循环（单一线程）调用。`width`/`height` 仅供参考（真实尺寸由已挂载
/// 的 surface 配置决定），保留它们以便将来的 resize 路径可在此重配置交换链。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_render<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jint {
    jni_export_guard!(&mut unowned_env, -1, |_env| render_inner(session_id as u64))
}

/// 由 `CursorInfo` 快照构建光标，并套用渲染状态中的光标色（用户主题）。
/// 光标样式恒为默认块状：样式覆盖已随光标样式设置一并移除。
fn build_cursor(
    render_state: &RenderState,
    cursor_info: &crate::terminal::ghostty_terminal::CursorInfo,
) -> crate::render::CellCursor {
    crate::render::CellCursor {
        row: cursor_info.row,
        col: cursor_info.col,
        visible: cursor_info.visible,
        style: cursor_info.style,
        color: render_state.cursor_color,
    }
}

/// 在掩码中把叠加行（搜索高亮）标脏。此类逐行视觉叠加的增删会改变像素而不触及单元内容。
/// 选区无需叠加处理：VT 线程把跟踪选区的反显烘焙进 `CellData`，选区变化经正常的
/// 脏路径作为新单元内容到达。
fn mark_overlay_dirty_rows(dirty_mask: &mut [bool], rows_usize: usize, highlight_rows: &[i32]) {
    // 搜索高亮行，当前与上次绘制的都要算：高亮是逐行叠加，增删移动会改变像素而
    // 不改变单元内容。
    for r in highlight_rows {
        if *r >= 0 && (*r as usize) < rows_usize {
            dirty_mask[*r as usize] = true;
        }
    }
}

/// 从当前与上次绘制的高亮中收集高亮行号。
fn collect_highlight_rows(render_state: &RenderState) -> Vec<i32> {
    render_state
        .search_highlights
        .iter()
        .chain(render_state.last_drawn_search_highlights.iter())
        .map(|hl| hl.row)
        .collect()
}

fn render_inner(session_id: u64) -> jint {
    // ── 阶段 1：渲染前准备（单次持有 RENDER_STATE 锁）───────────────────
    // 检查 surface 就绪状态、上传图集脏区、惰性创建管线——全部在**一次**加锁内完成
    // 而非原先的三次，使每帧的 RENDER_STATE 加锁往返从 3 次降到 2 次（第二次在阶段 3）。
    {
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return 0;
        };
        // 任何渲染工作之前必须已挂载 surface。
        if render_state.renderer.surface.is_none() {
            return 0;
        }
        // 惰性一次性创建管线。
        if render_state.renderer.cell_pipeline.is_none() {
            let (w, h) = render_state
                .renderer
                .surface_config
                .as_ref()
                .map_or((0, 0), |c| (c.width, c.height));
            if w == 0 || h == 0 {
                return 0;
            }
            render_state
                .renderer
                .initialize_pipeline_and_bind_group(ATLAS_SIZE, ATLAS_SIZE, w, h);
        }
        // 上传字形图集的脏区（即便在空闲帧：新光栅化的字形必须在下次绘制前抵达 GPU 纹理）。
        if let Some(rect) = render_state.font_pipeline.take_dirty_rect() {
            let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
            render_state.renderer.upload_atlas(
                render_state.font_pipeline.atlas_bitmap(),
                aw,
                ah,
                Some(rect),
            );
        }
    } // ── render_state lock released ──────────────────────────────────────

    // ── 阶段 2：收集单元数据（仅持会话锁）───────────────────────────────
    // 有新数据时从通道接收自有的 `CellData`（零拷贝移动）；空闲时只记录事实——
    // 阶段 3 将直接引用 `last_frame`，省去此处原有的 32KB 克隆。
    // 暂停期不消费通道：receive 是破坏性取数，暂停帧取走后永不呈现
    //（render_frame 直接丢弃），恢复后 VT 去重也不再重推——“IME 弹出
    // 时输入不可见”的主因。帧留在通道里，恢复后第一帧即最新。
    enum FrameData {
        New {
            cells: Vec<crate::terminal::ghostty_terminal::CellData>,
            cursor_info: crate::terminal::ghostty_terminal::CursorInfo,
            rows: u32,
            cols: u32,
            scroll_offset: i64,
        },
        Idle {},
    }
    let paused = {
        let state = render_state_mut();
        state
            .as_ref()
            .is_some_and(|render_state| render_state.renderer.render_paused)
    };
    if paused {
        return 0;
    }
    let frame_data = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&session_id) else {
            log::warn!("render: unknown session {session_id}");
            return -1;
        };
        let session = entry.session.lock();
        // 关键：**不要**在此调用 `session.terminal().scrollback_length()`——
        // 它是发往 VT 线程的同步 RPC，VT 线程繁忙时会把渲染线程阻塞最多 50ms。
        // 回滚长度搭载在经单元数据通道传递的 `CursorInfo` 上
        // （见 `push_cell_data` → `CursorInfo.scrollback_length`）。
        match session.terminal().receive_cell_data() {
            Some((cells, cursor_info)) => {
                let (rows, cols) = session.grid_size();
                FrameData::New {
                    cells,
                    cursor_info,
                    rows,
                    cols,
                    scroll_offset: entry.last_scroll_offset,
                }
            }
            None => FrameData::Idle {},
        }
    }; // ── 会话锁在此释放 ─────────────────────────────────────────────────

    // ── 阶段 3：渲染（持渲染状态锁）────────────────────────────────────
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return -1;
    };
    if render_state.renderer.surface.is_none() {
        return 0;
    }
    // 单点读清内容脏标志。
    let content_dirty = render_state.dirty.swap(false, Ordering::AcqRel);

    // 按新数据/空闲分支——空闲路径引用 `last_frame` 而不克隆，每个空闲帧省下约 32KB。
    match frame_data {
        FrameData::New {
            cells,
            cursor_info,
            rows,
            cols,
            scroll_offset,
        } => {
            // 使用单元数据通道带来的回滚长度——无需同步 RPC。
            // VT 线程在每次推送 `CursorInfo` 时都附带它。
            let scrollback = cursor_info.scrollback_length;
            render_state.cached_scrollback = scrollback;
            // Kitty 同步：生成戳为 0 且无缓存时跳过查询（纯文本零开销）；
            // 会话/生成戳/滚动/网格任一变化才重查放置；图集仅在生成戳变化时
            // 打包重传，滚动/缩放只经无拷贝布局重建实例。
            let kitty_generation = cursor_info.kitty_generation;
            // 网格单元格像素（与 render_cell_data 同口径：字体度量×光栅缩放）。
            let (font_width, font_height) = render_state.font_pipeline.cell_metrics();
            let raster_scale = render_state.font_pipeline.get_raster_scale();
            let grid_cell_width = font_width * raster_scale;
            let grid_cell_height = font_height * raster_scale;
            let mut kitty_keys_changed = session_id != render_state.kitty_session
                || kitty_generation != render_state.kitty_generation
                || scroll_offset != render_state.kitty_scroll_offset
                || rows != render_state.kitty_rows
                || cols != render_state.kitty_cols;
            let kitty_cell_changed = (grid_cell_width - render_state.kitty_cell_width).abs()
                > f32::EPSILON
                || (grid_cell_height - render_state.kitty_cell_height).abs() > f32::EPSILON;
            // 单元格几何失步：终端侧仍为旧值时上游按旧几何重算 pixel 尺寸，
            // 与新 origin 口径不一致。先同步新尺寸到终端再重查
            // （VT 先排空命令积压再处理查询，命令先发即先生效，无竞态）。
            // 纯文本（generation==0）零开销跳过；首图/切会话无缓存帧也同步，
            // 否则缓存的新尺寸会永久掩盖终端侧的旧几何。
            if kitty_generation != 0
                && (kitty_cell_changed || session_id != render_state.kitty_session)
            {
                let cell_width = grid_cell_width.max(1.0) as u32;
                let cell_height = grid_cell_height.max(1.0) as u32;
                let registry = rlock_session_registry();
                if let Some(entry) = registry.get(&session_id) {
                    entry
                        .session
                        .lock()
                        .terminal()
                        .set_cell_pixel_size(cell_width, cell_height);
                }
                // 强制重查：旧帧 pixel 尺寸按旧几何解算，必须按新几何重算。
                kitty_keys_changed = true;
            }
            if kitty_keys_changed || kitty_cell_changed {
                let generation_changed = kitty_generation != render_state.kitty_generation
                    || session_id != render_state.kitty_session;
                render_state.kitty_session = session_id;
                render_state.kitty_generation = kitty_generation;
                render_state.kitty_scroll_offset = scroll_offset;
                render_state.kitty_rows = rows;
                render_state.kitty_cols = cols;
                render_state.kitty_cell_width = grid_cell_width;
                render_state.kitty_cell_height = grid_cell_height;
                if kitty_generation == 0 {
                    render_state.kitty_frames.clear();
                    render_state.kitty_instances.clear();
                    if render_state.kitty_uploaded_generation != 0 {
                        render_state.renderer.set_kgp_atlas(&[], 0, 0);
                        render_state.kitty_uploaded_generation = 0;
                    }
                } else {
                    // 渲染状态锁内 RPC：仅放置键变化时触发；VT 刚推送帧，
                    // 查询通常毫秒级返回（卡住时由 500ms 超时兜底）。
                    if kitty_keys_changed {
                        let frames = {
                            let registry = rlock_session_registry();
                            registry
                                .get(&session_id)
                                .map(|entry| {
                                    entry.session.lock().terminal().take_kitty_placements()
                                })
                                .unwrap_or_default()
                        };
                        render_state.kitty_frames = frames;
                    }
                    if generation_changed {
                        repack_kitty_atlas(
                            render_state,
                            grid_cell_width,
                            grid_cell_height,
                            kitty_generation,
                        );
                    } else if let Some((atlas_width, atlas_height, entries)) =
                        crate::render::kitty::layout_entries(&render_state.kitty_frames)
                    {
                        // 同图集复用：仅重建实例。防御已上传图集尺寸漂移则全量重传。
                        let (uploaded_width, uploaded_height) = (
                            render_state.renderer.kgp_atlas_width,
                            render_state.renderer.kgp_atlas_height,
                        );
                        if atlas_width == uploaded_width && atlas_height == uploaded_height {
                            render_state.kitty_instances =
                                crate::render::kitty::build_kitty_instances(
                                    &render_state.kitty_frames,
                                    atlas_width,
                                    atlas_height,
                                    &entries,
                                    grid_cell_width,
                                    grid_cell_height,
                                );
                        } else {
                            repack_kitty_atlas(
                                render_state,
                                grid_cell_width,
                                grid_cell_height,
                                kitty_generation,
                            );
                        }
                    } else {
                        render_state.kitty_instances.clear();
                    }
                }
            }
            let cursor = build_cursor(render_state, &cursor_info);
            // 用预分配的缓冲构建脏掩码。
            let rows_usize = rows as usize;
            // 先取不可变快照：掩码是对 `render_state.dirty_mask` 的 `&mut` 借用，
            // 无法与下方对 `render_state` 其他字段的读取共存。
            let scroll_up_rows: Option<u32> = None;
            let previous_cursor_row = render_state
                .last_frame
                .as_ref()
                .map(|(_, old_cursor_info, _, _)| old_cursor_info.row);
            let highlight_rows = collect_highlight_rows(render_state);
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            match &render_state.last_frame {
                Some((old_cells, _, old_rows, old_cols))
                    if *old_rows == rows && *old_cols == cols =>
                {
                    if let Some(s) = scroll_up_rows {
                        // 纯滚动：只有新露出的底部行带有新内容，其余经累加器 blit 到达。
                        let start = rows_usize - s as usize;
                        dirty_mask[start..rows_usize].fill(true);
                    } else {
                        crate::render::cell_builder::diff_dirty_rows_into(
                            old_cells,
                            &cells,
                            rows,
                            &mut dirty_mask[..rows_usize],
                        );
                    }
                }
                _ => {
                    dirty_mask[..rows_usize].fill(true);
                }
            }
            // 光标行：无论是否可见，**当前与上一次**的光标行都标脏——光标是叠加在实例
            // 之上的（可见性/位置变化会改变像素而不改单元内容），而脏带渲染必须重绘
            // 每一受影响的行，否则陈旧的光标像素会残留。
            if (cursor.row as usize) < rows_usize {
                dirty_mask[cursor.row as usize] = true;
            }
            if let Some(prev_row) = previous_cursor_row
                && (prev_row as usize) < rows_usize
            {
                dirty_mask[prev_row as usize] = true;
            }
            mark_overlay_dirty_rows(dirty_mask, rows_usize, &highlight_rows);
            let result = render_state.renderer.render_cell_data(
                &cells,
                rows,
                cols,
                cursor,
                &mut render_state.font_pipeline,
                ATLAS_SIZE as f32,
                ATLAS_SIZE as f32,
                &render_state.search_highlights,
                Some(&render_state.dirty_mask),
                scroll_up_rows,
                &render_state.kitty_instances,
            );
            if result.is_ok() {
                render_state.last_frame = Some((cells, cursor_info, rows, cols));
                render_state.last_drawn_search_highlights = render_state.search_highlights.clone();
                render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
            }
            match result {
                Ok(()) => 1,
                Err(error) => {
                    log::error!("render: frame failed: {error}");
                    -1
                }
            }
        }
        FrameData::Idle {} => {
            // 空闲路径：引用 `last_frame`——零克隆。
            let Some((ref cached_cells, cached_cursor, cached_rows, cached_cols)) =
                render_state.last_frame
            else {
                return 0;
            };
            let cursor = build_cursor(render_state, &cached_cursor);
            // 空闲重绘门控：仅当确有变化时重绘——搜索高亮、滚动偏移、内容脏标志被置位，
            // 或累加器失效（surface 重挂载/resize：新交换链从未收到过帧，空闲 shell
            // 会永远黑屏）。选区经 VT 线程作为新单元内容到达，故无需自己的门控。
            let highlights_changed =
                render_state.search_highlights != render_state.last_drawn_search_highlights;
            let scroll_px_changed =
                (render_state.renderer.viewport_scroll_px - render_state.last_scroll_px).abs()
                    > f32::EPSILON;
            let needs_repaint = highlights_changed
                || scroll_px_changed
                || content_dirty
                || render_state.renderer.frame_invalidated;
            if !needs_repaint {
                return 0;
            }
            let rows_usize = cached_rows as usize;
            let highlight_rows = collect_highlight_rows(render_state);
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            // 光标行无论可见性都标脏：光标是叠加层，其像素变化不触及单元内容。
            if (cursor.row as usize) < rows_usize {
                dirty_mask[cursor.row as usize] = true;
            }
            mark_overlay_dirty_rows(dirty_mask, rows_usize, &highlight_rows);
            // Idle Kitty 同步：字体/缩放变化不经过 VT（无 New 帧），缓存实例
            // 会按旧单元格尺寸错位。纯本地经布局重建（无 RPC，图集未变不重传）。
            if !render_state.kitty_frames.is_empty() {
                let (idle_font_width, idle_font_height) = render_state.font_pipeline.cell_metrics();
                let idle_scale = render_state.font_pipeline.get_raster_scale();
                let idle_cell_width = idle_font_width * idle_scale;
                let idle_cell_height = idle_font_height * idle_scale;
                let idle_cell_changed = (idle_cell_width - render_state.kitty_cell_width).abs()
                    > f32::EPSILON
                    || (idle_cell_height - render_state.kitty_cell_height).abs() > f32::EPSILON;
                if idle_cell_changed {
                    let rebuilt = crate::render::kitty::layout_entries(&render_state.kitty_frames)
                        .filter(|(layout_width, layout_height, _)| {
                            *layout_width == render_state.renderer.kgp_atlas_width
                                && *layout_height == render_state.renderer.kgp_atlas_height
                        })
                        .map(|(layout_width, layout_height, entries)| {
                            crate::render::kitty::build_kitty_instances(
                                &render_state.kitty_frames,
                                layout_width,
                                layout_height,
                                &entries,
                                idle_cell_width,
                                idle_cell_height,
                            )
                        });
                    if let Some(instances) = rebuilt {
                        render_state.kitty_instances = instances;
                    }
                    render_state.kitty_cell_width = idle_cell_width;
                    render_state.kitty_cell_height = idle_cell_height;
                }
            }
            let result = render_state.renderer.render_cell_data(
                cached_cells,
                cached_rows,
                cached_cols,
                cursor,
                &mut render_state.font_pipeline,
                ATLAS_SIZE as f32,
                ATLAS_SIZE as f32,
                &render_state.search_highlights,
                Some(&render_state.dirty_mask),
                None,
                &render_state.kitty_instances,
            );
            if result.is_ok() {
                render_state.last_drawn_search_highlights = render_state.search_highlights.clone();
                render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
                // 注意：空闲时**不**更新 `last_frame`——单元未变。
            }
            match result {
                Ok(()) => 1,
                Err(error) => {
                    log::error!("render: frame failed: {error}");
                    -1
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：renderWithNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// 合并的 render + consumeNewOutput：单次 JNI 穿越即完成渲染一帧**与**读取每会话的
/// `new_output` 标志，比分开两次调用每帧省约 0.1-0.3ms。
///
/// 返回打包的 `jlong`：位 0..31 = 渲染计数（语义同 `render()`）；位 32 = `new_output`
/// 标志（1 = 已摄入 PTY 输出，0 = 空闲）；位 33..48 = 视口光标行（0xFFFF = 隐藏/
/// 视口外）；位 49..63 = 0（保留）。
///
/// Kotlin 必须分别掩码两个字段：裸读 `(packed shr 32) != 0` 会把光标位误当作输出。
/// 出错时渲染计数为负、`new_output` 为 0、光标行为 0xFFFF。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_renderWithNewOutput<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jlong {
    let count = jni_export_guard!(&mut unowned_env, -1i32, |_env| render_inner(
        session_id as u64
    ));
    let mut new_output: i64 = 0;
    let mut cursor_bits: i64 = 0xFFFF;
    if count > 0 {
        // 就地消费 `new_output` 标志（逻辑同 `consumeNewOutput` 但省一次 JNI 穿越），
        // 并从同一个已加锁的会话采样视口光标行，使跟随输入法的平移看到本帧绘制的坐标。
        let registry = rlock_session_registry();
        if let Some(entry) = registry.get(&(session_id as u64)) {
            let session = entry.session.lock();
            if session.take_new_output() {
                new_output = 1;
            }
            if let Some((row, _)) = session.terminal().render_cursor() {
                cursor_bits = (row as i64) & 0xFFFF;
            }
        }
    }
    (new_output << 32) | (cursor_bits << 33) | (count as i64 & 0xFFFF_FFFF)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：detachWindow
// ══════════════════════════════════════════════════════════════════════════

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
fn detach_window_inner(_env: &mut Env, _class: JClass, _session_id: jlong) {
    let session_id = _session_id as u64;
    // 只有拥有已挂载 surface 的会话才能丢弃它：`switchSession` 先挂载新会话的 surface
    // 再卸载旧的，无条件卸载会抹掉刚挂载的 surface（首个之后的每个会话都黑屏）。
    //
    // 属主检查与 surface 丢弃在**同一个** `RENDER_STATE` 锁内完成
    // （`attach_window_inner` 也取该锁）：若拆成“先检查后丢弃”，并发的挂载
    // （已存入新属主）可能插入其间而其新建的 surface 被丢弃。
    let Ok(mut state) = RENDER_STATE.lock() else {
        log::error!("detachWindow: render state lock poisoned");
        return;
    };
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

// ══════════════════════════════════════════════════════════════════════════
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：clipboardResult——Kotlin 应答 OSC 52 读取请求
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clipboardResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        clipboard_result_inner(env, _class, session_id, request_id, text)
    })
}

fn clipboard_result_inner<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;

    if let Some(tx) = REQUEST_REGISTRY.lock().remove(&(session_id, request_id)) {
        let text_str: String = text.try_to_string(env).unwrap_or_default();
        let _ = tx.send(text_str);
    }
}

// ══════════════════════════════════════════════════════════════════════════
/// 返回活跃会话 id 的 JSON 数组。
///
/// 注：由 `NativeBridgeSmokeTest` 覆盖（会话枚举的 JVM 边界）；生产 Kotlin 侧
/// 在 `TerminalRuntime.sessionIds` 中跟踪 id。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listSessions<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        list_sessions_inner(env, _class)
    })
}

fn list_sessions_inner<'local>(env: &mut Env<'local>, _class: JClass<'local>) -> jstring {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();

    let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    match env.new_string(&json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 字体管线默认的图集单元尺寸（像素）。
const DEFAULT_FONT_CELL_SIZE: f32 = 14.0;

/// 管线初始化、render_cell_data 与字体管线共用的图集尺寸，集中一处：改动需三处
/// 调用点一致，否则字形 UV 会错位。
///
/// 取 2048²（原为 1024²）：真机约 3x 显示密度下 14sp 字形光栅化后约 40px，
/// 原图集只能容纳约 700 个字形——几百个不同字符的滚动日志屏会持续冲刷 LRU，
/// 逐帧淘汰并重新光栅化同一批字形（CPU 密集）。2048² 使容量翻两番（约 2800 个字形），
/// 且内存代价有界（设备 16MB + 暂存位图 16MB）。
const ATLAS_SIZE: u32 = 2048;

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：TerminalQueryPort（搜索/回滚/文本/字体）
//
// Kotlin TerminalQueryPort 接缝的原生侧。每个导出都遵循 resize_inner 的模式：
// 注册表读锁 → 会话锁 → 引擎查询；会话 id 未知时抛 IllegalArgumentException。
// 字体查询不需要会话 id，直接读全局 RENDER_STATE。
// ══════════════════════════════════════════════════════════════════════════

/// 返回某会话的终端标题（OSC 0/2），无则 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTitle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("getTitle: session not found"),
            );
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let title = session.terminal().title();
        drop(session);
        drop(registry);
        match env.new_string(&title) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

// ── 文本与滚动查询 ──────────────────────────────────────────────
/// 返回某会话的回滚行数。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLength(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("scrollbackLength: session not found"),
            );
            return Ok(0);
        };
        let session = entry.session.lock();
        session.terminal().scrollback_length() as jint
    })
}

/// 返回终端光标的视口位置，打包为 `(y << 32) | x`；光标隐藏或在视口外时返回 -1。
///
/// 可观测性：经 `build_cell_data` 读取——即渲染线程消费的**同一**数据源，使埋点层
/// 看到的坐标与 GPU 绘制的完全一致。取值为从 0 开始的视口行列。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCursorViewportPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, -1, |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("getCursorViewportPacked: session not found"),
            );
            return Ok(-1);
        };
        let session = entry.session.lock();
        let terminal = session.terminal();
        let Some((row, col)) = terminal.render_cursor() else {
            return Ok(-1);
        };
        ((row as jlong) << 32) | (col as jlong)
    })
}

/// 返回单个回滚行去除首尾空白后的文本，空行返回 null。
///
/// **惰性访问语义**——Ghostty 只保留被显式读取过的行，多数下标返回 null。
/// 不可用于全文迭代——应改用 `dump_grid`（经 `getTerminalText`）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLine<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("scrollbackLine: session not found"),
            );
            return Ok(std::ptr::null_mut());
        };
        let Ok(row) = u32::try_from(row) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("scrollbackLine: row must be non-negative"),
            );
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        // Kotlin 传入的是绝对行号（回滚 + 视口偏移，经
        // `scrollbackLength - scrollOffset + row` 计算），故直接透传给期望绝对行号的
        // `read_line_text`。
        let text = session.terminal().read_line_text(row);
        drop(session);
        drop(registry);
        match text {
            Some(text) => match env.new_string(&text) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    })
}

/// 返回可见区与回滚区以换行拼接的文本（`dump_grid` 路径）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTerminalText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("getTerminalText: session not found"),
            );
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let grid = session.terminal().dump_grid();
        drop(session);
        drop(registry);

        let mut lines: Vec<String> = Vec::with_capacity(grid.scrollback.len() + grid.rows as usize);
        for row_cells in &grid.scrollback {
            let line: String = row_cells
                .iter()
                .filter_map(|c| char::from_u32(c.codepoint).filter(|ch| *ch != '\0'))
                .collect();
            lines.push(line.trim_end().to_string());
        }
        for row in 0..grid.rows as usize {
            let start = row * grid.cols as usize;
            let end = start
                .saturating_add(grid.cols as usize)
                .min(grid.visible.len());
            if start >= end {
                continue;
            }
            let line: String = grid.visible[start..end]
                .iter()
                .filter_map(|c| char::from_u32(c.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string();
            lines.push(line);
        }
        let text = lines.join("\n");
        match env.new_string(&text) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 用 Ghostty 原生格式化器提取选中文本（换行感知、宽字符安全，语义同
/// termux 的 `TerminalBuffer.getSelectedText`）。
/// `startRow`/`startCol`/`endRow`/`endCol` 为网格行列（绝对：第 0 行是回滚顶部，
/// 与 `scrollbackLine` 一致）。出错时返回 ""。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectionText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let text = session.terminal().selection_text(
            (start_row.max(0) as u32, start_col.max(0) as u32),
            (end_row.max(0) as u32, end_col.max(0) as u32),
        );
        drop(session);
        drop(registry);
        match env.new_string(&text) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 有序选区界限（绝对网格坐标，0 = 回滚顶部）→ JNI `IntArray
/// [startRow, startCol, endRow, endCol]`；`None`（无可选内容）→ null。
fn bounds_to_int_array(env: &mut Env, bounds: Option<((u32, u32), (u32, u32))>) -> jintArray {
    let Some(((start_row, start_col), (end_row, end_col))) = bounds else {
        return std::ptr::null_mut();
    };
    let values = [
        start_row.min(jint::MAX as u32) as jint,
        start_col.min(jint::MAX as u32) as jint,
        end_row.min(jint::MAX as u32) as jint,
        end_col.min(jint::MAX as u32) as jint,
    ];
    let Ok(array) = env.new_int_array(values.len()) else {
        return std::ptr::null_mut();
    };
    if array.set_region(env, 0, &values).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

/// 以落点为锚在终端安装选区并回传界限。会话锁与注册表读锁都在构造界限
/// 后立即释放：选区查询走 VT 线程，绝不能持锁跨越。
fn select_bounds_export<'local>(
    mut unowned_env: EnvUnowned<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
    select: impl FnOnce(&GhosttyTerminal, u32, u32) -> Option<((u32, u32), (u32, u32))>,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let bounds = select(session.terminal(), row.max(0) as u32, col.max(0) as u32);
        drop(session);
        drop(registry);
        bounds_to_int_array(env, bounds)
    })
}

/// 上游 select_word：以落点为锚派生词选区并安装到终端（ghostty 默认词
/// 边界），回传 `[startRow, startCol, endRow, endCol]`（绝对网格坐标）；
/// 落点无可选词或查询失败返回 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectWordAt<'local>(
    unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jintArray {
    select_bounds_export(unowned_env, session_id, row, col, |terminal, row, col| {
        terminal.select_word_at(row, col)
    })
}

/// 上游 select_line：落点所在整行派生并安装（语义提示边界关），回传与
/// 失败语义同 selectWordAt。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectLineAt<'local>(
    unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jintArray {
    select_bounds_export(unowned_env, session_id, row, col, |terminal, row, col| {
        terminal.select_line_at(row, col)
    })
}

/// 上游 select_all：全部内容（回滚 + 视口，界限不含尾部空行/空列）派生
/// 并安装，回传与失败语义同 selectWordAt。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectAll<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let bounds = session.terminal().select_all();
        drop(session);
        drop(registry);
        bounds_to_int_array(env, bounds)
    })
}

/// 查询网格单元处的 OSC 8 超链接 URI（第 0 行 = 回滚顶部，与 `scrollbackLine` 一致），
/// 无链接时返回 null。纯文本裸 URL 不做识别：libghostty-vt 只提供 OSC 8 超链接。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_hyperlinkAt<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let url = session
            .terminal()
            .hyperlink_at(row.max(0) as u32, col.max(0) as u32);
        drop(session);
        drop(registry);
        match url {
            Some(url) => match env.new_string(&url) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    })
}

// ── 搜索与选择 ──────────────────────────────────────────────────
/// 返回 `{row,start_col,end_col}` 搜索匹配的 JSON 数组，超时/断连时返回 `[]`。
/// 列下标为字符列。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchAllInScrollback<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    query: JString<'local>,
    case_sensitive: jboolean,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let Ok(query) = query.try_to_string(env) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("searchAllInScrollback: bad query string"),
            );
            return Ok(std::ptr::null_mut());
        };
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("searchAllInScrollback: session not found"),
            );
            return Ok(std::ptr::null_mut());
        };
        let session = entry.session.lock();
        let matches = session
            .terminal()
            .search_all_in_scrollback(&query, case_sensitive);
        log::info!(
            "searchAllInScrollback: query={query:?} matches={}",
            matches.len(),
        );
        drop(session);
        drop(registry);

        let json = serde_json::to_string(
            &matches
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "row": m.row,
                        "start_col": m.start_col,
                        "end_col": m.end_col,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".into());
        match env.new_string(&json) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 该单元没有可打印码点时返回 true。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_isCellEmpty(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_TRUE, |env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("isCellEmpty: session not found"),
            );
            return Ok(JNI_TRUE);
        };
        let Ok(row) = u32::try_from(row) else {
            return Ok(JNI_TRUE);
        };
        let session = entry.session.lock();
        // Kotlin 传入的 `gridRow` 已是绝对行号（回滚 + 可视偏移 - 滚动偏移）。
        // **不要**再加一次回滚长度——那会重复计数。
        let absolute = row as usize;
        let visible_rows = session.terminal().rows();
        let scrollback = session.terminal().scrollback_length();
        let mut empty = true;
        if (absolute as u32) < visible_rows + scrollback {
            if let Some(line) = session.terminal().read_line_text(row) {
                // `col` 是**字符**列，而原始行是 UTF-8——拿 `line.len()`（字节数）比较会
                // 把多字节单元（CJK/emoji）误判为空。此处改为统计码点。
                let char_col = col.max(0) as usize;
                let char_len = line.chars().count();
                log::debug!(
                    "isCellEmpty({row},{col}): scrollback={scrollback} rows={visible_rows} absolute={absolute} line={line:?} char_len={char_len}"
                );
                empty = char_col >= char_len;
            } else {
                log::debug!(
                    "isCellEmpty({row},{col}): read_line_text({absolute}) returned None (scrollback={scrollback})"
                );
            }
        } else {
            log::debug!(
                "isCellEmpty({row},{col}): absolute={absolute} out of range rows+scrollback={}",
                visible_rows + scrollback
            );
        }
        if empty { JNI_TRUE } else { JNI_FALSE }
    })
}

// ── 字体与主题 ──────────────────────────────────────────────────
/// 返回字体库的族名列表（fonts.xml 声明的文件集 + 用户投放目录）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listFontFamilies<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(std::ptr::null_mut());
        };
        let families = render_state.font_pipeline.list_monospace_fonts();
        drop(state);

        let string_class = env.find_class(jni_str!("java/lang/String"));
        let Ok(string_class) = string_class else {
            return Ok(std::ptr::null_mut());
        };
        let array = env.new_object_array(families.len() as jsize, string_class, JObject::null());
        let Ok(array) = array else {
            return Ok(std::ptr::null_mut());
        };
        for (index, family) in families.iter().enumerate() {
            if let Ok(family) = env.new_string(family) {
                let _ = array.set_element(env, index, &family);
            }
        }
        array.into_raw()
    })
}

/// 返回默认字体族名。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getDefaultFontName<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(std::ptr::null_mut());
        };
        let name = render_state.font_pipeline.default_font_name();
        match env.new_string(&name) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 返回字体信息字符串（当前字体 + CJK 回退）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getFontInfo<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(std::ptr::null_mut());
        };
        let info = render_state.font_pipeline.font_info();
        drop(state);
        match serde_json::to_string(&info) {
            Ok(json) => match env.new_string(&json) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 清除渲染器下一帧的搜索高亮区间。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clearSearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights.clear();
            // 置脏标志：高亮清除必须送达屏幕，即便终端处于空闲
            // （否则陈旧高亮会一直残留到下次 PTY 输出——正是 #5 这类回归）。
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 设置搜索高亮区间。`data` 按字节打包：先是 4 字节的匹配数（i32 LE，权威长度），
/// 随后是同样数量的 16 字节记录：row(i32) start(i32) end(i32) RGBA(u8x4)。
/// Kotlin 的 `TerminalSurface` 按此格式打包并调用
/// `bridge.setSearchHighlights(data.copyOf())`；渲染循环在下一帧消费解析出的列表。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`data` 须为本次调用期间有效的 `jbyteArray`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
        // `from_raw` 只包装指针而不取得所有权；局部引用在本 native 方法返回时释放
        // （与 `feed_pty_inner` 同一模式）。
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
        let Some(bytes) = env.convert_byte_array(&byte_array).ok() else {
            return Ok(());
        };
        // 线格式（对应 Kotlin `TerminalScreen` 的搜索结果打包）：
        //   [0..4]   匹配数 (i32 LE)——该计数前缀**就是**权威长度；尾部残留字节
        //            （最后一条不完整的 16 字节记录）防御性地忽略。
        //   [4..]    16 字节记录：row(i32) start(i32) end(i32) RGBA(u8x4)
        let Some(prefix) = bytes.get(0..4) else {
            return Ok(());
        };
        let count =
            i32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]).max(0) as usize;
        // 把声称的匹配数封顶为实际存在的完整记录数：损坏/巨大的计数不得导致
        // 无界的 `Vec::with_capacity` 分配。
        let count = count.min(bytes.len().saturating_sub(4) / 16);
        let mut highlights = Vec::with_capacity(count);
        let payload = &bytes[4..];
        let mut offset = 0;
        for _ in 0..count {
            if offset + 16 > payload.len() {
                break;
            }
            let row = i32::from_le_bytes(
                payload[offset..offset + 4]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let start_col = i32::from_le_bytes(
                payload[offset + 4..offset + 8]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let end_col_exclusive = i32::from_le_bytes(
                payload[offset + 8..offset + 12]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let color = [
                payload[offset + 12],
                payload[offset + 13],
                payload[offset + 14],
                payload[offset + 15],
            ];
            highlights.push(crate::render::cell_builder::SearchHighlight {
                row,
                start_col,
                end_col_exclusive,
                color,
            });
            offset += 16;
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights = highlights;
            // 置脏标志：无 PTY 流量时新高亮也必须绘制到下一帧
            //（#5 空闲屏回归）。
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    });
}

/// 设置下一渲染帧的活动文本选区。
///
/// 终端持有选区安装口：坐标为绝对网格行（row 0 = 回滚顶部），与控制柄逻辑一致。
/// 选区经 `GhosttyTerminal::set_selection` 安装为终端状态（跟踪引用，随滚动/
/// 输出/重排跟随文本），高亮由 VT 线程按行级选区反白直接烘焙进 CellData；
/// 本层不再存储单元格位置，仅透传。`hasSelection=false` 清除选区。
/// 选区恒为线性（块选通道已随 mode 一并移除）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSelection(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
    has_selection: jboolean,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(());
        };
        let session = entry.session.lock();
        if has_selection == jni::sys::JNI_TRUE {
            session.terminal().set_selection(
                (start_row.max(0) as u32, start_col.max(0) as u32),
                (end_row.max(0) as u32, end_col.max(0) as u32),
            );
        } else {
            session.terminal().clear_selection();
        }
        drop(session);
        drop(registry);
        // 置脏标志：选区变化必须重绘，即便终端空闲（VT 线程也会重推 CellData，
        // 此处只是唤醒渲染门控而不必等待它）。
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    });
}

/// 应用主题：54 字节 = 背景 RGB(3) + 前景 RGB(3) + 16 个 ANSI 调色板色(48)。
/// 对应 `GhosttyTerminal::set_theme`。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`data` 须为本次调用期间有效的 `jbyteArray`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setTheme(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let id = session_id as u64;
        // SAFETY: `data` 是本导出被调用前 JVM 已校验的 JNI `jbyteArray` 参数；
        // jni crate 的 `from_raw` 只包装指针，而 `convert_byte_array` 会按数组实际
        // 长度做边界检查。
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
        let bytes = match env.convert_byte_array(&byte_array) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = env.throw_new(
                    jni_str!("java/lang/IllegalArgumentException"),
                    jni_str!("setTheme: cannot read byte array"),
                );
                return Ok(());
            }
        };
        if bytes.len() != 54 {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("setTheme: expected exactly 54 bytes (background3 foreground3 ansi48)"),
            );
            return Ok(());
        }
        let background = [bytes[0], bytes[1], bytes[2]];
        let foreground = [bytes[3], bytes[4], bytes[5]];
        let mut ansi = [[0u8; 3]; 16];
        for (i, color) in ansi.iter_mut().enumerate() {
            let base = 6 + i * 3;
            *color = [bytes[base], bytes[base + 1], bytes[base + 2]];
        }
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("setTheme: session not found"),
            );
            return Ok(());
        };
        let session = entry.session.lock();
        session.terminal().set_theme(background, foreground, ansi);
        // 单元格着色器的 Fix F 判定会把每个单元格的背景与 `uniforms.default_background`
        // 比对，后者取自 `Renderer::background`。不同步时终端主题与渲染器默认值不一致，
        // `is_default_background` 恒为 false，背景色判定失效。
        {
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                render_state.renderer.set_background_color(background);
                // 主题色存于实例缓存，闲时无新 CellData 则颜色过期，一并重绘。
                render_state.renderer.cell_cache = None;
                render_state.dirty.store(true, Ordering::Relaxed);
            }
        }
        log::info!(
            "setTheme: session {id} background={background:02X?} foreground={foreground:02X?}"
        );
    })
}

/// 暂停/恢复渲染器（如设置界面打开期间或 surface 已销毁时）。Rust 渲染器已在
/// `render_frame` 中检查 `render_paused`；本 JNI 导出补上缺失的传输通道。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRenderPaused(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    paused: jboolean,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.renderer.set_render_paused(paused);
            // 恢复时强制全量重绘：暂停期消费的 New 帧永不呈现
            // （`render_frame` 直接返回），且 `last_frame` 已是新数据。
            // 只置 `dirty=true` 会走 Idle 稀疏路径（光标行 + 高亮行），暂停期写入的
            // 内容行不在 bands 内、被 Load 残留覆盖——“隐藏后仍不可见、滑动后部分
            // 出现”。故走 `frame_invalidated` 全量路径。
            if !paused {
                render_state.renderer.frame_invalidated = true;
            }
            log::info!("setRenderPaused: paused={}", paused);
        }
    })
}

/// 应用层光标色覆盖（`r` | `g` | `b`，0..1 线性 RGB）。叠加在主题光标色之上，
/// 使用户主题的光标色能抵达渲染器（54 字节的 `setTheme` 载荷没有对应槽位）。
/// `None` 清除覆盖（跟随终端）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorColor(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    r: f32,
    g: f32,
    b: f32,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.cursor_color =
                Some([r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0]);
            log::info!("setCursorColor: ({r}, {g}, {b})");
        }
    })
}

/// 设置渲染器字体管线的字体族；找到该族时返回 true。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamily(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    family: JString,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
        let family_str = match family.try_to_string(env) {
            Ok(s) => s,
            Err(_) => return Ok(JNI_FALSE),
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(JNI_FALSE);
        };
        let found = render_state.font_pipeline.set_font_family(&family_str);
        // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，必须整库丢弃并重绘。
        render_state.renderer.cell_cache = None;
        render_state.dirty.store(true, Ordering::Relaxed);
        log::info!("setFontFamily: {family_str} found={found}");
        if found { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 设置字号（单位为十分之一像素，与 Kotlin 滑块一致）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontSizeInPlace(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    size_tenths: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let size = (size_tenths as f32) / 10.0;
        if !(4.0..=100.0).contains(&size) {
            return Ok(());
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // 同值跳过：手势 preview 高频推送同一字号时不清图集，
            // 避免每帧重光栅 ASCII + 丢实例缓存（缩放撕裂/卡顿）。
            if (render_state.font_pipeline.font_size - size).abs() < f32::EPSILON {
                return Ok(());
            }
            let (cw, ch) = render_state.font_pipeline.set_font_size_in_place(size);
            // 置脏标志：字号变化必须重绘，即便终端空闲（字形度量已变 → 缓存帧过期）。
            // 同时丢弃实例缓存：干净行存的是旧图集 UV 与旧行高，同网格尺寸仍判兼容，
            // 不丢则丢字/错位/行高混杂（压扁·撕裂）。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
            log::info!(
                "setFontSizeInPlace: {} -> cell {cw:.1}x{ch:.1}",
                size_tenths
            );
        }
    })
}

/// 设置字体光栅化缩放（设备像素密度）。字形位图按 `font_size * raster_scale`
/// 光栅化，使高密度屏上的文字保持锐利。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRasterScale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    scale: jfloat,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        if !(0.5..=8.0).contains(&scale) {
            return Ok(());
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // **两条**管线必须在 `raster_scale` 上一致——字体管线按
            // `font_size*scale` 光栅化字形位图，渲染器则把同一缩放送入单元着色器的
            // uniform。失步时着色器按 1x 采样位图而 `cell_builder` 按 Nx 计算 bearing，
            // 字形会被画成畸变的角落三角形。
            render_state.font_pipeline.set_raster_scale(scale);
            render_state.renderer.set_raster_scale(scale);
            // 位图全部重光栅：旧 UV 指向错误图块，丢缓存并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 把自定义字体文件载入渲染器的字体库。返回文件中的首个字体族名，失败时 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_loadFontFile<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    path: JString<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let path_str = match path.try_to_string(env) {
            Ok(s) => s,
            Err(_) => return Ok(std::ptr::null_mut()),
        };
        // 自定义字体加载是 Android 专属特性（带额外路径的字体库仅 Android 有；
        // 桌面构建使用系统字体）。其他目标上拒绝加载。
        #[cfg(target_os = "android")]
        let family = {
            // 用全新的 fontdb 探测文件以获知字体族名。
            let mut db = crate::render::font::font_db::load_font_database();
            let ids =
                db.load_font_source(fontdb::Source::File(std::path::PathBuf::from(&path_str)));
            let Some(family) = ids
                .iter()
                .filter_map(|id| db.face(*id))
                .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
                .next()
            else {
                log::warn!("loadFontFile: no family name in {path_str}");
                return Ok(std::ptr::null_mut());
            };
            // 把文件登记到渲染器并重建其管线，使新字体族可被选中。
            // 追加而非覆盖：用户字体目录已在此前注册，覆盖会丢掉目录内字体。
            crate::render::font::font_db::add_extra_font_path(std::path::PathBuf::from(&path_str));
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
                let font_size = render_state.font_pipeline.font_size();
                let (aw, ah) = (aw as i32, ah as i32);
                render_state.font_pipeline =
                    crate::render::font::FontPipeline::new(aw, ah, font_size);
                let _ = render_state.font_pipeline.set_font_family(&family);
                // 管线整体替换：旧实例 UV 全部失效。
                render_state.renderer.cell_cache = None;
                render_state.dirty.store(true, Ordering::Relaxed);
            }
            family
        };
        #[cfg(not(target_os = "android"))]
        let family: String = {
            log::warn!("loadFontFile: unsupported on this target");
            String::new()
        };
        if family.is_empty() {
            return Ok(std::ptr::null_mut());
        }
        log::info!("loadFontFile: {} -> family {family}", path_str);
        match env.new_string(&family) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// 设置系统 locale（BCP 47，如 `zh-CN`）：决定 fonts.xml 里选哪个区域回退族。
///
/// 进程启动时 Kotlin 就会调一次（此时还没有渲染管线），故 locale 同时写入渲染层
/// 进程级静态供 `load_font_database` 取用；管线已存在时
/// 再推给它并作废同 UV 缓存（回退族变化会改变字形来源）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSystemLocale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    locale: JString,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let locale_str = match locale.try_to_string(env) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        log::info!("setSystemLocale: {locale_str}");
        // 管线创建前的区域决策读渲染层静态（与 setExtraFontPaths 同一门控惯例）。
        #[cfg(target_os = "android")]
        crate::render::font::font_db::set_current_locale(locale_str.clone());
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.font_pipeline.set_system_locale(&locale_str);
            // CJK 回退排序变化会改变字形来源，同 UV 缓存过期。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 登记额外的字体目录/文件（应用私有字体目录）。渲染器管线在创建时读取这些路径；
/// 若已存在则重建，使新字体可被选中。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`paths` 须为本次调用期间有效的 `String[]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setExtraFontPaths(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    paths: jobjectArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        #[cfg(target_os = "android")]
        {
            // 从 Java 的 `String[]` 解析。
            let mut path_list: Vec<std::path::PathBuf> = Vec::new();
            // SAFETY: `paths` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效
            // （与 `feed_pty_inner` 同一模式）。
            let array = unsafe { jni::objects::JObjectArray::<JString>::from_raw(env, paths) };
            let len = array.len(env).unwrap_or(0);
            for i in 0..len {
                if let Ok(item) = array.get_element(env, i) {
                    if let Ok(text) = item.try_to_string(env) {
                        path_list.push(std::path::PathBuf::from(text));
                    }
                }
            }
            crate::render::font::font_db::set_extra_font_paths(path_list);
            // 若管线已存在则重建，使新字体立即可选。
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
                let font_size = render_state.font_pipeline.font_size();
                let (aw, ah) = (aw as i32, ah as i32);
                render_state.font_pipeline =
                    crate::render::font::FontPipeline::new(aw, ah, font_size);
            }
            log::info!("setExtraFontPaths: registered extra font paths");
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (env, paths);
            log::warn!("setExtraFontPaths: unsupported on this target");
        }
    })
}

// ── 网格尺寸查询 ────────────────────────────────────────────────
/// 当前单元格宽度（像素，取自渲染器的字体管线）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellWidth(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) -> jfloat {
    jni_export_guard!(&mut unowned_env, 0.0, |_env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(0.0);
        };
        render_state.font_pipeline.cell_metrics().0
    })
}

/// 当前单元格高度（像素，取自渲染器的字体管线）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellHeight(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) -> jfloat {
    jni_export_guard!(&mut unowned_env, 0.0, |_env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(0.0);
        };
        render_state.font_pipeline.cell_metrics().1
    })
}

/// 当前网格尺寸，打包为 `(rows << 32) | cols`；会话未知时为 0。
/// 供 `Bridge.getGridRowsColsPacked` 使用：Kotlin 桩实现原本永远返回 0，
/// 导致 resize 被丢弃后 `syncGridDimensions` 永远无法收敛到原生网格。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getGridRowsColsPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(_session_id as u64)) else {
            return Ok(0);
        };
        let session = entry.session.lock();
        let (rows, cols) = session.grid_size();
        ((rows as i64) << 32) | (cols as i64)
    })
}

// ── 滚动、模式与状态查询 ────────────────────────────────────────
/// 设置视口滚动偏移（回滚区内的行数，0 = 活跃屏幕）。与上次偏移的差值在 VT 线程
/// 经 `scroll_viewport(Delta)` 应用，故下一次 CellData 推送即携带滚动后的视图
/// （此前在 Kotlin 侧是空操作）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollOffset(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    offset: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let target = offset.max(0) as i64;
        let sent = {
            let mut registry = wlock_session_registry();
            let Some(entry) = registry.get_mut(&(_session_id as u64)) else {
                return Ok(());
            };
            // 每会话独立计算增量：此前代码按单一全局 `render_state.scroll_offset`
            // 计算，切换会话会污染恢复后会话的视口。
            let delta = target - entry.last_scroll_offset;
            if delta == 0 {
                return Ok(());
            }
            let session = entry.session.lock();
            // `scroll_viewport` 的增量语义（主机与模拟器已验证）：负值 = 向上滚入历史，
            // 正值 = 回到底部。Kotlin 的 `scrollOffset` 在用户向上滑（进入历史）时增大，
            // 故此处必须取反：此前符号写反，向下滑到底部时发的是负增量，
            // 结果反而滚进了历史。
            let sent = session.terminal().scroll_viewport(-(delta as isize));
            if sent {
                entry.last_scroll_offset = target;
                log::debug!("setScrollOffset: target={target} delta={delta}");
            } else {
                // 命令通道已满或 VT 线程已消失：**不**推进 `last_scroll_offset`，
                // 使下次以相同目标调用时能重试该增量而非静默丢弃。此前先推进全局
                // 偏移，发送失败后视口会永久停留在陈旧状态。
                log::warn!(
                    "setScrollOffset: scroll_viewport send failed (target={target} delta={delta}); will retry"
                );
            }
            sent
        };
        // 行级滚动必须立即重绘：VT 线程推送新 CellData 是异步的，滑动期间无 New 帧时
        // Idle 门控（highlights / scroll_px / dirty / invalidated）不含行偏移，
        // 会吞掉本次滑动直到下一次输出或点击才刷出来（“滑动不动、点一下才出”）。
        if sent {
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                render_state.dirty.store(true, Ordering::Relaxed);
            }
        }
    })
}

/// 设置逐像素平滑滚动用的视口 Y 像素偏移（正值 = 内容下移，与 Kotlin 手势余数
/// 符号一致）。立即套用到共享的单元 uniforms，使下一呈现帧携带该偏移；
/// 渲染线程的空闲门控也经 `last_scroll_px` 观察到变化。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollYPx(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    offset_px: jfloat,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        if !offset_px.is_finite() {
            return Ok(());
        }
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        render_state.renderer.set_viewport_scroll_px(offset_px);
        let (width, height) = (
            render_state.renderer.projection_width as f32,
            render_state.renderer.projection_height as f32,
        );
        render_state.renderer.refresh_cell_uniforms(width, height);
        log::debug!("setScrollYPx: {offset_px}");
    })
}

/// 远端当前是否处于备用屏幕缓冲（vim / less / htop）。无锁读取 VT 线程在每次
/// `Query::AltScreen` 查询时维护的镜像——可在 Android 输入路径的每次触摸滚动事件
/// 中调用而不阻塞 UI 线程。
///
/// 供 `Bridge.isAltScreenActive` 使用，使备用屏上的触摸滚动能作为滚轮转义序列
/// 转发给远端，而非滚动本地回滚（备用屏会吞掉滚轮）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getAltScreenState(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |_env| {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return Ok(JNI_FALSE);
        };
        let session = entry.session.lock();
        if session.terminal().alt_screen_active_atomic() {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    })
}

/// 查询终端模式（ghostty `mode_get`）。`kind` 选择模式命名空间：0 = DEC 私有模式，
/// 非 0 = ANSI 模式。供 Kotlin 键编码器查询 DECCKM（应用光标键，DEC 私有模式 1），
/// 以在方向键的 SS3（`ESC OA`）与 CSI（`ESC [ A`）之间切换。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getMode(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    mode_num: jint,
    kind: jint,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |_env| {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return Ok(JNI_FALSE);
        };
        let session = entry.session.lock();
        if session
            .terminal()
            .mode_get(mode_num.max(0) as u16, kind.max(0) as u8)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    })
}
