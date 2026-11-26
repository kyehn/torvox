package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.bridge.Bridge
import terminal.emulator.ui.BackgroundRenderPause
import java.util.concurrent.atomic.AtomicLong

/** 遮罩周期回读间隔：连续输出时至多每 600ms 回读一次。 */
internal const val COVER_CAPTURE_INTERVAL_MS = 600L

/**
 * 开孔遮罩：把保留的上一帧回读成 CPU 像素并按需发布。
 *
 * 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
 * SurfaceView 的 surface 随之被拆；回前台时交换链必须重建，那几十毫秒里窗口的
 * 透明开孔没有缓冲——不遮就是「切后台返回闪一下」。
 *
 * 遮罩的取法：宿主每处理一帧就唤醒一次（[onFramePresented]），把帧累加器的当前
 * 内容回读成 CPU 像素并缓存；切后台时宿主先发布缓存，再尽力回读一次最新帧替下它。
 * 累加器是纯设备纹理（不引用 surface），故回读与窗口是否已死无关；但设备本身会
 * 丢失——那时回读必然失败，所以 MUST 在设备还活着时周期备好一帧，而不是只在切后台
 * 那一刻才回读。
 *
 * 回读在渲染线程之外（[start] 的协程）执行：GPU 同步等待实测可达数十毫秒，放在
 * 渲染线程会拖住所有会话的呈现。
 */
internal class CoverCapture(
    private val scope: CoroutineScope,
    private val backgroundPause: BackgroundRenderPause,
    private val holeJournal: HoleStateJournal,
    private val bufferPresent: () -> Boolean,
    private val activeSession: () -> Pair<Long, Bridge>?,
    private val readFrame: (Bridge) -> Bridge.CoverFrame? = { bridge -> bridge.readCoverFrame() },
) {
    private val frameFlow = MutableStateFlow<Bridge.CoverFrame?>(null)

    /** 当前发布的遮罩；视图在**当前窗口**里画过一帧之后撤掉（[clear]）。 */
    val frame: StateFlow<Bridge.CoverFrame?> = frameFlow.asStateFlow()

    /** 已呈现帧的序号：每有一帧提交进缓冲区就 +1（渲染线程写）。 */
    private val generation = AtomicLong(0)
    private var capturedGeneration = -1L

    /** 强制回读标志：绕开周期回读的节流与暂停检查。 */
    @Volatile private var captureForced = false

    /** 开孔当前是否等着一份遮罩：发布请求已发出、遮罩尚未被视图用掉。 */
    @Volatile private var publishRequested = false

    /** 缓存的遮罩帧：最近一次成功回读的**已呈现帧**。 */
    @Volatile private var storedFrame: Bridge.CoverFrame? = null

    /** 唤醒回读：首帧（缓冲区从无到有）立即回读一次，其后至多每 [COVER_CAPTURE_INTERVAL_MS] 一次。 */
    private val captureSignal = Channel<Unit>(Channel.CONFLATED)

    /**
     * 启动回读循环：随进程存活。切后台时 GPU 回读被暂停（[BackgroundRenderPause]），
     * 恢复后由首帧信号重新唤醒。
     */
    fun start() {
        scope.launch {
            while (currentCoroutineContext().isActive) {
                withTimeoutOrNull(COVER_CAPTURE_INTERVAL_MS) { captureSignal.receive() }
                captureIfNeeded()
            }
        }
    }

    /**
     * 上报一帧已提交进缓冲区。
     *
     * [immediateCapture] 为真（此前无缓冲区）时立即唤醒一次回读：回前台后用户马上
     * 再切后台时手里就有刚离开的那一帧。
     */
    fun onFramePresented(immediateCapture: Boolean) {
        generation.incrementAndGet()
        if (immediateCapture) captureSignal.trySend(Unit)
    }

    /**
     * 把缓存的开孔遮罩发布出去。
     *
     * 切后台与 Surface 销毁都走这里：先把缓存发布出去（只发状态，不碰 GPU）。没有
     * 缓存（冷启动后从未出过帧）时不发布，开孔继续用平底色遮。
     *
     * [refresh] 为真时再强制回读一次：周期回读至多每 [COVER_CAPTURE_INTERVAL_MS]
     * 一次，缓存可能落后于最后一帧，强制回读取到的就是「即将失去窗口缓冲」那一刻
     * 用户最后看到的画面。surface 销毁路径传 false：切后台那次已经刷过，此时设备
     * 往往已随 surface 一同丢失，再回读只会多一条失败日志。
     */
    fun publishStored(refresh: Boolean) {
        publishRequested = true
        storedFrame?.let { publish(it) }
        if (refresh) {
            captureForced = true
            captureSignal.trySend(Unit)
        }
    }

    /**
     * 换了会话：上一会话的遮罩帧不属于新会话的开孔，缓存与已发布帧一起作废。
     */
    fun onSessionSwitched() {
        storedFrame = null
        capturedGeneration = -1L
        clear()
    }

    /**
     * 遮罩已在当前窗口画进开孔：使命完成，撤掉并释放像素。
     *
     * 只由视图的可见绘制调用：原生侧提交帧早于窗口重新可见，运行期无从判断
     * 「这一帧窗口到底有没有画过」。
     */
    fun clear() {
        publishRequested = false
        if (frameFlow.value == null) return
        holeJournal.resetPainted()
        frameFlow.value = null
        holeJournal.record(bufferPresent(), false)
    }

    /** 仅供测试：缓存里是否已有一帧可发布的开孔遮罩。 */
    fun hasStored(): Boolean = storedFrame != null

    /**
     * 发布遮罩帧；尺寸与像素数不符时丢弃——错位的位图比没有遮罩更糟。
     *
     * 两个调用方：主线程的「窗口缓冲即将消失」路径（发布缓存），与 IO 线程的
     * 强制回读路径（发布刚回读到的最新帧）。故本方法只碰线程安全的状态。
     */
    fun publish(frame: Bridge.CoverFrame) {
        val expected = frame.width.toLong() * frame.height.toLong() * 4L
        if (frame.width <= 0 || frame.height <= 0 || frame.pixels.size.toLong() != expected) {
            LogUtil.warn(
                RUNTIME_LOG_TAG,
                "publish: 尺寸与像素数不符（${frame.width}x${frame.height}，" +
                    "${frame.pixels.size} 字节），丢弃遮罩",
            )
            return
        }
        // 新遮罩：上一份「已上屏」的记录随之作废。
        holeJournal.resetPainted()
        frameFlow.value = frame
        holeJournal.record(bufferPresent(), true)
    }

    /**
     * 有一帧尚未回读（或被首帧/强制信号唤醒）时回读一次。
     */
    private fun captureIfNeeded() {
        // 只认进入回读时的那一代：回读期间新到的帧必须留下自己的信号再回读一次，
        // 否则会用旧帧的画面把新帧的序号也标记成已回读，最后一帧永远不更新。
        val targetGeneration = generation.get()
        val forced = captureForced
        if (!forced) {
            // 周期回读：暂停期间不碰 GPU（设备可能在后台丢失，回读必然失败），
            // 且只在有新帧时才回读（没有新画面，回读到的还是同一帧）。
            if (backgroundPause.isPaused) return
            if (targetGeneration == capturedGeneration) return
        }
        // 先清标志再取会话：会话已不在时这次强制回读作废，不留在标志里反复重试。
        captureForced = false
        val (sessionId, bridge) = activeSession() ?: return
        val frame =
            readCoverFrameSafely(
                read = { readFrame(bridge) },
                onFailure = { exception ->
                    LogUtil.warn(
                        RUNTIME_LOG_TAG,
                        "captureCoverFrame: 遮罩回读失败，沿用上一份缓存",
                        exception,
                    )
                },
            ) ?: return
        // 回读期间可能已切换会话：别把旧会话的画面盖到新会话的开孔上。
        if (activeSession()?.first != sessionId) return
        storedFrame = frame
        capturedGeneration = targetGeneration
        LogUtil.debug(
            RUNTIME_LOG_TAG,
            "captureCoverFrame: 已缓存 ${frame.width}x${frame.height} 的开孔遮罩",
        )
        // 开孔正等着一份遮罩（切后台那一刻发出的发布请求）：把刚回读到的新帧发布
        // 出去。周期回读被节流，缓存可能落后于最后一帧；不换上去，遮罩里留的是旧画面。
        if (publishRequested) publish(frame)
    }
}

/**
 * 遮罩回读的安全取回：失败不致命，沿用上一份缓存。
 *
 * 回读发生在 IO 线程的周期回读循环里（[CoverCapture.start]）：那里的异常会终止
 * 回读协程，此后遮罩再也不会更新。故异常在此就地兜住并如实上报——遮罩拿不到
 * 只是沿用旧缓存（用户看到的是闪一下），而回读循环死掉是更坏的结果。
 *
 * 回读**不在**渲染线程的退出路径上：那条路径上设备可能已丢失，回读必然失败
 * （2026-11-17 真机日志），异常还会掀掉整个进程。
 *
 * `CancellationException` MUST 继续向上抛：那是协程取消信号，不是回读失败。
 *
 * @param read 取回遮罩帧；返回 null 表示没有可遮罩的内容。
 * @param onFailure 失败时的上报（异常照实交出，不静默吞掉）。
 */
internal inline fun <T> readCoverFrameSafely(read: () -> T?, onFailure: (Exception) -> Unit): T? = try {
    read()
} catch (exception: CancellationException) {
    throw exception
} catch (exception: Exception) {
    onFailure(exception)
    null
}
