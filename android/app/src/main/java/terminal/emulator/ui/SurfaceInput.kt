package terminal.emulator.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.SystemClock
import android.view.GestureDetector
import android.view.InputDevice
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.ViewConfiguration
import android.view.WindowInsets.Type
import androidx.core.net.toUri
import androidx.core.view.HapticFeedbackConstantsCompat
import terminal.emulator.input.KeyModifiers
import terminal.emulator.runtime.LogUtil
import java.util.Locale
import kotlin.math.abs
import kotlin.math.max

/** 单次触摸手势转发的滚轮行数上限：无界 repeat 会在主线程逐行同步
 * 等待原生查询（VT 忙时每行最长 500ms），大幅滑动即冻结 UI。
 * 超限行数由后续手势事件携带新坐标补发——丢弃旧坐标而非阻塞等待。
 */
private const val MAX_WHEEL_LINES_PER_GESTURE = 8

private const val ZOOM_THRESHOLD_LOW = 0.9f
private const val ZOOM_THRESHOLD_HIGH = 1.1f

// ⑥ 双指缩放：预览边界与 TerminalScreen 的 FONT_SIZE 钳位一致；
// 预览频率为每秒数次，使塑形手感平滑而无需逐帧重排 ghostty
// （模拟器上帧基线 41ms）。
private const val ZOOM_PREVIEW_INTERVAL_NANOS = 60_000_000L // 60ms

/** 从屏幕边缘起算的抽屉手势宽度（dp）。 */
private const val DRAWER_EDGE_GESTURE_DP = 32

/**
 * 拥有终端 Surface 的输入处理：手势识别、触摸派发、惯性滚动、长按选词与鼠标上报。
 *
 * 视图只负责把平台事件转交本类并保留自身状态（网格、选区、输入法遮挡）；
 * 手势策略与惯性物理全在此处，便于按输入语义单点修改。
 */
internal class SurfaceInput(private val surface: TerminalSurface) {
    /**
     * 惯性滚动物理：由本视图按 vsync 自驱的指数衰减（[flingOffsetPx]），
     * 不再交给 [android.widget.OverScroller]。[postOnAnimation] 把各步对齐到 vsync。
     *
     * 为什么不用平台 scroller：厂商 ROM 会改写它的样条。真机日志（厂商 ROM）里
     * `fling, after Optimize, velocityY=-14061 … adjust Duration, min to mFinal`
     * 紧跟一行 `force Finished` —— 整段惯性被折叠成一帧落点，一次猛甩直接跳十几行
     * 并戛然而止，用户看到的就是「上下滑动异常并且卡顿」。自建模型是纯函数：
     * 跨设备一致、可单测、没有平台隐藏状态。
     *
     * 行程单位是**像素**：逐帧增量交给拖动同一条管道（[applyScrollStep]），
     * 整行与亚行余量一起算，内容才能按像素跟随惯性；按行驱动时每帧落点被截成
     * 整行，表现为逐行跳动（STATUS ⑩）。上界含键盘位移的显出量容量，
     * 猛甩也能走到「显出被位移裁掉的网格顶部」的行程末端（STATUS ⑧）。
     */
    private val flingStepRunnable = Runnable { doFlingStep() }

    /** 惯性起点（偏移空间像素：`surface.scrollOffset * surface.cellHeight + 亚行余量`）。 */
    private var flingStartPx = 0f

    /** 惯性起点时刻（[SystemClock.uptimeMillis]，与 vsync 同一时基）。 */
    private var flingStartUptimeMs = 0L

    /** 手势抬起时的速度（像素/秒，手势坐标系：手指上移为负）。 */
    private var flingVelocityPxPerS = 0f

    /** 偏移空间的上界：回滚长度 + 键盘位移显出量容量。 */
    private var flingLimitPx = 0f

    private var flingActive = false

    /** 上一惯性帧的像素位置：逐帧差值即本帧行程。 */
    private var lastFlingPx = 0f

    /** 把惯性滚动动画推进一个 vsync 节拍。 */
    private fun doFlingStep() {
        if (!flingActive) return
        val elapsedMs = SystemClock.uptimeMillis() - flingStartUptimeMs
        val travelPx = flingOffsetPx(flingVelocityPxPerS, elapsedMs)
        val targetPx = flingStartPx + travelPx
        val clampedPx = targetPx.coerceIn(0f, flingLimitPx)
        val deltaPx = clampedPx - lastFlingPx
        lastFlingPx = clampedPx
        if (deltaPx != 0f) {
            val scrollbackLen = surface.currentScrollbackLength()
            // 与 onScroll 同符号：手指下移 distanceY 为负、偏移增大（更旧），
            // 而惯性像素增大的方向也是更旧，故取负后再喂。
            surface.applyScrollStep(
                applyScrollDistance(
                    surface.scrollAccumulatorPx,
                    -deltaPx,
                    surface.cellHeight,
                    surface.scrollOffset,
                    scrollbackLen,
                    surface.ime.imeShiftRevealPx,
                    surface.ime.imeShiftCapacityPx.toFloat(),
                ),
                scrollbackLen,
            )
        }
        // 撞到边界（被钳制）或衰减到行程已尽：收尾，不再空转 vsync 回调。
        val reachedBound = clampedPx != targetPx
        val settled =
            elapsedMs >= FLING_MAX_DURATION_MS ||
                abs(flingOffsetPx(flingVelocityPxPerS, FLING_MAX_DURATION_MS) - travelPx) < FLING_SETTLE_PX
        if (reachedBound || settled) {
            finishFlingAnimation()
            return
        }
        surface.postOnAnimation(flingStepRunnable)
    }

    /** 停止进行中的惯性动画（触点按下、程序化滚动）。
     *  惯性尚未自然结束时，这也会执行收尾语义——清除亚像素余量并触发
     *  surface.onScrollingStateChanged(false)——使中断后渲染循环的 shouldResetScroll
     *  不会被陈旧的滚动激活标志阻塞。收尾本身复用 [finishFlingAnimation]，
     *  两条路径不得漂移：少清一项就会让 scrollActive 残留、阻塞自动回底。 */
    internal fun stopFlingAnimation() {
        if (flingActive) {
            finishFlingAnimation()
        }
    }

    private fun finishFlingAnimation() {
        flingActive = false
        surface.removeCallbacks(flingStepRunnable)
        surface.isScrolling = false
        surface.scrollAccumulatorPx = 0f
        surface.viewModel?.runtime?.setScrollRemainderPx(0f)
        surface.onScrollingStateChanged?.invoke(false)
    }

    @JvmField var isAfterLongPress = false

    @JvmField var scaleFactor = 1.0f

    // ⑥ 双指缩放状态：手势以已渲染字号为锚，每次 onScale 插值；
    // 预览推入度量而不 resize，onScaleEnd 时终结一次。
    private var zoomActive = false
    private var zoomBaseFontSizeSp = 0f
    private var lastZoomPreviewNanos = 0L

    private var longPressDragging = false
    private var longPressStartX = 0f
    private var longPressStartY = 0f

    /**
     * 长按后是否已越过触摸阈值，转入真正的拖动扩选。
     *
     * 长按选词后手指的微动（长按自身的抖动，真机上必然出现）不该改写刚选出的词：
     * 越过阈值前 ACTION_MOVE 一律不动选区。少了这道守卫，1px 位移就把
     * `(1,6)-(1,10)` 的整词塌缩成 `(1,6)-(1,7)`，菜单随选区重锚，
     * 用户点下去的一瞬正好点空——表象即「所有菜单按钮失效、点一下菜单自己消失」。
     */
    private var longPressDragStarted = false

    /** 最近按下的鼠标键号（0=左、1=右、2=中）；ACTION_UP 的按键状态已清零，须沿用它。 */
    private var lastMouseButton = 0

    /**
     * 本次手势是否已把拖动转发为滚轮（见 onScroll）：转发过的手势其惯性段
     * 同样属于应用，onFling 只吞不滚，绝不让本地回滚接管（见 onFling）。
     * 手势开始时在 onDown 重置。
     */
    private var wheelForwarded = false

    private val gestureListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(motionEvent: MotionEvent): Boolean {
                // 手势开始时重置滚轮转发标记：惯性段只属于做出转发的那次手势。
                wheelForwarded = false
                // 手势开始时重置亚单元格累加器，使首次 onScroll 距离从干净的起点算起。
                // 同步本地偏移与运行时真源：渲染线程回底后本地仍旧值，下次手势若从旧值起算会跳变。
                surface.viewModel?.runtime?.activeSessionScrollOffset()?.let {
                    surface.scrollOffset = it
                    // 会话切换连回滚长度一起换掉，位移显出量的上界随之变化（见 applyImeShift）。
                    surface.ime.applyImeShift()
                }
                surface.scrollAccumulatorPx = 0f
                surface.viewModel?.runtime?.setScrollRemainderPx(0f)
                return true
            }

            override fun onShowPress(motionEvent: MotionEvent) {
                isAfterLongPress = false
            }

            override fun onScroll(
                firstEvent: MotionEvent?,
                secondEvent: MotionEvent,
                distanceX: Float,
                distanceY: Float,
            ): Boolean {
                // 选词中移动由 ACTION_MOVE 长按拖动分支消费；此处吞掉避免双路径竞争致摇晃。
                if (surface.isSelectingText) return true
                // 触摸拖动 = 应用滚动：应用启用鼠标上报时经 Ghostty 鼠标编码器
                // 把每个滚过的行转发成一个滚轮事件，对标 termux TerminalView 的
                // scrolledWithFinger——yazi 等主屏 TUI 里拖动即应用内滚动。
                // 应用未启用上报时编码器返回 false（不写 PTY），主屏回落到本地
                // 回滚；备用屏（vim/less/htop）没有回滚，拖动静默吞掉。
                // 与 onGenericMotionEvent 中的外接鼠标路径一致。
                val wheelBridge = surface.viewModel?.runtime?.bridge()
                if (wheelBridge != null && wheelBridge.isMouseTrackingActive()) {
                    val mouseCellWidth = surface.viewModel?.runtime?.cellWidth ?: 1f
                    val mouseCellHeight = surface.viewModel?.runtime?.cellHeight ?: 1f
                    val pointerXPx = secondEvent.x
                    val pointerYPx = secondEvent.y
                    // 走满一个单元格高度 = 一个滚轮行，与下方的本地滚动映射
                    // （distanceY / surface.cellHeight）一致。
                    val lines = max(1, abs((distanceY / surface.cellHeight).toInt()))
                    // 手指上移（distanceY > 0）= 滚轮上（3，较旧）；手指下移 = 滚轮下（4，较新）
                    val button = if (distanceY > 0f) 3 else 4
                    val cappedLines = lines.coerceAtMost(MAX_WHEEL_LINES_PER_GESTURE)
                    // 一次往返编码全部档位：逐档 encodeMouseEvent 会把 UI 线程
                    // 按在每档一次 VT 应答上（滚动手势一次可滚十余行）。
                    wheelForwarded =
                        wheelBridge.encodeMouseWheel(
                            pointerXPx,
                            pointerYPx,
                            button,
                            KeyModifiers.ghosttyMods(secondEvent.metaState),
                            mouseCellWidth,
                            mouseCellHeight,
                            cappedLines,
                        )
                }
                if (wheelForwarded) return true
                if (wheelBridge != null && wheelBridge.isAltScreenActive()) return true
                // 基线重基到当前真值口径：浏览期间新输出把活动区底边推远，旧偏移会让
                // 本次增量算在错误的起点上（回到底边时更会停在底边之上）。
                val scrollbackLen = surface.rebaseScrollOffset()
                if (!surface.isScrolling) {
                    surface.isScrolling = true
                    surface.onScrollingStateChanged?.invoke(true)
                }
                // 亚单元格累加器：distanceY < surface.cellHeight 绝不能被丢弃，
                // 只发出整行，余量带入下一次 onScroll。
                // 方向：手指下移（distanceY<0，currentY - previousY）→ 更旧的历史
                // （偏移增大，视口顶部行号减小）→
                // 对应 termux TerminalView:onScroll 的 deltaRows = distanceY / lineSpacing
                // 与 doScroll rowsDown>0 → mTopRow+1（更新）、rowsDown<0 → mTopRow-1（更旧）。
                // 即 distanceY 为负(下移)时 deltaRows 为负,对应 older,与本实现 surface.scrollOffset 增加一致。
                // 截断趋向零（toInt）：正/负亚行阈值对称，消除 floor 非对称导致的漂移。
                // 注意符号:distanceY = previousY - currentY,下移为负,需取反累加才能使下移增加偏移。
                val scrollStep =
                    applyScrollDistance(
                        surface.scrollAccumulatorPx,
                        distanceY,
                        surface.cellHeight,
                        surface.scrollOffset,
                        scrollbackLen,
                        surface.ime.imeShiftRevealPx,
                        surface.ime.imeShiftCapacityPx.toFloat(),
                    )
                surface.applyScrollStep(scrollStep, scrollbackLen)
                return true
            }

            override fun onFling(
                firstEvent: MotionEvent?,
                secondEvent: MotionEvent,
                velocityX: Float,
                velocityY: Float,
            ): Boolean {
                // 选词中 fling 同样吞掉，避免落到滚动路径致视口跳变。
                if (surface.isSelectingText) return true
                // 本次手势的拖动已把滚轮转发给应用：惯性段属于应用，
                // 直接吞掉，绝不让本地回滚接管（见 onScroll 的 wheelForwarded）。
                if (wheelForwarded) return true
                // 在备用屏（vim/less/htop）上，fling 绝不能滚动本地回滚
                // ——该手势属于远端。此处直接丢弃（消费掉）而不转发，
                // 因为惯性速度没有干净的滚轮行数映射；
                // 拖动滚动（onScroll）已经按行转发滚轮事件。
                val flingBridge = surface.viewModel?.runtime?.bridge()
                if (flingBridge != null && flingBridge.isAltScreenActive()) {
                    return true
                }
                val scrollbackLen = surface.rebaseScrollOffset()
                val absoluteX = abs(velocityX)
                val absoluteY = abs(velocityY)

                // 横向 fling 消费掉但不做任何事：DESIGN 只要求修饰键栏左右滑动，
                // 终端内容区的横向甩动没有声明语义。此前它被映射成向运行中程序注入
                // ESC / Tab（500px/s 的门槛极低），会在 vim/less 或半行命令中直接
                // 破坏用户输入。
                if (absoluteX > absoluteY) {
                    return true
                }

                // velocityY 系像素/秒，偏移空间同样是像素：不再除行高换成行/秒
                // （那样逐帧落点是整行，内容只能逐行跳，STATUS ⑩）。行程起点带上
                // 亚行余量、上界带上键盘位移的显出量容量，见 [flingOffsetPx]。
                stopFlingAnimation()
                surface.isScrolling = true
                surface.onScrollingStateChanged?.invoke(true)
                flingStartPx = surface.scrollOffset * surface.cellHeight + surface.scrollAccumulatorPx
                flingVelocityPxPerS = velocityY
                flingLimitPx = scrollbackLen * surface.cellHeight + surface.ime.imeShiftCapacityPx
                flingStartUptimeMs = SystemClock.uptimeMillis()
                // 起点先夹进范围：不夹的话首帧差值会多出被夹掉的一段、方向相反。
                lastFlingPx = flingStartPx.coerceIn(0f, flingLimitPx)
                flingActive = true
                surface.postOnAnimation(flingStepRunnable)
                return true
            }

            override fun onSingleTapUp(event: MotionEvent): Boolean {
                // 长按抬手不是轻击：直接返回，不触碰选区。
                if (isAfterLongPress) {
                    isAfterLongPress = false
                    longPressDragging = false
                    return true
                }
                val now = SystemClock.uptimeMillis()
                // 300ms 隐藏保护：手柄拖动松手后的首次轻击属于拖动手势的收尾，
                // 而非新的轻击。先于其他判定拦截，避免拖尾关闭刚重显的菜单。
                if (shouldSuppressTapAfterDragEnd(now, surface.handleDragController.lastDragEndUptimeMs)) {
                    return true
                }
                // 抽屉关闭动画会让遮罩轻击穿透到 Surface；
                // 不要把它当作终端轻击（那会清除选区）。
                if (System.nanoTime() < surface.suppressUntilNanos) {
                    return true
                }
                if (surface.isScrolling) {
                    // 只结束滚动状态；绝不要把 surface.scrollOffset 重置为 0，
                    // 否则每次轻击都会撤销用户的滚动，使回滚浏览不可用
                    // （「滚动不起作用」的反馈）。像素余量确实会被重置，
                    // 使视图停在整行而非行中间。
                    surface.isScrolling = false
                    surface.scrollAccumulatorPx = 0f
                    surface.viewModel?.runtime?.setScrollRemainderPx(0f)
                    surface.onScrollingStateChanged?.invoke(false)
                    return true
                }
                // 鼠标上报中的 TUI（yazi 等）：轻点即左键单击，对标 termux
                // TerminalView.onUp 的语义——编码器在应用未启用鼠标上报时返回 false
                // 且不写 PTY，轻点照常走链接/选区/焦点路径。开启上报时点击属于
                // 运行中的应用，链接不抢在它之前。
                if (!surface.isSelectingText && encodeMouseTap(event)) {
                    return true
                }
                // OSC 8 超链接轻点（对标 termux TerminalView.openLinkAt）：
                // 轻点超链接单元格会打开该 URI，而不是拉起键盘。
                if (openLinkAt(event.x, event.y)) {
                    return true
                }
                if (surface.isSelectingText) {
                    surface.selectionHandles.hideSelectionHandles()
                    surface.viewModel?.clearSelection()
                    surface.post {
                        // minSdk 33：可直接使用平台的 WindowInsetsController；ViewCompat 的辅助方法已弃用。
                        val controller = surface.windowInsetsController
                        controller?.hide(
                            Type.ime(),
                        )
                    }
                    return true
                }
                surface.viewModel?.clearSelection()
                surface.suppressUntilNanos = 0L
                // 焦点照常请求：硬件键盘的按键靠它送达，与软键盘是否可用无关。
                surface.requestFocus()
                // 软键盘已被 KEYBOARD 键禁用（`enable/disable`）：轻点只结束选区与滚动。
                // 这里若继续置 surface.keyboardRequested 并 show，禁用就等于形同虚设。
                if (!surface.softKeyboardEnabled) return true
                surface.keyboardRequested = true
                surface.post {
                    val controller = surface.windowInsetsController
                    controller?.show(
                        Type.ime(),
                    )
                }
                return true
            }

            override fun onLongPress(event: MotionEvent) {
                if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
                isAfterLongPress = true
                longPressDragging = true
                longPressDragStarted = false
                longPressStartX = event.x
                longPressStartY = event.y
                handleLongPress(event.x, event.y)
            }
        }

    private val gestureDetector =
        GestureDetector(surface.context, gestureListener).also {
            // 不支持双击/多击选择：禁用框架双击检测，使每次点击都触发 onSingleTapUp。
            it.setOnDoubleTapListener(null)
        }

    private val scaleDetector =
        ScaleGestureDetector(
            surface.context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                    if (surface.isSelectingText) return false
                    zoomBaseFontSizeSp = surface.viewModel?.runtime?.appliedFontSizeSp() ?: return false
                    zoomActive = true
                    // scaleFactor 同时充当长按守卫：双指缩放占用触摸序列期间，onLongPress 会跳过。
                    scaleFactor = 1.0f
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    if (!zoomActive || surface.isSelectingText) return false
                    scaleFactor *= detector.scaleFactor
                    val runtime = surface.viewModel?.runtime ?: return false
                    val sizeSp =
                        zoomFontSize(
                            zoomBaseFontSizeSp,
                            scaleFactor,
                            runtime.spToPxScale,
                            runtime.screenWidthDp,
                        )
                    val now = System.nanoTime()
                    if (now - lastZoomPreviewNanos >= ZOOM_PREVIEW_INTERVAL_NANOS) {
                        lastZoomPreviewNanos = now
                        surface.onZoomPreview?.invoke(sizeSp)
                    }
                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector) {
                    if (!zoomActive) return
                    zoomActive = false
                    val runtime = surface.viewModel?.runtime ?: return
                    // 落定判定用收敛后的字号：它就是将被落盘的值，且与基准同为档位值，
                    // 相等即「字号未变」（手势没动，或只动到同一档）。
                    val sizeSp =
                        zoomFontSize(
                            zoomBaseFontSizeSp,
                            scaleFactor,
                            runtime.spToPxScale,
                            runtime.screenWidthDp,
                        )
                    scaleFactor = 1.0f
                    if (zoomSettledOnNewSize(zoomBaseFontSizeSp, sizeSp)) {
                        // 手势稳定在新尺寸上：持久化并完整应用（单次网格重排）。
                        surface.onZoomChanged?.invoke(sizeSp)
                    } else {
                        // 回到锚定尺寸：撤销已推入原生度量的预览。
                        surface.onZoomPreview?.invoke(zoomBaseFontSizeSp)
                    }
                }
            },
        )

    fun handleLongPress(pixelX: Float, pixelY: Float) {
        // 长按 → 词选择。不支持双击/多击选择，词选择只走长按一条路径。
        if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
        isAfterLongPress = true

        surface.performHapticFeedback(HapticFeedbackConstantsCompat.LONG_PRESS)

        surface.selectionHandles.hideSelectionHandles()

        val bridge = surface.viewModel?.runtime?.bridge()
        val rawCol = (pixelX / surface.cellWidth).toInt().coerceIn(0, (surface.cols - 1).coerceAtLeast(0))
        val row = (pixelY / surface.cellHeight).toInt().coerceIn(0, (surface.rows - 1).coerceAtLeast(0))
        val gridRow = surface.currentViewportTopGrid() + row
        // 落点先吸附到字符起始列：宽字符后半格在行文本里是空格占位，不吸附就会被
        // 当成空白而弹出仅粘贴菜单，长按整个字都选不中。
        val col = surface.handleDragController.snapToWideCharBoundary(gridRow, rawCol)

        // 我们则落到单格反色 + 粘贴菜单。
        val line = bridge?.scrollbackLine(gridRow)
        // 空白目标 = 空行、空白单元格，或行尾之后的任何列
        // ——termux 的 getSelectedText(x,y,x,y) 对三者都返回 ""，
        // 故它们都必须归类为仅粘贴。原先的 `col < line.length` 合取条件
        // 把行尾各列归类为文本，这正是「在提示符右侧长按
        // 却弹出带 PASTE 的完整菜单」的根因。
        val isOnWhitespace = isWhitespaceCell(line, col)

        if (isOnWhitespace) {
            surface.viewModel?.startSelection(gridRow, col, pasteOnly = true)
            surface.viewModel?.endSelection()

            // （termux 对等）：空白单元格的选区也显示两个手柄叠在该单元格上
            // ——拖动任一手柄都会从空白处扩展出范围选区，
            // 与 termux 的 setInitialTextSelectionPosition 流程完全一致。
            surface.selectionHandles.showSelectionHandles(gridRow, col, gridRow, col, surface.getAccentColor())

            LogUtil.debug(
                "Selection",
                "LONG_PRESS whitespace: row=$row col=$col gridRow=$gridRow " +
                    "menu=PASTE_ONLY",
            )
        } else {
            // 上游 select_word 派生词界（ghostty 默认边界：空白与
            // `'"\`│|:;,()[]{}<>$`），长按走 native 派生；
            // native 侧已安装选区，回传的有序界限驱动状态与控制柄。
            val bounds = resolveWordSelectionBounds(bridge?.selectWordAt(gridRow, col), gridRow, col)

            LogUtil.debug(
                "Selection",
                "LONG_PRESS text: tapRow=$row tapCol=$col gridRow=$gridRow " +
                    "expanded start=(${bounds.startRow},${bounds.startCol}) " +
                    "end=(${bounds.endRow},${bounds.endCol}) " +
                    "menu=FULL cellW=${surface.cellWidth} cellH=${surface.cellHeight} surface.rows=${surface.rows} surface.cols=${surface.cols}",
            )

            surface.viewModel?.startSelection(bounds.startRow, bounds.startCol)
            surface.viewModel?.updateSelection(bounds.endRow, bounds.endCol)
            surface.viewModel?.endSelection()
            surface.selectionHandles.showSelectionHandles(
                bounds.startRow,
                bounds.startCol,
                bounds.endRow,
                bounds.endCol,
                surface.getAccentColor(),
            )
        }
    }

    /**
     * 鼠标上报中的 TUI（yazi 等）轻点即左键单击：经 Ghostty 鼠标编码器把
     * 按下/抬起各编码一次写入 PTY。编码器在应用未启用鼠标上报时返回 false
     * 且不写 PTY（见 Bridge.encodeMouseEvent），此时返回 false 让轻点照常
     * 走链接/选区/焦点路径；两次编码间会话被重建时可能只送出按下，与
     * termux TerminalView.onUp 的行为一致——远端由自己的超时兜底。
     */
    private fun encodeMouseTap(event: MotionEvent): Boolean {
        val runtime = surface.viewModel?.runtime ?: return false
        val bridge = runtime.bridge() ?: return false
        val modifiers = KeyModifiers.ghosttyMods(event.metaState)
        val mouseCellWidth = runtime.cellWidth
        val mouseCellHeight = runtime.cellHeight
        val press = bridge.encodeMouseEvent(event.x, event.y, 0, 0, modifiers, mouseCellWidth, mouseCellHeight)
        if (!press) {
            return false
        }
        bridge.encodeMouseEvent(event.x, event.y, 1, 0, modifiers, mouseCellWidth, mouseCellHeight)
        return true
    }

    /**
     * 在视口像素 (pixelX, pixelY) 处打开 OSC 8 超链接（若有）。对标 termux 的
     * TerminalView.openLinkAt：经 surface.cellWidth/surface.cellHeight 完成像素→单元格映射，
     * 再查询原生超链接 URI 并启动系统处理器。成功打开链接时返回 true。
     */
    private fun openLinkAt(pixelX: Float, pixelY: Float): Boolean {
        if (surface.cellWidth <= 0f || surface.cellHeight <= 0f) return false
        val col = pixelToCell(pixelX, surface.cellWidth, surface.cols)
        val row = pixelToCell(pixelY, surface.cellHeight, surface.rows)
        val gridRow = surface.currentViewportTopGrid() + row
        val bridge = surface.viewModel?.runtime?.bridge() ?: return false
        val url = bridge.hyperlinkAt(gridRow, col) ?: return false
        if (url.isBlank()) return false
        val uri =
            try {
                url.trim().toUri()
            } catch (urlError: IllegalArgumentException) {
                LogUtil.warn(TerminalSurface.TAG, "openLinkAt: bad URI", urlError)
                return false
            }
        // 协议白名单：终端输出不可信，故只允许打开 http(s)
        // （阻止 OSC 8 中的 intent:/file:/javascript:）。
        val scheme = uri.scheme?.lowercase(Locale.ROOT)
        if (scheme != "http" && scheme != "https") {
            LogUtil.warn(TerminalSurface.TAG, "openLinkAt: rejected non-http(s) scheme: $scheme")
            return false
        }
        return try {
            val intent =
                Intent(Intent.ACTION_VIEW, uri)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            surface.context.startActivity(intent)
            true
        } catch (missingHandler: ActivityNotFoundException) {
            LogUtil.warn(TerminalSurface.TAG, "openLinkAt: no handler for $uri", missingHandler)
            false
        } catch (blockedError: SecurityException) {
            LogUtil.warn(TerminalSurface.TAG, "openLinkAt: blocked for $uri", blockedError)
            false
        }
    }

    private fun startWordSelectionAt(event: MotionEvent) {
        val col = pixelToCell(event.x, surface.cellWidth, surface.cols)
        val row = pixelToCell(event.y, surface.cellHeight, surface.rows)
        // 上游 select_word 词界 — 与长按完全同一 native 派生、同一回退函数。
        val gridRow = surface.currentViewportTopGrid() + row
        val bounds =
            resolveWordSelectionBounds(
                surface.viewModel?.runtime?.bridge()?.selectWordAt(gridRow, col),
                gridRow,
                col,
            )
        surface.viewModel?.startSelection(bounds.startRow, bounds.startCol)
        surface.viewModel?.updateSelection(bounds.endRow, bounds.endCol)
        // 回退分支同样须收尾：只 startSelection 会把选区永久留在拖动态，
        // 此后输入不再自动贴底。
        surface.viewModel?.endSelection()
        LogUtil.debug(
            "Selection",
            "RIGHT_CLICK word: tapRow=$row tapCol=$col " +
                "expanded start=(${bounds.startRow},${bounds.startCol}) " +
                "end=(${bounds.endRow},${bounds.endCol})",
        )

        surface.showMagnifierAt(event.rawX, event.rawY)
    }

    // 派发约 15 种彼此不同的手势/意图，涵盖选区、滚动、长按与硬件按键交互。
    fun onTouchEvent(event: MotionEvent): Boolean {
        val isRelease = event.actionMasked == MotionEvent.ACTION_UP
        // 覆盖整条事件流，不只是抬手：叠加层可见时 Surface 必须把 DOWN/MOVE 一并让出，
        // 否则它会消费 DOWN（连同 requestDisallowIntercept）把事件流从叠加层手里抢走，
        // 且在叠加层底下启动选区拖动。
        if (!surface.touchEnabled) {
            scaleDetector.onTouchEvent(event)
            gestureDetector.onTouchEvent(event)
            return false
        }
        if (event.action == MotionEvent.ACTION_DOWN) {
            surface.parent?.requestDisallowInterceptTouchEvent(true)
            // 新触摸会停止进行中的惯性动画（标准 Android 可滚动行为；termux 在 onTouchEvent 中亦同）。
            stopFlingAnimation()
        }
        // 抽屉的边缘滑动手势始于屏幕边缘区（~32dp）。Surface 绝不能消费这些触摸：
        // 当 surface.drawerOpen 为假时，Surface 本会收到没有 MOVE 的 DOWN + UP
        // （MOVE 都被抽屉手势取走），GestureDetector 会把它判为轻击而清除选区。
        // 边缘触摸始终属于抽屉手势。抽屉打开时，Surface 必须把全部触摸让给
        // 遮罩（其关闭手势），否则 DOWN 上的 requestDisallowIntercept
        // 会抢走事件流，抽屉将永远无法关闭。
        val drawerEdgePixels =
            (DRAWER_EDGE_GESTURE_DP * surface.resources.displayMetrics.density).toInt()
        if (surface.drawerOpen || event.x < drawerEdgePixels) {
            return false
        }
        // 走到这里才说明这次抬手属于终端 Surface（未被抽屉遮罩或边缘手势取走），
        // 此时才播报点击。提前播报会让「轻点遮罩关闭抽屉」「抽屉边缘滑动收尾」
        // 这两条根本不属于终端的抬手被朗读成「已点击」。
        // （滚动与长按拖动的抬手仍在下游被消费，同样会走到这里。）
        if (isRelease) surface.performClick()

        val fromMouse = event.isFromSource(InputDevice.SOURCE_MOUSE)

        if (fromMouse) {
            // 鼠标模式上报（DECSET 1000/1002/1003）：经 Ghostty 鼠标编码器
            // 把鼠标事件路由到终端。应用未启用鼠标上报时编码器返回空序列，
            // 此时事件落到下方的应用手势（右键选词、中键粘贴）。
            val runtime = surface.viewModel?.runtime
            val bridge = runtime?.bridge()
            if (bridge != null) {
                val mouseCellWidth = runtime.cellWidth
                val mouseCellHeight = runtime.cellHeight
                // ACTION_UP/CANCEL 的 getButtonState 已清零（isButtonPressed 全假），
                // 释放帧必须沿用按下时记下的键号：否则 SGR 上报把右/中键释放编码成
                // 左键释放（`<0;x;ym`），远端（tmux、TUI）按下的键永远配不上抬起。
                val pressedButton =
                    when {
                        event.isButtonPressed(MotionEvent.BUTTON_SECONDARY) -> 1
                        event.isButtonPressed(MotionEvent.BUTTON_TERTIARY) -> 2
                        event.isButtonPressed(MotionEvent.BUTTON_PRIMARY) -> 0
                        else -> null
                    }
                if (pressedButton != null) lastMouseButton = pressedButton
                val button = pressedButton ?: lastMouseButton
                val action =
                    when (event.actionMasked) {
                        MotionEvent.ACTION_UP,
                        MotionEvent.ACTION_CANCEL,
                        -> 1

                        MotionEvent.ACTION_MOVE -> 2

                        else -> 0
                    }
                if (
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        action,
                        button,
                        KeyModifiers.ghosttyMods(event.metaState),
                        mouseCellWidth,
                        mouseCellHeight,
                    )
                ) {
                    return true
                }
            }

            when {
                event.isButtonPressed(MotionEvent.BUTTON_SECONDARY) -> {
                    when (event.action) {
                        MotionEvent.ACTION_DOWN -> {
                            surface.viewModel?.clearSelection()
                            startWordSelectionAt(event)
                        }

                        // 本分支对 DOWN 之外的动作也 `return true`，走不到手势收尾处的
                        // dismiss——而放大镜正是 DOWN 这里弹出的（startWordSelectionAt）。
                        // 不在此配对收起，右键选词后放大镜会常驻到视图 detach。
                        MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                            surface.dismissMagnifier("secondary button release")
                        }
                    }
                    return true
                }

                event.isButtonPressed(MotionEvent.BUTTON_TERTIARY) -> {
                    if (event.action == MotionEvent.ACTION_DOWN) {
                        surface.viewModel?.pasteFromClipboard()
                    }
                    return true
                }
            }
        }

        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                if (surface.isSelectingText) {
                    // 手柄拖动由唯一的覆盖层窗口拥有
                    // （生命周期单一 owner）——落在手柄上的 DOWN 不会再抵达 Surface。
                    // 任何确实抵达此处的 DOWN 都是非手柄轻击：
                    // 与之前一样清除选区。
                    surface.viewModel?.clearSelection()
                    surface.selectionHandles.hideSelectionHandles()
                    surface.hideSelectionMenu("nonHandleTap")
                }
            }

            MotionEvent.ACTION_MOVE -> {
                if (surface.isSelectingText && surface.handleDragController.state != HandleDrag.NONE) {
                    // 指针 id 锁定：只有锁定拖动的那根手指才能改变选区。
                    // 坐标取自该指针的槽位；由第二根手指携带的移动
                    // ——或它抬起之后的移动——一律吞掉。
                    // 注意：指针 id 与槽位下标是两回事，不能直接比较——
                    // Android 在 ACTION_POINTER_UP 后会回收并复用 id，
                    // 同一手势里两者并不相等。
                    val dragPointerId = surface.handleDragController.pointerId
                    val lockedIdx = dragPointerId?.let { event.findPointerIndex(it) }
                    val lockedMissing = dragPointerId != null && (lockedIdx == null || lockedIdx < 0)
                    if (!lockedMissing) {
                        val touchX = if (lockedIdx != null && lockedIdx >= 0) event.getX(lockedIdx) else event.x
                        val touchY = if (lockedIdx != null && lockedIdx >= 0) event.getY(lockedIdx) else event.y
                        surface.driveHandleDragMove(touchX, touchY)
                    }
                } else if (longPressDragging && surface.isSelectingText) {
                    // 长按自身的抖动不算拖动：越过触摸阈值前一律不动刚选出的词。
                    // 少了这道守卫，1px 位移就把整词选区塌缩成单格（实测
                    // `(1,6)-(1,10)` → `(1,6)-(1,7)`），菜单随选区重锚走位，
                    // 用户点下去的一瞬正好点空——表象即「菜单按钮全部失效」。
                    if (!longPressDragStarted) {
                        val touchSlop = ViewConfiguration.get(surface.context).scaledTouchSlop
                        val deltaX = event.x - longPressStartX
                        val deltaY = event.y - longPressStartY
                        longPressDragStarted = deltaX * deltaX + deltaY * deltaY > touchSlop * touchSlop
                    }
                    if (longPressDragStarted) {
                        val col = pixelToCell(event.x, surface.cellWidth, surface.cols)
                        val row = pixelToCell(event.y, surface.cellHeight, surface.rows)
                        val gridRow = surface.currentViewportTopGrid() + row
                        surface.viewModel?.updateSelection(gridRow, col)
                        val selection = surface.viewModel?.state?.value?.selection
                        if (selection?.start != null && selection.end != null) {
                            // 重定位而非重建：showSelectionHandles 会 dismiss 并重建
                            // 2 个 PopupWindow（4 次 WindowManager IPC + 分配）
                            // ——在 60-120Hz 的 ACTION_MOVE 下是必然掉帧的来源。
                            // repositionHandle 改用 PopupWindow.update（进程内）。
                            surface.selectionHandles.repositionHandle(
                                HandleDrag.START,
                                selection.start.row,
                                selection.start.col,
                            )
                            surface.selectionHandles.repositionHandle(
                                HandleDrag.END,
                                selection.end.row,
                                selection.end.col,
                            )
                        }
                    }
                }
            }

            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_CANCEL,
            -> {
                // 手势收尾（慢速拖放无 fling、无 tap 回调时唯一的 settle 点）：
                // 不重置行偏移（保留滚动位置），只结束滚动态并归零余量，
                // 否则残留半行偏移且新输出不再回底（飘移虚浮）。
                if (surface.isScrolling) {
                    surface.isScrolling = false
                    surface.scrollAccumulatorPx = 0f
                    surface.viewModel?.runtime?.setScrollRemainderPx(0f)
                    surface.onScrollingStateChanged?.invoke(false)
                }
                if (longPressDragging) {
                    longPressDragging = false
                    // 长按拖动同样终结一次选区手势：启用轻击保护，
                    // 使松手轻击无法关闭菜单。
                    surface.handleDragController.markDragEnded()
                    val selection = surface.viewModel?.state?.value?.selection
                    if (selection?.start != null && selection.end != null) {
                        surface.viewModel?.endSelection()
                        // （termux 对等）：仅粘贴的单格选区同样获得手柄
                        // ——从空白处开始范围选区正是靠拖动它们。
                        // 工具栏经 onGetContentRect 的手柄高度偏移锚定在手柄之上。
                        surface.selectionHandles.showSelectionHandles(
                            selection.start.row,
                            selection.start.col,
                            selection.end.row,
                            selection.end.col,
                            surface.getAccentColor(),
                        )
                        surface.showSelectionMenuForCurrentSelection()
                    }
                }
                if (surface.isSelectingText && surface.handleDragController.state != HandleDrag.NONE) {
                    surface.finishHandleDrag()
                }
                surface.handleDragController.reset()
                surface.dismissMagnifier("gesture end")
                scaleFactor = 1.0f
            }
        }
        return true
    }

    /**
     * 鼠标滚轮事件（外接鼠标/触控板）。应用启用鼠标上报时
     * （DECSET 1000/1002/1003），滚轮事件经 Ghostty 鼠标编码器编码为按钮 4/5
     * 并写入 PTY；未启用时在主屏滚动本地回滚，备用屏保持不滚动。
     */
    fun onGenericMotionEvent(event: MotionEvent): Boolean? {
        val fromPointer =
            event.isFromSource(InputDevice.SOURCE_MOUSE) ||
                event.isFromSource(InputDevice.SOURCE_TOUCHPAD)
        if (event.action == MotionEvent.ACTION_SCROLL && fromPointer) {
            val runtime = surface.viewModel?.runtime
            val bridge = runtime?.bridge()
            val delta = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
            if (delta == 0f) return null
            if (bridge != null) {
                val button = if (delta > 0f) 3 else 4 // wheel-up=3, wheel-down=4 (Rust mapping)
                if (
                    bridge.encodeMouseWheel(
                        event.x,
                        event.y,
                        button,
                        KeyModifiers.ghosttyMods(event.metaState),
                        runtime.cellWidth,
                        runtime.cellHeight,
                        1,
                    )
                ) {
                    return true
                }
            }
            if (runtime != null && !runtime.altScreenActiveFlow.value) {
                val rowHeight = surface.cellHeight
                val scrollbackLen = surface.currentScrollbackLength()
                if (rowHeight > 0f && scrollbackLen > 0) {
                    stopFlingAnimation()
                    // 与触摸同一条管道：滚轮向上一档进入更早历史。
                    surface.applyScrollStep(
                        applyScrollDistance(
                            surface.scrollAccumulatorPx,
                            -delta * rowHeight,
                            rowHeight,
                            surface.scrollOffset,
                            scrollbackLen,
                            surface.ime.imeShiftRevealPx,
                            surface.ime.imeShiftCapacityPx.toFloat(),
                        ),
                        scrollbackLen,
                    )
                    return true
                }
            }
        }
        return null
    }

    init {
        // 平台快速缩放（双指拖动即缩放）会与本视图的缩放手势冲突，故关闭。
        scaleDetector.isQuickScaleEnabled = false
    }
}
