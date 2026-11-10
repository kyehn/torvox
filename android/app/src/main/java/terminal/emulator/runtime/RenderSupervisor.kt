package terminal.emulator.runtime

import android.os.Process
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import terminal.emulator.monitor.LivenessRecovery
import terminal.emulator.monitor.LivenessThresholds
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.monitor.SESSION_CLOSE_THROTTLE_NANOS
import terminal.emulator.monitor.SessionLivenessDiagnosis
import terminal.emulator.monitor.SessionLivenessInputs
import terminal.emulator.monitor.diagnoseSessionLiveness
import terminal.emulator.monitor.recoveryActionFor
import terminal.emulator.monitor.shouldCloseSession

/**
 * 拥有渲染线程生命周期：监视循环、死线程检测、重启/退避与线程启停。
 *
 * 内部类：直接访问 TerminalRuntime 的会话注册表与锁，无需经构造函数层层传递
 * （纯代码搬迁，行为零变化）。
 */
internal class RenderSupervisor(private val runtime: TerminalRuntime) {
    internal fun startRenderMonitor() {
        synchronized(runtime.monitorLock) {
            if (runtime.renderMonitorJob?.isActive == true) return
            runtime.renderMonitorJob = runtime.scope.launch {
                while (isActive) {
                    delay(RENDER_MONITOR_INTERVAL_MS)
                    checkSessions()
                }
            }
        }
    }

    internal fun stopRenderMonitor() {
        synchronized(runtime.monitorLock) {
            runtime.renderMonitorJob?.cancel()
            runtime.renderMonitorJob = null
        }
    }

    internal fun checkSessions() {
        val deadSessions = mutableListOf<SessionEntry>()
        val healthySessions = mutableListOf<SessionEntry>()
        synchronized(runtime.sessionLock) {
            scanSessionsForDeath(deadSessions, healthySessions)
        }
        decayRestartCounts(healthySessions)
        for (entry in deadSessions) {
            runtime.scope.launch {
                handleDeadRenderThread(entry)
            }
        }
    }

    internal fun scanSessionsForDeath(
        deadSessions: MutableList<SessionEntry>,
        healthySessions: MutableList<SessionEntry>,
    ) {
        for (entry in runtime.sessions.values) {
            if (!entry.running) continue
            if (isRenderThreadDead(entry.renderThreadExited, entry.renderThreadRef?.isAlive)) {
                deadSessions.add(entry)
            } else if (entry.restartAttempts > 0) {
                healthySessions.add(entry)
            }
        }
    }

    internal fun decayRestartCounts(healthySessions: List<SessionEntry>) {
        if (healthySessions.isEmpty()) return
        // 整批一次加锁：逐条加锁会在会话多时反复进出总闸，而这里只是清计数。
        synchronized(runtime.sessionLock) {
            for (entry in healthySessions) {
                // 扫描与本次衰减之间条目可能已死（线程在两步之间退出）：
                // 只有仍在运行且线程确实存活的条目才算「健康期」，否则会把
                // 刚死条目的重启预算清零，使 RENDER_MAX_RESTART_ATTEMPTS 永不生效。
                if (!entry.running) continue
                if (isRenderThreadDead(entry.renderThreadExited, entry.renderThreadRef?.isAlive)) continue
                if (entry.restartAttempts > 0) {
                    entry.restartAttempts = 0
                    entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                }
            }
        }
    }

    internal suspend fun handleDeadRenderThread(entry: SessionEntry) {
        LogUtil.warn(
            "Runtime",
            "session ${entry.sessionId} render thread exited, restart attempt ${entry.restartAttempts + 1}",
        )

        // 阶段 1（加锁）：仅做快速状态检查。阶段 2（不持锁）：
        // stopDeadRenderThreadResources 会 join 旧线程（最长 1s），
        // 跨 join 持有 runtime.sessionLock 会阻塞所有会话操作。
        synchronized(runtime.sessionLock) {
            if (!entry.running) return
            if (!entry.renderThreadExited) {
                val thread = entry.renderThreadRef
                if (thread != null && thread.isAlive) return
                entry.renderThreadExited = true
            }
        }

        // 阶段 2（不持锁）：下方 join 在渲染线程卡死于原生 GPU 代码时
        // 最长阻塞 THREAD_JOIN_TIMEOUT_MS——这正是若持有 runtime.sessionLock
        // 就会拖住所有会话操作的情形，故刻意置于锁外。
        stopDeadRenderThreadResources(entry)

        synchronized(runtime.sessionLock) {
            // 上方 join 后 running 仍为 true（它是会话意图标志，只有 pause/stop 置假）。
            // 在此重查，使在阶段 1 与此刻之间运行的并发 pauseRendering()
            // 能取消重启，而非在已销毁的 Surface 上启动渲染线程。
            if (!entry.running) return
            if (!runtime.sessions.containsKey(entry.sessionId)) return
            if (entry.restartScheduled) return
            entry.restartScheduled = true
            entry.restartAttempts++
        }
        // runtime.closeDeadSession 在锁外运行：它会关闭 bridge（Session::drop 会 kill
        // 并 join 线程，~100ms+）并可能启动接替渲染线程（join 最长
        // THREAD_JOIN_TIMEOUT_MS），两者绝不可在持有 runtime.sessionLock 时运行。
        // 尝试计数单调递增且 runtime.closeDeadSession 会重查 runtime.sessions.containsKey，
        // 故并发竞争调用无害（第二次调用立即返回）。
        if (shouldCloseDeadRender(entry.restartAttempts, RENDER_MAX_RESTART_ATTEMPTS)) {
            runtime.closeDeadSession(entry, "exceeded max restart attempts")
            return
        }
        val restartDelayMillis =
            synchronized(runtime.sessionLock) {
                val next = entry.nextRestartDelayMs
                entry.nextRestartDelayMs =
                    nextRestartDelayMs(entry.nextRestartDelayMs, MAX_RESTART_DELAY_MS)
                next
            }

        delay(restartDelayMillis)
        restartRenderThreadAfterDelay(entry)
        delay(GRACE_PERIOD_AFTER_RESTART_MS)
        confirmRestartGrace(entry)
    }

    internal fun stopDeadRenderThreadResources(entry: SessionEntry) {
        detachWatchDog(entry)
        // 注意：此处绝不要把 entry.running 置假。running 是会话意图标志
        // （pause/stop 置假，start/resume 置真）。死线程重启路径依赖 running 保持为真，
        // 使延迟重启（restartRenderThreadAfterDelay）仍能执行；
        // 也使并发的 pauseRendering()（将 running 置假）能正确取消待执行的重启。
        entry.renderThreadRef?.let { renderThread ->
            entry.wakeRenderThread(renderThread)
            renderThread.join(THREAD_JOIN_TIMEOUT_MS)
            if (renderThread.isAlive) {
                LogUtil.warn("Runtime", "Render thread did not exit within timeout, continuing anyway")
                entry.renderThreadPossiblyAlive = true
                entry.hungRenderThread = renderThread
                return
            }
        }
        entry.renderThreadRef = null
        entry.renderSignaled.set(false)
        // renderThreadRef 的 join 成功，并不能证明先前 join 超时时记录的挂起线程也已死亡
        // ——它可能仍在原生渲染代码中。仅在未记录挂起线程时才清标志，
        // 或先给挂起线程最后一次 join；无条件清除会让关闭路径在存活线程下
        // 销毁原生会话（use-after-free）。
        val hung = entry.hungRenderThread
        if (entry.renderThreadPossiblyAlive && hung != null && hung.isAlive) {
            hung.interrupt()
            hung.join(THREAD_JOIN_TIMEOUT_MS)
        }
        if (entry.renderThreadPossiblyAlive && hung != null && !hung.isAlive) {
            entry.renderThreadPossiblyAlive = false
            entry.hungRenderThread = null
        }
        // 此处跳过 releaseGpuSurface——新渲染线程会经 attachSurface 重新配置 Surface。
    }

    internal suspend fun restartRenderThreadAfterDelay(entry: SessionEntry) {
        synchronized(runtime.sessionLock) {
            // 先消费调度标记：无论本次重启执行还是被取消（暂停/关闭），
            // 后续监视器派发都必须能调度一次全新的重启。
            entry.restartScheduled = false
            if (!runtime.sessions.containsKey(entry.sessionId)) return
            // 仅当会话仍意图运行时才重启。已暂停的会话（Surface 已销毁）
            // 不应获得渲染线程——恢复路径会在 Surface 回来时启动它。
            if (!entry.running) return
            // 其他线程可能已启动（例如恢复与延迟重启竞争）；不要启动第二个。
            if (entry.renderThreadRef?.isAlive == true) return
            entry.renderThreadExited = false
            startRenderThread(entry)
            LogUtil.debug(
                "Runtime",
                "session ${entry.sessionId} render thread restarted (attempt ${entry.restartAttempts})",
            )
        }
    }

    internal suspend fun confirmRestartGrace(entry: SessionEntry) {
        synchronized(runtime.sessionLock) {
            if (!runtime.sessions.containsKey(entry.sessionId)) return
            if (!entry.running) return
            if (entry.renderThreadExited) return
            val thread = entry.renderThreadRef
            if (thread != null && thread.isAlive) {
                entry.restartAttempts = 0
                entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                LogUtil.debug("Runtime", "session ${entry.sessionId} render thread healthy after restart")
            }
        }
    }

    internal fun startRenderThread(entry: SessionEntry) {
        // 拒绝为正在关闭的会话启动线程：closeSession 在启动不持锁的
        // 停止/关闭序列之前会在 runtime.sessionLock 下置 entry.closing，
        // 且本函数的每个调用方都持有 runtime.sessionLock
        // ——因此正在关闭的条目绝不可能获得新渲染线程（孤儿线程 TOCTOU，见 closing 字段说明）。
        // 同时重置运行意图标志：调用方（resumeRendering/switchSession/closeSession 阶段 3）
        // 在调用前已将其置真，若保留为真会使 closeSession 的锁内重查
        // 把该条目误判为运行中而跳过 bridge.close（泄漏原生会话与子进程）。
        if (entry.closing) {
            entry.running = false
            LogUtil.debug("Runtime", "session ${entry.sessionId} closing — refusing to start render thread")
            return
        }
        detachWatchDog(entry)
        entry.renderThreadExited = false
        entry.running = false
        entry.renderSignaled.set(false)
        // 首帧计时只落定一次：渲染线程重启不重置（重启不改变「这个会话出过帧没有」
        // 这一事实），切后台暂停也不重置（暂停期间不出帧是正常的）。
        if (entry.firstFrameWaitStartNanos == 0L) {
            entry.firstFrameWaitStartNanos = System.nanoTime()
        }
        val oldThread = entry.renderThreadRef
        entry.renderThreadRef = null
        oldThread?.let { thread ->
            if (thread === Thread.currentThread()) {
                // join 自身必然超时并错误地把当前线程标记为挂起；跳过
                // （调用方就是渲染线程本身，如 runtime.handleSessionExit 的接替）。
                LogUtil.warn(
                    "Runtime",
                    "session ${entry.sessionId} startRenderThread called from its own render thread — skipping join",
                )
            } else {
                entry.wakeRenderThread(thread)
                thread.join(THREAD_JOIN_TIMEOUT_MS)
                if (thread.isAlive) {
                    LogUtil.warn(
                        "Runtime",
                        "session ${entry.sessionId} previous render thread still alive after join — forcing new thread anyway",
                    )
                    // 旧线程可能仍在原生渲染代码中。记录之，
                    // 使关闭路径跳过 releaseGpuSurface/close，
                    // 并保留引用以便退出时做最后一次 join。
                    entry.renderThreadPossiblyAlive = true
                    entry.hungRenderThread = thread
                } else {
                    // 仅当已 join 的线程就是记录的挂起线程（或未记录）时才清标志。
                    // 早先 GPU 停滞留下的另一个挂起线程可能仍在原生代码中存活；
                    // 在此清除会让关闭路径在其下销毁会话（use-after-free）。
                    if (entry.hungRenderThread == null || entry.hungRenderThread === thread) {
                        entry.renderThreadPossiblyAlive = false
                        entry.hungRenderThread = null
                    }
                }
            }
        }
        val generation = runtime.renderGeneration.incrementAndGet()
        entry.running = true
        val renderThread =
            Thread(
                {
                    try {
                        runBlocking {
                            // 显示优先级的渲染线程：输入法打开或 Surface 频繁变动时，
                            // 帧管线会与 UI 线程争抢 CPU。THREAD_PRIORITY_DISPLAY 让帧生产
                            // 优先于 UI 线程时效性较低的工作，在真机与 SwiftShader 模拟器上
                            // 都可降低帧时间抖动。
                            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY)
                            var diagCount = 0
                            // 渲染循环的失败账本：连续错误数（退避与致命上限）与四类
                            // 失败诊断的窗口闸门都在这里，理由见 RenderErrorLedger。
                            val renderErrorLedger = RenderErrorLedger()
                            var lastScrollOffset = Int.MAX_VALUE
                            var lastScrollRemainderPx = Float.NaN
                            var lastSelection = TerminalRuntime.SelectionStateSnapshot(0, 0, 0, 0, false)
                            // 慢循环帧的阶段分解日志节流：连续慢帧（如 500ms 空闲闭锁）
                            // 不刷屏，最密 2 行/秒；行内带窗口内慢循环次数。
                            val slowLoopThrottle = CountedLogThrottle()
                            // 慢帧告警同样节流：滚动时每帧都超 30ms，逐帧写 logcat
                            // 会让诊断本身成为负担；帧时长统计窗口仍保留完整分布。
                            val slowFrameThrottle = CountedLogThrottle()
                            // 本会话是否已收到 PTY 输出：首个输出即 shell 真的开始说话，
                            // 它也是外部驱动（宏基准）判定「可以往终端敲键」的信号——
                            // shell 就绪前写入会被 tty 初始化冲掉。
                            var hasReceivedOutput = false
                            // 每线程帧时长统计：渲染线程重启时重置（新生涯，不带陈旧历史）。
                            val frameTiming = FrameTimingStats()
                            // 整循环周期统计（渲染 + pollAll + 事件派发 + waitOutput）：
                            // 上方 frameTiming 只覆盖 bridge.render()；
                            // 循环窗口的倒数才是真实帧率，且能区分「原生渲染慢」
                            // 与「循环中其他环节慢」。
                            val loopTiming = FrameTimingStats()
                            // 门控之后、渲染 JNI 之前的「循环自身工作」统计：推送选区/
                            // 滚动偏移与滚动余量。它必须恒为亚毫秒级——一旦在这段里做
                            // GPU 调用（曾经把 uniform 写入放在这里），洪流期间会排到
                            // 100ms+，把整帧拖慢而 SLOW_LOOP 的 render 段看起来正常。
                            val loopPreTiming = FrameTimingStats()
                            // 基线自适应降级检测器：学习设备自身的帧时间基线，
                            // 对回归至其约 3 倍以上的窗口告警——同一套机制
                            // 既适用于软件模拟器（~555ms/帧）也适用于真机（~17ms），
                            // 无需固定阈值。
                            val frameTimingTrend = FrameTimingTrend()
                            LogUtil.debug(
                                "Runtime",
                                "render thread started for session ${entry.sessionId} generation=$generation",
                            )
                            while (entry.running && runtime.renderGeneration.get() == generation) {
                                // 用户在 [Process completed] 提示上按了 Enter
                                // ——writeToPty 只发信号；关闭路径在此渲染线程上执行，
                                // 避免在存活的渲染循环下销毁 bridge。
                                if (entry.processCompletedConfirmed) {
                                    runtime.handleSessionExit(entry, entry.processExitCode)
                                    break
                                }
                                try {
                                    val loopFrameStart = System.nanoTime()
                                    // 渲染失败退避的实测时长：主动等的等待，单独计量
                                    // 以便从慢循环判定中扣除（见 slowLoopWorkNanos）。
                                    var loopBackoffNanos = 0L
                                    val bridge = entry.bridge
                                    // ── vsync 唤醒门控 ──
                                    // 循环仅在有唤醒源触发时才渲染：
                                    //   ① vsyncRequested — Choreographer 帧回调
                                    //     （显示帧信号；该回调只发信号，绝不触碰 surface
                                    //     Mutex，也绝不自行调用 render），
                                    //   ② forceRenderRequested — 即时反馈旁路
                                    //     （forceRender()，语义不变），
                                    //   ③ renderSignaled — 任意 notifyRender() 生产者
                                    //     （滚动/选区/PTY 相邻的 UI 信号）。
                                    // 否则驻留在输出闭锁上。闭锁超时（活跃 17–34ms / 空闲 500ms）
                                    // 同时充当安全网节奏：超时返回仍会落下一次渲染尝试，
                                    // 使主线程阻塞或 vsync 信号丢失时延迟字段的消费不会停摆。
                                    // 关于 PTY 输出：waitOutput 是纯 park——PTY 到达无法提前唤醒它；
                                    // 新输出在下一次 vsync 尝试（~16.7ms）或本超时回退时
                                    // 被取走，此时 receive_cell_data 消费待处理数据。
                                    // 唤醒后 render() 无条件调用——空闲门控决策完全位于原生门控内部。
                                    if (
                                        !entry.vsyncRequested &&
                                        !entry.forceRenderRequested &&
                                        !entry.renderSignaled.get()
                                    ) {
                                        val nowNanos = System.nanoTime()
                                        val timeoutNanos: Long =
                                            if (
                                                shouldUseIdleLatch(
                                                    idleNanos =
                                                    nowNanos - entry.lastSignalNanos,
                                                    hasScrollMotion =
                                                    entry.hasScrollMotion(),
                                                    idleThresholdNanos =
                                                    RENDER_IDLE_THRESHOLD_NANOS,
                                                )
                                            ) {
                                                RENDER_LATCH_IDLE_TIMEOUT_NANOS
                                            } else {
                                                activeLatchTimeoutNanos(
                                                    vsyncFresh =
                                                    nowNanos - entry.lastVsyncNanos <=
                                                        RENDER_VSYNC_STALE_NANOS,
                                                )
                                            }
                                        entry.wakeGeneration =
                                            bridge.waitOutput(
                                                entry.wakeGeneration,
                                                timeoutNanos / 1_000_000L,
                                            )
                                        if (Thread.interrupted()) throw InterruptedException()
                                    }
                                    val loopGateDoneNanos = System.nanoTime()
                                    // 渲染前消费唤醒标志。三者都无条件清除：
                                    // 落在门控检查与本次清除之间的信号会并入下方的渲染；
                                    // 落在渲染期间的信号则等下一次 vsync/超时节拍
                                    // ——最多多一帧延迟，绝不会丢唤醒。
                                    // 在此（而非在驻留分支内）清除 renderSignaled 修掉了忙等泄漏：
                                    // 当 vsyncRequested 赢得门控检查时，renderSignaled 会永远为真，
                                    // 之后每次迭代都跳过驻留，
                                    // 线程在两次渲染之间全速空转空耗 CPU。
                                    entry.vsyncRequested = false
                                    entry.forceRenderRequested = false
                                    entry.renderSignaled.set(false)
                                    // 写入可能被推迟（会话锁被 PTY 写入持有）：此时不推进
                                    // 记账，下一帧以同一快照重试。绝不在这里等锁——那会把
                                    // 每帧拖到 PTY 输出的时长（真机日志 pre 段 100ms+）。
                                    val selectionSnapshot = runtime.selectionState.get()
                                    if (
                                        selectionSnapshot != lastSelection &&
                                        bridge.setSelection(
                                            selectionSnapshot.startRow,
                                            selectionSnapshot.startCol,
                                            selectionSnapshot.endRow,
                                            selectionSnapshot.endCol,
                                            selectionSnapshot.hasSelection,
                                        )
                                    ) {
                                        lastSelection = selectionSnapshot
                                    }
                                    val currentScrollOffset = entry.scrollOffset
                                    // 行偏移未被原生接受时（会话锁占用或命令通道满），
                                    // 原生视口仍是旧的：此时下发亚行余量就是「旧视口配
                                    // 新余量」，内容会先上跳半行、待新视口到达再下跳整行
                                    // （上下滑动晃动）。故余量随偏移同拍提交，偏移未
                                    // 接受时保留旧值、下一帧重试。
                                    val scrollOffsetAccepted =
                                        currentScrollOffset == lastScrollOffset ||
                                            bridge.setScrollOffset(currentScrollOffset)
                                    if (scrollOffsetAccepted) {
                                        lastScrollOffset = currentScrollOffset
                                    }
                                    val currentRemainderPx = entry.scrollRemainderPx
                                    if (
                                        scrollOffsetAccepted &&
                                        currentRemainderPx != lastScrollRemainderPx
                                    ) {
                                        // 只登记待用偏移：原生侧下一帧的 begin_frame
                                        // 会按它重写 uniform，此处不做任何 GPU 调用。
                                        bridge.setScrollYPx(currentRemainderPx)
                                        lastScrollRemainderPx = currentRemainderPx
                                    }
                                    val loopPrerenderDoneNanos = System.nanoTime()
                                    entry.frameMarks = entry.frameMarks.copy(startNanos = System.nanoTime())
                                    // 渲染与 new_output 消费合并为单次 JNI 穿越（每帧省 ~0.1-0.3ms）。
                                    // 解构超过 3 项被 detekt 禁止，故取对象再逐字段读。
                                    val renderResult = bridge.renderWithNewOutput()
                                    val count = renderResult.count
                                    if (count > 0) {
                                        runtime.publishSurfaceBufferPresent(true)
                                        // 按会话记下「已出过帧」：进程级标志在首个会话出帧后
                                        // 恒为真，后建会话是否出过帧只能靠这一位判断。
                                        entry.firstFramePresented = true
                                    }
                                    // count > 0 = 本帧真的渲染并提交了缓冲（空闲帧为 0，
                                    // 失败帧为负）：据此判定首帧已呈现给 Surface。
                                    if (count > 0 && !runtime.hasPresentedFrame) {
                                        runtime.hasPresentedFrame = true
                                        // 冷启动耗时的唯一终点标记：系统启动屏等到这里才撤下，
                                        // 故「进程启动 → 本行」就是用户看到的启动时长。
                                        LogUtil.info("Runtime", "first frame presented")
                                        runtime.publishStartupMetrics(
                                            startupToFirstFrameMs = runtime.elapsedSinceProcessStartMs(),
                                        )
                                    }
                                    val newOutput = renderResult.newOutput
                                    val lastContentRow = renderResult.lastContentRow
                                    if (lastContentRow != entry.lastContentRow) {
                                        entry.lastContentRow = lastContentRow
                                        if (entry.sessionId == runtime.activeSessionId) {
                                            runtime.lastContentRowFlowInternal.value = lastContentRow
                                        }
                                    }
                                    // 备用屏同样只随每帧上报：输入法弹出动画期间它会翻转
                                    // （启动 helix 的同时键盘正收起），另发查询得到的缓存
                                    // 必然滞后于当帧的位移计算，表现为顶部被推出屏幕。
                                    val altScreenActive = renderResult.altScreenActive
                                    if (altScreenActive != entry.altScreenActive) {
                                        entry.altScreenActive = altScreenActive
                                        if (entry.sessionId == runtime.activeSessionId) {
                                            runtime.altScreenActiveFlowInternal.value = altScreenActive
                                        }
                                    }
                                    if (renderResult.surfaceInvalidated) {
                                        entry.surfaceInvalidated = true
                                        runtime.maybeRequestSurfaceRecreate(entry)
                                    } else if (entry.surfaceInvalidated) {
                                        // 恢复即清零预算：下一次判死仍从满额开始。
                                        entry.surfaceInvalidated = false
                                        entry.surfaceRecreateBudget.set(SessionEntry.SurfaceRecreateBudget())
                                    }
                                    // 渲染窗口到此为止：pollAll、事件派发与标题查询
                                    // 属于循环而非渲染，计入 frameTiming 会让
                                    // loopTiming 与它测同一个量、二者再也分不开。
                                    val renderDoneNanos = System.nanoTime()
                                    frameTiming.record(renderDoneNanos - entry.frameMarks.startNanos)
                                    val frameMs = (renderDoneNanos - entry.frameMarks.startNanos) / 1_000_000.0
                                    if (frameMs > SLOW_FRAME_LOG_THRESHOLD_MS) {
                                        slowFrameThrottle.record()?.let { slowFrames ->
                                            LogUtil.warn(
                                                "Runtime",
                                                "SLOW_FRAME session=${entry.sessionId} render=$frameMs count=$count slow_frames=$slowFrames newOutput=$newOutput scrollOffset=$currentScrollOffset scrollRemainderPx=${entry.scrollRemainderPx}",
                                            )
                                        }
                                    }
                                    if (newOutput) {
                                        // 只有真实的 PTY 摄入才刷新空闲时钟：count 同样统计静态网格的
                                        // 空闲重绘，会使 lastSignalNanos 永远处于空闲阈值内，
                                        // 500ms 空闲闭锁因而永不生效。持续输出流
                                        // （tail -f、ping、gradle）总是携带 newOutput，故保持活跃。
                                        entry.lastSignalNanos = System.nanoTime()
                                        // 输出让会话重新活跃：帧回调链可能已因空闲停下，
                                        // 拉起它才能继续按 vsync 对齐出帧。
                                        runtime.ensureVsyncChainRunning()
                                        if (!hasReceivedOutput) {
                                            hasReceivedOutput = true
                                            LogUtil.info(
                                                "Runtime",
                                                "session ${entry.sessionId} first output received",
                                            )
                                        }
                                        // 首个 PTY 输出 = shell 真的开始说话了（提示符已落格）：
                                        // 「进程启动 → 这一刻」是用户口径的「shell 启动慢」。
                                        runtime.publishStartupMetrics(
                                            startupToFirstOutputMs = runtime.elapsedSinceProcessStartMs(),
                                        )
                                        // 滚动语义（termux onScreenUpdated 对等）：消费原生 new_output
                                        // 标志（PTY 摄入的旁路标志，而非同样统计空闲重绘的
                                        // render() 计数），且仅在无选区/拖拽、SCROLL 锁关闭、
                                        // 且 RECENT_SCROLL_WINDOW_NANOS 内无滚动手势时才把视口复位到底部。
                                        // 被跳过的复位就此丢弃（「跳过 = 放弃」）：标志已是读后即清，
                                        // 下次输出到达时会再次复位（termux：持续输出总是胜出）。
                                        if (
                                            entry.scrollOffset != 0 &&
                                            shouldResetScroll(
                                                scrollActive = entry.scrollActive || entry.scrollGestureActive,
                                                hasSelectionOrDrag =
                                                selectionSnapshot.hasSelection ||
                                                    selectionSnapshot.dragging,
                                                newOutput = newOutput,
                                                recentlyScrolled =
                                                System.nanoTime() - entry.lastScrollNanos <
                                                    RECENT_SCROLL_WINDOW_NANOS,
                                            )
                                        ) {
                                            // 渲染线程上的单点复位写入；lastScrollOffset 保持不动，
                                            // 使既有的差量推送在下一帧把偏移 0 转发给原生。
                                            entry.scrollOffset = 0
                                            // 搜索高亮按 Compose 镜像换算绘制：此处同步递增复位计数，
                                            // 使订阅方重读真实偏移，否则高亮按旧偏移错位。
                                            runtime.runtimeState.update { current ->
                                                current.copy(
                                                    scrollResetEpoch = current.scrollResetEpoch + 1,
                                                )
                                            }
                                        }
                                    }
                                    if (count < 0) {
                                        // 瞬时渲染错误（Surface 未就绪、快照不可用等），
                                        // 会自行恢复，不计入致命上限。
                                        // 绝不在此 break：本线程同时驱动 pollAll 泵送，
                                        // 退出会冻结原生输出处理且不保证重启
                                        // （旋转/切应用的 Surface 中断会让终端永久卡死）。
                                        // 正常退出仍走上方 entry.running / generation 条件。
                                        // 瞬时渲染错误按窗口汇总：成功一帧即把连续数清零，
                                        // 「失败—成功—失败」的交替形态下按连续数节流每次都
                                        // 命中首条分支，于是逐帧记日志。行内带窗口内次数。
                                        renderErrorLedger.onTransientError()?.let { transientCount ->
                                            val consecutive = renderErrorLedger.consecutiveErrors
                                            val message =
                                                "session ${entry.sessionId} transient render error code=$count " +
                                                    "(consecutive=$consecutive, " +
                                                    "transient_errors=$transientCount, surviving)"
                                            // 首次是正常退化（平台收走了窗口），重复才是异常：
                                            // 分级见 transientRenderErrorIsWarning。
                                            if (transientRenderErrorIsWarning(consecutive)) {
                                                LogUtil.warn("Runtime", message)
                                            } else {
                                                LogUtil.info("Runtime", message)
                                            }
                                        }
                                        // 帧已正常返回（只是渲染失败），渲染线程没有挂起。
                                        // 必须刷新完成时刻：`RenderWatchDog` 以
                                        // 「开始 > 完成」判定挂起，若此处不更新，
                                        // 持续失败（Surface 迟迟不就绪）会让完成时刻
                                        // 冻结在最后一帧成功处，10s 后看门狗把仍在循环的
                                        // 线程判为挂死，反复重启直至 `runtime.closeDeadSession`
                                        // 关掉用户的 shell。
                                        entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                        // 自适应退避：前若干次短退避，之后长退避
                                        // （档位见 renderErrorBackoffMillis）。
                                        val sleepMs = renderErrorBackoffMillis(renderErrorLedger.consecutiveErrors)
                                        // 退避是设计上主动等的等待：单独计量，既不进慢循环
                                        // 判定，也不并进 poll 段——否则平台收走窗口这条正常
                                        // 退化路径会在退避后记出 warn 级慢循环告警。
                                        val backoffStartNanos = System.nanoTime()
                                        delay(sleepMs)
                                        loopBackoffNanos = System.nanoTime() - backoffStartNanos
                                    } else {
                                        // 恢复消息同样可能逐帧发生（失败—成功交替），
                                        // 故按窗口汇总，行内带窗口内恢复次数。
                                        if (renderErrorLedger.consecutiveErrors > 0) {
                                            val consecutive = renderErrorLedger.consecutiveErrors
                                            renderErrorLedger.onRecovery()?.let { recoveries ->
                                                LogUtil.info(
                                                    "Runtime",
                                                    "session ${entry.sessionId} recovered " +
                                                        "(consecutive=$consecutive, recoveries=$recoveries)",
                                                )
                                            }
                                        }
                                        renderErrorLedger.onSuccess()
                                    }
                                    // 输出泵与渲染解耦：无论渲染成功与否都必须排空原生输出，
                                    // 否则一次持续渲染失败会阻塞读取线程并冻结全部会话。
                                    try {
                                        val poll = bridge.pollAll()
                                        // 退出优先在独立分支中处理：该事件已从原生队列消费且无法重放，
                                        // 故下方剪贴板处理中的异常绝不能跳过清理。
                                        if (poll.exit) {
                                            // 本帧退出的是否就是本条目（而非同帧里某个后台会话）。
                                            // 只有「自己退出」才允许结束本渲染循环；后台会话退出
                                            // 绝不能把活动会话的循环 break 掉——那会让可见终端
                                            // 冻结到看门狗重启（每次还累加重启计数，最终误关活会话）。
                                            val selfExited =
                                                poll.sessionId == 0L || poll.sessionId == entry.sessionId
                                            // 先回复空值，再做任何清理：这些剪贴板读取请求
                                            // 已从原生队列消费且永不会再被派发，
                                            // 不予回复会挂起请求方。
                                            // 先于 runtime.handleSessionExit 回复（后者可能关闭
                                            // bridge，~100ms+）也把延迟降到最低。
                                            // 每个应答在 runtime.dispatchClipboardRequests 内部各自保护，
                                            // 那里吞掉 JNI 失败正是为了不跳过下方的退出清理。
                                            runtime.dispatchClipboardRequests(poll.clipboardReads)
                                            if (poll.sessionId != 0L && poll.sessionId != entry.sessionId) {
                                                // 后台（非活动）会话的 shell 已退出。
                                                // 其渲染线程已停止，不会再有其他方回收它
                                                // ——原生清扫只经本队列上报一次。在此关闭它
                                                // （runtime.handleSessionExit 对非活动会话是安全的：
                                                // 接替分支以 entry.sessionId == runtime.activeSessionId 为闸门）。
                                                val exitedEntry =
                                                    synchronized(runtime.sessionLock) {
                                                        runtime.sessions[poll.sessionId]
                                                    }
                                                if (exitedEntry != null) {
                                                    LogUtil.info(
                                                        "Runtime",
                                                        "reaping background session ${poll.sessionId} (exit ${poll.exitCode})",
                                                    )
                                                    runtime.handleSessionExit(
                                                        exitedEntry,
                                                        poll.exitCode,
                                                    )
                                                }
                                            } else {
                                                // 完整清理（关闭 bridge、移除会话、更新状态）在此进行；
                                                // 渲染监视器跳过 !running 的条目，因而绝不会回收已退出会话。
                                                runtime.handleSessionExit(entry, poll.exitCode)
                                            }
                                            // 两个分支共用：回收同一帧内退出的其他会话
                                            // （首个已在上方处理）。它们的原生 exit_reported
                                            // 标志已置位且不重发。
                                            // 仅排除 poll.sessionId——列表中其他每个 id
                                            // （含后台分支下的 entry.sessionId）都必须回收，
                                            // 否则 Kotlin 条目、原生会话与僵尸子进程将永久泄漏。
                                            // runtime.handleSessionExit 幂等（内部重查 containsKey）。
                                            poll.exits.forEach { exitInfo ->
                                                if (exitInfo.sessionId != poll.sessionId) {
                                                    val extra =
                                                        synchronized(
                                                            runtime.sessionLock,
                                                        ) { runtime.sessions[exitInfo.sessionId] }
                                                    if (extra != null) {
                                                        LogUtil.info(
                                                            "Runtime",
                                                            "reaping same-frame exited session ${exitInfo.sessionId} (exit ${exitInfo.exitCode})",
                                                        )
                                                        runtime.handleSessionExit(
                                                            extra,
                                                            exitInfo.exitCode,
                                                        )
                                                    }
                                                }
                                            }
                                            if (selfExited && !entry.waitingForProcessCompleted) {
                                                break
                                            }
                                            // 正在显示 [Process completed] 提示
                                            // ——保持会话可见（running 保持为真）
                                            // 直到用户按 Enter。
                                            // 原生 exit_reported 已置位，故不会再有退出事件到达。
                                        }
                                        runtime.eventDispatcher.handle(poll)
                                    } catch (exception: Exception) {
                                        // 输出泵失败会逐帧发生（桥接层持续抛异常）：按窗口
                                        // 汇总，行内带窗口内失败次数。窗口内只有这一次时
                                        // 给完整栈——成堆重复时同一异常的栈并不变，每
                                        // 500ms 重复一份只会把 logcat 刷满。
                                        renderErrorLedger.onPollFailure()?.let { pollFailures ->
                                            LogUtil.error(
                                                "Runtime",
                                                "pollAll failed for session ${entry.sessionId}; " +
                                                    "deferred events dropped (poll_failures=$pollFailures, " +
                                                    "cause=${exception.javaClass.simpleName})",
                                                if (pollFailures == 1) exception else null,
                                            )
                                        }
                                    }
                                    val loopPollDoneNanos = System.nanoTime()
                                    if (count >= 0) {
                                        diagCount++
                                        if (diagCount == 1) {
                                            LogUtil.debug("Runtime", "session ${entry.sessionId} first render OK")
                                        }
                                        if (diagCount % RENDER_DIAGNOSTIC_FREQUENCY == 0) {
                                            // 标题刷新移出渲染线程：getTitle 是 VT 通道外置查询，
                                            // 要等 VT 线程下一次 50ms 空闲 tick 应答
                                            // （VT 忙时等满 QUERY_TIMEOUT_MS）。真机日志实测该
                                            // 查询每逢本分支（60 帧窗口 ≈ 1 秒）将渲染帧拉到
                                            // 60–115ms；压测环境下踩满 500ms。
                                            runtime.scope.launch {
                                                // 取标题失败由 NativeQueryPort 归为缺省值（空串），
                                                // 不抛异常：此处的 catch 是死代码，且它把取消
                                                // 信号也当成失败——应用收尾时留下一条假错误。
                                                val title = bridge.getActiveSessionTitle()
                                                if (title.isNotEmpty() && title != runtime.runtimeState.value.title) {
                                                    // CAS 更新：收集器与 IO 会话函数也会写 runtime.runtimeState，
                                                    // 此处非原子的读-改-写会覆盖它们的会话列表。
                                                    runtime.runtimeState.update { current ->
                                                        current.copy(title = title)
                                                    }
                                                }
                                            }
                                        }
                                        // 整循环的结束时刻（供 RenderWatchDog 判挂起）：
                                        // 与上面的 renderDoneNanos 是两个窗口，勿合并。
                                        entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                        frameTiming.takeReport()?.let { report ->
                                            // 趋势基线序列只有渲染线程按帧序推进——observe() 必须
                                            // 留在渲染线程串行喂入；摘要的 scrollbackLength() 是
                                            // 同一条 VT 通道外置查询（同 title 同款 50–500ms 等待），
                                            // 一起移出渲染线程——否则它把下一个窗口首帧拖慢。
                                            val trendDegraded = frameTimingTrend.observe(report.averageNanos)
                                            val baselineNanos = frameTimingTrend.currentBaselineNanos()
                                            runtime.scope.launch {
                                                runtime.logFrameTiming(
                                                    entry,
                                                    report,
                                                    trendDegraded,
                                                    baselineNanos?.div(NANOS_PER_MILLI),
                                                )
                                            }
                                        }
                                        // 尾部队列等待已移除——驻留现在发生在上方的循环顶部唤醒门控
                                        // （同一闭锁、同一活跃/空闲超时）。落到此处即直接返回门控。
                                    }
                                    // 整循环周期：平均值的倒数才是真实帧率
                                    // （含 waitOutput + pollAll + 事件派发）。
                                    // 若渲染均值 ~16ms 而此处 ~50ms，
                                    // 说明时间花在循环的其他环节，而非原生渲染路径。
                                    val loopDurationNanos = System.nanoTime() - loopFrameStart
                                    loopTiming.record(loopDurationNanos)
                                    loopPreTiming.record(loopPrerenderDoneNanos - loopGateDoneNanos)
                                    // 慢循环帧的定位仪器：整循环超阈值才看——但空闲会话的门控
                                    // 驻留（waitOutput 至多 500ms）与渲染失败退避都是设计上
                                    // 主动等的等待而非慢，按去掉两段等待后的工作时长判定，
                                    // 避免 release 里空终端永久 WARN 刷屏，也避免平台收走
                                    // 窗口这类正常退化路径在退避后记出 WARN。各阶段耗时
                                    // （渲染前推送/渲染 JNI/退避/输出泵+事件/循环尾）供真机
                                    // 日志定位吃时环节，各段之和等于 total。
                                    // SLOW_FRAME 只看渲染 JNI，循环其他环节静默。
                                    if (
                                        slowLoopWorkNanos(
                                            totalNanos = loopDurationNanos,
                                            gateNanos = loopGateDoneNanos - loopFrameStart,
                                            backoffNanos = loopBackoffNanos,
                                        ) > SLOW_FRAME_LOG_THRESHOLD_MS * 1_000_000L
                                    ) {
                                        slowLoopThrottle.record()?.let { slowLoops ->
                                            val tailNanos =
                                                loopDurationNanos - (loopPollDoneNanos - loopFrameStart)
                                            val gateMs =
                                                (loopGateDoneNanos - loopFrameStart) / NANOS_PER_MILLI
                                            val preMs =
                                                (loopPrerenderDoneNanos - loopGateDoneNanos) / NANOS_PER_MILLI
                                            val renderMs =
                                                (renderDoneNanos - loopPrerenderDoneNanos) / NANOS_PER_MILLI
                                            val backoffMs = loopBackoffNanos / NANOS_PER_MILLI
                                            val pollMs =
                                                (loopPollDoneNanos - renderDoneNanos - loopBackoffNanos) /
                                                    NANOS_PER_MILLI
                                            val totalMs = loopDurationNanos / NANOS_PER_MILLI
                                            LogUtil.warn(
                                                "Runtime",
                                                "SLOW_LOOP " +
                                                    "session=${entry.sessionId} " +
                                                    "total=${totalMs}ms " +
                                                    "gate=${gateMs}ms " +
                                                    "pre=${preMs}ms " +
                                                    "render=${renderMs}ms " +
                                                    "backoff=${backoffMs}ms " +
                                                    "poll=${pollMs}ms " +
                                                    "tail=${tailNanos / NANOS_PER_MILLI}ms " +
                                                    "slow_loops=$slowLoops",
                                            )
                                        }
                                    }
                                    val preReport = loopPreTiming.takeReport()
                                    loopTiming.takeReport()?.let { loopReport ->
                                        val loopAvgMs = loopReport.averageNanos / NANOS_PER_MILLI
                                        val loopP95Ms = loopReport.p95Nanos / NANOS_PER_MILLI
                                        val loopMaxMs = loopReport.maxNanos / NANOS_PER_MILLI
                                        val preMaxMs = (preReport?.maxNanos ?: 0L) / NANOS_PER_MILLI
                                        val preP95Ms = (preReport?.p95Nanos ?: 0L) / NANOS_PER_MILLI
                                        val framesPerSecond =
                                            if (loopReport.averageNanos > 0) {
                                                (NANOS_PER_SECOND / loopReport.averageNanos).toInt()
                                            } else {
                                                0
                                            }
                                        LogUtil.info(
                                            "Runtime",
                                            "session ${entry.sessionId} loop timing window " +
                                                "(${loopReport.frameCount} frames): " +
                                                "avg=${loopAvgMs}ms p95=${loopP95Ms}ms " +
                                                "max=${loopMaxMs}ms pre_max=${preMaxMs}ms " +
                                                "pre_p95=${preP95Ms}ms " +
                                                "≈${framesPerSecond}fps vsync=${runtime.displayPeriodText()}",
                                        )
                                    }
                                } catch (exception: InterruptedException) {
                                    // 渲染线程在关闭期间（会话切换/运行期停止）被中断。
                                    // 这是预期信号而非渲染失败——干净地退出循环。
                                    Thread.currentThread().interrupt()
                                    break
                                } catch (exception: Exception) {
                                    if (isRenderThreadInterruption(exception)) {
                                        // 关闭路径的中断：runBlocking 以取消异常收尾，
                                        // 其 cause 是 InterruptedException。预期信号而非
                                        // 渲染失败——干净地退出循环，不记 error。
                                        Thread.currentThread().interrupt()
                                        LogUtil.debug(
                                            "Runtime",
                                            "session ${entry.sessionId} render thread cancelled by interruption",
                                        )
                                        break
                                    }
                                    // 渲染异常按窗口汇总：成功一帧即把连续数清零，交替
                                    // 形态下旧的「首条即记」分支每次都命中，逐帧记
                                    // error 加栈。行内带窗口内异常次数。
                                    renderErrorLedger.onRenderException()?.let { renderErrors ->
                                        LogUtil.error(
                                            "Runtime",
                                            "session ${entry.sessionId} render exception " +
                                                "(render_errors=$renderErrors, " +
                                                "consecutive=${renderErrorLedger.consecutiveErrors}, " +
                                                "cause=${exception.javaClass.simpleName})",
                                            if (renderErrors == 1) exception else null,
                                        )
                                    }
                                    // 异常帧同样是「渲染线程没挂」的证据：
                                    // 不刷新完成时刻，连续异常会让 `start > done`
                                    // 一直成立，`RenderWatchDog` 在 10s 后把仍在循环的
                                    // 线程判为挂死。当前上限恰好（约 5s）小于超时，
                                    // 但那只是两个常量的巧合，任一改动即成误杀。
                                    entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                    if (renderErrorLedger.consecutiveErrors > RENDER_MAX_CONSECUTIVE_ERRORS) {
                                        LogUtil.error(
                                            "Runtime",
                                            "session ${entry.sessionId} too many render exceptions " +
                                                "(${renderErrorLedger.consecutiveErrors}), stopping render thread",
                                            exception,
                                        )
                                        break
                                    }
                                    delay(RENDER_ERROR_SLEEP_MS)
                                }
                            }
                            entry.markRenderThreadExited()
                            LogUtil.debug("Runtime", "render thread stopped for session ${entry.sessionId}")
                        }
                    } catch (expected: InterruptedException) {
                        // runBlocking 体外的中断（join 前 interrupt 已送达
                        // 但循环尚未进入 try 区）：join 是协作式关闭，
                        // 中断即退出信号而非崩溃，instrumentation 不应报红。
                        Thread.currentThread().interrupt()
                        entry.markRenderThreadExited()
                        LogUtil.debug("Runtime", "render thread interrupted for session ${entry.sessionId}")
                    }
                },
                "Render-${entry.sessionId}",
            )
                .apply {
                    isDaemon = true
                }
        entry.renderThreadRef = renderThread
        renderThread.start()
        // 以显示 vsync 驱动新渲染循环的唤醒门控
        // （每进程一条自调度的 Choreographer 链）。
        runtime.ensureVsyncChainRunning()
        entry.renderWatchDog =
            RenderWatchDog(
                diagnose = { diagnoseEntryLiveness(entry) },
                isRunning = {
                    entry.running && !entry.renderThreadExited && runtime.activeSessionId == entry.sessionId
                },
                onHangDetected = { watchdog, diagnosis -> handleSessionHang(entry, watchdog, diagnosis) },
            )
                .also { it.start() }
    }

    internal fun stopRenderThread(entry: SessionEntry): Boolean {
        detachWatchDog(entry)
        entry.running = false
        val thread = entry.renderThreadRef
        entry.renderThreadRef = null
        entry.renderSignaled.set(false)
        thread?.let { thread ->
            entry.wakeRenderThread(thread)
            thread.join(THREAD_JOIN_TIMEOUT_MS)
            if (thread.isAlive) {
                LogUtil.error(
                    "Runtime",
                    "session ${entry.sessionId} render thread still alive after join — possibly hung",
                )
                entry.renderThreadPossiblyAlive = true
                entry.hungRenderThread = thread
                return false
            }
        }
        // 仅当已 join 的线程就是记录的挂起线程（或未记录）时才清除存活标志；
        // 另一个挂起线程可能仍在原生代码中存活，在此清除会让关闭路径在其下
        // 销毁原生会话。
        if (entry.hungRenderThread == null || entry.hungRenderThread === thread) {
            entry.renderThreadPossiblyAlive = false
            entry.hungRenderThread = null
        }
        return true
    }

    /**
     * 采集判定挂起层级所需的输入快照。
     *
     * 帧起止一次读出：分两次读可能拿到新 start 配旧 done，伪造出「帧在途」并
     * 误杀健康的渲染线程。
     */
    internal fun diagnoseEntryLiveness(entry: SessionEntry): SessionLivenessDiagnosis {
        val marks = entry.frameMarks
        val renderFrameBusyNanos =
            if (marks.startNanos > marks.doneNanos) System.nanoTime() - marks.startNanos else 0L
        // 首帧未产出时长：按会话计，且只在计时已落定且尚未出过帧时非零。
        val firstFrameWaitNanos =
            if (entry.firstFramePresented || entry.firstFrameWaitStartNanos == 0L) {
                0L
            } else {
                System.nanoTime() - entry.firstFrameWaitStartNanos
            }
        return diagnoseSessionLiveness(
            SessionLivenessInputs(
                renderFrameBusyNanos = renderFrameBusyNanos,
                vtBusyNanos = entry.bridge.vtBusyNanos(),
                ptySendBusyNanos = entry.bridge.ptySendBusyNanos(),
                renderThreadAlive = entry.renderThreadRef?.isAlive == true,
                firstFrameWaitNanos = firstFrameWaitNanos,
            ),
            LivenessThresholds(renderHangNanos = RENDER_HANG_TIMEOUT_NANOS),
        )
    }

    /**
     * 摘除并停止该会话的存活看门狗。
     *
     * 在 [runtime.sessionLock] 内完成：处置回调（[handleSessionHang]）以「本实例仍是
     * 当前看门狗」为生效前提，摘除与该复核必须互斥，否则陈旧看门狗会在新
     * 看门狗就位后仍被判为当前，把已经恢复的会话关掉。
     *
     * 锁序固定为 runtime.sessionLock → 看门狗闸门锁：看门狗线程永不持闸门锁取
     * runtime.sessionLock（见 [RenderWatchDog]），故两把锁不会互等。
     */
    internal fun detachWatchDog(entry: SessionEntry) {
        synchronized(runtime.sessionLock) {
            val watchdog = entry.renderWatchDog
            entry.renderWatchDog = null
            watchdog?.stop()
        }
    }

    /**
     * 按挂起层级处置：渲染层重启线程，VT 层与输出投递阻塞关闭会话，生命周期层
     * 只记录。
     *
     * 关闭会话走 [runtime.closeDeadSession]：VT 状态在原生侧持有，无法安全热重建，
     * 且静默重建会丢掉用户的 shell 状态（工作目录、后台任务、已输入内容）。
     *
     * 身份复核：只有仍是本会话当前看门狗的判定才处置。渲染线程重启会换上一个
     * 新看门狗，陈旧看门狗的迟到判定不得作用于新线程——否则把健康的渲染线程
     * 标记为死亡，或关闭一个已经恢复的会话。复核与状态变更在同一次
     * [runtime.sessionLock] 获取内完成，故两者不会交错。
     */
    internal fun handleSessionHang(
        entry: SessionEntry,
        watchdog: RenderWatchDog,
        diagnosis: SessionLivenessDiagnosis,
    ) {
        val hungLayer = diagnosis.hungLayer ?: return
        when (recoveryActionFor(hungLayer)) {
            LivenessRecovery.RESTART_RENDER_THREAD -> {
                synchronized(runtime.sessionLock) {
                    if (entry.renderWatchDog !== watchdog) {
                        LogUtil.debug("Runtime", "session ${entry.sessionId} stale watchdog ignored")
                        return
                    }
                    // 把线程标记为死亡。渲染监视器（checkSessions）会侦测到并
                    // 以指数退避重启线程，避免因 GPU 挂起而杀掉整个进程。
                    entry.renderThreadExited = true
                }
                LogUtil.error(
                    "Runtime",
                    "session ${entry.sessionId} render thread hung (>${RENDER_HANG_TIMEOUT_NANOS / 1_000_000L}s) — marking thread for restart",
                )
            }

            LivenessRecovery.CLOSE_SESSION -> {
                val now = System.nanoTime()
                synchronized(runtime.sessionLock) {
                    if (entry.renderWatchDog !== watchdog) {
                        LogUtil.debug("Runtime", "session ${entry.sessionId} stale watchdog ignored")
                        return
                    }
                    // 自动关闭按 60s 节流：反复关闭又新建会让用户看到会话号乱跳。
                    // 在 runtime.sessionLock 内领取：两个看门狗回调并发时必须只有一个拿到
                    // 许可，否则同一会话会被重复关闭。
                    if (!shouldCloseSession(entry.lastAutoCloseNanos, now)) {
                        LogUtil.warn(
                            "Runtime",
                            "session ${entry.sessionId} hung at $hungLayer — close throttled (${SESSION_CLOSE_THROTTLE_NANOS / 1_000_000L}ms)",
                        )
                        return
                    }
                    entry.lastAutoCloseNanos = now
                }
                LogUtil.error(
                    "Runtime",
                    "session ${entry.sessionId} hung at $hungLayer (busy=${diagnosis.busyNanos / 1_000_000L}ms)",
                )
                runtime.closeDeadSession(entry, "hung at $hungLayer")
            }

            LivenessRecovery.REQUEST_REPAINT -> {
                // 首帧迟迟未产出：向 VT 补一次索要（非破坏性）。内容去重在 VT 侧
                // 随之失效，故终端毫无变化也会推出该帧。失败只记日志——会话仍在，
                // 下一次看门狗节拍会再试。
                //
                // 捕 Throwable 而非 Exception：此处位于看门狗协程内，未捕获的 Error
                // 会终止该协程并经默认异常处理器带崩进程，而恢复动作失败不应有
                // 这种杀伤力（见 Bridge.close 的同款理由）。
                val requested = try {
                    entry.bridge.requestRepaint()
                } catch (throwable: Throwable) {
                    LogUtil.error("Runtime", "session ${entry.sessionId} repaint request failed", throwable)
                    false
                }
                // 成功路径记 debug：看门狗已按 2s 节拍记过 error 级「挂起」行
                // （含层级与已忙时长），此处再记一条 error 只是把同一事实说两遍。
                LogUtil.debug(
                    "Runtime",
                    "session ${entry.sessionId} produced no frame within " +
                        "${diagnosis.busyNanos / 1_000_000L}ms — repaint requested=$requested",
                )
            }

            LivenessRecovery.RECORD_ONLY -> {
                LogUtil.warn(
                    "Runtime",
                    "session ${entry.sessionId} hung at $hungLayer — recorded only",
                )
            }
        }
    }
}
