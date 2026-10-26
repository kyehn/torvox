use super::super::*;
use super::frame::render_inner;

// ── 上报位与采样 ────────────────────────────────────────────────────
/// 「视口全空」哨兵：10 位全 1 模式。
pub(crate) const LAST_CONTENT_ROW_NONE_BITS: i64 = 0x3FF;
/// 打包位偏移：内容下沿 43（10 位）、surface 失效位 53、备用屏 54。
pub(crate) const SURFACE_INVALIDATED_BIT: i64 = 1 << 53;
/// 备用屏（helix/vim 等全屏 TUI）激活标志。随每帧渲染结果上报，与内容下沿同批采样，
/// 宿主据此让输入法跟随位移归零。
pub(crate) const ALT_SCREEN_ACTIVE_BIT: i64 = 1 << 54;
/// 采样段 panic 时的上报值：渲染计数为负（-1）、`new_output` 为 0、内容下沿取
/// 未知哨兵、失效位为 0——与本导出文档声明的出错位形一致。
pub(crate) const RENDER_SAMPLE_FAILURE_BITS: i64 = 0xFFFF_FFFF | (LAST_CONTENT_ROW_NONE_BITS << 43);

/// 把一帧的采样拼成上报位。
///
/// 抽成纯函数是为了让「保留位 33..42 MUST 为 0」这条不变量可测：直接在导出里写
/// `| (a << 32) | (b << 43) | …` 时，任何人加进一个新字段都可能悄悄落进保留位，
/// 而 Kotlin 侧不读那 10 位，于是没有任何一侧的测试会红。
#[must_use]
pub(crate) fn pack_render_sample(
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
pub(super) fn last_content_row_bits_for_frame(
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
        if inner_ms < 30.0
            && inner_ms + sample_ms >= 30.0
            && SLOW_RENDER_SAMPLE_THROTTLE.should_log()
        {
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
