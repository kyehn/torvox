package terminal.emulator.runtime

import android.view.Choreographer.FrameCallback
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * 开孔当前是否已有缓冲区在位，以及首帧提交到真正可见之间的门闸。
 *
 * `SurfaceView` 的原生 surface 位于应用窗口**之下**：窗口在该区域是透明开孔，
 * 缓冲区一旦缺失（切后台被系统销毁、重建窗口、冷启动首帧未出），开孔就透出后方
 * 的桌面/上一个应用——这就是切应用返回时的一闪。开孔无法用窗口背景遮住，只能由
 * 组合层在「还没有缓冲区」这段时间内临时涂上与终端一致的底色，缓冲区一到就撤掉。
 *
 * `present` 只把缓冲**排入** surface 的队列，SurfaceFlinger 要到下一次 vsync 才把
 * 它锁存上屏。若此时立刻撤掉遮罩，开孔在那一个 vsync 里既没有旧画面也没有新画面，
 * 只剩窗口底色——就是「切后台返回闪一下」。故 true 的发布延后
 * [DEFERRAL_FRAMES] 个显示帧；false（surface 被拆）仍然立即生效，遮罩必须马上回来。
 *
 * 为什么是两帧而不是一帧：交换链按 `desired_maximum_frame_latency = 2` 配置，队列
 * 可以压着两帧才交给合成器；应用侧的下一个 frame 回调与合成器的锁存发生在同一个
 * vsync，先后没有保证。差一帧撤遮罩，开孔就空一帧。
 *
 * 每个开孔有一个世代（[epoch]）：渲染线程在进入原生渲染前取一次，提交结果时连同
 * 它上报。两者之间开孔若已失效，那一帧提交的是被平台废弃的缓冲区，不能据此撤遮罩。
 */
internal class SurfacePresentTracker(
    private val postToMain: (() -> Unit) -> Unit,
    private val frameClock: FrameClock,
    private val onFramePresented: (immediateCapture: Boolean) -> Unit,
    private val onBufferPresentChanged: (present: Boolean) -> Unit,
) {
    private val lock = Any()
    private val presentFlow = MutableStateFlow(false)

    /** 当前开孔是否已有一帧提交进缓冲区（只在实际变化时发布）。 */
    val bufferPresent: StateFlow<Boolean> = presentFlow.asStateFlow()

    /** true 的发布正在等显示帧：已排进队列但还没到真正可见。 */
    private var pending = false

    /** 还要再等几个显示帧才发布 true。 */
    private var deferralFrames = 0

    /** 开孔世代：开孔失效（surface 被拆或换了实例）时 +1。 */
    private var token = 0L

    /** 主线程上排队的帧回调：false 发布时撤掉它。 */
    private var frameCallback: FrameCallback? = null

    /** 当前开孔的世代：开孔失效（surface 被拆或换了实例）时 +1。 */
    val epoch: Long get() = synchronized(lock) { token }

    /**
     * 上报「当前开孔是否已有一帧提交进缓冲区」。
     *
     * [reportedEpoch] 是渲染开始前取的 [epoch]：与当前世代不符说明这一帧是旧开孔的
     * 残余，此时上报 true 会把遮罩撤掉、让开孔空着——用户看到的就是「输入法弹出/
     * 切后台返回时闪一下」。故丢弃。
     */
    fun publish(present: Boolean, reportedEpoch: Long = epoch) {
        if (!present) {
            synchronized(lock) {
                // 令牌一变，已排队的帧回调即使稍后执行也不会再置 true。
                token++
                pending = false
                presentFlow.value = false
                onBufferPresentChanged(false)
            }
            postToMain {
                frameCallback?.let(frameClock::remove)
                frameCallback = null
            }
            return
        }
        synchronized(lock) {
            if (reportedEpoch != token) return
            // 新的一帧已呈现：遮罩回读的追赶起点。首帧（此前无缓冲区）立即唤醒回读，
            // 回前台后用户马上再切后台时手里就有刚离开的那一帧。
            onFramePresented(!presentFlow.value && !pending)
            if (presentFlow.value || pending) return
            pending = true
            deferralFrames = DEFERRAL_FRAMES
        }
        val capturedToken = synchronized(lock) { token }
        postToMain {
            val callback = deferralCallback(capturedToken)
            synchronized(lock) {
                if (capturedToken != token || !pending) return@postToMain
                frameCallback = callback
            }
            frameClock.post(callback)
        }
    }

    private fun deferralCallback(capturedToken: Long): FrameCallback = object : FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (consumeDisplayFrame(capturedToken)) {
                // 交换链还压着这一帧：再等一个显示帧，遮罩继续盖着。
                frameClock.post(this)
            }
        }
    }

    /** 消费一个显示帧；返回 true 表示交换链还压着缓冲区，需再等一帧。 */
    private fun consumeDisplayFrame(capturedToken: Long): Boolean {
        return synchronized(lock) {
            if (capturedToken != token || !pending) return@synchronized false
            if (deferralFrames > 0) {
                deferralFrames--
                return@synchronized true
            }
            pending = false
            frameCallback = null
            if (!presentFlow.value) {
                presentFlow.value = true
                // 遮罩不在此撤：这里只知道原生侧提交过帧，不知道**当前窗口**是否已经
                // 画过遮罩。撤下权交给视图的可见绘制（TerminalSurface.onDraw）。
                onBufferPresentChanged(true)
            }
            false
        }
    }

    private companion object {
        /** 首帧提交后到「真正能看见」之间的显示帧数（见类注释）。 */
        const val DEFERRAL_FRAMES = 2
    }
}
