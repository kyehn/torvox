//! JNI FFI 桥接层：导出 `extern "system"` 函数供 Kotlin 调用，命名遵循
//! `Java_terminal_emulator_bridge_NativeBridge_<方法名>`。会话经 `initSession` 注册、
//! `destroySession` 注销、`switchSession` 切换；事件推入全局队列由 `pollEvent` 排空。
//!
//! 线程模型：各导出的调用线程随触发源而异（生命周期与焦点多在 `Dispatchers.IO`，
//! 硬件按键在 UI 线程，渲染线程也会发起会话拆卸、切换与焦点重发）。
//! 无界等待只有两类：`initSession` 的 fork/execve（不在锁内调用），以及 GPU 初始化
//! （`render` 首次调用、`attachWindow` 与 `prefetchRenderState` 共用同一条路径，
//! GPU 故障时可挂起；其中 `attachWindow` 在 `sessionLock` 内调用，
//! 与该处锁内 join 同一取舍）。
//! 其余等待皆有限额（网格命令 `try_send`、查询 RPC 有界超时、PTY 排空限时，
//! 线程 join 限时，取纹理另有时限）。`pollEvent` 由活动会话的渲染线程按帧率调用。
//!
//! 并发：`SESSION_REGISTRY` 是 `RwLock`（读多于写），`EVENT_QUEUE` 是 `Mutex`，
//! `ACTIVE_SESSION_ID` 是 `AtomicU64`（`Acquire`/`Release`，0 = 无活跃会话）。
//! 锁顺序 `SESSION_REGISTRY` → `Session` → `exit_code`；`EVENT_QUEUE` 独立加锁，
//! 绝不在持有 `Session` 锁时获取。

use parking_lot::{Condvar, Mutex, RwLock};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::event::Event;
use jni::objects::{JClass, JString};
use jni::objects::{JLongArray, JObject};
use jni::strings::JNIString;
use jni::sys::{
    JNI_FALSE, JNI_TRUE, jboolean, jbyteArray, jfloat, jfloatArray, jint, jintArray, jlong,
    jlongArray, jobjectArray, jsize, jstring,
};
use jni::{Env, EnvUnowned, jni_str};

use super::text_utils::encode_meta_prefix;
use crate::render::cell_builder::SearchHighlight;
use crate::render::cell_builder::{SEARCH_HIGHLIGHT_CURRENT_ALPHA, SEARCH_HIGHLIGHT_OTHER_ALPHA};
use crate::terminal::ShellEnv;
use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::session::Session;
use crate::terminal::theme;
use libghostty_vt::key;
use std::sync::Arc;

/// 捕获逸出 JNI 导出函数体的 panic。panic 越过 `extern "system"` 边界属未定义行为
/// （进程 abort）——所有会话瞬间死亡且崩溃处理器不运行。本守卫把 panic 与 Rust 错误
/// 转为 Java RuntimeException，并向 Kotlin 返回**调用点声明**的失败值。
macro_rules! jni_export_guard {
    ($unowned:expr, $failure:expr, |$env_param:ident| $call:expr) => {{
        $unowned
            .with_env(|$env_param| -> jni::errors::Result<_> { Ok($call) })
            .resolve_with::<ThrowRuntimeExAndFailure<_>, _>(|| $failure)
    }};
}

/// 把错误与 panic 转为 Java `RuntimeException` 并返回调用点声明的失败值。
///
/// 与 `jni::errors::ThrowRuntimeExAndDefault` 的唯一差别是缺省值来源：后者恒返回
/// `T::default()`。若守卫照搬它，渲染导出声明的失败值 `-1` 会变成 `0`，而 `0` 在
/// Kotlin 侧意为「空闲帧」——panic 就此被伪装成一帧无输出，渲染线程据此继续空转。
struct ThrowRuntimeExAndFailure<T>(std::marker::PhantomData<T>);

impl<T: Copy + Default, E: std::error::Error> jni::errors::ErrorPolicy<T, E>
    for ThrowRuntimeExAndFailure<T>
{
    type Captures<'local: 'native_method, 'native_method> = T;

    fn on_error<'local: 'native_method, 'native_method>(
        env: &mut Env<'local>,
        failure: &mut T,
        error: E,
    ) -> jni::errors::Result<T> {
        if !env.exception_check() {
            let _ = env.throw(format!("Rust error: {error}"));
        }
        Ok(*failure)
    }

    fn on_panic<'local: 'native_method, 'native_method>(
        env: &mut Env<'local>,
        failure: &mut T,
        payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        if !env.exception_check() {
            let _ = env.throw(format!("Rust panic: {}", take_panic_text_and_leak(payload)));
        } else {
            // 已有 Java 异常在飞，载荷只需处置掉：仍走同一条「不销毁」路径。
            take_panic_text_and_leak(payload);
        }
        Ok(*failure)
    }
}

/// 取 panic 载荷的可读文本，并**故意泄漏**该载荷本身。
///
/// 本函数跑在**解卷路径**上：此刻已经有一个 panic 在途。载荷的 `Box` 若在此按值
/// 销毁、而载荷元素的 `Drop` 又 panic，就是「解卷中解卷」——Rust 不会尝试第二次
/// 解卷，而是直接 `abort`。这正是 `jni_export_guard!` 唯一还能漏出 abort 的机制：
/// 守卫接住了首个 panic，却在**处理**它时崩掉，调用方连 Java 异常都拿不到。
/// 真机日志里 `prefetchRenderState` 的 SIGABRT 即属此类（栈顶是 `abort`，而不是
/// `rust_panic` 的展开路径）。
///
/// 故按「崩溃路径上宁可漏几字节」取舍：`mem::forget` 掉整个 `Box`。panic 载荷
/// 通常是 `&'static str` 或 `String`，泄漏量可忽略；而把它误降级成整进程 abort 的
/// 代价比高得多——那会丢掉本已备好的诊断信息，且不可恢复。
///
/// 刻意做成独立函数而非内联进 `on_panic`：`on_panic` 需要活的 `&mut Env`（只有设备上
/// 才有），抽出来才能在宿主单测里钉住「绝不销毁」这一条性质。
fn take_panic_text_and_leak(payload: Box<dyn std::any::Any + Send + 'static>) -> String {
    // 必须写 `as_ref()`（即 `&*payload`），不能写 `&payload`。
    //
    // 后者靠 deref coercion 把 `&Box<dyn Any + Send>` 转成 `&(dyn Any + Send)`，
    // 转出来的 trait object 丢掉了具体类型：实测 `&payload` 调 `downcast_ref` 恒为
    // `None`，而 `&*payload` 正常命中。后果是**每一个真实 panic 的文本都被报成
    // "non-string panic payload"**——守卫明明接住了 panic，抛给 Java 的诊断却是
    // 一句占位话，等于把最该看清的那条信息抹掉。此处曾经就是这样错的。
    let text = crate::panic_text(payload.as_ref());
    std::mem::forget(payload);
    text
}

/// 把 `jint` 收窄到无符号小类型，越界抛 `IllegalArgumentException` 并返回 `None`。
///
/// `as` 在此处是静默截断：鼠标按钮 65536 会落进合法的小按钮号，越界的模式号同理，
/// 与「该模式确实关闭 / 确实无按键」无法区分。仓库内行列与像素尺寸早已改用
/// `try_from` + 抛错（见 `initSession`、`setPixelSize`），此处补齐其余跨语言收窄点。
fn narrow_jint<T>(env: &mut Env, method: &str, field: &str, value: jint) -> Option<T>
where
    T: TryFrom<jint>,
{
    match T::try_from(value) {
        Ok(narrowed) => Some(narrowed),
        Err(_) => {
            let message =
                jni::strings::JNIString::from(format!("{method}: {field} out of range: {value}"));
            let _ = env.throw_new(jni_str!("java/lang/IllegalArgumentException"), &message);
            None
        }
    }
}

/// 会话不存在（注册表查无此 id）时抛出。
///
/// 与其余故障**必须**分用异常类型：Kotlin 侧 [`Bridge.onSession`] 只把本类型当作
/// 会话竞态（Activity 重建/进程回收，见 DESIGN）并回缺省值，其余异常照实上抛。
/// 二者共用一个 `RuntimeException` 时，「PTY 写入失败」「主题不存在」「尺寸越界」
/// 等真实故障会被一并说成「会话已销毁」而无声消失——日志里只剩一句错误的归因。
fn throw_session_missing(env: &mut Env, method: &str) {
    let message = jni::strings::JNIString::from(format!("{method}: session not found"));
    let _ = env.throw_new(jni_str!("java/lang/IllegalStateException"), &message);
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

/// 光标载体无值：首帧尚未被渲染线程取走，或该帧的光标不可见（隐藏、滚出视口）。
const RENDERED_CURSOR_NONE: i64 = -1;

/// 渲染帧回滚长度无值：首帧尚未被渲染线程取走。
const RENDERED_SCROLLBACK_NONE: i32 = -1;

/// 渲染循环的唤醒源：一个「代数 + 条件变量」。
///
/// 渲染线程没有自己的输入队列可等，旧实现让它在 Kotlin 侧 `LockSupport.parkNanos`
/// 定时醒来轮询输出：活跃时每 17ms 空转一次，空闲闭锁 500ms 则让**没有输入**的
/// 后台输出最坏等满半秒才上屏。把等待搬到原生条件变量后，PTY 读线程与 Kotlin 侧
/// 信号（vsync/输入/滚动）都只是把代数 +1 并 `notify_all`：输出到达即刻返回，
/// 空闲时一次唤醒都不发。
///
/// 代数而非裸信号量：等待方把「上次醒来时的代数」带进来，若期间已有 notify，
/// `wait` 立即返回而不丢失唤醒，也不需要额外的「先检查后睡眠」窗口。
#[derive(Default)]
struct RenderWake {
    generation: Mutex<u64>,
    condvar: Condvar,
}

impl RenderWake {
    /// 通知等待方：代数 +1 并唤醒。可从任意线程调用（读线程、UI 线程、渲染线程自身）。
    fn notify(&self) {
        let mut generation = self.generation.lock();
        *generation = generation.wrapping_add(1);
        drop(generation);
        self.condvar.notify_all();
    }

    /// 等到代数变化或超时，返回当前代数。`timeout` 为零表示不等待。
    fn wait(&self, last_generation: u64, timeout: Duration) -> u64 {
        let mut generation = self.generation.lock();
        if *generation == last_generation && !timeout.is_zero() {
            // 超时结果无需区分：返回的代数相同即「没等到东西」，调用方照旧落一帧。
            let _timeout_result = self.condvar.wait_for(&mut generation, timeout);
        }
        *generation
    }
}

struct SessionEntry {
    session: Arc<Mutex<Session>>,
    /// 本会话渲染循环的唤醒源。PTY 输出到达（读线程）与 Kotlin 侧信号共用它。
    render_wake: Arc<RenderWake>,
    /// 渲染线程每消费一帧新数据就以 `Release` 写入本会话**那一帧**的光标格
    /// （编码见 [`pack_rendered_cursor`]）。读方是输入法锚点上报：主线程、逐键发生，
    /// 只做一次 `Acquire` 读——不发 VT 查询、不取会话锁。会话锁可能被 PTY 写入按住，
    /// 而锚点必须在同一拍里给出坐标（旧路走同步查询，实测每次组字回退等 18ms p50）。
    rendered_cursor: AtomicI64,
    /// 同一帧的回滚长度（`CursorInfo.scrollback_length`）。滚动钳位与选区手柄
    /// 每帧都要用；改读此处后不再向 VT 线程发同步 RPC——VT 忙于输出时那次查询
    /// 最长阻塞 500ms，正是滚动/拖柄的卡顿来源。
    rendered_scrollback: AtomicI32,
    /// PTY 摄入标志的共享镜像。渲染线程每帧读清它；放在注册表而非会话里，
    /// 是因为取会话锁可能被 VT 解析按住数十毫秒。
    new_output: Arc<AtomicBool>,
    /// 备用屏激活的共享镜像，与 `new_output` 同批上报，读取同样不取会话锁。
    alt_screen_active: Arc<AtomicBool>,
}

impl SessionEntry {
    fn new(session: Arc<Mutex<Session>>, render_wake: Arc<RenderWake>) -> Self {
        let (new_output, alt_screen_active) = {
            let session = session.lock();
            (session.new_output_flag(), session.alt_screen_active_flag())
        };
        Self {
            session,
            render_wake,
            rendered_cursor: AtomicI64::new(RENDERED_CURSOR_NONE),
            rendered_scrollback: AtomicI32::new(RENDERED_SCROLLBACK_NONE),
            new_output,
            alt_screen_active,
        }
    }
}

/// 一帧的 `CursorInfo` → [`SessionEntry::rendered_cursor`] 的编码，格式同
/// `getCursorViewportPacked`：`(row << 32) | col`，不可见时为 [`RENDERED_CURSOR_NONE`]。
///
/// 不含第二次解读：`CursorInfo` 的行列本就由 `cursor_in_viewport` 产出（隐藏与
/// 滚出视口都表现为 `visible = false`），故这里只做打包。
fn pack_rendered_cursor(info: &crate::terminal::ghostty_terminal::CursorInfo) -> i64 {
    if info.visible {
        ((info.row as i64) << 32) | (info.col as i64)
    } else {
        RENDERED_CURSOR_NONE
    }
}

/// 读某会话最近的帧内光标；会话未登记时返回 `None`（调用方据以区分「未知会话」与
/// 「已知会话但当前无可见光标」）。
fn rendered_cursor_packed(session_id: u64) -> Option<i64> {
    let registry = rlock_session_registry();
    let entry = registry.get(&session_id)?;
    Some(entry.rendered_cursor.load(Ordering::Acquire))
}

/// 读某会话最近一帧的回滚长度；会话未登记时返回 `None`，首帧未发布时返回
/// [`RENDERED_SCROLLBACK_NONE`]。
fn rendered_scrollback(session_id: u64) -> Option<i32> {
    let registry = rlock_session_registry();
    let entry = registry.get(&session_id)?;
    Some(entry.rendered_scrollback.load(Ordering::Acquire))
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

/// Kitty 放置缓存键：任一项变化都意味着放置几何可能已变，须重查放置。
///
/// 抽成结构体而非散落字段，是为了让失效规则可单测钉住。其中「回滚长度」最易漏：
/// 新输出把行推入回滚时放置的视口行号随之变化，但用户滚动偏移仍为 0，
/// 漏掉这一项会让图像钉在旧行、被后续文本覆盖（用户实测现象）。
///
/// 「占位符签名」覆盖 Unicode 虚拟放置：图像位置由可见占位符单元决定，
/// 生成戳不变时 TUI 仍可能重绘并移动占位符。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct KittyPlacementKeys {
    session_id: u64,
    generation: u64,
    scroll_offset: i64,
    scrollback_length: u32,
    placeholder_signature: u64,
    rows: u32,
    cols: u32,
}

/// Kitty 缓存失效判定：把「哪些变化要做什么」集中到一处，避免散落的条件比较
/// 随字段增删而漏判。
struct KittyCacheInvalidation {
    /// 须把单元格像素尺寸同步到终端侧：上游按该尺寸重算图像像素尺寸。
    sync_cell_size: bool,
    /// 须重查放置（渲染状态锁外查询会话）。
    refetch_placements: bool,
    /// 须整帧重绘：上一帧留在累加器里的图像像素已失效。
    overlay_dirty: bool,
    /// 须重打包图集（图像增删或换会话）。
    repack_atlas: bool,
    /// 仅视口滚动时把已有放置帧平移的行数（正值 = 图像在视口内下移）。
    ///
    /// 滚动每一步都去 VT 线程同步查一次放置，会让渲染线程每行等一次
    /// 完整查询（真机 yazi 预览下实测每行约 60ms，滚动掉到 ~16fps）。
    /// 放置帧是视口相对坐标，纯滚动可以用视口顶行的差量平移复原；
    /// 只有滚动停稳后再补一次真实查询，把被旧视口裁掉的边缘信息修正回来。
    viewport_shift_rows: i64,
}

/// 视口顶行的绝对行号：绝对行 0 = 回滚区顶部，视口贴底时等于回滚长度。
///
/// 饱和减法：渲染态初值用 `scroll_offset = i64::MIN` 作「从未查询」哨兵，
/// 直接相减会在 debug 构建里溢出 panic。
fn viewport_top_row(keys: KittyPlacementKeys) -> i64 {
    i64::from(keys.scrollback_length).saturating_sub(keys.scroll_offset)
}

/// 依据放置键与单元格几何变化判定 Kitty 缓存如何失效。
///
/// 纯文本会话（生成戳 0）没有图像路径：只按需清掉残留图像并整帧重绘。
/// 视口滚动不是「放置变化」：放置帧按 [`viewport_top_row`] 的差量平移即可，
/// 只有滚动停稳（`frames_approximate` 且本轮视口未再变化）才重查一次。
fn kitty_cache_invalidation(
    current: KittyPlacementKeys,
    previous: KittyPlacementKeys,
    cell_geometry_changed: bool,
    frames_approximate: bool,
) -> KittyCacheInvalidation {
    let session_changed = current.session_id != previous.session_id;
    let generation_changed = current.generation != previous.generation;
    // 与视口无关的放置键：任一变化都必须重查，平移无法表达。
    let content_changed = session_changed
        || generation_changed
        || current.placeholder_signature != previous.placeholder_signature
        || current.rows != previous.rows
        || current.cols != previous.cols;
    let viewport_shift_rows = viewport_top_row(previous).saturating_sub(viewport_top_row(current));
    if current.generation == 0 {
        return KittyCacheInvalidation {
            sync_cell_size: false,
            refetch_placements: false,
            // 撤下图像（或换会话）时累加器里仍有旧像素，必须整帧重绘清掉；
            // 纯文本会话的滚动不涉及叠加层，不必为此丢掉脏带渲染。
            overlay_dirty: session_changed || cell_geometry_changed || previous.generation != 0,
            repack_atlas: false,
            viewport_shift_rows: 0,
        };
    }
    if content_changed || cell_geometry_changed {
        return KittyCacheInvalidation {
            // 换会话时新会话终端侧的图像几何可能不同，即便网格尺寸未变也要同步。
            sync_cell_size: cell_geometry_changed || session_changed,
            refetch_placements: true,
            overlay_dirty: true,
            repack_atlas: generation_changed || session_changed,
            viewport_shift_rows: 0,
        };
    }
    if viewport_shift_rows != 0 {
        return KittyCacheInvalidation {
            sync_cell_size: false,
            refetch_placements: false,
            // 图像随视口移动：整帧重绘才能同时擦掉旧位置并画到新位置。
            overlay_dirty: true,
            repack_atlas: false,
            viewport_shift_rows,
        };
    }
    if frames_approximate {
        // 视口已停稳：补一次真实查询，恢复滚动期间按旧视口裁剪的源矩形。
        return KittyCacheInvalidation {
            sync_cell_size: false,
            refetch_placements: true,
            overlay_dirty: true,
            repack_atlas: false,
            viewport_shift_rows: 0,
        };
    }
    KittyCacheInvalidation {
        sync_cell_size: false,
        refetch_placements: false,
        overlay_dirty: false,
        repack_atlas: false,
        viewport_shift_rows: 0,
    }
}
struct RenderState {
    renderer: crate::render::context::Renderer,
    font_pipeline: crate::render::font::FontPipeline,
    /// 下一帧的搜索高亮区间。由 `applySearchHighlights` 写入（按字节打包的 rows/cols），
    /// `render_inner` 消费；只有最新输入有意义（渲染循环约 60fps 且每次按键都重设）。
    /// 存为已解析结构便于按切片传入，由 `clearSearchHighlights` 清除。
    ///
    /// 按会话键控，见 [`SessionScoped`]。
    search_highlights: SessionScoped<crate::render::cell_builder::SearchHighlight>,
    /// 上次渲染的帧（单元 + 光标 + 尺寸 + 会话）。`render()` 只在有新输出时绘制，故空闲
    /// 终端复用此缓存帧而非逐帧重新合成。会话标识避免切会话后把旧会话网格画到新会话。
    last_frame: Option<(
        Vec<crate::terminal::ghostty_terminal::CellData>,
        crate::terminal::ghostty_terminal::CursorInfo,
        u32,
        u32,
        u64,
    )>,
    /// 上次绘制时的视口 Y 像素偏移——供空闲重绘门控检测逐像素滚动余数变化并强制重绘
    /// （该偏移移动像素但不改单元内容）。
    last_scroll_px: f32,
    /// 当前生效的终端主题（静态清单项）。渲染态自身的颜色——光标色与搜索高亮色——
    /// 都取自它，故颜色真源只存在于 [`crate::terminal::theme`] 一处，Kotlin 无需
    /// 回传颜色。由 `setTheme` 写入，主题变更总是伴随重绘。
    theme: &'static crate::terminal::theme::Theme,
    /// 预分配的脏行掩码，跨帧复用以避免逐帧 `Vec<bool>` 分配（约 100-300 字节 × 120fps）。
    dirty_mask: Vec<bool>,
    /// Kitty 图像缓存（按会话键控）：VT 线程经生成戳推送变更通知，
    /// 渲染线程仅在放置键变化时查询放置，平时复用实例。
    kitty_keys: KittyPlacementKeys,
    kitty_cell_width: f32,
    kitty_cell_height: f32,
    kitty_frames: Vec<crate::terminal::ghostty_terminal::KittyPlacementFrame>,
    kitty_instances: Vec<crate::render::KittyGraphicsInstance>,
    kitty_uploaded_generation: u64,
    /// [`Self::kitty_frames`] 是否是滚动期间按视口差量平移出来的近似帧。
    ///
    /// 平移帧保留的是旧视口的源矩形裁剪；滚动停稳后要重查一次才能恢复边缘。
    kitty_frames_approximate: bool,
    /// 上一帧看到的视口键（滚动偏移, 回滚长度）。用于区分「滚动进行中」与「已停稳」：
    /// 连续两帧相同即视为停稳，此时才允许补那次昂贵的放置查询。
    last_kitty_scroll: (i64, u32),
    /// Kitty 叠加层自上次**成功呈现**以来是否发生过变化（新增/替换/删除图像）。
    ///
    /// 变化的那一帧必须全量重绘：旧图像的像素仍留在累加器里，不清就会留下残影。
    /// 图像保持不变时叠加层每帧照画，不需要全量——否则一张静止的图片会把终端
    /// 钉在全量帧的代价上（SwiftShader 实测全量帧 ~500ms，终端掉到 1-2fps）。
    /// 呈现失败时不清标记，下一帧仍按「有变化」处理，避免残影被永久留下。
    kitty_overlay_dirty: bool,
    /// 内容脏标志：由改动延迟渲染输入的 JNI 入口
    /// （`setSearchHighlights`/`clearSearchHighlights`、`setFontSizeInPlace`）置位，
    /// 渲染线程在 `render_inner` 中用一次 `getAndSet(false)` 交换消费。与每会话的
    /// `new_output` 标志（PTY 摄入）独立：高亮/字号变化必须重绘但绝不复位视口。
    ///
    /// 它**同时**是 Kotlin 渲染循环的唤醒信号与空闲门控的通过条件之一——绝不可作为
    /// 外层短路（空闲重绘决策在门控内部做出）。
    dirty: AtomicBool,
}

/// 按会话键控的一段渲染态数据。
///
/// 渲染态是进程级唯一的，而产生它的查询是**每会话**的：切会话时宿主重建搜索状态而
/// 不再发一次清除，于是新会话首帧会带着上一会话的命中框合成。键控让「这份数据属于
/// 谁」与数据本身同生共死——拆成并行字段时，任一入口漏清都会画到错误的会话上。
///
/// `drawn` 与 `current` 成对：脏带渲染靠二者判脏哪些行，故 `enter_session` **只**清
/// `current`——`drawn` 里那批正留在累加器像素里的旧内容必须再进一次脏掩码才会被重画掉
/// （判据与理由见 `enter_session`）。谁把两者一起清，谁就重新引入「关搜索后反色块
/// 不消失」：那些行不再进掩码，旧高亮就永久留在屏幕上。
struct SessionScoped<T> {
    session: u64,
    current: Vec<T>,
    drawn: Vec<T>,
}

impl<T> SessionScoped<T> {
    fn new() -> Self {
        Self {
            session: 0,
            current: Vec::new(),
            drawn: Vec::new(),
        }
    }

    /// 丢弃不属于本会话的内容。每帧在渲染前调用一次，两条帧路径共用同一份结论。
    ///
    /// `drawn` 故意**不**清：脏带渲染靠 `current ∪ drawn` 决定标脏哪些行，而被丢弃的
    /// 那批高亮正是此刻还留在累加器像素里的行，必须再进一次脏掩码才会被重画掉。
    /// 连它一起清，切会话与 `clearSearchHighlights` 后这些行不在掩码里，旧高亮就
    /// 一直留在屏幕上（实测：关闭搜索后反色块不消失，随后只有光标行在重绘）。
    /// 渲染成功后 `drawn` 被赋成 `current`（此刻已空），故这份残留只活一帧，自净。
    fn enter_session(&mut self, session: u64) {
        if self.session != session {
            self.session = session;
            self.current.clear();
        }
    }
}

/// 在无 surface 的情况下预热渲染器与字体库；spawn 后在后台线程调用一次。
///
/// 顺序说明：spawn 前算网格的 `getCellSizePacked` 走的也是 `render_state_mut`，正常启动里
/// 初始化因此已经发生，此处通常是**幂等兜底**——度量查询赶上 GPU 暂不可用而返回 0 时，
/// 这里再试一次。它真正保住的语义是「GPU 初始化失败不致命」：字体库无任何可用面时
/// `font_db::fatal` 是 `process::abort`（进程级终止，见下），而 GPU 失败只记账，
/// 致命判定留给渲染路径（那段 attach→首帧本就要等初始化，软件 GL 上约 3s）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        // GPU 初始化是预热，失败可重试：不可用时只记账，不致命。
        // 渲染路径的 fatal 判定不变（见 render_state_mut），故真无 GPU 的设备仍在
        // 首帧得到明确的 GPU initialization failed，而不会被一次瞬时失败提前 abort。
        match crate::render::context::try_global_gpu() {
            Ok(_) => {
                // 但**字体**部分不享有该豁免：render_state_mut 里的
                // font_db::fatal 是 process::abort —— 进程级终止，jni_export_guard
                // 与 Kotlin 的 try/catch 都接不住（真机 SIGABRT 即由此而来，见
                // DESIGN 字体节「输出日志并崩溃退出，不做复杂处理」）。
                drop(render_state_mut());
                log::info!("render state prefetched");
            }
            Err(initialization_error) => {
                log::error!("render state prefetch skipped: {initialization_error}");
            }
        }
    })
}

/// 重建字体管线，并失效一切由旧管线派生的状态。
///
/// 管线整体替换会重新分配图集，旧实例携带的 UV 指向旧图集；缓存失效与重绘请求
/// 必须与替换同处一个函数，否则某入口漏掉时既无日志也不重绘。
#[cfg(target_os = "android")]
fn rebuild_font_pipeline(render_state: &mut RenderState) {
    let (atlas_width, atlas_height) = render_state.font_pipeline.atlas_dimensions();
    let font_size = render_state.font_pipeline.font_size();
    render_state.font_pipeline =
        crate::render::font::FontPipeline::new(atlas_width as i32, atlas_height as i32, font_size);
    render_state.renderer.cell_cache = None;
    render_state.dirty.store(true, Ordering::Relaxed);
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
            search_highlights: SessionScoped::new(),
            last_frame: None,
            last_scroll_px: 0.0,
            theme: crate::terminal::theme::default_dark(),
            dirty_mask: Vec::new(),
            kitty_keys: KittyPlacementKeys {
                session_id: 0,
                generation: u64::MAX,
                scroll_offset: i64::MIN,
                scrollback_length: u32::MAX,
                placeholder_signature: u64::MAX,
                rows: u32::MAX,
                cols: u32::MAX,
            },
            kitty_cell_width: 0.0,
            kitty_cell_height: 0.0,
            kitty_frames: Vec::new(),
            kitty_instances: Vec::new(),
            kitty_uploaded_generation: u64::MAX,
            kitty_frames_approximate: false,
            last_kitty_scroll: (i64::MIN, u32::MAX),
            kitty_overlay_dirty: false,
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
    render_state.kitty_instances = match crate::render::kitty::pack_and_build(
        &render_state.kitty_frames,
        cell_width,
        cell_height,
    ) {
        Some((atlas, atlas_width, atlas_height, instances)) => {
            // 图集被拒（超设备上限、长度不符、尺寸溢出）时纹理为空，而留着按旧图集
            // 算好的实例会让 `kgp_present` 恒真：绑定组恒 `None` 图像永不出现，
            // 部分帧增量渲染却被永久关掉，且每帧都因尺寸漂移重打包一次。
            if render_state
                .renderer
                .set_kgp_atlas(&atlas, atlas_width, atlas_height)
            {
                instances
            } else {
                Vec::new()
            }
        }
        None => {
            render_state.renderer.set_kgp_atlas(&[], 0, 0);
            Vec::new()
        }
    };
    render_state.kitty_uploaded_generation = uploaded_generation;
}

// ══════════════════════════════════════════════════════════════════════════
// 事件队列
// ══════════════════════════════════════════════════════════════════════════

static EVENT_QUEUE: crate::event::EventQueue = crate::event::EventQueue::new();

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// 待宿主应用作答的剪贴板应答通道发送端，按 (会话 id, 请求 id) 存入
/// [`REQUEST_REGISTRY`]。
type ClipboardAnswerTx = std::sync::mpsc::Sender<Option<String>>;

static REQUEST_REGISTRY: LazyLock<Mutex<HashMap<(u64, u64), ClipboardAnswerTx>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 有界超时地等待宿主应用作答剪贴板。返回 `None` 表示读取失败或宿主未作答：
/// 此时不写回 OSC 52 应答——空串等于告诉远端「用户剪贴板是空的」，
/// tmux/ssh 会粘出空白且像是用户清空了剪贴板。
const CLIPBOARD_ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// 剪贴板应答的轮询节奏：在上述 2s 应答截止时间内，应答方以此频率检查一次性槽位。
const CLIPBOARD_POLL_INTERVAL_MS: u64 = 25;

fn wait_for_clipboard_answer(rx: std::sync::mpsc::Receiver<Option<String>>) -> Option<String> {
    let deadline = std::time::Instant::now() + CLIPBOARD_ANSWER_TIMEOUT;
    loop {
        match rx.try_recv() {
            Ok(answer) => return answer,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::error!("osc52: clipboard answer channel disconnected");
                return None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    log::error!("osc52: clipboard answer timed out");
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(CLIPBOARD_POLL_INTERVAL_MS));
            }
        }
    }
}

pub(crate) fn register_request(
    session_id: u64,
) -> (u64, std::sync::mpsc::Receiver<Option<String>>) {
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

/// 单会话未作答 OSC 52 读取上限：远端循环 `\e]52;c;?\a` flood 时，
/// 每个请求都在注册表占槽、spawn 应答线程并推一个不可淘汰的保护事件；
/// 无上限则 1024 槽被读请求填满后连 Exit 一起丢，会话永久泄漏。
/// 超限的请求不占槽，就地显式作答空串（xterm 兼容的空剪贴板响应）。
const MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION: usize = 8;

/// 每帧允许的洪泛削减回写次数上限。
///
/// 削减分支要同步写 PTY，而写 PTY 在子进程不读 stdin 时每次可等到 50ms。一帧十块
/// 输出就可能带来数千条读请求，全写一遍等于把渲染线程整个交给病态远端。取 16 是
/// 「足够让远端知道终端已拒绝」与「最坏等待远小于一帧预算」之间的折中。
const MAX_FLOOD_SHED_ANSWERS_PER_FRAME: usize = 16;

/// 单会话当前未作答的读请求数（上段上限的判据）。
pub(crate) fn unanswered_clipboard_reads(session_id: u64) -> usize {
    REQUEST_REGISTRY
        .lock()
        .keys()
        .filter(|key| key.0 == session_id)
        .count()
}

/// 读请求洪泛削减告警的限频（毫秒）：超限每帧都发生，逐条记会淹没 logcat。
static LAST_FLOOD_SHED_WARN_MS: AtomicU64 = AtomicU64::new(0);
const FLOOD_SHED_WARN_INTERVAL_MS: u64 = 5_000;

fn flood_shed_warn_throttled(session_id: u64, unanswered: usize) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_FLOOD_SHED_WARN_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) >= FLOOD_SHED_WARN_INTERVAL_MS
        && LAST_FLOOD_SHED_WARN_MS
            .compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        log::warn!(
            "osc52: clipboard read flood from session {session_id}, \
             answering empty past {unanswered} unanswered"
        );
    }
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
/// 读取 initSession 的 JNI 字符串参数，失败时抛 Java 异常并返回 None。
/// 解码一个 JNI 字符串参数。解码失败即抛 `RuntimeException` 并返回 `None`，
/// 由调用方按各自的返回类型提前退出。
///
/// 绝不把解码失败折成「参数不合法」或「未找到」那类正常空结果：前者会让
/// `Bridge.onSession` 记一条警告后继续，后者则零日志——而解码失败
/// 是 Kotlin 传了非法字符串的编程错误，静默返回会让它伪装成配置正常。
fn read_jni_string(env: &mut Env, value: &JString, caller: &str, name: &str) -> Option<String> {
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
        Err(e) => {
            if let Err(e) = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                JNIString::from(format!("resize: failed: {e}")),
            ) {
                log::error!("resize: throw_new failed: {e}");
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
            && matches!(io_error.raw_os_error(), Some(libc::EIO) | Some(libc::EPIPE))
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
fn empty_java_byte_array(env: &mut Env) -> jbyteArray {
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
fn java_bytes_or_empty(env: &mut Env, bytes: &[u8]) -> jbyteArray {
    if bytes.is_empty() {
        return empty_java_byte_array(env);
    }
    env.byte_array_from_slice(bytes)
        .map(|array| array.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn encode_mouse_event_inner(
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
struct MouseWheelEvent {
    x_px: jfloat,
    y_px: jfloat,
    button: jint,
    modifiers: jint,
    cell_width: jfloat,
    cell_height: jfloat,
    repeat: jint,
}

fn encode_mouse_wheel_inner(
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
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return Ok(JNI_FALSE);
        };
        let session = entry.session.lock();
        if session.terminal().mouse_tracking_active_atomic() {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
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

fn encode_key_event_inner(
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

fn encode_paste_inner(
    env: &mut Env,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) -> jbyteArray {
    // SAFETY: `data` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效。
    // `from_raw` 只包装指针而不取得所有权；局部引用由 JVM 在本 native 方法返回时释放。
    let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, data) };
    let input: Vec<u8> = match env.convert_byte_array(&byte_array) {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = env.throw_new(
                jni_str!("java/lang/RuntimeException"),
                jni_str!("encodePaste: failed to read input bytes"),
            );
            return std::ptr::null_mut();
        }
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
fn key_event_text(code_point: jint) -> Option<char> {
    let code_point = u32::try_from(code_point).ok()?;
    let character = char::from_u32(code_point)?;
    (!character.is_control() && !(0xF700..=0xF8FF).contains(&code_point)).then_some(character)
}

// ── 渲染唤醒 ────────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：waitForOutput / wakeRender
// ══════════════════════════════════════════════════════════════════════════

/// 渲染循环的驻留点：等新输出或任意唤醒信号，返回新的唤醒代数。
///
/// `last_generation` 是调用方上次拿到的代数；期间若已发生 notify，立即返回，
/// 因此「检查完信号再进场」与 notify 竞争也不会丢唤醒。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_waitForOutput(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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
        match wake {
            Some(wake) => wake.wait(last_generation, timeout) as jlong,
            None => {
                // 会话已销毁：按同一节奏睡一段再返回，调用方下一拍就会看到会话消失
                // 并退出循环。返回原代数而不是新值，避免伪造「有新信号」。
                std::thread::sleep(timeout);
                last_generation as jlong
            }
        }
    })
}

/// 唤醒指定会话的渲染循环（vsync、输入、滚动、选区等所有 Kotlin 侧信号）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_wakeRender(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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
    })
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

fn poll_event_inner<'local>(env: &mut Env<'local>, _class: JClass<'local>) -> jstring {
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

// ── 渲染生命周期 ──────────────────────────────────────────────────
/// 推进全部会话的终端状态，并把本轮产生的事件推入事件队列。
///
/// 与 [`poll_event_inner`] 的步骤 2 分离，使 [`Java_terminal_emulator_bridge_NativeBridge_pumpSessions`]
/// 可在渲染暂停期间单独调用：此时渲染线程已停，`pollEvent` 无人调用，若会话也不再
/// 推进，有界输出通道会填满 → 读线程阻塞 → PTY 内核缓冲填满 → 后台作业停摆。
/// 本函数不消费事件队列，故后台期间产生的剪贴板、退出等事件仍在队列中，
/// 渲染恢复后由 `pollEvent` 正常派发。
fn pump_sessions() {
    // 步骤 1：轮询活跃会话的新事件。先收集事件，释放会话锁后再推入，
    // 以保持锁顺序 `SESSION_REGISTRY` → `Session` → `EVENT_QUEUE`。
    // 绝不在持有 `Session` 锁时锁 `EVENT_QUEUE`。
    let mut pending_events: Vec<Event> = Vec::new();
    // 本帧可上报 Exit 事件的会话（退出码已就绪者）及其退出结果与存活时长。
    // 退出码在会话锁内就地判定：只读一次 Mutex，不再有原先「先锁存上报标志、再忙等
    // 最多 100ms 取码」的两段式——那会在渲染线程上引入可见停顿，且超时只能上报
    // 伪造的 0。改为每帧问一次「退出了吗」，未就绪就下一帧再问，零阻塞。
    let mut pending_exits: Vec<(u64, crate::terminal::session::ReportedExit)> = Vec::new();
    // 剪贴板/退出的轮询对活跃会话与所有后台会话完全相同，共用同一实现。
    let collect_session_events =
        |session_id: u64,
         session: &mut Session,
         events: &mut Vec<Event>,
         exits: &mut Vec<(u64, crate::terminal::session::ReportedExit)>| {
            if let Some(text) = session.poll_clipboard() {
                events.push(Event::Clipboard { session_id, text });
            }
            // 退出码就绪才上报（`take_reported_exit` 内部同时锁存「已上报」），后台扫描
            // 分支走同一判定：既保证退出事件恰好一次，也保证报出的是真实退出码。
            if let Some(reported) = session.take_reported_exit() {
                exits.push((session_id, reported));
            }
        };
    let mut pending_clipboard_reads: Vec<(u64, String)> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    // 采集本帧要处理的会话句柄：读锁内只克隆 Arc，不做任何会话工作。
    // 慢会话的 VT 解析/查询若顶着读锁跑，会把 setScrollOffset 等写锁饿死，
    // 并把单个会话的停顿放大成全局停顿。
    // 句柄是 Arc：锁外处理期间会话被销毁，也只是对着已死的 Arc 做无害功——
    // 事件都带 session_id（Kotlin 侧对未知会话本就是空操作/幂等回收），
    // take 系调用是锁存式的（恰好一次），不存在双重上报。
    let poll_batch: Vec<(u64, Arc<Mutex<Session>>, bool)> = {
        let registry = rlock_session_registry();
        let mut batch = Vec::new();
        if active_id != 0
            && let Some(entry) = registry.get(&active_id)
        {
            batch.push((active_id, entry.session.clone(), true));
        }
        // 后台会话同样先收句柄（跳过活跃者，避免一帧处理两次）。
        for (id, entry) in registry.iter() {
            if active_id != 0 && *id == active_id {
                continue;
            }
            batch.push((*id, entry.session.clone(), false));
        }
        batch
    };
    // 锁外逐个处理：活跃会话全量，其余每帧 2 块保管道流动。
    for (session_id, session_handle, is_active) in &poll_batch {
        let mut session = session_handle.lock();
        if *is_active {
            // 处理来自 PTY 读取线程的 VT 输出。这是驱动全部终端状态更新的关键路径：
            // 从 `output_rx` 读取数据送入 Ghostty 的 VT 解析器，并填充下方轮询的
            // 各类事件标志（剪贴板等）。缺少此调用则终端永远不处理输出，输出通道死锁。
            session.process_output();
            // 检查 OSC 52 剪贴板读取请求（`ESC ] 52 ; c ; ?`）。此处（会话锁内）
            // 取走全部待答 selection；一次性槽位与应答线程在注册表/会话锁释放后才建立
            // （见下方），保持锁顺序单一方向。超限由下方单会话上限显式作答。
            for selection in session.take_clipboard_reads() {
                pending_clipboard_reads.push((*session_id, selection));
            }
        } else {
            // 每个后台会话每帧 2 块：足以在持续输出下不让读取线程阻塞。
            // 后台会话的输出通道一旦填满（读取线程阻塞在有界发送 → PTY 内核缓冲填满
            // → 子进程写入阻塞）就会冻结后台作业；每帧排空若干块既保持管道流动，
            // 又不饿死活跃会话的帧预算。
            session.poll_pty_output(PTY_POLL_CHUNKS_PER_FRAME);
            // 后台会话同样收割读请求。只排活跃者会让本队列无界增长（后台程序持续
            // 发读请求时每帧进一条、从不取出），且那些程序永远收不到应答。走的是
            // 与活跃会话同一条削减路径，故成本同样受下方的每帧上限约束。
            for selection in session.take_clipboard_reads() {
                pending_clipboard_reads.push((*session_id, selection));
            }
        }
        // 立即消费事件标志并带上正确的 `session_id` 推入。若留着不管，
        // 它们会在数分钟后该会话重新活跃时被重放（陈旧重放）。
        collect_session_events(
            *session_id,
            &mut session,
            &mut pending_events,
            &mut pending_exits,
        );
        // 会话锁在此释放（循环迭代末尾）。
    }
    // 处理待处理的 OSC 52 剪贴板读取请求（在注册表/会话锁之外）：登记一次性应答槽位、
    // 推入事件，并启动短生命周期的应答线程把宿主应用的答复写回 PTY。
    // VT 线程绝不能阻塞等待宿主应用，且 `clipboardResult` 可能在任意线程到达。
    let mut flood_shed_answers = 0usize;
    for (session_id, selection) in pending_clipboard_reads {
        let unanswered = unanswered_clipboard_reads(session_id);
        if unanswered >= MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION {
            // 洪泛削减：就地显式作答空串，不注册、不推事件、不起应答线程。
            // 槽位、线程与保护事件三者都不再增长，Exit 永远有位置。
            //
            // 但「就地作答」本身要写 PTY（base64 + 带期限的写入，子进程不读 stdin
            // 时每次可等到 50ms），而这段代码跑在渲染线程上：一次 8KB 输出块里能塞
            // 约 900 条读请求，一帧十块即约 9000 次带锁写入，光等待就足以吃掉整帧
            // 预算。故削减回写也按帧封顶——被削减的请求本就是「拒绝」，不写即拒绝。
            if flood_shed_answers >= MAX_FLOOD_SHED_ANSWERS_PER_FRAME {
                continue;
            }
            flood_shed_answers += 1;
            flood_shed_warn_throttled(session_id, unanswered);
            let registry = wlock_session_registry();
            let session = registry.get(&session_id).map(|entry| entry.session.clone());
            if let Some(session) = session {
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(&selection, "") {
                    log::warn!("osc52: flood-shed read answer write-back failed: {error}");
                }
            }
            continue;
        }
        let (request_id, rx) = register_request(session_id);
        pending_events.push(Event::ClipboardRead {
            session_id,
            request_id,
            selection: selection.clone(),
        });
        if let Some(session) = session_handle(session_id) {
            std::thread::spawn(move || {
                let answer = wait_for_clipboard_answer(rx);
                // 应答线程是槽位的唯一所有者：无论宿主是否作答（事件被队列淘汰、
                // UI 未处理、答复晚于截止时间），槽位至多存活 CLIPBOARD_ANSWER_TIMEOUT。
                // 否则远端反复 `\e]52;c;?` 会让本表无界增长。
                // 迟到的 clipboardResult 在此之后到达即为无操作（见 clipboard_result_inner）。
                cancel_request(session_id, request_id);
                let Some(text) = answer else { return };
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

    for (id, reported) in pending_exits {
        pending_events.push(Event::Exit {
            session_id: id,
            code: match reported {
                crate::terminal::session::ReportedExit::Code(code) => Some(code),
                crate::terminal::session::ReportedExit::Unknown => None,
            },
        });
    }
    // 推入已收集的事件——此时不持有 `Session` 或 `SESSION_REGISTRY` 锁。
    for event in pending_events {
        EVENT_QUEUE.push(event);
    }
}

/// 不消费事件的会话推进泵：渲染暂停期间维持终端状态前进，避免后台作业停摆。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pumpSessions(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| pump_sessions());
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

/// 由 `CursorInfo` 快照构建光标，光标色取当前主题。
/// 光标样式恒为默认块状：样式覆盖已随光标样式设置一并移除。
fn build_cursor(
    render_state: &RenderState,
    cursor_info: &crate::terminal::ghostty_terminal::CursorInfo,
) -> crate::render::CellCursor {
    crate::render::CellCursor {
        // 光标落在宽字符尾列时 `col` 指的是第二格，而单元数据只为该宽字符发出一条
        // 头列记录（`width=2` 一条即整个字形）——渲染侧按列匹配，故在此折算成锚列。
        // 归位在本函数：它是 VT 真值与渲染锚点之间唯一的转换口，而 `getCursor-
        // ViewportPacked` 等可观测查询继续报 VT 的真实列。
        row: cursor_info.row,
        col: cursor_info.col - u32::from(cursor_info.at_wide_tail),
        visible: cursor_info.visible,
        style: cursor_info.style,
        color: crate::terminal::ghostty_terminal::GhosttyTerminal::color_to_float(
            render_state.theme.palette.cursor,
        ),
    }
}

/// 在掩码中把叠加行（搜索高亮）标脏。此类逐行视觉叠加的增删会改变像素而不触及单元内容。
/// 选区无需叠加处理：VT 线程把跟踪选区的反显烘焙进 `CellData`，选区变化经正常的
/// 脏路径作为新单元内容到达。
///
/// 脏带标记需覆盖的高亮行：当前高亮 ∪ 上次绘制的高亮。
///
/// 取并集是因为高亮被清除或移动时，陈旧高亮像素仍留在累加器里，漏掉任何一侧都会让
/// 那些行不再被重绘。两条渲染路径（VT 新帧与空闲同步）各需一次，故收在此处作单一真源。
/// 收两个切片而非 `&RenderState`：调用点已持有 `dirty_mask` 的可变借用，
/// 方法调用会连带借出整个结构体，与之冲突；字段级不相交借用则可用。
fn highlight_rows_to_mark<'a>(
    current: &'a [crate::render::cell_builder::SearchHighlight],
    last_drawn: &'a [crate::render::cell_builder::SearchHighlight],
) -> impl Iterator<Item = &'a crate::render::cell_builder::SearchHighlight> {
    current.iter().chain(last_drawn.iter())
}

/// 搜索高亮行，当前与上次绘制的都要算：高亮是逐行叠加，增删移动会改变像素而
/// 不改变单元内容。直接遍历两段切片，不为每帧的这次调用分配中间 `Vec<i32>`。
fn mark_overlay_dirty_rows<'a>(
    dirty_mask: &mut [bool],
    rows_usize: usize,
    highlights: impl Iterator<Item = &'a crate::render::cell_builder::SearchHighlight>,
) {
    for highlight_row in highlights.map(|highlight| highlight.row) {
        if highlight_row >= 0 && (highlight_row as usize) < rows_usize {
            dirty_mask[highlight_row as usize] = true;
        }
    }
}

#[cfg(test)]
mod search_highlight_repaint_tests {
    use super::{SessionScoped, highlight_rows_to_mark, mark_overlay_dirty_rows};
    use crate::render::cell_builder::SearchHighlight;

    fn highlight(row: i32) -> SearchHighlight {
        SearchHighlight {
            row,
            start_col: 0,
            end_col_exclusive: 6,
            color: [0, 0, 0, 255],
        }
    }

    /// 换会话（`clearSearchHighlights` 走同一条 `enter_session` 分支）后，上一次画过
    /// 高亮的行必须仍进脏掩码。
    ///
    /// 判别力：`enter_session` 若把 `drawn` 一并清空，掩码上就没有第 2 行，脏带渲染
    /// 跳过它，累加器里的旧高亮像素留在屏幕上——实测关闭搜索后反色块不消失，随后
    /// 只有光标行在重绘。
    #[test]
    fn discarded_highlights_stay_dirty_for_one_frame() {
        let mut scoped = SessionScoped::<SearchHighlight>::new();
        scoped.enter_session(1);
        scoped.current.push(highlight(2));
        scoped.drawn = scoped.current.clone();

        scoped.enter_session(2);
        assert!(scoped.current.is_empty(), "换会话后不得再画上一会话的高亮");
        assert_eq!(scoped.drawn.len(), 1, "被丢弃的高亮行要留一帧用于标脏");

        let mut dirty_mask = [false; 5];
        mark_overlay_dirty_rows(
            &mut dirty_mask,
            5,
            highlight_rows_to_mark(&scoped.current, &scoped.drawn),
        );
        assert!(
            dirty_mask[2],
            "旧高亮所在行必须标脏，否则旧像素永远留在累加器里"
        );
        assert!(!dirty_mask[3], "未受高亮影响的行不该被顺带标脏");
    }
}

/// 取会话句柄并请求其重推一帧。会话不存在时为 false。
///
/// 锁序：注册表 → 会话，且在**不持有** RENDER_STATE 时调用（见下方调用点）。
fn request_session_repaint(session_id: u64) -> bool {
    session_handle(session_id)
        .map(|session| session.lock().terminal().request_repaint())
        .is_some()
}

fn render_inner(session_id: u64) -> jint {
    let render_start = std::time::Instant::now();
    // ── 阶段 0：切会话后索要一帧 ────────────────────────────────────────
    // 累加器里是上一个会话的像素，而 `last_frame` 缓存也属于它：阶段 3 的空闲路径
    // 会因会话不一致而拒绘（防止把旧会话网格画到新会话），目标会话若无新输出就
    // 永远不推帧——屏幕一直停在旧会话内容上，直到碰巧有输入（实测切换后 25s 未变）。
    // 故此处按当前会话索要一次重推：内容去重在 VT 线程上被失效，下一帧即为
    // `FrameData::New`，阶段 3 对会话变化的 `_ => fill(true)` 分支整幅重画。
    // 只读一次 RENDER_STATE 并立即释放，随后才按注册表 → 会话的锁序取句柄。
    let cached_frame_session = {
        let state = render_state_mut();
        state
            .as_ref()
            .and_then(|render_state| render_state.last_frame.as_ref())
            .map(|(_, _, _, _, cached_session)| *cached_session)
    };
    if cached_frame_session.is_some_and(|cached_session| cached_session != session_id) {
        request_session_repaint(session_id);
    }

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
        // 切会话时按会话键丢弃上一会话的搜索高亮（见 `SessionScoped`）：在阶段 1 这一次
        // 加锁内完成，两条帧路径共用同一份结论。
        render_state.search_highlights.enter_session(session_id);
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
            // initialize_pipeline_and_bind_group 重建了**空的**图集纹理，而 CPU 侧的
            // 字形缓存仍全部命中 → 没有任何字形会被重新 blit，终端只剩背景、无文字，
            // 且不会自愈（重建路径只在 surface 格式变化时触发）。故整幅标脏重传。
            render_state.font_pipeline.reset_dirty_rect_full();
            render_state.renderer.cell_cache = None;
            render_state.renderer.frame_invalidated = true;
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
    } // ── 已释放 render_state 锁 ──────────────────────────────────────────
    let phase1_done = std::time::Instant::now();

    // ── 阶段 2：收集单元数据（仅持会话锁）───────────────────────────────
    // 有新数据时从通道接收自有的 `CellData`（零拷贝移动）；空闲时只记录事实——
    // 阶段 3 将直接引用 `last_frame`，省去此处原有的整帧克隆（CellData 96B×行列数）。
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
    // 无 ANativeWindow 时同样不得消费通道：receive 是破坏性取数，而 VT 的去重基线
    // 在**入队**时就推进（见 push_cell_data），故被本函数取走又丢弃的帧不会被补发——
    // 换 Surface 的空档期（releaseGpuSurface → attachSurface）产出的输出会一直缺失，
    // 直到终端内容再次变化才重绘。与 paused 同一条判据，故同处提前返回。
    //
    // 只读一次 render_state 并立即释放：下方阶段 3 才取 RENDER_STATE，
    // 此处不得跨 SESSION_REGISTRY/Session 两级锁持有它（见阶段 3 的锁序说明）。
    {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            log::error!("render: render state missing");
            return -1;
        };
        if render_state.renderer.render_paused || render_state.renderer.surface.is_none() {
            return 0;
        }
    }
    let frame_data = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&session_id) else {
            log::warn!("render: unknown session {session_id}");
            return -1;
        };
        let session = entry.session.lock();
        // 关键：**不要**在此调用 `session.terminal().scrollback_length()`——
        // 它是发往 VT 线程的同步 RPC，VT 线程繁忙时会把渲染线程阻塞最多 500ms。
        // 回滚长度搭载在经单元数据通道传递的 `CursorInfo` 上
        // （见 `push_cell_data` → `CursorInfo.scrollback_length`）。
        // 网格行列同理：`session.grid_size()` 在 resize 入队时提前发布，
        // VT 尚未应用新网格时与本帧 CellData 不是同一套尺寸；
        // `CursorInfo.rows/cols` 是产出本帧时的真实网格，与单元数据同源。
        match session.terminal().receive_cell_data() {
            Some((cells, cursor_info)) => {
                let rows = cursor_info.rows;
                let cols = cursor_info.cols;
                // 光标随帧发布给会话条目：输入法锚点上报（主线程、逐键）读它，
                // 不再为此发一次同步 VT 查询。发布点在注册表读锁 + 会话锁内，
                // 而载体本身是原子量——两侧都不因这次发布多等一把锁。
                entry
                    .rendered_cursor
                    .store(pack_rendered_cursor(&cursor_info), Ordering::Release);
                entry.rendered_scrollback.store(
                    i32::try_from(cursor_info.scrollback_length).unwrap_or(i32::MAX),
                    Ordering::Release,
                );
                FrameData::New {
                    cells,
                    cursor_info,
                    rows,
                    cols,
                    scroll_offset: session.last_scroll_offset(),
                }
            }
            None => FrameData::Idle {},
        }
    }; // ── 会话锁在此释放 ─────────────────────────────────────────────────
    let phase2_done = std::time::Instant::now();

    // ── 阶段 3：渲染（持渲染状态锁）────────────────────────────────────
    // 本阶段是全局锁序 SESSION_REGISTRY → Session → RENDER_STATE 中唯一反向的一处：
    // 其余入口取 RENDER_STATE 前必已释放前两级锁，故不构成环形等待。新增反向取锁的
    // 代码必须先释放注册表与会话锁（`setSelection`/`setTheme` 同款作用域写法）。
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return -1;
    };
    let lock_done = std::time::Instant::now();
    // 单点读清内容脏标志。
    let content_dirty = render_state.dirty.swap(false, Ordering::AcqRel);

    // 按新数据/空闲分支——空闲路径引用 `last_frame` 而不克隆，省去整帧克隆（CellData 96B×行列数）。
    let render_done;
    let count = match frame_data {
        FrameData::New {
            cells,
            cursor_info,
            rows,
            cols,
            scroll_offset,
        } => {
            // Kitty 同步：生成戳为 0 且无缓存时跳过查询（纯文本零开销）；
            // 会话/生成戳/滚动/网格任一变化才重查放置；图集仅在生成戳变化时
            // 打包重传，滚动/缩放只经无拷贝布局重建实例。
            // 全局锁序：渲染状态锁内绝不嵌套注册表与会话锁。会话查询在锁外完成
            // 后再重取渲染状态锁应用结果，消除渲染线程与主题线程的环形等待。
            let kitty_generation = cursor_info.kitty_generation;
            // 纯文本会话（生成戳为 0）不做逐格扫描：签名只服务于图像放置缓存。
            let placeholder_signature = if kitty_generation != 0 {
                placeholder_layout_signature(&cells)
            } else {
                0
            };
            // 网格单元格像素（与 render_cell_data 同口径：字体度量×光栅缩放）。
            let (font_width, font_height) = render_state.font_pipeline.cell_metrics();
            let raster_scale = render_state.font_pipeline.get_raster_scale();
            let grid_cell_width = font_width * raster_scale;
            let grid_cell_height = font_height * raster_scale;
            let kitty_keys = KittyPlacementKeys {
                session_id,
                generation: kitty_generation,
                scroll_offset,
                scrollback_length: cursor_info.scrollback_length,
                placeholder_signature,
                rows,
                cols,
            };
            let kitty_cell_changed = (grid_cell_width - render_state.kitty_cell_width).abs()
                > f32::EPSILON
                || (grid_cell_height - render_state.kitty_cell_height).abs() > f32::EPSILON;
            // 滚动进行中（视口键与上一帧不同）只平移已有放置帧；连续两帧视口键相同
            // 即视为停稳，才补一次真实查询。否则每一行滚动都要在渲染线程同步等一次
            // VT 查询（yazi 预览下实测约 60ms/行，滚动掉到 ~16fps）。
            let kitty_scroll = (scroll_offset, cursor_info.scrollback_length);
            let scroll_moving = kitty_scroll != render_state.last_kitty_scroll;
            render_state.last_kitty_scroll = kitty_scroll;
            // 失效规则集中在 `kitty_cache_invalidation`：放置键或单元格几何变化才重查，
            // 纯文本会话零开销跳过。
            let invalidation = kitty_cache_invalidation(
                kitty_keys,
                render_state.kitty_keys,
                kitty_cell_changed,
                !scroll_moving && render_state.kitty_frames_approximate,
            );
            // 单元格几何失步：终端侧仍为旧值时上游按旧几何重算像素尺寸，
            // 与新来源口径不一致。先同步新尺寸到终端再重查
            // （先排空命令积压再处理查询，命令先发即先生效，无竞态）。
            let need_cell_sync = invalidation.sync_cell_size;
            let need_kitty_fetch = invalidation.refetch_placements;
            let sync_cell_width = grid_cell_width.max(1.0) as u32;
            let sync_cell_height = grid_cell_height.max(1.0) as u32;
            drop(state);
            if need_cell_sync {
                let registry = rlock_session_registry();
                if let Some(entry) = registry.get(&session_id) {
                    entry
                        .session
                        .lock()
                        .terminal()
                        .set_cell_pixel_size(sync_cell_width, sync_cell_height);
                }
            }
            // 渲染状态锁与会话锁之外查询：仅放置键变化时触发；刚推送帧时通常
            // 毫秒级返回（卡住时由 500 毫秒超时兜底），期间其他渲染状态消费者
            // 不再排队，按帧取会话锁的写入路径也不再排队。
            let fetched_kitty_frames = if need_kitty_fetch {
                session_query_channel(session_id)
                    .map(|query_tx| GhosttyTerminal::take_kitty_placements_on(&query_tx))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut state = render_state_mut();
            let Some(render_state) = state.as_mut() else {
                log::error!("render: render state missing");
                return -1;
            };
            if render_state.renderer.surface.is_none() {
                return 0;
            }
            // 强制重查：旧帧像素尺寸按旧几何解算，必须按新几何重算。
            if invalidation.overlay_dirty {
                // 本帧刷新了 kitty 状态（图像增删或放置集合/几何变化）：上一帧留在
                // 累加器里的图像像素已失效，必须整帧重绘才能去掉残影。状态未变的
                // 帧不需要全量——叠加层每帧照画，文本层照旧走脏带。
                render_state.kitty_overlay_dirty = true;
                render_state.kitty_keys = kitty_keys;
                render_state.kitty_cell_width = grid_cell_width;
                render_state.kitty_cell_height = grid_cell_height;
                if kitty_generation == 0 {
                    render_state.kitty_frames.clear();
                    render_state.kitty_instances.clear();
                    render_state.kitty_frames_approximate = false;
                    if render_state.kitty_uploaded_generation != 0 {
                        render_state.renderer.set_kgp_atlas(&[], 0, 0);
                        render_state.kitty_uploaded_generation = 0;
                    }
                } else {
                    // 滚动中不查会话，只按视口顶行差量平移已有帧；帧坐标随之与当前
                    // 视口对齐，停稳后的那一次真实查询负责恢复被旧视口裁掉的边缘。
                    if invalidation.viewport_shift_rows != 0 {
                        let shift =
                            i32::try_from(invalidation.viewport_shift_rows).unwrap_or(i32::MAX);
                        for frame in &mut render_state.kitty_frames {
                            frame.viewport_row = frame.viewport_row.saturating_add(shift);
                        }
                        render_state.kitty_frames_approximate = true;
                    }
                    // 锁外已取回放置帧，此处仅应用，不再持渲染状态锁查询会话。
                    if invalidation.refetch_placements {
                        render_state.kitty_frames = fetched_kitty_frames;
                        render_state.kitty_frames_approximate = false;
                    }
                    if invalidation.repack_atlas {
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
            let previous_cursor_row = render_state.last_frame.as_ref().and_then(
                |(_, old_cursor_info, _, _, cached_session)| {
                    if *cached_session == session_id {
                        Some(old_cursor_info.row)
                    } else {
                        None
                    }
                },
            );
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            match &render_state.last_frame {
                Some((old_cells, _, old_rows, old_cols, cached_session))
                    if *cached_session == session_id && *old_rows == rows && *old_cols == cols =>
                {
                    crate::render::cell_builder::diff_dirty_rows_into(
                        old_cells,
                        &cells,
                        rows,
                        &mut dirty_mask[..rows_usize],
                    );
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
            mark_overlay_dirty_rows(
                dirty_mask,
                rows_usize,
                highlight_rows_to_mark(
                    &render_state.search_highlights.current,
                    &render_state.search_highlights.drawn,
                ),
            );
            let result = render_state.renderer.render_cell_data(
                &cells,
                rows,
                cols,
                cursor,
                &mut render_state.font_pipeline,
                &render_state.search_highlights.current,
                Some(&render_state.dirty_mask),
                &render_state.kitty_instances,
                render_state.kitty_overlay_dirty,
            );
            render_done = std::time::Instant::now();
            match result {
                Ok(()) => {
                    render_state.kitty_overlay_dirty = false;
                    render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
                    render_state.search_highlights.drawn =
                        render_state.search_highlights.current.clone();
                    render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
                    1
                }
                Err(error) => {
                    log::error!("render: frame failed: {error}");
                    // receive_cell_data() 是破坏性取数，VT 去重基线已推进，未呈现的帧
                    // 不会被 VT 补发。把本帧 cells 留在 last_frame，并置 dirty 确保
                    // 下一帧（Idle 或 New）重试，避免该帧内容永久丢失。
                    render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
                    render_state.dirty.store(true, Ordering::Release);
                    -1
                }
            }
        }
        FrameData::Idle {} => {
            // 空闲路径：引用 `last_frame`——零克隆。会话不一致时不重绘，避免把旧会话画到新会话。
            let Some((ref cached_cells, cached_cursor, cached_rows, cached_cols, cached_session)) =
                render_state.last_frame
            else {
                return 0;
            };
            if cached_session != session_id {
                return 0;
            }
            let cursor = build_cursor(render_state, &cached_cursor);
            // 空闲重绘门控：仅当确有变化时重绘——搜索高亮、滚动偏移、内容脏标志被置位，
            // 或累加器失效（surface 重挂载/resize：新交换链从未收到过帧，空闲 shell
            // 会永远黑屏）。选区经 VT 线程作为新单元内容到达，故无需自己的门控。
            let highlights_changed =
                render_state.search_highlights.current != render_state.search_highlights.drawn;
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
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            // 光标行无论可见性都标脏：光标是叠加层，其像素变化不触及单元内容。
            if (cursor.row as usize) < rows_usize {
                dirty_mask[cursor.row as usize] = true;
            }
            mark_overlay_dirty_rows(
                dirty_mask,
                rows_usize,
                highlight_rows_to_mark(
                    &render_state.search_highlights.current,
                    &render_state.search_highlights.drawn,
                ),
            );
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
                &render_state.search_highlights.current,
                Some(&render_state.dirty_mask),
                &render_state.kitty_instances,
                render_state.kitty_overlay_dirty,
            );
            render_done = std::time::Instant::now();
            if result.is_ok() {
                render_state.kitty_overlay_dirty = false;
                render_state.search_highlights.drawn =
                    render_state.search_highlights.current.clone();
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
    };
    let total = render_done.duration_since(render_start);
    if total >= std::time::Duration::from_millis(30) {
        let ms = |duration: std::time::Duration| duration.as_secs_f64() * 1000.0;
        log::warn!(
            "SLOW_RENDER_INNER phase1={:.3}ms receive={:.3}ms lock={:.3}ms render={:.3}ms total={:.3}ms count={count}",
            ms(phase1_done.duration_since(render_start)),
            ms(phase2_done.duration_since(phase1_done)),
            ms(lock_done.duration_since(phase2_done)),
            ms(render_done.duration_since(lock_done)),
            ms(total),
        );
    }
    count
}

// ══════════════════════════════════════════════════════════════════════════
/// 「视口全空」哨兵：10 位全 1 模式。
const LAST_CONTENT_ROW_NONE_BITS: i64 = 0x3FF;
/// 打包位偏移：内容下沿 43（10 位）、surface 失效位 53、备用屏 54。
const SURFACE_INVALIDATED_BIT: i64 = 1 << 53;
/// 备用屏（helix/vim 等全屏 TUI）激活标志。随每帧渲染结果上报，与内容下沿同批采样，
/// 宿主据此让输入法跟随位移归零。
const ALT_SCREEN_ACTIVE_BIT: i64 = 1 << 54;
/// 采样段 panic 时的上报值：渲染计数为负（-1）、`new_output` 为 0、内容下沿取
/// 未知哨兵、失效位为 0——与本导出文档声明的出错位形一致。
const RENDER_SAMPLE_FAILURE_BITS: i64 = 0xFFFF_FFFF | (LAST_CONTENT_ROW_NONE_BITS << 43);

/// 把一帧的采样拼成上报位。
///
/// 抽成纯函数是为了让「保留位 33..42 MUST 为 0」这条不变量可测：直接在导出里写
/// `| (a << 32) | (b << 43) | …` 时，任何人加进一个新字段都可能悄悄落进保留位，
/// 而 Kotlin 侧不读那 10 位，于是没有任何一侧的测试会红。
#[must_use]
fn pack_render_sample(
    count: i64,
    new_output: i64,
    content_row_bits: i64,
    surface_invalidated: i64,
    alt_screen_active: i64,
) -> i64 {
    count | (new_output << 32) | (content_row_bits << 43) | surface_invalidated | alt_screen_active
}

/// 把已渲染帧缓存的单元数据映射为上报位：视口全空时回未知哨兵，否则取视口内
/// 最后一个有内容的行（`cell_builder::last_content_row`）。
fn last_content_row_bits_for_frame(
    cells: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> i64 {
    // 超出 10 位即按哨兵处理（而非截断）：截断会把 1024 报成 0、1025 报成 1，
    // 让 Kotlin 按一个**貌似合理**的错误行号去算位移；哨兵则让调用方保守地不位移。
    match crate::render::cell_builder::last_content_row(cells, rows) {
        Some(row) if (row as i64) < LAST_CONTENT_ROW_NONE_BITS => row as i64,
        _ => LAST_CONTENT_ROW_NONE_BITS,
    }
}

/// 可见 Unicode 占位符的布局签名。
///
/// 虚拟放置的锚点由占位符单元反推；图像生成戳不变时，TUI 重绘仍可能把
/// 同一幅图挪到新位置。对全部占位符位置做滚动哈希即可感知任意移动，又不必
/// 每帧重查并重解码整幅 RGBA。
fn placeholder_layout_signature(cells: &[crate::terminal::ghostty_terminal::CellData]) -> u64 {
    let mut signature = 0xcbf2_9ce4_8422_2325u64;
    let mut count = 0u64;
    for cell in cells {
        if cell.codepoint != crate::terminal::ghostty_terminal::unicode_placement::PLACEHOLDER {
            continue;
        }
        signature ^= (u64::from(cell.row) << 32) | u64::from(cell.col);
        signature = signature.wrapping_mul(0x0000_0100_0000_01b3);
        count += 1;
    }
    if count == 0 { 0 } else { signature ^ count }
}

// JNI 导出：renderWithNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// 合并的 render + consumeNewOutput：单次 JNI 穿越即完成渲染一帧**与**读取每会话的
/// `new_output` 标志，比分开两次调用每帧省约 0.1-0.3ms。
///
/// 返回打包的 `jlong`：位 0..31 = 渲染计数（语义同 `render()`）；位 32 = `new_output`
/// 标志（1 = 已摄入 PTY 输出，0 = 空闲）；位 33..42 = **保留，必须为 0**（历史光标行
/// 位形：该字段已无消费方，随输入法位移改按内容下沿裁剪而删除）；位 43..52 = 视口最后
/// 一个有内容的行（0x3FF = 视口全空）；位 53 = surface 已判死（缓存的 `ANativeWindow`
/// 对应被遗弃的 BufferQueue，需宿主换新的 `ANativeWindow` 才能恢复；重建成功即回落
/// 为 0）；位 54 = 备用屏激活（全屏 TUI）。
///
/// 行字段占 10 位：Android 网格行数上限远小于 1024（最小字号行高 ≥18px、
/// 屏幕高 ≤4096px ⇒ ≤227 行），超出即按哨兵处理，内容未知退化为不位移，是保守方向。
///
/// 内容下沿与失效位的采样刻意**不**挂在渲染计数门下：空闲帧 `render_inner`
/// 返回 0（无新单元数据，无需 GPU 呈现——正确），但 IME 跟随平移恰在空闲定居后最需要
/// 这些坐标；而失效位只在失败帧翻转，空闲门控会把它永远压在 0。采样复用本帧已渲染缓存，
/// 不新增发往 VT 线程的同步查询，故空闲帧无阻塞风险。
///
/// Kotlin 必须分别掩码每个字段：裸读 `(packed shr 32) != 0` 会把更高的位误当作输出。
/// 出错时渲染计数为负、`new_output` 为 0、内容下沿为 0x3FF、失效位为 0。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_renderWithNewOutput<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jlong {
    let jni_start = std::time::Instant::now();
    let count = jni_export_guard!(&mut unowned_env, -1i32, |_env| render_inner(
        session_id as u64
    ));
    let inner_done = std::time::Instant::now();
    // 三个采样**必须**在计数门控之外：死 surface 的每一帧都以 -1 失败，把它们挂在
    // `count >= 0` 门下会让失效位永远送不到宿主（宿主因此永不换窗口，终端永久黑屏）。
    // 空闲帧（count == 0）同样要采样：空闲时无新单元数据、无需 GPU 呈现，但 IME 位移
    // 恰在空闲定居后最需要这些坐标。这里不调用 `render_cursor()`：它要走 VT 线程同步
    // 查询，缓存才是无阻塞且与绘制同源的坐标。
    //
    // 整段（连同 `new_output` 的消费）必须同在**一个**守卫内：守卫之间留任何一行裸
    // 代码，就等于把一次 panic 放回 `extern "system"` 边界外——那会 abort 进程而非抛
    // Java 异常。守卫内 `render_state_mut()` 在 `RENDER_STATE` 仍为 `None` 时会建渲染器
    // 并于 GPU 初始化失败时 panic，而 `render_inner` 刚失败（count == -1）的正是同一条
    // 路径。
    jni_export_guard!(&mut unowned_env, RENDER_SAMPLE_FAILURE_BITS, |_env| {
        // 就地消费 `new_output` 标志，省一次 JNI 穿越，
        // 并读取本帧已渲染缓存的光标与内容下沿，使跟随输入法的位移看到本帧绘制的坐标；
        // 同时上报渲染器判死的 surface，让宿主换新的 `ANativeWindow`（死窗口的
        // BufferQueue 不可 reconfigure 复活，实测会永久黑屏）。
        // `new_output` 是破坏性消费：只在计数非负（帧未被判失败）时取，未呈现的新数据
        // 必须留在通道里等下一帧，否则失败帧会吞掉它。
        // 会话锁序：注册表读锁 → 会话锁 → RENDER_STATE（`setSelection` 同款，
        // 不可跨作用域持有）。
        //
        // `new_output` 与备用屏标志都从注册表的共享原子量读取，**不取会话锁**：
        // 滚动期间 PTY 洪流会把会话锁按住数十毫秒，渲染线程若在此排队，Kotlin
        // 侧实测每帧 55-60ms（原生 render 本身 <30ms，故 SLOW_RENDER_INNER 不触发）。
        // `new_output` 仍是读后即清；失败帧（count < 0）不消费，留给下一帧。
        let (new_output, alt_screen_active) = {
            let registry = rlock_session_registry();
            match registry.get(&(session_id as u64)) {
                Some(entry) => {
                    let new_output = count >= 0 && entry.new_output.swap(false, Ordering::AcqRel);
                    let alt_screen_active = entry.alt_screen_active.load(Ordering::Acquire);
                    (
                        i64::from(new_output),
                        i64::from(alt_screen_active) * ALT_SCREEN_ACTIVE_BIT,
                    )
                }
                None => (0, 0),
            }
        };
        let render_state = render_state_mut();
        let last_frame = render_state
            .as_ref()
            .and_then(|render_state| render_state.last_frame.as_ref())
            .filter(|(.., cached_session)| *cached_session == session_id as u64);
        let content_row_bits = match last_frame {
            Some((cells, _, rows, _, _)) => last_content_row_bits_for_frame(cells, *rows),
            None => LAST_CONTENT_ROW_NONE_BITS,
        };
        let surface_invalidated = i64::from(
            render_state
                .as_ref()
                .is_some_and(|render_state| render_state.renderer.surface_invalidated()),
        ) * SURFACE_INVALIDATED_BIT;
        // 原生 render 本身未达 30ms（SLOW_RENDER_INNER 不触发）而整个 JNI 穿越
        // 超过 30ms：差额只可能落在渲染后的采样段（注册表/会话锁或 RENDER_STATE）。
        // 这条日志把「Kotlin 看到 58ms、原生 render 日志却沉默」的盲区钉住。
        let inner_ms = inner_done.duration_since(jni_start).as_secs_f64() * 1000.0;
        let sample_ms = inner_done.elapsed().as_secs_f64() * 1000.0;
        if inner_ms < 30.0 && inner_ms + sample_ms >= 30.0 {
            log::warn!(
                "SLOW_RENDER_SAMPLE inner={inner_ms:.3}ms sample={sample_ms:.3}ms count={count}"
            );
        }
        pack_render_sample(
            count as i64 & 0xFFFF_FFFF,
            new_output,
            content_row_bits,
            surface_invalidated,
            alt_screen_active,
        )
    })
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

    // 先取出并立即释放 guard：`try_to_string` 可能触发 JVM GC safepoint，
    // guard 若跨过 JNI 调用会把锁持有时间拉长。
    let tx = REQUEST_REGISTRY.lock().remove(&(session_id, request_id));
    if let Some(tx) = tx {
        // null 是宿主读取失败，解码失败同理：都不能退化成空串，
        // 否则远端把「读不到剪贴板」当成「用户清空了剪贴板」（见 wait_for_clipboard_answer）。
        let answer = if text.is_null() {
            None
        } else {
            match text.try_to_string(env) {
                Ok(decoded) => Some(decoded),
                Err(error) => {
                    log::error!("osc52: clipboard answer decode failed: {error}");
                    None
                }
            }
        };
        let _ = tx.send(answer);
    }
}

// ══════════════════════════════════════════════════════════════════════════
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

fn get_session_titles_inner<'local>(
    env: &mut Env<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    // SAFETY：`session_ids` 是本次 JNI 调用的实参，生命周期覆盖本函数。
    let ids = unsafe { JLongArray::from_raw(env, session_ids) };
    let Ok(length) = ids.len(env) else {
        log::error!("getSessionTitles: cannot read session id array length");
        return std::ptr::null_mut();
    };
    let mut session_id_values = vec![0 as jlong; length];
    if ids.get_region(env, 0, &mut session_id_values).is_err() {
        log::error!("getSessionTitles: cannot read session ids");
        return std::ptr::null_mut();
    }
    let Ok(string_class) = env.find_class(jni_str!("java/lang/String")) else {
        log::error!("getSessionTitles: find_class java/lang/String failed");
        return std::ptr::null_mut();
    };
    let Ok(array) = env.new_object_array(
        session_id_values.len() as jsize,
        string_class,
        JObject::null(),
    ) else {
        log::error!("getSessionTitles: new_object_array failed");
        return std::ptr::null_mut();
    };
    for (index, id) in session_id_values.iter().enumerate() {
        // 未知会话留 null：调用方按空标题处理，不抛异常。
        let element: JObject = match session_title(*id as u64) {
            Some(title) => match env.new_string(title) {
                Ok(text) => text.into(),
                Err(error) => {
                    log::error!("getSessionTitles: cannot encode title for {id}: {error}");
                    return std::ptr::null_mut();
                }
            },
            None => JObject::null(),
        };
        if let Err(error) = array.set_element(env, index, &element) {
            log::error!("getSessionTitles: cannot store title {index}: {error}");
            return std::ptr::null_mut();
        }
    }
    array.into_raw()
}

/// Rust 字符串 → JNI `jstring`：编码失败返回 null，不抛异常、不吞错（调用方按 null 处理）。
fn string_to_jstring(env: &mut Env, text: &str) -> jstring {
    match env.new_string(text) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 会话标识列表 → JNI `LongArray`；注册表为空时返回空数组。
fn list_sessions_inner<'local>(env: &mut Env<'local>, _class: JClass<'local>) -> jlongArray {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();
    u64_vec_to_long_array(env, &ids)
}

/// 字体管线默认的图集单元尺寸（像素）。
const DEFAULT_FONT_CELL_SIZE: f32 = 14.0;

/// 字形管线初始化用的图集尺寸。UV 归一化的尺寸由管线自身持有并对外提供，
/// 渲染侧不再并传第二份，故此处是唯一声明点。
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
// 注册表读锁 → 会话锁 → 引擎查询；会话 id 未知时抛 IllegalStateException。
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
fn session_title(session_id: u64) -> Option<String> {
    let query_tx = session_query_channel(session_id)?;
    Some(crate::terminal::ghostty_terminal::GhosttyTerminal::title_on(&query_tx))
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
        // 通道外置：查询在两把锁之外执行（理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "scrollbackLength");
            return Ok(0);
        };
        crate::terminal::ghostty_terminal::GhosttyTerminal::scrollback_length_on(&query_tx) as jint
    })
}

/// 返回终端光标的视口位置，打包为 `(y << 32) | x`；光标隐藏或在视口外时返回 -1。
///
/// **当前真值、同步查询**：`Query::RenderCursor` → `cursor_in_viewport`，取值即 VT
/// 快照此刻的光标格，代价是一次 VT 线程往返（忙时上限两次查询超时）。正确性断言
/// 要「刚写进去的数据立刻反映出来」时用本方法；逐帧/逐键的热路径 MUST 改用
/// [`Java_terminal_emulator_bridge_NativeBridge_getRenderedCursorPacked`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCursorViewportPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, -1, |env| {
        // 通道外置：光标坐标取自整网格构建，锁内等待会把按帧取这两把锁的渲染线程
        // 一并堵住（理由同 `getTitle`，此处代价更高）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getCursorViewportPacked");
            return Ok(-1);
        };
        let Some((row, col)) =
            crate::terminal::ghostty_terminal::GhosttyTerminal::render_cursor_on(&query_tx)
        else {
            return Ok(-1);
        };
        ((row as jlong) << 32) | (col as jlong)
    })
}

/// 返回**最近一帧**渲染线程取走的光标视口位置，打包同 `getCursorViewportPacked`；
/// 首帧被取走之前（或该帧光标不可见）返回 -1。
///
/// 与 `getCursorViewportPacked` 同口径（同出 `cursor_in_viewport`）而不同时效：这里读
/// 的是已消费帧的光标，代价是一次原子读——不发查询、不取会话锁、不等待 VT 线程。
/// 逐键发生的输入法锚点上报取本方法；需要「刚写入的数据立刻反映」的断言取前者。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRenderedCursorPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, RENDERED_CURSOR_NONE, |env| {
        let Some(packed) = rendered_cursor_packed(session_id as u64) else {
            throw_session_missing(env, "getRenderedCursorPacked");
            return Ok(RENDERED_CURSOR_NONE);
        };
        packed
    })
}

/// 最近一帧的回滚长度（0 起）。首帧尚未取走时为 -1；未知会话抛异常（与光标载体一致，
/// 让调用方能区分「会话没了」与「暂时没有帧」）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRenderedScrollbackLength(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, RENDERED_SCROLLBACK_NONE, |env| {
        let Some(length) = rendered_scrollback(session_id as u64) else {
            throw_session_missing(env, "getRenderedScrollbackLength");
            return Ok(RENDERED_SCROLLBACK_NONE);
        };
        length
    })
}

/// 会话**已应用**的滚动偏移（0 = 视口贴底）。
///
/// 与 `TerminalRuntime.activeSessionScrollOffset()` 的区别是「已应用」：后者是渲染线程
/// 尚未消费的命令槽，读到的是转瞬即逝的中间态。仪器测试要断言「视口归位」时读这里——
/// 一次原子读，不写终端、不发 VT 查询（旧做法向网格注入标记行，会把真实内容挤出
/// 无回滚的视口，连带把提示符丢出屏幕）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionScrollOffset(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        session_handle(session_id as u64).map_or(0, |session| {
            i32::try_from(session.lock().last_scroll_offset()).unwrap_or(i32::MAX)
        })
    })
}

/// 返回单个回滚行去除首尾空白后的文本，空行返回 null。
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
        let Ok(row) = u32::try_from(row) else {
            // 负行号表示无数据而非调用错误：视口拖过回滚顶部时正常出现，
            // 按查询契约返回 null，不抛异常。
            return Ok(std::ptr::null_mut());
        };
        // 通道外置：查询在两把锁之外执行（滚动时逐行拉取，理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "scrollbackLine");
            return Ok(std::ptr::null_mut());
        };
        // Kotlin 传入的是绝对行号（回滚 + 视口偏移，经
        // `scrollbackLength - scrollOffset + row` 计算），故直接透传给期望绝对行号的
        // `read_line_text`。
        let text =
            crate::terminal::ghostty_terminal::GhosttyTerminal::read_line_text_on(&query_tx, row);
        match text {
            Some(text) => string_to_jstring(env, &text),
            None => std::ptr::null_mut(),
        }
    })
}

/// 返回可见区与回滚区以换行拼接的文本。
///
/// 只需字符，故走 `read_all_text`（每格一次 codepoint 读取）而非 `dump_grid`：
/// 后者每格还要取样式、解析颜色与 flags，回滚区填满时（上限 2000 行）单次查询
/// 会把 VT 线程按在数十万次跨语言调用上，挤占命令通道——测试与 UI 的查询突发
/// 随即双双超时回退，表现为「标记不落格」与渲染掉帧。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTerminalText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 通道外置：全回滚扫描最慢，必须在两把锁之外跑，否则期间渲染与事件
        // 收割全部停摆（该导出每次全选/复制都调用）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getTerminalText");
            return Ok(std::ptr::null_mut());
        };
        let text = crate::terminal::ghostty_terminal::GhosttyTerminal::read_all_text_on(&query_tx);
        string_to_jstring(env, &text)
    })
}

/// 返回当前**可见视口**的文本（每行以换行拼接，行数 == 网格行数）。
///
/// 与 `getTerminalText`（视口 + 全回滚）相对：调用方要判断「某行是否看得见」时
/// 用它，代价随网格面积而非回滚深度走。仪器测试以此确认视口确实停在底部。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getVisibleText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getVisibleText");
            return Ok(std::ptr::null_mut());
        };
        let text =
            crate::terminal::ghostty_terminal::GhosttyTerminal::read_visible_text_on(&query_tx);
        string_to_jstring(env, &text)
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
        // 通道外置：格式化取文在两把锁之外跑（复制动作，期间渲染不该停摆）。
        let Some(query_tx) = session_query_channel(id) else {
            return Ok(std::ptr::null_mut());
        };
        let text = crate::terminal::ghostty_terminal::GhosttyTerminal::selection_text_on(
            &query_tx,
            (start_row.max(0) as u32, start_col.max(0) as u32),
            (end_row.max(0) as u32, end_col.max(0) as u32),
        );
        string_to_jstring(env, &text)
    })
}

/// `jint` 切片 → JNI `IntArray`；分配失败返回 null。
fn jint_slice_to_int_array(env: &mut Env, values: &[jint]) -> jintArray {
    let Ok(array) = env.new_int_array(values.len()) else {
        return std::ptr::null_mut();
    };
    if array.set_region(env, 0, values).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

/// 会话标识 → JNI `LongArray`（空表即无会话）；分配失败返回 null。
fn u64_vec_to_long_array(env: &mut Env, values: &[u64]) -> jlongArray {
    let wide: Vec<jlong> = values.iter().map(|value| *value as jlong).collect();
    let Ok(array) = env.new_long_array(wide.len()) else {
        return std::ptr::null_mut();
    };
    if array.set_region(env, 0, &wide).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
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
    jint_slice_to_int_array(env, &values)
}

/// `u32` 列号序列 → JNI `IntArray`（宽字符尾格查询回传）；分配失败返回 null。
fn u32_vec_to_int_array(env: &mut Env, values: &[u32]) -> jintArray {
    let clamped: Vec<jint> = values
        .iter()
        .map(|value| (*value).min(jint::MAX as u32) as jint)
        .collect();
    jint_slice_to_int_array(env, &clamped)
}

/// 以落点为锚在终端安装选区并回传界限。会话锁与注册表读锁都在构造界限
/// 后立即释放：选区查询走 VT 线程，绝不能持锁跨越。
fn select_bounds_export<'local>(
    mut unowned_env: EnvUnowned<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
    // 通道外置：选区派生在两把锁之外跑（理由同 `getTitle`）。
    select: impl FnOnce(
        &flume::Sender<crate::terminal::ghostty_terminal::Query>,
        u32,
        u32,
    ) -> Option<((u32, u32), (u32, u32))>,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let bounds = select(&query_tx, row.max(0) as u32, col.max(0) as u32);
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
    select_bounds_export(unowned_env, session_id, row, col, |query_tx, row, col| {
        GhosttyTerminal::select_word_at_on(query_tx, row, col)
    })
}

/// 该行中作为宽字符后半格（`SpacerTail`）的列号，升序。
///
/// 会话不存在或查询失败时返回空数组（等价于无吸附）：保持原列，不猜。会话锁与
/// 通道外置：查询在两把锁之外执行（每次拖动手柄都调用，锁内等 VT 应答会连同
/// 按帧取锁的渲染一起卡住）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_wideCharTailCols<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(u32_vec_to_int_array(env, &[]));
        };
        let tail_cols = crate::terminal::ghostty_terminal::GhosttyTerminal::wide_char_tail_cols_on(
            &query_tx,
            row.max(0) as u32,
        );
        u32_vec_to_int_array(env, &tail_cols)
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
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let bounds = crate::terminal::ghostty_terminal::GhosttyTerminal::select_all_on(&query_tx);
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
        // 通道外置：查询在两把锁之外执行（长按取链接时调用，理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let url = GhosttyTerminal::hyperlink_at_on(&query_tx, row.max(0) as u32, col.max(0) as u32);
        match url {
            Some(url) => string_to_jstring(env, &url),
            None => std::ptr::null_mut(),
        }
    })
}

// ── 搜索与选择 ──────────────────────────────────────────────────
/// 设置搜索 needle 并同步跑完搜索，返回命中总数。会话不存在、查询超时或断连时返回 0。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchSetNeedle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    needle: JString<'local>,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |env| {
        let id = session_id as u64;
        let Some(needle) = read_jni_string(env, &needle, "searchSetNeedle", "needle") else {
            return Ok(0);
        };
        let Some(query_tx) = session_query_channel(id) else {
            throw_session_missing(env, "searchSetNeedle");
            return Ok(0);
        };
        let total = crate::terminal::ghostty_terminal::GhosttyTerminal::search_set_needle_on(
            &query_tx, &needle,
        );
        log::info!("searchSetNeedle: needle={needle:?} matches={total}");
        total as jint
    })
}

/// 选中下一个（`forward = true`）或上一个命中，返回 `[序号, 总数, 行, 起始列, 结束列]`；
/// 无命中、会话不存在、查询超时或断连时返回 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchSelect<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    forward: jboolean,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let Some(query_tx) = session_query_channel(id) else {
            throw_session_missing(env, "searchSelect");
            return Ok(std::ptr::null_mut());
        };
        let Some(selected) = crate::terminal::ghostty_terminal::GhosttyTerminal::search_select_on(
            &query_tx, forward,
        ) else {
            return Ok(std::ptr::null_mut());
        };
        let values = vec![
            selected.index as jint,
            selected.total as jint,
            selected.matched.row.min(jint::MAX as u32) as jint,
            selected.matched.start_col.min(jint::MAX as u32) as jint,
            selected.matched.end_col.min(jint::MAX as u32) as jint,
        ];
        jint_slice_to_int_array(env, &values)
    })
}

/// 取会话句柄。注册表读锁与**会话锁**都在此释放：两者都可能被 PTY 写入按住数秒
/// （`feed_pty` 的写入面正是这样克隆出去再写），持锁跨越会让销毁会话的写锁饥饿，
/// 渲染线程更不该在注册表**写**锁内排队——只读查找一律走本函数。
fn session_handle(session_id: u64) -> Option<Arc<Mutex<Session>>> {
    rlock_session_registry()
        .get(&session_id)
        .map(|entry| entry.session.clone())
}

/// [`Self::session_handle`] 的查询通道版。查询经 500 毫秒有界超时，VT 查询本身
/// 在会话锁外跑。会话不存在时返回 None，由调用方抛出。
fn session_query_channel(
    session_id: u64,
) -> Option<flume::Sender<crate::terminal::ghostty_terminal::Query>> {
    Some(
        session_handle(session_id)?
            .lock()
            .terminal()
            .query_channel(),
    )
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
        // 元素初值为 null：任一 `new_string` / `set_element` 失败都会留下 null，
        // 而 Kotlin 侧声明是 `Array<String>`，null 元素会在字体选择器里变成 NPE。
        // 故失败即整体作废，由 Kotlin 侧按「无数据」处理。
        for (index, family) in families.iter().enumerate() {
            let Ok(element) = env.new_string(family) else {
                log::error!("listFontFamilies: cannot encode family {family:?}, aborting list");
                return Ok(std::ptr::null_mut());
            };
            if let Err(error) = array.set_element(env, index, &element) {
                log::error!(
                    "listFontFamilies: cannot store family {family:?}, aborting list: {error}"
                );
                return Ok(std::ptr::null_mut());
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
        drop(state);
        string_to_jstring(env, &name)
    })
}

/// 搜索查询串长度上限（原生为唯一真源，UI 侧据此截断输入）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchQueryMaxChars<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jint {
    jni_export_guard!(unowned_env, -1, |_env| {
        GhosttyTerminal::MAX_SEARCH_QUERY_CHARS as jint
    })
}

/// 主题目录与全部调色板的一次性打包（布局见 [`crate::terminal::theme::pack_catalog`]）。
///
/// 一次同步调用取代「逐个标识取调色板」的 `1 + N` 次往返：目录在构建期即固定，
/// 进程内不变，而设置页要一次显示全部主题。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getThemeCatalog(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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

/// 返回字体信息的打包字节（布局见本文件的 `pack_font_info`）。渲染状态尚未初始化时为 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getFontInfo(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(std::ptr::null_mut());
        };
        let packed = pack_font_info(&render_state.font_pipeline.font_info());
        drop(state);
        match env.byte_array_from_slice(&packed) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("getFontInfo: byte_array_from_slice failed: {error}");
                std::ptr::null_mut()
            }
        }
    })
}

/// 字体信息的打包头部字节数：标志 4 + 三个 `f32` 12 + 名称数 2 + CJK 名称数 2。
const FONT_INFO_HEADER_BYTES: usize = 20;

/// 字体信息的跨语言打包布局（Kotlin 侧对应 `FontInfo` 的解码）。
///
/// ```text
/// u32  flags         bit0 主字体等宽；CJK 状态取判别序号（见 CjkFallbackState）
/// f32 ×3             单元格宽高（物理像素）、逻辑字号（sp）
/// u16  名称个数       主字体 + CJK 回退族 + 其他被使用族
/// u16  CJK 名称数     紧随其后的一段是 CJK 回退族，其余为其他被使用族
/// 名称 × n            UTF-8、NUL 分隔，先主字体族名（无主字体则空串），
///                     再 CJK 回退族名，最后其他被使用族名
/// ```
pub(crate) fn pack_font_info(info: &crate::render::font::pipeline::FontInfo) -> Vec<u8> {
    // CJK 状态占 bit1 起（bit0 留给等宽标记），与 Kotlin 的 `CjkState.entries[flags ushr 1]` 对齐。
    let mut flags = (info.cjk_state as u32) << 1;
    if info.active.as_ref().is_some_and(|active| active.monospaced) {
        flags |= 1;
    }
    let mut names = info.cjk_families.clone();
    names.extend(info.other_families.iter().cloned());
    names.insert(
        0,
        info.active
            .as_ref()
            .map_or_else(String::new, |active| active.name.clone()),
    );
    let mut packed = Vec::with_capacity(
        FONT_INFO_HEADER_BYTES + names.iter().map(|name| name.len() + 1).sum::<usize>(),
    );
    packed.extend_from_slice(&flags.to_le_bytes());
    for value in [info.cell_width_px, info.cell_height_px, info.font_size] {
        packed.extend_from_slice(&value.to_le_bytes());
    }
    // 计数用 u16 承载：族名数量一旦超出就明确报错，不静默回绕
    // （回绕会让 Kotlin 侧的「名称数与实际个数相符」校验抛异常，
    //  表现为设置页字体信息崩溃）。panic 由 JNI 守卫转成 Java 异常。
    let name_count =
        u16::try_from(names.len()).expect("font family count exceeds the u16 wire field");
    let cjk_count = u16::try_from(info.cjk_families.len())
        .expect("cjk family count exceeds the u16 wire field");
    packed.extend_from_slice(&name_count.to_le_bytes());
    packed.extend_from_slice(&cjk_count.to_le_bytes());
    for name in &names {
        packed.extend_from_slice(name.as_bytes());
        packed.push(0);
    }
    packed
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
            render_state.search_highlights.current.clear();
            // 不动 `session`：清空 `current` 已足以让本会话不再有高亮，而改键会让下一帧
            // 走进 `enter_session` 的换会话分支——那里保留 `drawn` 正是为了把旧高亮行
            // 再标脏一次，键一改这条补救路径就白走一趟。
            // 置脏标志：高亮清除必须送达屏幕，即便终端处于空闲
            // （否则陈旧高亮会一直残留到下次 PTY 输出——正是 #5 这类回归）。
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 把当前搜索的视口命中写入渲染态，供下一帧绘制。
///
/// 命中与选中态取自上游 `search::Search`（VT 线程内），Kotlin 只提供视口滚动偏移与
/// 两个主题色：视口折算口径 `gridRow = 绝对行 - (回滚长度 - 滚动偏移)`，`scrollOffset`
/// 为 0 表示贴底。落在视口外的命中不写入——高亮只画在可见格上。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_applySearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    scroll_offset: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let query_tx = match session_query_channel(session_id as u64) {
            Some(query_tx) => query_tx,
            None => {
                log::error!("applySearchHighlights: session {session_id} not found");
                return Ok(());
            }
        };
        let highlights =
            crate::terminal::ghostty_terminal::GhosttyTerminal::search_viewport_highlights_on(
                &query_tx,
            );
        // 视口顶行的绝对行号。`scroll_offset` 与 VT 视口是同一来源（宿主统一滚动），
        // 上游默认的 `select_next` 自动滚动已关闭，二者不会各走各的。
        let viewport_top = highlights.scrollback_lines as i32 - scroll_offset;
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        // 上游 `viewport_matches` 会按页多给一些视口外的命中（文档明确说明），
        // 折算后必须两侧都裁：只裁负行会让视口下方的命中进入高亮列表，
        // 一旦偏移换算有误差就画到无关行上。
        let grid_rows = render_state
            .last_frame
            .as_ref()
            .map_or(i32::MAX, |(_, _, rows, _, _)| *rows as i32);
        // 当前选中命中用主题前景色，其余用主题选区色：颜色真源是渲染态持有的
        // 当前主题（见 `setTheme`），Kotlin 不参与。
        let palette = &render_state.theme.palette;
        render_state.search_highlights.current = highlights
            .matches
            .iter()
            .filter_map(|matched| {
                let row = matched.row as i32 - viewport_top;
                if row < 0 || row >= grid_rows {
                    return None;
                }
                let color = if highlights.selected.as_ref() == Some(matched) {
                    let current = palette.foreground;
                    [
                        current.r,
                        current.g,
                        current.b,
                        SEARCH_HIGHLIGHT_CURRENT_ALPHA,
                    ]
                } else {
                    let other = palette.selection_background;
                    [other.r, other.g, other.b, SEARCH_HIGHLIGHT_OTHER_ALPHA]
                };
                Some(SearchHighlight {
                    row,
                    start_col: matched.start_col as i32,
                    end_col_exclusive: matched.end_col as i32,
                    color,
                })
            })
            .collect();
        render_state.search_highlights.session = session_id as u64;
        // 置脏标志：无 PTY 流量时新高亮也必须绘制到下一帧
        //（#5 空闲屏回归）。
        render_state.dirty.store(true, Ordering::Relaxed);
    })
}

/// 设置下一渲染帧的活动文本选区。
///
/// 终端持有选区安装口：坐标为绝对网格行（row 0 = 回滚顶部），与控制柄逻辑一致。
/// 设置当前文本选区（绝对网格行列，0 = 回滚顶部）；高亮颜色由终端侧按主题调色板烘焙，本通道不传颜色参数。
/// 选区经 `GhosttyTerminal::set_selection` 安装为终端状态（跟踪引用，随滚动/
/// 输出/重排跟随文本），高亮由 VT 线程按行级选区反白直接烘焙进 CellData；
/// 本层不再存储单元格位置，仅透传。`hasSelection=false` 清除选区。
/// 选区恒为线性（块选通道已随 mode 一并移除）。
///
/// 返回 true 表示本次写入已生效（或无需写入）。会话锁被 PTY 写入按住时返回 false 且
/// **不阻塞**：调用方（渲染循环）保留上一次已提交的值，下一帧重试。拖动选区时每帧都会
/// 调用本函数，阻塞在这里等于把每帧拖到 PTY 输出的时长（真机实测单帧 100ms+）。
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
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_TRUE, |_env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(JNI_TRUE);
        };
        let Some(session) = entry.session.try_lock() else {
            return Ok(JNI_FALSE);
        };
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
        JNI_TRUE
    })
}

/// 按标识应用主题：终端默认色与调色板进 VT 线程，光标色与搜索高亮色供渲染态取用。
/// 标识未知即抛异常——主题真源在 [`crate::terminal::theme`]，Kotlin 无从构造清单外的主题。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setTheme(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    theme_name: JString,
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
        let background = entry.palette.background;
        {
            let registry = rlock_session_registry();
            let Some(session_entry) = registry.get(&id) else {
                throw_session_missing(env, "setTheme");
                return Ok(());
            };
            let session = session_entry.session.lock();
            session.terminal().set_theme(entry);
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
            render_state.theme = entry;
            render_state.renderer.set_background_color(background);
            // 主题色存于实例缓存，闲时无新 CellData 则颜色过期，一并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
        drop(state);
        log::info!("setTheme: session {id} theme={name}");
    })
}

/// 开启/关闭持续的 surface 级取纹理失败（见 `Renderer::set_surface_loss_injected_for_test`）。
///
/// 仅供仪器化用例复刻「BufferQueue 被遗弃」以验证自愈：无生产调用方；关闭后立刻
/// 恢复真实取纹理。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSurfaceLossInjected<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    injected: jboolean,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, false, |_env| {
        let mut state = render_state_mut();
        match state.as_mut() {
            Some(render_state) => {
                render_state
                    .renderer
                    .set_surface_loss_injected_for_test(injected);
                true
            }
            None => false,
        }
    })
}

/// 暂停/恢复渲染器（如设置界面打开期间或 surface 已销毁时）。Rust 渲染器已在
/// `render_frame` 中检查 `render_paused`；本 JNI 导出补上缺失的传输通道。
///
/// 暂停标志挂在**唯一的全局渲染器**上，与会话无关，故不收会话 id：调用方
/// 曾经按会话逐个调用（每次都要抢被整帧 GPU 工作持有的渲染状态锁），
/// 而零会话时又永远清不掉标志——下一个会话会渲染纯黑。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRenderPaused(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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

/// 设置渲染器字体管线的字体族；找到该族时返回 true。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamily(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    family: JString,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
        let Some(family_str) = read_jni_string(env, &family, "setFontFamily", "family") else {
            return Ok(JNI_FALSE);
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(JNI_FALSE);
        };
        let previous = render_state.font_pipeline.font_id;
        let found = render_state.font_pipeline.set_font_family(&family_str);
        let changed = render_state.font_pipeline.font_id != previous;
        if changed {
            // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，必须整库丢弃并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
            log::info!("setFontFamily: {family_str} found={found} changed={changed}");
        } else {
            log::debug!("setFontFamily: {family_str} found={found} changed={changed}");
        }
        if found { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 原生接受的光栅缩放（sp→px 系数）区间。**唯一真源**。
///
/// 同一区间此前在两端各存一份字面量——Rust 的 `setRasterScale` 守卫与 Kotlin 的
/// `coerceSpToPxScale` 钳位——互相只由注释绑定。它们漂移的后果与字号上界那次
/// 完全同型：Kotlin 用被原生拒收的系数算出 `FONT_SIZE_MAX_PX / 系数` 的字号上界，
/// 而原生静默拒收该系数（记 error），滑块上界与实际渲染重新脱节。故守卫与对外
/// 查询（[Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange]）同取此值。
const RASTER_SCALE_MIN: f32 = 0.5;
const RASTER_SCALE_MAX: f32 = 8.0;

/// 缩放是否落在原生接受区间内。守卫与对外查询同走这一处判定，故「Kotlin 钳到的
/// 区间」与「原生接受的区间」在结构上就是同一个区间，不依赖两处字面量对齐。
fn is_raster_scale_acceptable(scale: f32) -> bool {
    (RASTER_SCALE_MIN..=RASTER_SCALE_MAX).contains(&scale)
}

/// 原生接受的最大字号（sp）：字形位图必须放得进图集，故上界由图集边长推导。
///
/// 这是**唯一**的真实约束；用户可选区间由 Kotlin 侧 `SettingsRepository.fontSizeMaxSp`
/// （对标 Termux 的 256px 上限）给出，本值必须始终不小于它，否则滑块能划到的字号会被
/// 原生拒绝，用户看到的就是「设置条范围与实际可设置范围不一致」。
/// `font_size_cap_tests` 钉住这一关系。
///
/// 边长取自**管线自身**（[FontPipeline::atlas_width]，即实际分配的那张图集）而非本模块
/// 的 `ATLAS_SIZE` 常量：后者只是初始化时的入参，两者一旦不同步，「由图集推导」就成了
/// 假话，上界会悄悄偏到另一个尺寸上。
fn font_size_cap_sp(atlas_edge_px: f32, raster_scale: f32) -> f32 {
    // 系数已由 `FontPipeline::get_raster_scale` 钳到 ≥ EPSILON，此处不再重复。
    atlas_edge_px / raster_scale
}

/// 字号是否落在原生可接受区间内。
fn is_font_size_selectable(size_sp: f32, cap_sp: f32) -> bool {
    size_sp.is_finite() && size_sp > 0.0 && size_sp <= cap_sp
}

/// 设置字号（单位为十分之一 sp，与 Kotlin 滑块一致）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontSizeInPlace(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    size_tenths: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let size = (size_tenths as f32) / 10.0;
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        // 用户可选区间只有一处定义：`SettingsRepository.fontSizeMaxSp`（Termux 的
        // 256px 上限换算）。此前这里另有一份 `4.0..=100.0` 的魔数守卫，且对超限值
        // **静默丢弃**——Kotlin 侧那份重复常量一旦漂移，用户拖到的字号就被原生悄悄
        // 丢掉，表现正是「设置条范围和实际可设置范围不一致」。守卫上界改由图集边长
        // 推导（唯一真实约束），越界记错误日志而非无声忽略。
        let max_size = font_size_cap_sp(
            render_state.font_pipeline.atlas_width as f32,
            render_state.font_pipeline.get_raster_scale(),
        );
        if !is_font_size_selectable(size, max_size) {
            // warn 而非 error：捏合缩放的 preview 路径以动画帧率调用本函数，
            // 一次越界会刷出每帧一条 error，把真正的错误日志淹掉。
            log::warn!(
                "setFontSizeInPlace: 字号 {size} 越界（合法区间 (0, {max_size}] sp，字形位图必须放进图集）"
            );
            return Ok(());
        }
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
    })
}

/// 原生接受的光栅缩放区间 `[min, max]` → JNI `FloatArray`（原生为唯一真源）。
///
/// Kotlin 侧的 `coerceSpToPxScale` 必须钳到本区间：字号的 sp→px 换算
/// （`SettingsRepository.fontSizeMaxSp`）以该系数为分母，系数一旦落在区间外，
/// 算出的字号上界就不再对应原生真实的光栅尺度。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jfloatArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let bounds = [RASTER_SCALE_MIN, RASTER_SCALE_MAX];
        let Ok(array) = env.new_float_array(bounds.len()) else {
            return Ok(std::ptr::null_mut());
        };
        if array.set_region(env, 0, &bounds).is_err() {
            return Ok(std::ptr::null_mut());
        }
        array.into_raw()
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
        // 越界记错误日志而非静默丢弃：Kotlin 侧 `coerceSpToPxScale` 已钳到同区间
        // （该区间经 `getRasterScaleRange` 从这里取，二者是同一常量），两端常量
        // 漂移时必须在此留痕，否则字号上限换算与实际光栅尺度悄悄脱节。
        if !is_raster_scale_acceptable(scale) {
            log::error!(
                "setRasterScale: 缩放 {scale} 越界（合法区间 {RASTER_SCALE_MIN}..={RASTER_SCALE_MAX}）"
            );
            return Ok(());
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // 缩放只落在字体管线上：字形位图按 `font_size*scale` 光栅化，
            // 单元几何（`cell_builder`）与 bearing 也按同一 scale 算，着色器一律
            // 1:1 采样。渲染器侧曾有一个同名的 uniform 字段，但它从不被着色器读取
            // （再乘一次反而会把字形缩回逻辑像素），故已删除而非同步两处。
            render_state.font_pipeline.set_raster_scale(scale);
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
        let Some(path_str) = read_jni_string(env, &path, "loadFontFile", "path") else {
            return Ok(std::ptr::null_mut());
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
                rebuild_font_pipeline(render_state);
                if !render_state.font_pipeline.set_font_family(&family) {
                    // 丢弃它会让设置页显示新字体名而终端仍用管线的默认字体渲染，
                    // 且无任何日志（对比 setFontFamily 导出：同一结果在此被上报为 false）。
                    log::error!("loadFontFile: cannot apply family {family:?} from {path_str}");
                    return Ok(std::ptr::null_mut());
                }
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
        string_to_jstring(env, &family)
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
        let Some(locale_str) = read_jni_string(env, &locale, "setSystemLocale", "locale") else {
            return Ok(());
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

/// 登记额外的字体目录/文件（应用私有字体目录）：供字体列表枚举（`family_index`）
/// 与选中时按需装入（`load_family`）使用；渲染常驻字体库不读这些路径
/// （见 `load_font_database`）。若管线已存在则重建，保持与既有行为一致。
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
            // 从 Java 的 `String[]` 解析。任一项解码失败即抛异常并整体放弃：
            // 异常待决期间继续调 JNI 是未定义行为（JNI 规范只允许极少数调用），
            // 且本次调用的结果本就被该异常作废——继续读剩余项毫无意义。
            let mut path_list: Vec<std::path::PathBuf> = Vec::new();
            // SAFETY: `paths` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效
            // （与 `feed_pty_inner` 同一模式）。
            let array = unsafe { jni::objects::JObjectArray::<JString>::from_raw(env, paths) };
            // 读长度失败不得当成空数组：本次调用会**替换**已登记的路径，空列表等于
            // 把用户的 `.termux/fonts` 静默注销、字体列表随之缩水，且无任何报错。
            let count = match array.len(env) {
                Ok(count) => count,
                Err(error) => {
                    let message = jni::strings::JNIString::from(format!(
                        "setExtraFontPaths: cannot read array length: {error}"
                    ));
                    let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
                    return Ok(());
                }
            };
            for font_index in 0..count {
                let Some(item) = array.get_element(env, font_index).ok() else {
                    log::warn!("setExtraFontPaths: 读不到第 {font_index} 个元素，放弃");
                    return Ok(());
                };
                let Some(text) = read_jni_string(env, &item, "setExtraFontPaths", "paths[]") else {
                    return Ok(());
                };
                path_list.push(std::path::PathBuf::from(text));
            }
            crate::render::font::font_db::set_extra_font_paths(path_list);
            // 若管线已存在则重建，使新字体立即可选。
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                rebuild_font_pipeline(render_state);
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
/// 当前单元格宽高（字号单位，取自渲染器的字体管线；物理像素再乘 `raster_scale`），
/// 打包为一个 `jlong`：低 32 位是宽度的 `f32` 位、高 32 位是高度。无渲染状态时为 0。
///
/// 宽高一并取回：两者只可能被同一处消费（触摸映射与网格重算），分两次查询只是多
/// 一次 JNI 往返。宽度对字号**严格线性**（`advance × size / upem`，等宽分支无
/// 取整），故它是「原生是否真的接受了该字号」最可靠的判据：原生拒收时单元格度量
/// 不变。行高经过 `.ceil()`，对字号非线性。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellSizePacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) -> jlong {
    jni_export_guard!(unowned_env, 0, |_env| {
        // 这次查询只是给 spawn 算网格：GPU 暂不可用时返回 0，Kotlin 侧保留旧度量或退回
        // config 默认网格（`refreshCellMetricsFrom` 已写明 0 的语义），**不**在这里 abort。
        // 与 prefetchRenderState 同口径：GPU 失败可重试、只记账，字体库不可用才致命；
        // 真无 GPU 的设备仍在首帧由渲染路径给出明确错误。
        if let Err(initialization_error) = crate::render::context::try_global_gpu() {
            log::error!("cell metrics query skipped: {initialization_error}");
            return Ok(0);
        }
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(0);
        };
        let (width, height) = render_state.font_pipeline.cell_metrics();
        cell_metrics_packed_from(width, height)
    })
}

/// 宽高 → 打包 `jlong`。宽度 MUST 用零扩展（`u64::from`）：写成 `as u64` 会把
/// 宽度位模式的最高位符号扩展进高半区，高度随之变成一个天文数字。
fn cell_metrics_packed_from(width: f32, height: f32) -> jlong {
    (((u64::from(height.to_bits())) << 32) | u64::from(width.to_bits())) as jlong
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
///
/// 返回 true 表示本次写入已生效（或无需写入）。会话锁被 PTY 写入按住时返回 false 且
/// **不阻塞**：调用方保留上一次已提交的值，下一帧重试。滚动期间每帧都调用本函数，
/// 在这里等锁等于把帧率钉在 PTY 输出的节奏上（真机日志实测 pre 段 100ms+）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollOffset(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    offset: jint,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_TRUE, |_env| {
        let target = offset.max(0) as i64;
        let sent = {
            let Some(session) = session_handle(_session_id as u64) else {
                return Ok(JNI_TRUE);
            };
            // 每会话独立计算增量：此前代码按单一全局 `render_state.scroll_offset`
            // 计算，切换会话会污染恢复后会话的视口。
            let Some(session) = session.try_lock() else {
                return Ok(JNI_FALSE);
            };
            let delta = target - session.last_scroll_offset();
            if delta == 0 {
                return Ok(JNI_TRUE);
            }
            // `scroll_viewport` 的增量语义（主机与模拟器已验证）：负值 = 向上滚入历史，
            // 正值 = 回到底部。Kotlin 的 `scrollOffset` 在用户向上滑（进入历史）时增大，
            // 故此处必须取反：此前符号写反，向下滑到底部时发的是负增量，
            // 结果反而滚进了历史。
            let sent = session.terminal().scroll_viewport(-(delta as isize));
            if sent {
                session.set_last_scroll_offset(target);
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
        if sent { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 设置逐像素平滑滚动用的视口 Y 像素偏移（正值 = 内容下移，与 Kotlin 手势余数
/// 符号一致）。只登记待用值：真正写 uniform 的是随后那一帧的 `begin_frame`，
/// 它每帧都按当前偏移重写同一份字节。
///
/// 此前这里当场 `refresh_cell_uniforms`（一次 `queue.write_buffer`）——与同一线程
/// 紧接着开帧的那次写入完全重复，却在洪流期间排队等 GPU 队列 ~100ms。渲染循环的
/// pre 段因此从 <1ms 涨到 100ms+，真机日志（SLOW_LOOP pre=102-123ms）即此。
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
        // 触摸滚动的每个 MOVE 都会走到这里（数十次/手势），日志级别取 trace：
        // 值本身在 `setScrollOffset` 里已有落点记录，debug 级只会把 CI 日志刷满。
        log::trace!("setScrollYPx: {offset_px}");
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

#[cfg(test)]
mod panic_payload_tests {
    use super::take_panic_text_and_leak;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// 载荷的 `Drop` 会置位。用它来判定「载荷有没有被销毁过」。
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// 回归钉：解卷路径上**绝不销毁** panic 载荷。
    ///
    /// 载荷若在此按值销毁、而它的 `Drop` 又 panic，就是「解卷中解卷」——Rust 直接
    /// `abort`，`jni_export_guard!` 接住的 panic 就此变成整进程崩溃，调用方连
    /// Java 异常都拿不到。代价只是漏几字节（载荷通常是 `&str`/`String`），
    /// 换来的却是保住已备好的诊断信息。
    ///
    /// 判别力：去掉 `mem::forget` → 载荷被销毁 → `DropFlag` 置位 → 用例判红。
    #[test]
    fn panic_payload_is_never_dropped_on_the_unwinding_path() {
        let dropped = Arc::new(AtomicBool::new(false));
        let payload: Box<dyn std::any::Any + Send> = Box::new(DropFlag(Arc::clone(&dropped)));
        let _text = take_panic_text_and_leak(payload);
        assert!(
            !dropped.load(Ordering::SeqCst),
            "解卷路径上销毁 panic 载荷会招致双重 panic（进程 abort）"
        );
    }

    /// 文本仍要取得到：泄漏的是载荷本身，不是诊断信息。
    #[test]
    fn panic_payload_text_is_still_reported() {
        // 显式标注 `&'static str`：否则推断可能选成与本函数泛型无关的短生命周期
        // `&str`，载荷的 `TypeId` 便对不上，downcast 落空。
        let static_text: &'static str = "boom";
        assert_eq!(
            take_panic_text_and_leak(Box::new(static_text)),
            "boom",
            "&'static str 载荷应原样透出"
        );
        assert_eq!(
            take_panic_text_and_leak(Box::new(String::from("owned boom"))),
            "owned boom",
            "String 载荷应取出内容"
        );
        assert_eq!(
            take_panic_text_and_leak(Box::new(42u32)),
            "non-string panic payload",
            "未知载荷退化为占位文本，不得 panic"
        );
    }
}

#[cfg(test)]
mod clipboard_read_flood_tests {
    use super::{cancel_request, register_request, unanswered_clipboard_reads};

    /// 未作答计数按会话隔离，且只计入驻留槽位。
    #[test]
    fn unanswered_reads_count_is_per_session() {
        let (first_a, _first_rx) = register_request(4101);
        let (second_a, _second_rx) = register_request(4101);
        let (only_b, _only_rx) = register_request(4102);
        assert_eq!(unanswered_clipboard_reads(4101), 2);
        assert_eq!(unanswered_clipboard_reads(4102), 1);
        assert_eq!(unanswered_clipboard_reads(4103), 0);
        cancel_request(4101, first_a);
        assert_eq!(unanswered_clipboard_reads(4101), 1);
        cancel_request(4101, second_a);
        cancel_request(4102, only_b);
        assert_eq!(unanswered_clipboard_reads(4101), 0);
        assert_eq!(unanswered_clipboard_reads(4102), 0);
    }
}

#[cfg(test)]
mod render_wake_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::RenderWake;

    /// 代数语义：期间已通知过，带旧代数进场必须立即返回（不丢唤醒）。
    #[test]
    fn wait_returns_at_once_when_notified_before_entry() {
        let wake = RenderWake::default();
        assert_eq!(wake.wait(0, Duration::ZERO), 0);
        wake.notify();
        let start = Instant::now();
        assert_eq!(wake.wait(0, Duration::from_secs(5)), 1);
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "已有通知时不应真的等待：{:?}",
            start.elapsed()
        );
    }

    /// 无通知时等满超时并返回同一代数——调用方据此照常落一帧，不 busy-spin。
    #[test]
    fn wait_times_out_and_keeps_generation() {
        let wake = RenderWake::default();
        let start = Instant::now();
        // 初始代数为 0：带 0 进场且无人通知 → 等满超时后仍返回 0。
        assert_eq!(wake.wait(0, Duration::from_millis(50)), 0);
        assert!(
            start.elapsed() >= Duration::from_millis(40),
            "应至少等满大部分超时：{:?}",
            start.elapsed()
        );
    }

    /// 跨线程通知：等待中的线程必须被立刻叫醒（PTY 读线程→渲染线程的真实形态）。
    #[test]
    fn notify_wakes_another_thread() {
        let wake = Arc::new(RenderWake::default());
        let waiter = {
            let wake = Arc::clone(&wake);
            std::thread::spawn(move || wake.wait(0, Duration::from_secs(5)))
        };
        std::thread::sleep(Duration::from_millis(50));
        wake.notify();
        assert_eq!(waiter.join().expect("waiter panicked"), 1);
    }
}

#[cfg(test)]
mod session_registry_tests {
    use std::sync::{Arc, Mutex};

    use super::{
        ACTIVE_SESSION_ID, MAX_FLOOD_SHED_ANSWERS_PER_FRAME,
        MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION, Ordering, RENDERED_CURSOR_NONE,
        RENDERED_SCROLLBACK_NONE, RenderWake, SessionEntry, pack_rendered_cursor, pump_sessions,
        rendered_cursor_packed, rendered_scrollback, wlock_session_registry,
    };
    use crate::terminal::pty::Pty;
    use crate::terminal::session::Session;

    /// 会话注册表与活跃会话 id 是进程级全局量：两个用例各自登记会话再驱动
    /// `pump_sessions`，并行跑时会把对方的会话也泵一遍（谁先取走对方的读请求，
    /// 谁的断言就落空）。故本模块内共用一把锁串行化。
    static REGISTRY_ACCESS: Mutex<()> = Mutex::new(());

    /// OSC 52 读取请求（`ESC ] 52 ; c ; ? BEL`）：上游忽略它，本仓代答。
    const READ_REQUEST: &[u8] = b"\x1b]52;c;?\x07";
    /// 洪泛削减的空串应答。
    const EMPTY_ANSWER: &[u8] = b"\x1b]52;c;\x07";

    /// 后台会话夹具：`MockPty` 包一个会话，返回写回观察柄。
    fn background_session() -> (
        Arc<parking_lot::Mutex<Session>>,
        crate::terminal::mock_pty::MockPtyHandle,
    ) {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Arc::new(parking_lot::Mutex::new(
            Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
                .expect("with_pty must succeed"),
        ));
        (session, handle)
    }

    /// 把会话登记为**非活跃**会话并驱动一帧，随后还原活跃 id 并摘除登记。
    ///
    /// 两个洪泛用例共用这段登记/驱动骨架：各写一份时任何一处漏掉「还原活跃 id」
    /// 或「摘除登记」都会让后续用例读到别人的会话——这类串味只会偶发，
    /// 而在本模块外它读到的正是生产注册表。
    fn pump_once_as_background(session: &Arc<parking_lot::Mutex<Session>>, session_id: u64) {
        wlock_session_registry().insert(
            session_id,
            SessionEntry::new(session.clone(), Arc::new(RenderWake::default())),
        );
        let previous_active = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
        ACTIVE_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
        pump_sessions();
        ACTIVE_SESSION_ID.store(previous_active, std::sync::atomic::Ordering::Release);
        wlock_session_registry().remove(&session_id);
    }

    /// 帧内光标载体的读回：未发布 → 无值，发布可见格 → 该格，发布隐藏格 → 无值，
    /// 未登记的会话 → `None`（与「已知会话但当前无光标」必须分得开）。
    ///
    /// 判别力：把「无值」与「未知会话」塌缩成同一个返回值时，刚建会话、首帧还没被
    /// 取走的那段时间里，调用方分不清「会话没了」和「暂时没有光标」——前者该报错，
    /// 后者该静默。
    #[test]
    fn rendered_cursor_reads_published_cell_and_separates_unknown_session() {
        let _access = REGISTRY_ACCESS.lock();
        let (session, _handle) = background_session();
        let session_id = 0x5EED_C0DE_u64;
        wlock_session_registry().insert(
            session_id,
            SessionEntry::new(session, Arc::new(RenderWake::default())),
        );
        let publish = |id: u64, packed: i64| {
            wlock_session_registry()
                .get(&id)
                .expect("entry must be registered")
                .rendered_cursor
                .store(packed, Ordering::Release);
        };
        let info = |row, col, visible| crate::terminal::ghostty_terminal::CursorInfo {
            row,
            col,
            at_wide_tail: false,
            visible,
            style: crate::terminal::ghostty_terminal::CursorStyle::Block,
            scrollback_length: 0,
            rows: 24,
            cols: 80,
            kitty_generation: 0,
        };

        // 首帧尚未被渲染线程取走。
        assert_eq!(
            rendered_cursor_packed(session_id),
            Some(RENDERED_CURSOR_NONE)
        );

        publish(session_id, pack_rendered_cursor(&info(3, 7, true)));
        assert_eq!(rendered_cursor_packed(session_id), Some((3 << 32) | 7));

        // 该帧光标隐藏（含滚出视口）：没有可用光标，但会话是已知的。
        publish(session_id, pack_rendered_cursor(&info(0, 0, false)));
        assert_eq!(
            rendered_cursor_packed(session_id),
            Some(RENDERED_CURSOR_NONE)
        );

        assert_eq!(rendered_cursor_packed(session_id + 1), None);

        wlock_session_registry().remove(&session_id);
    }

    /// 帧内回滚长度载体：首帧未取走 → 无值；发布后 → 该帧长度；未知会话 → `None`。
    /// 判别力：把「无值」当成 0 会让滚动钳位在首帧前把用户按住不动，
    /// 把「未知会话」当成 0 则让切会话后的旧查询静默成功。
    #[test]
    fn rendered_scrollback_separates_unpublished_from_unknown_session() {
        let _access = REGISTRY_ACCESS.lock();
        let (session, _handle) = background_session();
        let session_id = 0x5EED_5C40_u64;
        wlock_session_registry().insert(
            session_id,
            SessionEntry::new(session, Arc::new(RenderWake::default())),
        );

        assert_eq!(
            rendered_scrollback(session_id),
            Some(RENDERED_SCROLLBACK_NONE),
            "首帧尚未被渲染线程取走时必须与真实的 0 行回滚区分开"
        );

        wlock_session_registry()
            .get(&session_id)
            .expect("entry must be registered")
            .rendered_scrollback
            .store(524, Ordering::Release);
        assert_eq!(rendered_scrollback(session_id), Some(524));

        assert_eq!(rendered_scrollback(session_id + 1), None);

        wlock_session_registry().remove(&session_id);
    }

    /// 后台会话的读请求同样必须被收割并应答。
    ///
    /// 判别力：只排活跃会话时，后台会话的读请求队列进一条、从不取出——队列无界
    /// 增长，而远端程序永远收不到任何应答。此处断言的是**可观测结果**：读请求最终
    /// 变成了写回 PTY 的 `ESC ] 52 ; c ; BEL`（洪泛削减的空串应答）。
    #[test]
    fn background_session_read_requests_are_answered() {
        let _registry_access = REGISTRY_ACCESS.lock();
        let (session, handle) = background_session();
        // 单块输出里塞够请求数：超上限的那些走洪泛削减，就地作答空串——
        // 那条应答就是「请求确实被处理了」的外部可见证据。
        let mut flood = Vec::new();
        for _ in 0..MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION + 4 {
            flood.extend_from_slice(READ_REQUEST);
        }
        session.lock().inject_output_for_test(&flood);
        pump_once_as_background(&session, 98_060_401);

        let answered = String::from_utf8_lossy(&handle.written()).into_owned();
        assert!(
            answered.contains("\x1b]52;c;\x07"),
            "后台会话的读请求必须被应答，实际写回 {answered:?}"
        );
        assert!(
            session.lock().take_clipboard_reads().is_empty(),
            "读请求队列必须每帧排空，否则后台程序持续发请求时无界增长"
        );
    }

    /// 洪泛削减的回写同样按帧封顶。
    ///
    /// 削减分支要同步写 PTY（子进程不读 stdin 时每次可等到 50ms），而这段代码跑在
    /// 渲染线程上。单块 8KB 输出能塞约 900 条读请求，两块即近 1800 条——不封顶就是
    /// 渲染线程一帧内上千次带锁写入。断言的是「写回次数有上界」这一可观测结果。
    #[test]
    fn flood_shed_answers_are_capped_per_frame() {
        let _registry_access = REGISTRY_ACCESS.lock();
        let (session, handle) = background_session();
        // 两块输出，每块塞满请求：远超每帧回写上限，也远超单会话未作答上限。
        // 一块输出按 8KB 计（PTY 读取缓冲），塞满请求即约 900 条一块。
        let mut flood = Vec::new();
        for _ in 0..2 * 8192 / READ_REQUEST.len() {
            flood.extend_from_slice(READ_REQUEST);
        }
        for _ in 0..2 {
            session.lock().inject_output_for_test(&flood);
        }
        pump_once_as_background(&session, 98_060_402);

        let written = handle.written();
        let answers = written
            .windows(EMPTY_ANSWER.len())
            .filter(|window| *window == EMPTY_ANSWER)
            .count();
        assert!(
            answers > 0,
            "削减分支必须仍然作答，否则远端永远等不到应答（写回 {:?}）",
            String::from_utf8_lossy(&written)
        );
        assert!(
            answers <= MAX_FLOOD_SHED_ANSWERS_PER_FRAME,
            "削减回写每帧 {answers} 次，超过上限 {MAX_FLOOD_SHED_ANSWERS_PER_FRAME}"
        );
    }
}

#[cfg(test)]
mod cell_size_packing_tests {
    use super::cell_metrics_packed_from;

    /// 宽高按两个 `f32` 位落进同一个 `jlong`：低位宽、高位高。
    ///
    /// 判别力在**零扩展**：`u64::from(width.to_bits())` 这一步若写成 `as u64`，
    /// 宽度位模式的最高位会被填成符号位，宽度在小数值上取回一个天文数字。
    #[test]
    fn width_and_height_occupy_low_and_high_halves() {
        let packed = cell_metrics_packed_from(8.5, 17.25);
        let width = f32::from_bits((packed as u64 & 0xFFFF_FFFF) as u32);
        let height = f32::from_bits(((packed as u64) >> 32) as u32);
        assert_eq!(width, 8.5);
        assert_eq!(height, 17.25);
    }

    /// 负宽位模式（NaN/负数的位模式带最高位）不得污染高度半区。
    #[test]
    fn width_bits_do_not_leak_into_the_height_half() {
        let packed = cell_metrics_packed_from(-0.0, 1.0);
        let width = f32::from_bits((packed as u64 & 0xFFFF_FFFF) as u32);
        let height = f32::from_bits(((packed as u64) >> 32) as u32);
        assert_eq!(width, -0.0);
        assert!(width.is_sign_negative());
        assert_eq!(height, 1.0);
    }
}

#[cfg(test)]
mod font_size_cap_tests {
    use super::{
        ATLAS_SIZE, RASTER_SCALE_MAX, RASTER_SCALE_MIN, font_size_cap_sp, is_font_size_selectable,
    };

    /// Kotlin `SettingsRepository.fontSizeMaxSp` 的**权威取值**，即 Termux 自身的
    /// 公开常量：像素上限 256、调整步长 2sp、下限 4sp。这里刻意写死外部数值而非
    /// 复刻 Kotlin 公式——后者会随 Kotlin 侧一起漂移，就等于没测。
    const TERMUX_MAX_PX: f32 = 256.0;
    const TERMUX_STEP_SP: f32 = 2.0;
    const SLIDER_MIN_SP: f32 = 4.0;

    /// Termux 的可选上界（sp）：256px 换算后按步长向下取整。
    fn termux_selectable_max_sp(raster_scale: f32) -> f32 {
        let raw = TERMUX_MAX_PX / raster_scale;
        let stepped = (raw / TERMUX_STEP_SP).floor() * TERMUX_STEP_SP;
        stepped.max(SLIDER_MIN_SP + TERMUX_STEP_SP)
    }

    use super::is_raster_scale_acceptable;

    /// `coerceSpToPxScale` 允许的全系数区间即 `ffi.rs` 的唯一真源
    /// `RASTER_SCALE_MIN/MAX`：Kotlin 侧的钳位区间经 `getRasterScaleRange` 取同一
    /// 常量，两端不再各存一份字面量，测试再抄一份只会掩盖漂移。
    const RASTER_SCALE_SAMPLES: usize = 64;

    /// 在合法区间内取第 `i` 个采样系数（含两端）。
    fn sampled_scale(i: usize) -> f32 {
        RASTER_SCALE_MIN
            + (RASTER_SCALE_MAX - RASTER_SCALE_MIN) * i as f32 / RASTER_SCALE_SAMPLES as f32
    }

    /// 守卫（`setRasterScale`）实际调用的那个判定：区间外的相邻可表示值必须被拒收。
    ///
    /// 断言取具体边界而非「判定 == 区间包含」这个定义式——后者只复刻实现。
    #[test]
    fn the_guard_accepts_the_whole_interval_and_rejects_just_outside() {
        assert!(is_raster_scale_acceptable(RASTER_SCALE_MIN));
        assert!(is_raster_scale_acceptable(RASTER_SCALE_MAX));
        let just_below = f32::from_bits(RASTER_SCALE_MIN.to_bits() - 1);
        let just_above = f32::from_bits(RASTER_SCALE_MAX.to_bits() + 1);
        assert!(!is_raster_scale_acceptable(just_below));
        assert!(!is_raster_scale_acceptable(just_above));
        // 非有限值不得落进区间：NaN 的 `contains` 为 false，但 Kotlin 侧
        // `coerceIn` 会把 NaN 原样送出，故此处只保证原生拒收。
        assert!(!is_raster_scale_acceptable(f32::NAN));
    }

    #[test]
    fn cap_never_rejects_a_selectable_font_size() {
        // 核心不变量：滑块能划到的每个字号，原生都必须接受。
        // 图集一旦缩小到接近 256px，这条即失效——那正是「范围与实际可设置范围不一致」。
        //
        // Kotlin 侧的上界实际是两条约束取紧后再兜底：
        //   min(Termux 像素上限, 「至少放得下 MIN_USABLE_COLUMNS 列」)
        //   .coerceAtLeast(自适应默认值)
        // 于是真正需要覆盖的不是 Termux 上界本身，而是它的**上包界**。
        // Termux 上界恒不小于那條兜底值（故此处仍是全覆盖）由 JVM 侧断言
        // `NativeBridgeSmokeTest` 的用例钉住——那边能直接读 Kotlin 的
        // `ADAPTIVE_DEFAULT_MAX_SP` 与经 `getRasterScaleRange` 导出的原生端点，
        // 不需要在本文件再抄一份 Kotlin 私有策略值。抄一份的后果：Kotlin 改 24→28
        // 时本文件仍全绿，覆盖面证明静默失效。
        for i in 0..=RASTER_SCALE_SAMPLES {
            let scale = sampled_scale(i);
            let cap = font_size_cap_sp(ATLAS_SIZE as f32, scale);
            let selectable_max = termux_selectable_max_sp(scale);
            assert!(
                is_font_size_selectable(selectable_max, cap),
                "raster_scale={scale}：滑块上界 {selectable_max}sp 超过原生上界 {cap}sp"
            );
            assert!(
                is_font_size_selectable(SLIDER_MIN_SP, cap),
                "raster_scale={scale}：原生拒绝滑块下界 {SLIDER_MIN_SP}sp"
            );
        }
    }

    /// 余量断言：图集边长在**最大**合法系数下仍须显著高于 Termux 的像素上限。
    ///
    /// 与 `cap_never_rejects_a_selectable_font_size` 互补：那条只要求「上界不低于可选
    /// 上界」，把 `ATLAS_SIZE` 砍到 300 它仍全绿（300/8 = 37.5 ≥ 32），但那时字形位图
    /// 只剩 Termux 上限的四倍余量，容错已经很小——这条守住余量本身。
    #[test]
    fn atlas_edge_keeps_headroom_over_the_termux_ceiling() {
        let cap = font_size_cap_sp(ATLAS_SIZE as f32, RASTER_SCALE_MAX);
        assert!(
            cap * RASTER_SCALE_MAX > TERMUX_MAX_PX * 2.0,
            "图集 {ATLAS_SIZE}px 在最大系数 {RASTER_SCALE_MAX} 下换算出的字号上限 {cap}sp 未留出两倍余量"
        );
    }

    #[test]
    fn cap_is_the_atlas_edge_expressed_in_sp() {
        // 对照**字面量**而非「cap × scale == ATLAS_SIZE」这个定义式：后者只是把除法
        // 还原，边长取错或系数取错都照样通过。2048 是图集边长的具体取值，
        // 2048 / 0.5 = 4096 是它在最小系数下的具体上界。
        assert_eq!(font_size_cap_sp(2048.0, 0.5), 4096.0);
        assert_eq!(font_size_cap_sp(2048.0, 8.0), 256.0);
        // 而生产路径用的边长必须就是 2048：cap 是 `atlas_width / raster_scale`，
        // 图集边长一旦不是 2048，上面的字面量与真实行为就分家了。
        assert_eq!(
            ATLAS_SIZE, 2048,
            "图集边长改变时须同步更新本模块的字面量断言"
        );
    }

    #[test]
    fn non_finite_and_non_positive_sizes_are_rejected() {
        let cap = font_size_cap_sp(ATLAS_SIZE as f32, 1.0);
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(
                !is_font_size_selectable(bad, cap),
                "非法字号 {bad} 竟被接受"
            );
        }
    }
}

#[cfg(test)]
mod font_info_packing_tests {
    use super::pack_font_info;
    use crate::render::font::pipeline::FontPipeline;

    /// 字体信息打包：`u32` 标志 + 三个 `f32` + 两个 `u16` 计数 + NUL 分隔名称。
    /// 判别序号与浮点按小端摆放，Kotlin 侧按同一布局解包。
    #[test]
    fn font_info_packs_flags_metrics_and_names() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale("en-US");
        let packed = pack_font_info(&pipeline.font_info());
        let flags = u32::from_le_bytes(packed[0..4].try_into().unwrap());
        assert_eq!(
            flags >> 1,
            crate::render::font::pipeline::CjkFallbackState::SkippedNonCjkLocale as u32,
            "非 CJK 系统的回退层按设计跳过，不得报成缺失"
        );
        assert_eq!(
            f32::from_le_bytes(packed[12..16].try_into().unwrap()),
            14.0,
            "字号按 f32 原样传递"
        );
        assert!(f32::from_le_bytes(packed[8..12].try_into().unwrap()) > 0.0);
        let name_count = u16::from_le_bytes(packed[16..18].try_into().unwrap());
        let cjk_count = u16::from_le_bytes(packed[18..20].try_into().unwrap());
        assert_eq!(cjk_count, 0, "无 CJK 回退族");
        assert_eq!(name_count, 1, "只有主字体名，无 CJK 回退族与其他被使用族");
        let names: Vec<&[u8]> = packed[20..]
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(!names[0].is_empty(), "主字体族名须非空");
    }
}

#[cfg(test)]
mod session_entry_flag_tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::{RenderWake, SessionEntry};
    use crate::terminal::mock_pty::MockPty;
    use crate::terminal::session::Session;

    /// 渲染采样读的是注册表里的共享原子量，不是会话锁后的字段：一旦有人改成
    /// 另建一个标志，这条指针相等断言会先红，避免慢帧修复被静默回退。
    #[test]
    fn entry_shares_session_flags_for_lock_free_sampling() {
        let (pty, _handle) = MockPty::new(24, 80);
        let session =
            Session::with_pty_for_test(Box::new(pty) as Box<dyn crate::terminal::pty::Pty>, 24, 80)
                .expect("session");
        let new_output = session.new_output_flag();
        let alt_screen = session.alt_screen_active_flag();
        let entry = SessionEntry::new(
            Arc::new(Mutex::new(session)),
            Arc::new(RenderWake::default()),
        );
        assert!(Arc::ptr_eq(&entry.new_output, &new_output));
        assert!(Arc::ptr_eq(&entry.alt_screen_active, &alt_screen));
    }
}

#[cfg(test)]
mod render_sample_bits_tests {
    use super::pack_render_sample;

    /// 位 33..42：已删除的光标行位形，保留 MUST 恒为 0。
    const RESERVED_ROW_BITS: i64 = 0x3FF << 33;

    /// 位 33..42 之外全部在用的位（计数 0..31、输出 32、内容下沿 43..52、
    /// 失效 53、备用屏 54）。它与保留位相交为空即证明没有任何字段占用 33..42。
    const ALLOCATED_SAMPLE_BITS: i64 = 0xFFFF_FFFF
        | (1 << 32)
        | (super::LAST_CONTENT_ROW_NONE_BITS << 43)
        | super::SURFACE_INVALIDATED_BIT
        | super::ALT_SCREEN_ACTIVE_BIT;

    #[test]
    fn no_allocated_field_overlaps_the_reserved_row_bits() {
        // 位 33..42 是已删除的光标行位形。Kotlin 侧不读它，故一旦有字段落进去，
        // 两侧都不会判红——这里是唯一的闸。
        assert_eq!(
            ALLOCATED_SAMPLE_BITS & RESERVED_ROW_BITS,
            0,
            "已分配字段与保留位 33..42 重叠",
        );
    }

    #[test]
    fn packing_never_sets_the_reserved_row_bits() {
        // 直接对**打包表达式**取值，而不是对常量：逐字段取遍有标志与哨兵的组合。
        for count in [0i64, -1, 7] {
            for new_output in [0i64, 1] {
                for content in [0i64, 5, super::LAST_CONTENT_ROW_NONE_BITS] {
                    for surface in [0i64, super::SURFACE_INVALIDATED_BIT] {
                        for alt in [0i64, super::ALT_SCREEN_ACTIVE_BIT] {
                            let packed = pack_render_sample(
                                count & 0xFFFF_FFFF,
                                new_output,
                                content,
                                surface,
                                alt,
                            );
                            assert_eq!(
                                packed & RESERVED_ROW_BITS,
                                0,
                                "保留位 33..42 被污染：count={count} new_output={new_output} content={content} surface={surface} alt={alt}",
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packed_fields_reach_their_own_bits() {
        // 反向：每个字段确实出现在自己的位上（纯 0 打包恒真，故要非 0 输入）。
        let packed = pack_render_sample(
            7,
            1,
            5,
            super::SURFACE_INVALIDATED_BIT,
            super::ALT_SCREEN_ACTIVE_BIT,
        );
        assert_eq!(packed & 0xFFFF_FFFF, 7);
        assert_eq!(packed >> 32 & 0x1, 1);
        assert_eq!(packed >> 43 & 0x3FF, 5);
        assert_eq!(packed >> 53 & 0x1, 1);
        assert_eq!(packed >> 54 & 0x1, 1);
    }

    #[test]
    fn sample_failure_bits_report_failure_count_and_unknown_rows() {
        // 渲染计数位（0..31）全 1 即 Kotlin 侧读到的 -1；内容下沿取未知哨兵。
        assert_eq!(
            super::RENDER_SAMPLE_FAILURE_BITS & 0xFFFF_FFFF,
            0xFFFF_FFFF,
            "渲染计数必须为负，否则 panic 会被读成空闲帧",
        );
        assert_eq!(
            (super::RENDER_SAMPLE_FAILURE_BITS >> 43) & 0x3FF,
            super::LAST_CONTENT_ROW_NONE_BITS,
        );
        assert_eq!(super::RENDER_SAMPLE_FAILURE_BITS >> 32 & 0x1, 0);
        assert_eq!(super::RENDER_SAMPLE_FAILURE_BITS >> 53 & 0x1, 0);
        assert_eq!(super::RENDER_SAMPLE_FAILURE_BITS >> 54 & 0x1, 0);
    }

    #[test]
    fn alt_screen_bit_is_distinct_and_outside_the_other_fields() {
        // 备用屏位必须落在所有既有字段之外，否则会被任一掩码读成别的含义
        // （例如被内容下沿的 10 位掩码读成行号，或让失败位形带上脏位）。
        assert_eq!(super::ALT_SCREEN_ACTIVE_BIT, 1 << 54);
        assert_eq!(super::RENDER_SAMPLE_FAILURE_BITS >> 54 & 0x1, 0);
        // 与内容下沿（43..52）、失效位（53）、输出位（32）均不重叠。
        assert_eq!(super::ALT_SCREEN_ACTIVE_BIT >> 32 & 0x1, 0);
        assert_eq!((super::ALT_SCREEN_ACTIVE_BIT >> 43) & 0x3FF, 0);
        assert_eq!(super::ALT_SCREEN_ACTIVE_BIT >> 53 & 0x1, 0);
    }
}

#[cfg(test)]
mod scroll_bookkeeping_tests {
    use super::{RenderWake, SessionEntry, rlock_session_registry, wlock_session_registry};
    use crate::terminal::pty::Pty;
    use crate::terminal::session::Session;
    use std::sync::Arc;

    /// 滚动记账归会话，重置须把它一并归零。
    ///
    /// 该值此前挂在注册表项上，于是滚动与重置两条路径都得在**注册表写锁**内取会话锁
    /// ——而会话锁可被 PTY 写入按住数秒，一条只读查找因此能冻住整台终端。归位到会话
    /// 内（原子量）后两条路径只取读锁，语义不变。
    #[test]
    fn scroll_bookkeeping_lives_on_the_session_and_reset_clears_it() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Arc::new(parking_lot::Mutex::new(
            Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
                .expect("with_pty must succeed"),
        ));
        let session_id = 98_090_101u64;
        wlock_session_registry().insert(
            session_id,
            SessionEntry::new(session.clone(), Arc::new(RenderWake::default())),
        );

        session.lock().set_last_scroll_offset(37);
        // 经注册表读锁取回的句柄看到的是同一份记账（两个会话各自独立，见下一条）。
        let through_registry = rlock_session_registry()
            .get(&session_id)
            .expect("已登记")
            .session
            .clone();
        assert_eq!(through_registry.lock().last_scroll_offset(), 37);

        through_registry.lock().reset_terminal();
        assert_eq!(
            session.lock().last_scroll_offset(),
            0,
            "重置后 VT 视口已归零，滚动记账必须同步归零，否则下一次滚动按 stale 值算 delta"
        );

        wlock_session_registry().remove(&session_id);
    }

    /// 两个会话的滚动记账互不相干。
    ///
    /// 判别力：记账若挂在全局或注册表某处（此前正是「单一全局偏移」的历史成因），
    /// B 会话滚动后 A 的记账也会变。
    #[test]
    fn scroll_bookkeeping_is_per_session() {
        let new_session = || {
            let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
            Arc::new(parking_lot::Mutex::new(
                Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
                    .expect("with_pty must succeed"),
            ))
        };
        let first = new_session();
        let second = new_session();
        first.lock().set_last_scroll_offset(11);
        assert_eq!(
            second.lock().last_scroll_offset(),
            0,
            "另一会话的滚动不得推进本会话的记账"
        );
    }
}

#[cfg(test)]
mod session_scoped_tests {
    use super::SessionScoped;

    /// 切会话只丢 `current`，`drawn` 必须留一帧。
    ///
    /// 判别力：渲染态是进程级唯一的，而高亮来自每会话的查询——`current` 是本会话的
    /// 命中框，不清就把上一会话的命中框合成到新会话网格上。`drawn` 是「上一帧画进累加器
    /// 的高亮行」，两者之差正是空闲重绘路径的门控（`needs_repaint` 里
    /// `current != drawn`）：把它一并清掉，判据当场变假，空闲帧不再重绘，累加器里的旧
    /// 高亮像素就留在屏幕上（实测：关闭搜索后反色块不消失，随后只有光标行在重绘）。
    /// 留着的这份残留只活一帧：两条渲染路径都在成功后把 `drawn` 赋成此刻已空的
    /// `current`，自净。
    ///
    /// 曾有一条反向断言（「`drawn` 一并清，否则脏带判重读成高亮未变」）已被推翻：
    /// `highlight_rows_to_mark` 恒取 `current ∪ drawn`，没有「读成未变」这条分支。
    #[test]
    fn entering_another_session_drops_current_but_keeps_drawn_for_one_frame() {
        let mut scoped = SessionScoped::new();
        scoped.session = 1;
        scoped.current = vec![10, 11];
        scoped.drawn = vec![10, 11];

        scoped.enter_session(2);
        assert!(scoped.current.is_empty(), "命中框不得带到另一会话");
        assert_eq!(
            scoped.drawn,
            vec![10, 11],
            "上一帧画过的高亮行要留一帧：清掉这些行就不再重绘，旧像素留在累加器里"
        );
        assert_ne!(
            scoped.current, scoped.drawn,
            "两者之差正是空闲路径重绘的门控条件，清成一致会让本帧静默跳过"
        );

        // 同一会话重复进入是每帧常态：不得清自己的高亮，也不得制造无谓重绘。
        scoped.session = 2;
        scoped.current = vec![20];
        scoped.drawn = vec![20];
        scoped.enter_session(2);
        assert_eq!(scoped.current, vec![20], "同一会话的高亮须保留");
        assert_eq!(scoped.drawn, vec![20]);
        assert_eq!(scoped.current, scoped.drawn, "同一会话内门控不得被点亮");
    }
}

#[cfg(test)]
mod placeholder_signature_tests {
    use super::placeholder_layout_signature;
    use crate::terminal::ghostty_terminal::CellData;
    use crate::terminal::ghostty_terminal::unicode_placement::PLACEHOLDER;

    fn placeholder(row: u32, col: u32) -> CellData {
        let mut cell: CellData = bytemuck::Zeroable::zeroed();
        cell.codepoint = PLACEHOLDER;
        cell.row = row;
        cell.col = col;
        cell
    }

    /// 签名必须随占位符位置变化：同一幅图被 TUI 重绘到新行时，
    /// 渲染线程要重查放置，否则图像钉在旧位置。
    #[test]
    fn signature_tracks_placeholder_movement() {
        let at_origin = [placeholder(2, 3)];
        let moved = [placeholder(4, 3)];
        assert_ne!(
            placeholder_layout_signature(&at_origin),
            placeholder_layout_signature(&moved)
        );
        assert_eq!(
            placeholder_layout_signature(&at_origin),
            placeholder_layout_signature(&[placeholder(2, 3)])
        );
    }

    /// 纯文本（无占位符）必须返回稳定的 0，避免每帧误判为布局变化。
    #[test]
    fn no_placeholders_is_stable() {
        let mut text: CellData = bytemuck::Zeroable::zeroed();
        text.codepoint = u32::from('a');
        text.row = 1;
        text.col = 1;
        assert_eq!(placeholder_layout_signature(&[text]), 0);
        assert_eq!(placeholder_layout_signature(&[]), 0);
    }
}

#[cfg(test)]
mod kitty_placement_keys_tests {
    use super::{KittyPlacementKeys, kitty_cache_invalidation};

    fn keys() -> KittyPlacementKeys {
        KittyPlacementKeys {
            session_id: 7,
            generation: 3,
            scroll_offset: 0,
            scrollback_length: 100,
            placeholder_signature: 0xabcd,
            rows: 24,
            cols: 80,
        }
    }

    /// 与视口无关的放置键变化必须重查放置。
    ///
    /// 判别力：键集合此前是渲染状态里的散落字段，逐字段比较易漏——「占位符签名」
    /// 曾缺失时 TUI 移动图像后仍钉在旧位置；这些变化无法用平移表达，只能重查。
    #[test]
    fn content_key_change_triggers_refetch() {
        let previous = keys();
        for changed in [
            KittyPlacementKeys {
                session_id: 8,
                ..previous
            },
            KittyPlacementKeys {
                generation: 4,
                ..previous
            },
            KittyPlacementKeys {
                placeholder_signature: 0x1234,
                ..previous
            },
            KittyPlacementKeys {
                rows: 25,
                ..previous
            },
            KittyPlacementKeys {
                cols: 81,
                ..previous
            },
        ] {
            let invalidation = kitty_cache_invalidation(changed, previous, false, false);
            assert!(
                invalidation.refetch_placements,
                "放置键变化必须重查放置: {changed:?}"
            );
            assert!(
                invalidation.overlay_dirty,
                "放置键变化必须整帧重绘以去掉累加器里的旧图像: {changed:?}"
            );
            assert_eq!(invalidation.viewport_shift_rows, 0, "内容变化不靠平移");
        }
    }

    /// 纯滚动不重查放置：放置帧是视口相对坐标，按视口顶行差量平移即可。
    ///
    /// 判别力：把滚动偏移放进「重查」集合时，每一行滚动都要在渲染线程同步等一次
    /// VT 查询（yazi 预览下实测约 60ms/行），滚动掉到 ~16fps——用户主诉的卡顿。
    #[test]
    fn viewport_scroll_shifts_cached_frames_without_refetch() {
        let previous = keys();
        // 向上滚入历史一行：视口顶行 100 -> 99，图像在视口内下移一行。
        let scrolled = KittyPlacementKeys {
            scroll_offset: 1,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(scrolled, previous, false, false);
        assert!(!invalidation.refetch_placements, "纯滚动不得同步查放置");
        assert!(!invalidation.sync_cell_size);
        assert!(!invalidation.repack_atlas);
        assert!(
            invalidation.overlay_dirty,
            "图像随视口移动，必须整帧重绘才能擦掉旧位置"
        );
        assert_eq!(invalidation.viewport_shift_rows, 1);

        // 底部新输出把行推入回滚：视口顶行 100 -> 101，图像在视口内上移一行。
        let pushed = KittyPlacementKeys {
            scrollback_length: 101,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(pushed, previous, false, false);
        assert!(!invalidation.refetch_placements);
        assert_eq!(invalidation.viewport_shift_rows, -1);

        // 滚动偏移与回滚长度同时变化而视口顶行不变：屏幕内容没动，无需任何动作。
        let same_top = KittyPlacementKeys {
            scroll_offset: 1,
            scrollback_length: 101,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(same_top, previous, false, false);
        assert!(!invalidation.refetch_placements);
        assert!(!invalidation.overlay_dirty);
        assert_eq!(invalidation.viewport_shift_rows, 0);
    }

    /// 滚动停稳后补一次真实查询：滚动期间平移的是旧视口裁剪过的源矩形，
    /// 不补那次查询，新露出的边缘会一直缺一条。
    #[test]
    fn settled_scroll_refetches_approximate_frames_once() {
        let aligned = keys();
        let invalidation = kitty_cache_invalidation(aligned, aligned, false, true);
        assert!(invalidation.refetch_placements, "停稳后须补一次真实查询");
        assert!(invalidation.overlay_dirty);
        assert_eq!(invalidation.viewport_shift_rows, 0);

        // 帧已由真实查询刷新（非近似）时，停稳不会再查一次。
        let settled = kitty_cache_invalidation(aligned, aligned, false, false);
        assert!(!settled.refetch_placements);
        assert!(!settled.overlay_dirty);
    }

    /// 渲染态初值的哨兵键不得让失效判定溢出。
    ///
    /// 判别力：视口顶行由 `回滚长度 - 滚动偏移` 求得，而「从未查询」的初值用
    /// `scroll_offset = i64::MIN`；直接相减在 debug 构建里 panic（进程级崩溃），
    /// release 则静默回绕成无意义值。
    #[test]
    fn sentinel_keys_do_not_overflow() {
        let sentinel = KittyPlacementKeys {
            session_id: 0,
            generation: u64::MAX,
            scroll_offset: i64::MIN,
            scrollback_length: u32::MAX,
            placeholder_signature: u64::MAX,
            rows: u32::MAX,
            cols: u32::MAX,
        };
        let plain = KittyPlacementKeys {
            generation: 0,
            scroll_offset: 0,
            scrollback_length: 100,
            ..sentinel
        };
        let invalidation = kitty_cache_invalidation(plain, sentinel, false, false);
        assert!(!invalidation.refetch_placements, "纯文本首帧不查放置");
        assert!(invalidation.overlay_dirty, "首帧须整帧重绘");
        assert_eq!(invalidation.viewport_shift_rows, 0);
    }

    /// 键与几何都没变时不重查、不重绘：否则每帧都重解码整幅图。
    #[test]
    fn unchanged_keys_keep_cache() {
        let previous = keys();
        let invalidation = kitty_cache_invalidation(previous, previous, false, false);
        assert!(!invalidation.refetch_placements);
        assert!(!invalidation.overlay_dirty);
        assert!(!invalidation.sync_cell_size);
        assert!(!invalidation.repack_atlas);
    }

    /// 单元格几何变化要先同步尺寸到终端侧再重查：旧帧像素尺寸按旧几何解算，
    /// 不同步则图像按错误尺寸重算。
    #[test]
    fn cell_geometry_change_syncs_size_before_refetch() {
        let previous = keys();
        let invalidation = kitty_cache_invalidation(previous, previous, true, false);
        assert!(invalidation.sync_cell_size, "几何变化须同步尺寸到终端侧");
        assert!(invalidation.refetch_placements, "几何变化须重查放置");
        assert!(invalidation.overlay_dirty);
        assert!(!invalidation.repack_atlas, "几何变化不换图集");
    }

    /// 换会话要同步尺寸并重打包图集：新会话的图像集与几何都与旧会话无关。
    #[test]
    fn session_change_syncs_size_and_repacks_atlas() {
        let previous = keys();
        let changed = KittyPlacementKeys {
            session_id: 8,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(changed, previous, false, false);
        assert!(invalidation.sync_cell_size, "换会话须把尺寸同步到新会话");
        assert!(invalidation.refetch_placements);
        assert!(invalidation.repack_atlas, "换会话须重打包图集");
        assert!(invalidation.overlay_dirty);
    }

    /// 生成戳变化表示图像增删：须重打包图集，但不必改单元格尺寸。
    #[test]
    fn generation_change_repacks_atlas_only() {
        let previous = keys();
        let changed = KittyPlacementKeys {
            generation: 4,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(changed, previous, false, false);
        assert!(invalidation.repack_atlas, "生成戳变化须重打包图集");
        assert!(invalidation.refetch_placements);
        assert!(!invalidation.sync_cell_size, "生成戳变化不涉及单元格尺寸");
    }

    /// 纯文本会话（生成戳 0）不触发任何图像路径，但撤下图像时仍须整帧重绘，
    /// 否则累加器里残留的图像像素不会消失。
    #[test]
    fn pure_text_session_skips_image_paths_but_clears_residue() {
        let previous = keys();
        let plain = KittyPlacementKeys {
            generation: 0,
            ..previous
        };
        let invalidation = kitty_cache_invalidation(plain, previous, false, false);
        assert!(!invalidation.refetch_placements, "纯文本不重查放置");
        assert!(!invalidation.sync_cell_size);
        assert!(!invalidation.repack_atlas);
        assert!(
            invalidation.overlay_dirty,
            "图像撤下后须整帧重绘以清掉残留像素"
        );

        let still_plain = kitty_cache_invalidation(plain, plain, false, false);
        assert!(!still_plain.overlay_dirty, "纯文本会话状态未变时不重绘");
    }
}

#[cfg(test)]
mod cursor_anchor_tests {
    use super::build_cursor;
    use crate::terminal::ghostty_terminal::CursorInfo;
    use crate::terminal::ghostty_terminal::CursorStyle;

    /// 落在宽字符尾列的光标必须折算成该宽字符的头列。
    ///
    /// 判别力：`build_cell_data` 对一个宽字符只发出一条头列记录（`width=2`，一条即
    /// 整个字形），spacer 单元不产出记录。渲染侧按 `row`/`col` 相等匹配光标，故不折算
    /// 时尾列上的光标匹配不到任何记录，那几帧终端看上去「重绘了但没有光标」——
    /// 方块/竖线/下划线三种样式全中，因为匹配发生在样式分支之前。
    #[test]
    fn cursor_column_folds_onto_the_wide_characters_head() {
        let mut wide_tail = CursorInfo {
            row: 3,
            col: 7,
            at_wide_tail: true,
            visible: true,
            style: CursorStyle::Block,
            scrollback_length: 0,
            rows: 24,
            cols: 80,
            kitty_generation: 0,
        };
        let mut render_state = super::render_state_mut();
        let render_state = render_state
            .as_mut()
            .expect("渲染状态在无 GPU 环境下仍应可用（无 surface 的渲染器不碰设备）");
        let cursor = build_cursor(render_state, &wide_tail);
        assert_eq!(cursor.col, 6, "尾列（第 7 格）折算到头列（第 6 格）");

        // 折算只认尾列标志：普通列与宽度为 1 的单元都不受影响。
        wide_tail.at_wide_tail = false;
        assert_eq!(build_cursor(render_state, &wide_tail).col, 7);
    }
}
