package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.system.measureTimeMillis

/**
 * RenderWatchDog 在判定为挂起时触发回调，并把挂起层级交给会话管理器。
 *
 * 判定本身在 [diagnoseSessionLiveness]（纯函数，另有一组单测）；本组只测看门狗
 * 的 plumbing：周期触发、不误报、代次闸门、`stop` 非阻塞。
 *
 * 间隔由构造参数注入，使 JVM 测试跑在毫秒级而不是生产的 2s。
 */
class RenderWatchDogTest {

    private companion object {
        /** `stop` 的允许耗时上限。旧实现阻塞满 2s，测试留出 20 倍余量仍能判红。 */
        const val STOP_BUDGET_MS = 100L
    }

    private val fastInterval = 10L // 10 ms

    /** 构造一份「某层挂起」的诊断。 */
    private fun hungDiagnosis(layer: HungLayer = HungLayer.RENDER): SessionLivenessDiagnosis =
        SessionLivenessDiagnosis(hungLayer = layer, busyNanos = 30_000_000_000L)

    @Test
    fun `stalled session triggers the hang callback with its layer`() {
        var hangs = 0
        var reported: HungLayer? = null
        val watchdog =
            RenderWatchDog(
                diagnose = { hungDiagnosis(HungLayer.VT) },
                isRunning = { true },
                onHangDetected = { _, diagnosis ->
                    hangs++
                    reported = diagnosis.hungLayer
                },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        // 轮询而非固定睡眠：负载高时 Default 调度器会推迟首拍，
        // 固定 200ms 会在构建机上偶发判红（回调本身只要一拍就会到达）。
        val deadline = System.currentTimeMillis() + 5_000
        while (hangs == 0 && System.currentTimeMillis() < deadline) {
            Thread.sleep(10)
        }
        watchdog.stop()
        assertTrue("stalled session must trigger the callback", hangs > 0)
        assertEquals("callback must carry the hung layer", HungLayer.VT, reported)
    }

    @Test
    fun `healthy session stays silent`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                diagnose = { SessionLivenessDiagnosis(hungLayer = null, busyNanos = 0L) },
                isRunning = { true },
                onHangDetected = { _, _ -> hangs++ },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        Thread.sleep(200)
        watchdog.stop()
        assertEquals("a healthy session must not trigger", 0, hangs)
    }

    @Test
    fun `stopped watchdog never fires`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                diagnose = { hungDiagnosis() },
                isRunning = { true },
                onHangDetected = { _, _ -> hangs++ },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        watchdog.stop()
        Thread.sleep(100)
        assertEquals("stop must silence the watchdog", 0, hangs)
    }

    /**
     * `stop` 在 `sessionLock` 内被调用（`pauseRendering` 的执行器线程、
     * `handleSessionExit` 的渲染线程），因此必须是非阻塞的：任何等待都会按会话数
     * 累加到主线程的 `stopForegroundServiceIfIdle` 上。
     *
     * 探针停在不可取消的阻塞上时，旧实现的 `runBlocking { cancelAndJoin() }`
     * 会阻塞满等待上限（生产 2s/会话）。
     */
    @Test
    fun `stop returns without waiting for the watchdog probe`() {
        val probeEntered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val watchdog =
            RenderWatchDog(
                diagnose = {
                    probeEntered.countDown()
                    release.await()
                    SessionLivenessDiagnosis(hungLayer = null, busyNanos = 0L)
                },
                isRunning = { true },
                onHangDetected = { _, _ -> },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        assertTrue("watchdog must reach the probe", probeEntered.await(2, TimeUnit.SECONDS))

        val stopMillis = measureTimeMillis { watchdog.stop() }
        release.countDown()

        assertTrue("stop blocked for ${stopMillis}ms", stopMillis < STOP_BUDGET_MS)
    }

    @Test
    fun `not-running session does not fire even when hung`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                diagnose = { hungDiagnosis() },
                isRunning = { false },
                onHangDetected = { _, _ -> hangs++ },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        Thread.sleep(200)
        watchdog.stop()
        assertEquals("a stopped session must not be reported as hung", 0, hangs)
    }

    /**
     * 回归钉：`stop` 后再 `start` 的同一实例，旧循环不得越过闸门。
     *
     * 旧实现用一个布尔 `stopped` 表达「已停止」，而 `start` 会把它复位——于是上一代
     * 循环（它正卡在不可取消的探测里，取消来不及）看到复位后的 `false` 照样回调，
     * 把**新**渲染线程误判为死亡。
     *
     * 判别力：第一次探测被刻意放行时返回「已挂起」的诊断，故唯一能挡住回调的就是
     * 代次判据本身——把这行判据去掉，本例即由 0 次变正数。
     */
    @Test
    fun `restarted instance does not let the stale loop fire`() {
        val firstProbeEntered = CountDownLatch(1)
        val releaseFirstProbe = CountDownLatch(1)
        val probeCount = java.util.concurrent.atomic.AtomicInteger()
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                diagnose = {
                    if (probeCount.getAndIncrement() == 0) {
                        // 第一代：卡在不可取消的等待里，模拟「正处于探测与回调之间」。
                        firstProbeEntered.countDown()
                        releaseFirstProbe.await()
                        // 放行时给一份「已挂起」的诊断：回调判据完全成立。
                        hungDiagnosis()
                    } else {
                        SessionLivenessDiagnosis(hungLayer = null, busyNanos = 0L)
                    }
                },
                isRunning = { true },
                onHangDetected = { _, _ -> hangs++ },
                checkIntervalMs = fastInterval,
            )

        watchdog.start()
        assertTrue("first generation must reach the probe", firstProbeEntered.await(2, TimeUnit.SECONDS))
        watchdog.stop()
        // 复用同一实例：旧实现在此处把 stopped 复位，旧循环因此得以回调。
        watchdog.start()
        releaseFirstProbe.countDown()
        Thread.sleep(300)
        watchdog.stop()

        assertEquals("stale generation must not fire after restart", 0, hangs)
    }

    /**
     * 挂起回调必须带上触发它的看门狗实例：处置方据此复核「本实例仍是会话当前的看门狗」。
     *
     * 判别力：回调若不带实例（或带别的实例），处置方无从分辨新旧看门狗，
     * 陈旧实例的迟到判定会作用到已经换上新渲染线程的会话上。
     */
    @Test
    fun `hang callback carries the watchdog instance that produced it`() {
        var reported: RenderWatchDog? = null
        val watchdog =
            RenderWatchDog(
                diagnose = { hungDiagnosis() },
                isRunning = { true },
                onHangDetected = { source, _ -> reported = source },
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        val deadline = System.currentTimeMillis() + 5_000
        while (reported == null && System.currentTimeMillis() < deadline) {
            Thread.sleep(10)
        }
        watchdog.stop()
        assertSame("the callback must carry the triggering watchdog", watchdog, reported)
    }

    /**
     * 回归钉：挂起回调不得持有闸门锁。
     *
     * 处置回调要取会话锁（领取关闭许可、关闭会话），而 `stop` 是在持有会话锁的路径上
     * 被调用的——它要取闸门锁。持闸门锁回调即成 AB-BA 死锁：看门狗线程持闸门锁等
     * 会话锁，会话线程持会话锁等闸门锁，整个会话子系统冻住。
     *
     * 判别力：把闸门锁重新裹回回调（旧实现），本例的两个线程互相等待，
     * 回调永不返回——本例即由「完成」变「超时」。
     */
    @Test
    fun `hang callback must not hold the gate lock while taking the session lock`() {
        val sessionLock = Any()
        val stopperHoldingSessionLock = CountDownLatch(1)
        val callbackEntered = CountDownLatch(1)
        val callbackCompleted = CountDownLatch(1)
        val callbackAcquiredSessionLock = AtomicBoolean(false)
        val watchdog =
            RenderWatchDog(
                diagnose = { hungDiagnosis() },
                isRunning = { true },
                onHangDetected = { _, _ ->
                    callbackEntered.countDown()
                    synchronized(sessionLock) { callbackAcquiredSessionLock.set(true) }
                    callbackCompleted.countDown()
                },
                checkIntervalMs = fastInterval,
            )
        // 模拟持会话锁调用 stop 的会话线程：stop 要取闸门锁。
        val stopper =
            Thread {
                synchronized(sessionLock) {
                    stopperHoldingSessionLock.countDown()
                    callbackEntered.await(2, TimeUnit.SECONDS)
                    watchdog.stop()
                }
            }
        stopper.isDaemon = true
        stopper.start()
        assertTrue(
            "the stopper must hold the session lock before the watchdog fires",
            stopperHoldingSessionLock.await(2, TimeUnit.SECONDS),
        )
        watchdog.start()

        val completed = callbackCompleted.await(5, TimeUnit.SECONDS)
        assertTrue("the hang callback must complete — no AB-BA deadlock", completed)
        watchdog.stop()
        stopper.join(2_000L)
        assertTrue("the callback must have taken the session lock", callbackAcquiredSessionLock.get())
    }
}
