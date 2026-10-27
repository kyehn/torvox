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
use crate::render::GpuError;
use crate::render::cell_builder::SearchHighlight;
use crate::render::cell_builder::{SEARCH_HIGHLIGHT_CURRENT_ALPHA, SEARCH_HIGHLIGHT_OTHER_ALPHA};
use crate::terminal::ShellEnv;
use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::ghostty_terminal::{placeholder_layout_signature, placeholder_row_signatures};
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
    /// 图像内容生成戳：只驱动图集重传（像素变了吗）。
    generation: u64,
    /// 放置生成戳：驱动放置重查（图的位置/集合变了吗）。
    placement_generation: u64,
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
    let placement_generation_changed =
        current.placement_generation != previous.placement_generation;
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
    // 放置移动但视口没动：ED2 撤下图像、TUI 原地重排都走这里。图像存储生成戳
    // 在这些场景不变，只看它会让缓存保留已被撤下的放置，首次 clear 后图像残留。
    // 视口同时移动时交给下面的平移分支，停稳后由补查修正。
    if placement_generation_changed && viewport_shift_rows == 0 {
        return KittyCacheInvalidation {
            sync_cell_size: false,
            refetch_placements: true,
            overlay_dirty: true,
            repack_atlas: false,
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
    /// 主题变更等需要整屏重绘的标志。置位后下一帧所有行标脏，
    /// 避免仅 cursor/highlight 行重绘导致旧底色残留在其余行。
    full_repaint: bool,
    /// 滚动命令在途：滚动已发给 VT 线程，屏幕内容尚未反映它。
    ///
    /// 空闲重绘 MUST NOT 在此期间发生——它只能用缓存帧（旧视口）配新余量重绘，
    /// 画出的内容比实际视口少滚了整行数（滑动慢一拍），且那是一次多余的全量提交。
    /// 滚动后的帧到达即解除（见 [`render_inner`] 的新帧路径）。
    scroll_frame_pending: bool,
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
                placement_generation: u64::MAX,
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
            full_repaint: false,
            scroll_frame_pending: false,
        });
        log::info!("render state initialized (renderer + font pipeline)");
    }
    guard
}

/// 在渲染状态锁内取值，取完即释放锁。
///
/// 锁的持有期只覆盖「读渲染状态」这一段：调用方拿到值后再去调 JNI，而 JNI
/// 可能回调宿主，回调回头取同一把锁即死锁——故取值与交还 JNI 必须分成两段。
fn with_render_state<T>(read: impl FnOnce(&RenderState) -> T) -> Option<T> {
    let state = render_state_mut();
    let value = read(state.as_ref()?);
    drop(state);
    Some(value)
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
const FLOOD_SHED_WARN_INTERVAL_MS: u64 = 5_000;
static FLOOD_SHED_WARN_THROTTLE: crate::slow_log::SlowLogThrottle =
    crate::slow_log::SlowLogThrottle::with_interval(FLOOD_SHED_WARN_INTERVAL_MS);

/// 帧内分段慢帧诊断的限频闸门（两次输出间隔不小于 500ms）。
static SLOW_RENDER_INNER_THROTTLE: crate::slow_log::SlowLogThrottle =
    crate::slow_log::SlowLogThrottle::new();
/// 采样段慢帧诊断的限频闸门（与帧内分段各自限频，互不吞掉）。
static SLOW_RENDER_SAMPLE_THROTTLE: crate::slow_log::SlowLogThrottle =
    crate::slow_log::SlowLogThrottle::new();

fn flood_shed_warn_throttled(session_id: u64, unanswered: usize) {
    if !FLOOD_SHED_WARN_THROTTLE.should_log() {
        return;
    }
    log::warn!(
        "osc52: clipboard read flood from session {session_id}, \
         answering empty past {unanswered} unanswered"
    );
}
mod font;
mod input;
mod query;
mod render;
mod session;
mod state;
mod wake;

use render::{ATLAS_SIZE, DEFAULT_FONT_CELL_SIZE};

#[cfg(test)]
mod cell_size_packing_tests;
#[cfg(test)]
mod clipboard_read_flood_tests;
#[cfg(test)]
mod cursor_anchor_tests;
#[cfg(test)]
mod font_info_packing_tests;
#[cfg(test)]
mod font_size_cap_tests;
#[cfg(test)]
mod idle_repaint_gate_tests;
#[cfg(test)]
mod kitty_placement_keys_tests;
#[cfg(test)]
mod panic_payload_tests;
#[cfg(test)]
mod placeholder_signature_tests;
#[cfg(test)]
mod render_sample_bits_tests;
#[cfg(test)]
mod render_wake_tests;
#[cfg(test)]
mod scroll_bookkeeping_tests;
#[cfg(test)]
mod search_highlight_repaint_tests;
#[cfg(test)]
mod search_highlight_tests;
#[cfg(test)]
mod session_entry_flag_tests;
#[cfg(test)]
mod session_registry_tests;
#[cfg(test)]
mod session_scoped_tests;
