package terminal.emulator.runtime

// 滚动新鲜度：手势发生后的一小段时间内，新输出不得把视口拽回底部，
// 渲染循环也 MUST 保持活跃闭锁。两个窗口与视口复位判定都在此。

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
