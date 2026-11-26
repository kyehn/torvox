package terminal.emulator.runtime

// 运行期的纯计算：网格与视口尺寸、渲染节拍与失败退避、会话与选区判定。
// 这些函数无副作用、无平台状态，故可表驱动单测；把它们与 [TerminalRuntime] 的
// 编排逻辑分开，运行期门面只留「何时调用」与「谁来调用」。
/**
 * 用户滚动手势后的窗口期，期间新输出不得把视口拽回底部（见 [shouldResetScroll]）。
 * 默认 termux 对等路径传入零时间戳，故该保护保持关闭。
 */
const val RECENT_SCROLL_WINDOW_NANOS: Long = 100_000_000L

/**
 * 渲染循环节奏门控的运动新鲜度窗口。手势仍在移动时（运动事件时间戳落在窗口内），
 * 循环保持 17ms 活跃闭锁，即使空闲时钟已因先前 >5s 空闲而陈旧。
 *
 * 滚动卡顿的根因：两个节奏门都只看空闲时钟新鲜度。空闲超 5s 后 vsync 停止驱动且循环驻留
 * 500ms 空闲闭锁，事件间隙（模拟器运动事件稀疏、或主线程忙于手势与重组）内无唤醒源，
 * 循环驻留 ~500ms，滚动帧率跌至 ~2fps。手势一停止即在一帧内老化，空闲闭锁重新生效。
 */
const val SCROLL_MOTION_WINDOW_NANOS: Long = 250_000_000L

/**
 * 滚动复位决策（termux `onScreenUpdated` 对等语义），纯函数无副作用以便表驱动测试。
 *
 * termux 语义：除文本选区激活或自动滚动被显式关闭外，新输出都会把视口滚回底部。
 * termux 的 `isAutoScrollDisabled()` 是宿主应用的显式开关，不由用户滚动触发，
 * 故默认走分支 A（recentlyScrolled 保持 false）。
 *
 * @param scrollActive SCROLL 按钮显式锁定（用户想停留在回浏览位置）。
 * @param hasSelectionOrDrag 选区激活或手柄拖拽进行中。
 * @param newOutput 本帧消费到的原生 PTY 摄入标志（不是 render() 计数，后者含空闲重绘）。
 * @param recentlyScrolled UI 滚动手势后 [RECENT_SCROLL_WINDOW_NANOS] 内为真，
 *   使进行中的滚动不被落在手势中途的输出拽回。
 */
internal fun shouldResetScroll(
    scrollActive: Boolean,
    hasSelectionOrDrag: Boolean,
    newOutput: Boolean,
    recentlyScrolled: Boolean = false,
): Boolean = newOutput && !hasSelectionOrDrag && !scrollActive && !recentlyScrolled

/**
 * 初始同步渲染重试（switchSession）：只有硬失败（< 0：会话未注册、渲染态异常等）
 * 才重试。**`0` 不是失败**：它是「本帧没有可画的内容」，surface 尚未就绪、目标会话
 * 暂无新帧、缓存帧属于别的会话都会返回 0——切会话后重推请求已由原生渲染路径发出，
 * 该帧到达即呈现，重试只会空转。纯决策，从重试循环中抽出以便测试。
 */
internal fun initialRenderRetryNeeded(result: Int, attempts: Int, maxAttempts: Int): Boolean =
    result < 0 && attempts < maxAttempts

/**
 * switchSession 阶段 3 的并发切换守卫：首帧渲染期间（在 SessionRegistry 的总闸之外），
 * 另一个 switchSession 可能发布了不同的活动会话。返回在发布本次切换之前
 * 必须停止其渲染线程的会话 id；当活动会话未变（或正是本目标）时返回 null。
 */
internal fun concurrentRenderThreadToStop(
    activeSessionIdAfterRender: Long?,
    previousActiveId: Long?,
    targetId: Long,
    concurrentSessionId: Long?,
): Long? {
    if (activeSessionIdAfterRender == previousActiveId) return null
    val concurrentId = concurrentSessionId ?: return null
    if (concurrentId == targetId) return null
    return concurrentId
}

/** switchSession 阶段 1 的失败恢复：spawn 失败后重启前一个活动会话，除非它正是刚刚失败的那个会话。 */
internal fun shouldRestorePreviousSession(previousId: Long?, failedTargetId: Long): Boolean =
    previousId != null && previousId != failedTargetId
