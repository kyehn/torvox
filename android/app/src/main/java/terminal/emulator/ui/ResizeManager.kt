package terminal.emulator.ui

import terminal.emulator.TerminalViewModel
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.computeGridAvailableHeight
import terminal.emulator.runtime.computeGridDimensions

/**
 * 拥有网格/尺寸计算：视图尺寸 → 行列 → PTY resize 与交换链重配置。
 *
 * 尺寸相关的状态（已配置尺寸、在途防抖、被丢弃尺寸的暂存值）全在本类内；
 * [TerminalSurface] 只转发尺寸事件并读取结果。
 */
internal class ResizeManager(private val surface: TerminalSurface) {
    /** 已配置给原生的 Surface 尺寸；宽为 0 表示尚未配置过。 */
    private var lastConfiguredWidth = 0
    private var lastConfiguredHeight = 0

    /** 在途的交换链尺寸防抖回调。 */
    private var pendingSurfaceResize: Runnable? = null

    /**
     * 在途 [pendingSurfaceResize] 的暂停凭证；取消或执行后 MUST 归还并清空。
     *
     * 账本由运行期持有（`TerminalRuntime.acquireRenderPause` / `releaseRenderPause`）：
     * 设置页、切后台、Surface 销毁与本文件的两个防抖共用同一本，否则各写同一个
     * 非计数布尔，先结束的那一个会把别人还在等的暂停放掉。不变量见
     * [RenderPauseLedger]。
     */
    private var pendingSurfaceResizeToken: Long? = null

    /**
     * 尺寸在 Surface 生效前到达时的暂存值。`onSizeChanged` 先于
     * `surfaceCreated`/`surfaceChanged` 触发（布局阶段 SurfaceHolder 尚无有效 Surface），
     * [applyResizeNormal] 在该分支直接返回、尺寸被永久丢弃：网格停在上次 spawn 的默认
     * 24×80，屏幕下半部空白，直到下一次外部尺寸事件（旋转）才恢复。
     */
    private var pendingRetryWidth: Int = 0
    private var pendingRetryHeight: Int = 0

    /**
     * 单一网格公式，与 `recomputeGridFromFontMetrics` 共用：
     * rows = (surface − ModifierBar − 备用屏输入法遮挡) / cell，cols = surface / cell。
     *
     * 输入法遮挡**只在备用屏扣除**（见 [imeInsetPx]）：主屏靠纯平移跟随键盘
     * （TESTING.md 要求上移后底部像素与上移前完全相同），其显示/隐藏绝不能改变
     * rows/cols，否则会有重排闪烁、换行错乱、底部行丢失。
     */
    internal fun applyGridResize(width: Int, height: Int) {
        val runtime = surface.viewModel?.runtime ?: return
        val cellWidth = runtime.cellWidth
        val cellHeight = runtime.cellHeight
        if (cellWidth <= 0f || cellHeight <= 0f) return
        // 备用屏按可见高度重排（SIGWINCH），主屏恒为 0。取运行期的单一值：
        // 三条网格路径（此处、`recomputeRowsColsImmediate`、运行期的
        // `recomputeGridFromFontMetrics`）必须扣同一个数，否则备用屏会在字号变化
        // 后被撑回被键盘遮住的高度且不会自愈。
        val imeReserve = runtime.imeGridReserve()
        // 高度是 SurfaceView 的布局高度。ModifierBar 覆盖其底部，
        // 故计算 rows 之前减去其高度——与运行期施加的预留量相同。
        val availableHeight =
            computeGridAvailableHeight(
                surfaceHeight = height,
                modifierBarHeightPx = runtime.modifierBarHeightPx,
                imeReserve = imeReserve,
            )
        val (newRows, newCols) =
            computeGridDimensions(
                surfaceWidth = width,
                surfaceHeight = availableHeight,
                cellWidth = cellWidth,
                cellHeight = cellHeight,
            )
        if (newRows == 0 || newCols == 0) return
        LogUtil.debug(
            TerminalSurface.TAG,
            "applyGridResize: $width x $height cell=($cellWidth,$cellHeight) " +
                "-> ${newRows}x$newCols (was ${surface.rows}x${surface.cols})",
        )
        if (newRows != surface.rows || newCols != surface.cols) {
            runtime.resize(newRows, newCols)
            surface.rows = newRows
            surface.cols = newCols
        }
        // 推入像素尺寸与网格变化解耦：输入法遮挡变化后行数可能恰好不变
        // （矮键盘 + 大单元格），但 ws_ypixel 描述的网格区域已经变了。
        // 契约见 NativeBridge.setPixelSize：每次网格 resize 都随 surface 的
        // 像素尺寸一并下发。availableHeight 已排除键栏与输入法遮挡——
        // 正是 rows 覆盖的网格区域。
        runtime.setPixelSize(width, availableHeight)
    }

    internal fun recomputeRowsColsImmediate(width: Int, height: Int) {
        val viewModel = surface.viewModel
        if (viewModel != null) {
            val cellWidth = viewModel.runtime.cellWidth
            val cellHeight = viewModel.runtime.cellHeight
            if (cellWidth > 0f && cellHeight > 0f) {
                // 与运行期网格相同的预留量：不减去工具栏高度时，
                // 此镜像会相差工具栏那几行，并在每次重组时被迫 requestLayout()。
                val barPx = viewModel.runtime.modifierBarHeightPx
                val availableHeight =
                    computeGridAvailableHeight(
                        surfaceHeight = height,
                        modifierBarHeightPx = barPx,
                        imeReserve = viewModel.runtime.imeGridReserve(),
                    )
                val (newRows, newCols) =
                    computeGridDimensions(
                        surfaceWidth = width,
                        surfaceHeight = availableHeight,
                        cellWidth = cellWidth,
                        cellHeight = cellHeight,
                    )
                if (newRows == 0 || newCols == 0) return
                surface.cols = newCols
                surface.rows = newRows
                return
            }
        }
        // 度量尚未就绪（原生字体还没回读）时的兜底：由上次配置的像素尺寸反推
        // 单元格尺寸。同样扣掉键栏与输入法遮挡——`lastConfigured*` 是整块 Surface
        // 尺寸，而 `rows` 是扣减后算出的行数，两者口径必须一致，否则这里算出的
        // 行数会与随后由真度量算出的结果跳变。
        if (lastConfiguredWidth > 0 && lastConfiguredHeight > 0 && surface.rows > 0 && surface.cols > 0) {
            val runtime = surface.viewModel?.runtime
            val cellWidthPx = lastConfiguredWidth.toFloat() / surface.cols
            // 扣减走同一个算式（与上方度量就绪分支逐字一致）：此前此处把
            // 「键栏 + 输入法遮挡」手工相加且没有下限钳位，只是碰巧在
            // Surface 被完全遮住时与另一条分支算出同一个结果。
            val cellHeightPx =
                computeGridAvailableHeight(
                    surfaceHeight = lastConfiguredHeight,
                    modifierBarHeightPx = runtime?.modifierBarHeightPx ?: 0,
                    imeReserve = runtime?.imeGridReserve() ?: 0,
                ).toFloat() / surface.rows
            // 上界：兜底用的 lastConfigured* 严重失配时，cellWidthPx 可能小到
            // 让商溢出 Int（Float.toInt() 饱和成 Int.MAX_VALUE），进而把 PTY 网格
            // 撑到 65535 列。手机上的终端最多几百列，512 足够宽裕。
            surface.cols = (width.toFloat() / cellWidthPx).toInt().coerceIn(1, TerminalSurface.MAX_FALLBACK_GRID_SIZE)
            surface.rows =
                (
                    computeGridAvailableHeight(
                        surfaceHeight = height,
                        modifierBarHeightPx = runtime?.modifierBarHeightPx ?: 0,
                        imeReserve = runtime?.imeGridReserve() ?: 0,
                    ).toFloat() / cellHeightPx
                    ).toInt().coerceIn(1, TerminalSurface.MAX_FALLBACK_GRID_SIZE)
        }
    }

    internal fun applySurfaceResize(width: Int, height: Int) {
        if (width <= 0 || height <= 0) return
        if (
            width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0
        ) {
            return
        }
        // 首次布局：立即应用（避免启动黑屏闪烁）。
        // 后续变化（输入法动画帧、Gboard 候选栏闪烁）：
        // 经 IME_RESIZE_DEBOUNCE_MS 防抖到稳定尺寸
        // ——否则每个中间尺寸都会强制一次完整的交换链重配置
        // 与 PTY 网格重排（掉帧、CellData 竞争错误、耗电）。
        // 防抖期间暂停渲染，使陈旧尺寸的缓冲绝不会被拉伸到正在动画的视图上
        // （显示时压扁、隐藏时拉伸）；下方的稳定触发会恢复并呈现一帧新画面。
        if (lastConfiguredWidth == 0) {
            applySurfaceResizeNow(width, height)
            return
        }
        // 顺序 MUST 是「先领新的、再归还旧的」：两者都持有时 ledger 的持有者数
        // 走 1 → 2 → 1，全程不落到 0，因而不发出任何恢复/暂停回调。反过来
        // （先归还再领）会让它经过 0，每个替换帧都产生一对多余的
        // `setRenderPaused(false)`/`(true)`，与「备用屏重排期间不出帧」冲突，
        // 还多出两次 JNI 往返。
        //
        // 被 `removeCallbacks` 丢弃的 runnable 永不执行，它领到的那次暂停必须在此
        // 归还（见 [RenderPauseLedger.release]）；`onSizeChanged` 与
        // `surfaceChanged` 常在同一帧用同一尺寸各调一次本方法，正是这条替换路径的
        // 常见触发。
        val token = surface.viewModel?.runtime?.acquireRenderPause()
        cancelPendingSurfaceResize()
        pendingSurfaceResizeToken = token
        pendingSurfaceResize =
            Runnable {
                pendingSurfaceResize = null
                // 以最新尺寸为准：onSizeChanged 已存储了它。
                applySurfaceResizeNow(surface.surfaceWidthPixels, surface.surfaceHeightPixels)
                // 稳定触发可能恰好落在已配置的尺寸上而提前返回、来不及执行自身的
                // 恢复——故始终在此归还。
                //
                // 归还的是**本 runnable 领到的那一枚**，且要确认它仍是当前在途的
                // 那一枚：视图 detach 时 `removeCallbacks` 够不到 attach 期间投递的
                // runnable（`mAttachInfo` 已置空，改投 `mRunQueue`），同一实例重新
                // attach 后它仍会执行——若归还是无条件的，它会去还掉新防抖刚领的那枚。
                if (token != null && pendingSurfaceResizeToken == token) {
                    pendingSurfaceResizeToken = null
                    surface.viewModel?.runtime?.releaseRenderPause(token)
                }
            }
                .also { surface.postDelayed(it, TerminalSurface.IME_RESIZE_DEBOUNCE_MS) }
    }

    /** 取消在途的交换链尺寸防抖并归还其持有的渲染暂停。 */
    internal fun cancelPendingSurfaceResize() {
        pendingSurfaceResize?.let { surface.removeCallbacks(it) }
        pendingSurfaceResize = null
        // 必须归还而不是只置空：清掉字段不等于归还持有，一次漏还即让账本里
        // 永远留着一次暂停——此后 acquire() 不再发出暂停（旗标已是真，
        // 布尔幂等），而 release() 也再不会让它归零，所有防抖与设置页的暂停
        // 就此静默失效。只归还本视图领到的这一个凭证：其余持有（设置页、
        // 切后台、Surface 销毁）都不属于本视图。
        pendingSurfaceResizeToken?.let { surface.viewModel?.runtime?.releaseRenderPause(it) }
        pendingSurfaceResizeToken = null
    }

    /** Surface 生效后重放被丢弃的尺寸；无暂存值或尺寸未变时为空操作。 */
    internal fun applyPendingSurfaceResize() {
        val width = pendingRetryWidth
        val height = pendingRetryHeight
        if (width <= 0 || height <= 0) return
        pendingRetryWidth = 0
        pendingRetryHeight = 0
        applySurfaceResizeNow(width, height)
    }

    internal fun applySurfaceResizeNow(width: Int, height: Int) {
        if (width <= 0 || height <= 0) return
        // 尺寸振荡后的延迟触发可能又落回已配置的尺寸——跳过冗余的重配置。
        if (
            width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0
        ) {
            return
        }
        val terminalViewModel = surface.viewModel ?: return
        terminalViewModel.surfaceWidth = width
        terminalViewModel.surfaceHeight = height

        // 尺寸经下方的 attachSurface 交给原生；不存在独立的 Surface 尺寸通道。
        applyResizeNormal(width, height, terminalViewModel)
    }

    internal fun applyResizeNormal(width: Int, height: Int, terminalViewModel: TerminalViewModel) {
        terminalViewModel.runtime.recomputeGrid()
        val holderSurface = surface.holder.surface
        if (!holderSurface.isValid) {
            LogUtil.debug(TerminalSurface.TAG, "applySurfaceResize: surface not valid yet, deferring")
            pendingRetryWidth = width
            pendingRetryHeight = height
            return
        }
        terminalViewModel.currentSurface = holderSurface
        // 把 Surface 交给原生；渲染器据此创建 wgpu surface
        // （attachWindow JNI 在 Rust 内部提取 ANativeWindow）。
        terminalViewModel.runtime.attachSurface(holderSurface, width, height)
        val runtimeState = terminalViewModel.runtime.state.value
        if (runtimeState.rows > 0 && runtimeState.cols > 0) {
            surface.rows = runtimeState.rows
            surface.cols = runtimeState.cols
        } else if (!runtimeState.isRunning) {
            // start() 在过小的 Surface 上会提前退出（分屏、自由窗口、
            // 可折叠半屏）且没有任何重试——窗口变大后终端将永远空白，
            // 因为 surfaceChanged 只会 resize。趁 Surface 此刻有效且已定尺寸，
            // 在此重试会话创建。
            LogUtil.info(TerminalSurface.TAG, "applySurfaceResize: runtime not started, retrying default session")
            terminalViewModel.ensureDefaultSession()
        }
        lastConfiguredWidth = width
        lastConfiguredHeight = height
        // 切后台返回经 surfaceChanged 重建交换链后强制一帧：闲时无新输出也呈现，避免黑屏。
        //
        // 但后台期间 MUST NOT 恢复：ON_PAUSE 收起输入法会触发一次尺寸变化，
        // 那时窗口可能已被系统回收，恢复渲染只会取帧失败（永久黑屏），
        // 还会多出一个与推进泵重复消费全局事件队列的渲染线程。
        if (!terminalViewModel.runtime.isBackgroundPaused) {
            surface.forceResumeRendering()
            terminalViewModel.runtime.resumeRendering()
            terminalViewModel.runtime.forceRender()
        }
        // 旋转/窗口尺寸变化（无输入法事件时）永远不会到达 runtime.resize：
        // 另一个触发点只有 insets 派发回调。
        // 使用共享公式使两条路径对网格的认知一致。
        // 仅在真实单元格度量到达后生效（此前为空操作）。
        // 输入法 inset 只在备用屏影响网格（见 imeGridReserve）：主屏靠纯平移
        // 跟随键盘，改它的 rows/cols 会有重排闪烁与底部行丢失。
        applyGridResize(width, height)
    }

    internal fun setDimensions(rows: Int, cols: Int) {
        surface.rows = rows
        surface.cols = cols
    }

    /** 记录已配置给原生的尺寸（Surface 重新交付后调用）。 */
    internal fun markConfiguredSize(width: Int, height: Int) {
        lastConfiguredWidth = width
        lastConfiguredHeight = height
    }

    /** Surface 拆除后作废已配置尺寸：下次尺寸变化 MUST 重新配置交换链。 */
    internal fun resetConfiguredSize() {
        lastConfiguredWidth = 0
        lastConfiguredHeight = 0
    }
}
