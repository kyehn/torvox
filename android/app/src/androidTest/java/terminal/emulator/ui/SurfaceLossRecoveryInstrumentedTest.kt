package terminal.emulator.ui

import android.view.View
import androidx.compose.ui.graphics.toArgb
import androidx.lifecycle.Lifecycle
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.findTerminalSurface
import terminal.emulator.pumpedTerminalText
import terminal.emulator.requireBridge
import terminal.emulator.ui.theme.ThemeRepository
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 原生 surface 失效后的自愈：持续注入 surface 级失败 → 原生判死并上报 →
 * 宿主换掉 `SurfaceView` 取到新窗口 → 画面重新有墨迹且新命令仍能落格。
 *
 * 真机上「BufferQueue 被遗弃」由 SurfaceFlinger 回收制造，应用侧无法复现；
 * 不注入就只能等系统回收，本路径永远没有回归护栏（模拟器全量套件实测：
 * 废弃后 22 分钟 3074 帧全败，27 个用例退化为「零像素」断言）。
 */
@RunWith(JUnit4::class)
class SurfaceLossRecoveryInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        /** 注入后等待自愈的轮询上限：换视图 + 新窗口交付 + 首帧，模拟器上给足余量。 */
        private const val RECOVERY_TIMEOUT_MS = 30_000L

        /** 轮询间隔。 */
        private const val RECOVERY_POLL_MILLIS = 300L

        /** 等待画面出现墨迹的轮询上限。 */
        private const val INK_TIMEOUT_MS = 30_000L

        /** 判定「画面有墨迹」的最低墨量（相对背景的像素差之和）。 */
        private const val MIN_INK_DELTA = 4_000L

        /** 采样步长（x 与 y 同用）。 */
        private const val INK_SAMPLE_STEP_PX = 3

        /**
         * 单个像素与期望画面的通道差之和上限：色彩管理会让屏幕取色与帧内像素
         * 差一两级，仍远小于「透出别的画面」的差异（后者是几十以上）。
         */
        private const val COVER_PIXEL_TOLERANCE = 24

        /**
         * 开孔区域与期望画面的不符像素占比上限（百分比）。
         *
         * 留一点余量：期望画面是切后台时回读的帧，期间终端可能又输出过内容，
         * 少量行会与帧内像素不同。
         */
        private const val MAX_COVER_MISMATCH_PERCENT = 20
    }

    /**
     * 终端底色：与 [terminal.emulator.ui.TerminalScreen] 同一条解析链
     * （应用深浅 → 终端主题名 → 目录），而不是写死某个十六进制值。
     * 主题换色时测试跟着走，不会因为基准过期而误判「透出桌面」。
     */
    private val terminalBackgroundArgb: Int by lazy {
        val state = composeTestRule.activity.terminalViewModel.settings.value
        val configuration = composeTestRule.activity.resources.configuration
        val systemDark =
            (configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
        val isDark = resolveAppDarkMode(state.appThemeMode, systemDark)
        val name =
            resolveTerminalThemeName(
                mode = state.themeMode,
                fixedName = state.themeName,
                dayName = state.dayThemeName,
                nightName = state.nightThemeName,
                isDark = isDark,
            )
        val theme = ThemeRepository.byNameOrNull(name) ?: ThemeRepository.byName(ThemeRepository.defaultDarkName())
        theme.background.toArgb()
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        // 本类量像素墨迹（`inkPixels` 采样），须再等原生 Surface 挂上，否则量到全零——
        // 判红原因与被测的 Surface 丢失恢复行为无关。见 waitForTerminalPixels。
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
    }

    /** 经 shell 打印一行，回显重发至多 3 次（冷 stdin 竞态，同其他用例口径）。 */
    private fun printAndAwait(command: String, needle: String) {
        awaitShellPrompt(composeTestRule.requireBridge())
        var seen: Long? = null
        repeat(3) {
            composeTestRule.requireBridge().writeToPty("$command\n".toByteArray(Charsets.UTF_8))
            seen =
                UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                    composeTestRule.pumpedTerminalText()?.contains(needle) == true
                }
            if (seen != null) return
        }
        assertNotNull("标记必须落格: $needle", seen)
    }

    /**
     * 终端 Surface 区域内相对背景的墨量（色差之和）：判定「画面真的有内容」。
     *
     * 返回 `null` 表示截图不可用：换视图期间窗口正在拆装，UiAutomator 的截图服务会
     * 偶发产出无法解码的文件（实测 `takeScreenshot produced an undecodable file`）。
     * 那是观测手段失效，不是被测行为，故由调用方按「继续等」处理。
     */
    private fun terminalInk(): Long? = runCatchingCancellable { terminalInkOrThrow() }.getOrNull()

    private fun terminalInkOrThrow(): Long {
        val bitmap = UxTestUtils.screenshot(device)
        val surface = findTerminalSurface(composeTestRule.activity)
        val location = IntArray(2)
        surface.getLocationOnScreen(location)
        val left = location[0].coerceIn(0, bitmap.width - 1)
        val top = location[1].coerceIn(0, bitmap.height - 1)
        val right = (left + surface.width).coerceIn(left + 1, bitmap.width)
        val bottom = (top + surface.height / 2).coerceIn(top + 1, bitmap.height)
        var inkPixels = 0L
        var pixelY = top
        while (pixelY < bottom) {
            var pixelX = left
            while (pixelX < right) {
                inkPixels +=
                    kotlin.math.abs(bitmap.getPixel(pixelX, pixelY) - terminalBackgroundArgb).toLong() and 0xFFFFFF
                pixelX += INK_SAMPLE_STEP_PX
            }
            pixelY += INK_SAMPLE_STEP_PX
        }
        return inkPixels
    }

    private fun awaitInk(phase: String): Long {
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = INK_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                // 必须推进测试时钟：宿主换 SurfaceView 由组合重组驱动（`key(surfaceKey)`），
                // 而重组由 Compose 测试的帧时钟推进。只截图不推进时钟时，换视图的信号
                // 一直排在队列里——原生侧拿不到新窗口，出帧一直失败，画面始终无墨迹
                // （实测注入失效后 30s 无墨迹，用例判红；推进时钟后同一路径自愈）。
                composeTestRule.waitForIdle()
                (terminalInk() ?: 0L) >= MIN_INK_DELTA
            }
        assertNotNull("$phase 终端画面必须已有墨迹（截图或渲染均无内容）", seen)
        return terminalInk() ?: 0L
    }

    @Test
    fun persistentSurfaceLossTriggersHostRebuildAndKeepsRendering() {
        // 前置：确认画面本来就有内容（否则「恢复后有墨迹」可能是从未有过墨迹）。
        printAndAwait("echo PRE_INK_MARKER", "PRE_INK_MARKER")
        val inkBefore = awaitInk("注入前：")

        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        assertTrue("失效注入必须被原生接受", composeTestRule.requireBridge().setSurfaceLossInjectedForTest(true))

        // 自愈的第一半必须可观测：宿主换掉 SurfaceView（实例变化）才拿到新的原生
        // 窗口。只断言「最后仍有墨迹」是不够的——画面可能从未掉过墨迹而全程未自愈。
        // 每轮必须制造新输出：空闲帧根本不渲染（渲染循环无脏不取纹理），不喂输出就
        // 永远等不到注入生效的那一帧。
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                composeTestRule.requireBridge().writeToPty("echo TICK\n".toByteArray(Charsets.UTF_8))
                composeTestRule.pumpedTerminalText()
                // 必须推进测试时钟：组合的重算与 LaunchedEffect 都由 Compose 测试的帧
                // 时钟驱动，只在 waitForIdle/advanceTimeBy 期间前进。不调用它，界面
                // 在整个轮询里冻结——实测信号已送达、状态已写，视图却始终不换。
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull(
            "注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView（未读到失效位或未请求重建）",
            rebuilt,
        )

        // 关闭注入：此后取纹理恢复真实行为（真实场景由新窗口的重建自然完成）。
        assertTrue("关闭失效注入必须被原生接受", composeTestRule.requireBridge().setSurfaceLossInjectedForTest(false))

        // 自愈的第二半：新窗口必须真的开始出帧。
        awaitInk("换 SurfaceView 后：")

        // 恢复必须是「新窗口 + 真正在渲染」，而非停在最后一帧：再打印一条必须落格。
        printAndAwait("echo POST_RECOVERY_MARKER", "POST_RECOVERY_MARKER")
        awaitInk("恢复后再输出：")
    }

    @Test
    fun deviceLossRebuildsGpuOnFreshSurfaceAndKeepsRendering() {
        // 真机由 Mali 在切后台时制造（2026-11-14 日志：`Parent device is lost` 之后
        // 207 条 panic，宿主反复换窗口却都挂在同一个死设备上，终端永久黑屏）。
        // 模拟器不会真的丢设备，不注入就没有回归护栏。
        printAndAwait("echo PRE_DEVICE_LOSS", "PRE_DEVICE_LOSS")
        awaitInk("设备丢失前：")

        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        val bridge = composeTestRule.requireBridge()
        try {
            bridge.setDeviceLostInjectedForTest(true)

            // 前半段与 surface 失效同源：每轮喂输出，渲染循环才会取帧并读到失效位；
            // 必须推进组合时钟，宿主换 SurfaceView 的重组才会发生。
            val rebuilt =
                UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                    bridge.writeToPty("echo LOST_TICK\n".toByteArray(Charsets.UTF_8))
                    composeTestRule.pumpedTerminalText()
                    composeTestRule.waitForIdle()
                    val current =
                        runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                    current != null && current !== surfaceBefore
                }
            assertNotNull(
                "注入设备丢失后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView（未读到失效位或未请求重建）",
                rebuilt,
            )

            // 后半段是设备丢失独有的：新窗口 attach 必须重建 VkInstance/Device 与
            // Renderer 才可能出帧。只换视图而仍在死设备上取帧的话，这里永远零墨迹。
            awaitInk("设备丢失重建后：")
        } finally {
            // 正常路径已由 attach 重建清除标志；用例中途失败时也必须清，否则后续
            // 用例的渲染循环会一直被跳过。
            bridge.setDeviceLostInjectedForTest(false)
        }

        printAndAwait("echo POST_DEVICE_LOSS", "POST_DEVICE_LOSS")
        awaitInk("重建后再输出：")
    }

    @Test
    fun selectionMenuSurvivesSurfaceRebuild() {
        // 选区菜单是**独立系统窗口**，由当前那一个 Surface 定位。宿主换 SurfaceView 后
        // 旧 Surface 已脱离窗口，若菜单不随新 Surface 重显，用户看到的是
        // 「已选中文字却没有任何菜单」——选区状态仍在 ViewModel 里，不会自愈。
        printAndAwait("echo SEL_MENU_MARKER", "SEL_MENU_MARKER")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            // 选区取**全屏**（0,0 → 末行末列），既不取固定的「0 行 2..5 列」，也不取
            // 「最后一行」：全新会话的内容写在**顶部**行（提示符就在第 0 行），任何写死的
            // 行都可能在整套连跑时落在空白上——而空文本选区按产品语义不挂「复制/分享」
            // （menuActionsForSelection），菜单其实出现了，断言却被读成菜单缺失，且该分支
            // 不打任何日志。全屏选择与回滚位置、网格尺寸、列宽都无关。
            val packed = composeTestRule.requireBridge().getGridRowsColsPacked()
            val rows = (packed ushr 32).toInt()
            val cols = (packed and 0xFFFFFFFFL).toInt()
            activity.terminalViewModel.startSelection(0, 0)
            activity.terminalViewModel.updateSelection(rows - 1, cols - 1)
            activity.terminalViewModel.endSelection()
            assertTrue(
                "全屏选区必须含文本，否则菜单不挂「复制」",
                activity.terminalViewModel.selectionTextForAction().isNotEmpty(),
            )
        }
        composeTestRule.waitForIdle()
        assertNotNull("选区菜单必须先出现", awaitCopyAction())

        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        assertTrue("失效注入必须被原生接受", composeTestRule.requireBridge().setSurfaceLossInjectedForTest(true))
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                composeTestRule.requireBridge().writeToPty("echo TICK\n".toByteArray(Charsets.UTF_8))
                composeTestRule.pumpedTerminalText()
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull("注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView", rebuilt)
        assertTrue("关闭失效注入必须被原生接受", composeTestRule.requireBridge().setSurfaceLossInjectedForTest(false))

        assertNotNull("换 SurfaceView 后选区菜单必须重现", awaitCopyAction())
    }

    /**
     * 无缓冲区期间窗口层必须盖住开孔底色，而不是透出后方桌面。
     *
     * 症状是「切应用返回闪一下」：SurfaceView 的原生 surface 位于窗口**之下**，窗口
     * 在这块是透明开孔。缓冲区缺失时若组合层不着色，开孔就透出桌面/上一个应用。
     * 用持续注入把「无缓冲区」拉长成稳定状态，才能既断言运行期标志，又量到像素：
     * 开孔区域必须是**均匀的终端底色**（透出桌面时不会是纯底色）。
     */
    @Test
    fun noBufferWindowIsCoveredByThemeBackground() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.waitForIdle()
        assertNotNull(
            "正常出帧后必须报告「已有缓冲区」，否则覆盖层永远撤不掉",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                runtime.surfaceBufferPresent.value
            },
        )

        assertTrue("失效注入必须被原生接受", composeTestRule.requireBridge().setSurfaceLossInjectedForTest(true))
        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        // 持续注入让新 Surface 也拿不到缓冲：宿主换视图后「无缓冲区」成为稳定态。
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                composeTestRule.requireBridge().writeToPty("echo COVER_TICK\n".toByteArray(Charsets.UTF_8))
                composeTestRule.pumpedTerminalText()
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull("注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView", rebuilt)
        assertNotNull(
            "换视图后、首帧未到时必须报告「无缓冲区」，否则不会着色遮住开孔",
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = 50) {
                !runtime.surfaceBufferPresent.value
            },
        )
        // 等组合层把「无缓冲区」这个事实推给新视图：视图收到该事实才在自己的
        // `onDraw` 里补画底色（开孔只能由 SurfaceView 自己填，见
        // TerminalSurface.onDraw）。事实推达之前截到的画面是平台在洞里留的
        // 空白——透出的是后方桌面，不是应用画的遮罩。
        assertNotNull(
            "组合层必须把「无缓冲区」推给换后的 SurfaceView",
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = 50) {
                (findTerminalSurface(composeTestRule.activity) as? TerminalSurface)
                    ?.surfaceHasBuffer == false
            },
        )
        // 视图收到事实后要等一帧才画出来：推进组合时钟并等它空闲。
        composeTestRule.waitForIdle()
        // 像素证据要在「注入仍生效」的这一瞬抓：先把画面拿到手，随后立刻撤注入。
        // 反过来先轮询像素会把注入拖上几秒，重建预算被耗光，之后的恢复就无从发生。
        val shot = runCatchingCancellable { UxTestUtils.screenshot(device) }.getOrNull()
        val released = composeTestRule.requireBridge().setSurfaceLossInjectedForTest(false)
        assertTrue("关闭失效注入必须被原生接受（$released）", released)
        val bitmap = checkNotNull(shot) { "无缓冲区期间必须能截到开孔区域" }
        val surface = findTerminalSurface(composeTestRule.activity)
        val location = IntArray(2)
        surface.getLocationOnScreen(location)
        // 开孔里应当是**终端自己的画面**：切后台时回读的保留帧（有保留帧时），
        // 否则是终端底色。两者都不是「后方桌面」——后者是壁纸与图标，像素既不
        // 均匀、颜色也与终端底色无关。
        //
        // 断言取逐像素比对：开孔的采样像素必须与期望画面（保留帧或终端底色）
        // 一致，容忍色彩管理带来的一两级偏差。之前断言「开孔必须是均匀的终端
        // 底色」，那是保留帧落地之前的旧契约：保留帧里有字，像素本就不均匀。
        val cover = runtime.coverFrame.value
        val mismatch = coverMismatchRatio(bitmap, location, surface, cover)
        UxTestUtils.metric("cover_mismatch_percent", mismatch.toLong())
        assertTrue(
            "无缓冲区期间开孔区域必须显示终端自己的画面（保留帧或终端底色），" +
                "实测与期望不符的像素占 $mismatch%，否则就是透出了后方画面",
            mismatch <= MAX_COVER_MISMATCH_PERCENT,
        )
        // 恢复与「换视图」同源：失效位持续为 1 时宿主会换新 SurfaceView，新窗口出帧后
        // 覆盖层才该撤。每轮都要制造输出——空闲帧不渲染，不喂输出就既驱动不出重试，
        // 也看不到新窗口的首帧。
        assertNotNull(
            "恢复出帧后必须撤销覆盖层",
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                composeTestRule.requireBridge().writeToPty("echo COVER_RESUME\n".toByteArray(Charsets.UTF_8))
                composeTestRule.pumpedTerminalText()
                composeTestRule.waitForIdle()
                runtime.surfaceBufferPresent.value
            },
        )
        awaitInk("恢复后：")
    }

    /**
     * 切后台再回来：系统拆除 Surface 后必须重新交付、重新出帧，开孔覆盖状态先落回
     * 「无缓冲区」再随首帧撤掉；回来后画面必须有墨迹（不是永久黑屏）。
     *
     * 覆盖状态由 `surfaceBufferPresent` 表达，而它最关键的一次写入（Surface 被拆除）
     * 发生在 `onStop` 之后。组合层若用生命周期门控的收集方式，`ON_START` 重启收集
     * 与首帧绘制会抢跑——首帧按旧值（有缓冲区）画完，开孔区便透出上一个应用，
     * 即「切换应用返回一闪」。
     */
    @Test
    fun backgroundStopKeepsCoverStateAndResumePresentsAgain() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.waitForIdle()
        assertNotNull(
            "正常出帧后必须报告「已有缓冲区」",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                runtime.surfaceBufferPresent.value
            },
        )

        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        // Surface 是否在 onStop 被拆除由系统决定（部分配置会保留窗口）：只记录不判红，
        // 否则用例会因机型的窗口策略而非被测行为失败。
        val dropped =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                !runtime.surfaceBufferPresent.value
            }
        UxTestUtils.metric("surface_dropped_on_stop", if (dropped != null) 1 else 0)

        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        assertNotNull(
            "切回前台后 30s 内必须重新出帧（覆盖层撤掉），否则就是黑屏",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                composeTestRule.requireBridge().writeToPty("echo RESUME_TICK\n".toByteArray(Charsets.UTF_8))
                runtime.surfaceBufferPresent.value
            },
        )
        awaitInk("切回后：")
    }

    /**
     * 切后台时必须把离开时的画面回读成开孔遮罩。
     *
     * 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
     * 回前台时交换链必须重建，那几十毫秒里窗口的透明开孔没有缓冲：没有遮罩时
     * 开孔透出后方桌面（有平底色遮罩时是一块平底色），用户看到的都是
     * 「切后台返回闪一下」。遮罩把离开时的画面留在开孔里。
     *
     * 断言两件事：遮罩已发布（状态），且它装的是**画面**而非单色——单色遮罩与
     * 现有的平底色遮罩没有区别，闪烁照旧。
     */
    @Test
    fun backgroundStopPublishesTheLastFrameAsHoleCover() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.waitForIdle()
        assertNotNull(
            "正常出帧后必须报告「已有缓冲区」",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                runtime.surfaceBufferPresent.value
            },
        )
        // 画面里要有墨迹：纯空白的终端回读出来也是单色，测不出「装的是画面」。
        composeTestRule.requireBridge().writeToPty("echo COVER_FRAME_MARK\n".toByteArray(Charsets.UTF_8))
        composeTestRule.pumpedTerminalText()

        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        assertNotNull(
            "切后台后必须回读出开孔遮罩",
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                runtime.coverFrame.value != null
            },
        )
        val cover = checkNotNull(runtime.coverFrame.value) { "切后台后必须回读出开孔遮罩" }
        assertTrue(
            "遮罩尺寸必须与 surface 尺寸一致（宽 ${cover.width}）",
            cover.width > 0 && cover.height > 0,
        )
        assertEquals(
            "遮罩像素数必须等于宽×高×4",
            cover.width.toLong() * cover.height.toLong() * 4L,
            cover.pixels.size.toLong(),
        )
        // 画面必须不是单色：单色遮罩与平底色遮罩无异，闪烁照旧。
        val distinct = cover.pixels.toHashSet()
        assertTrue(
            "遮罩必须装的是画面（实测只有 ${distinct.size} 种像素值，单色即等于没遮）",
            distinct.size > 4,
        )

        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        assertNotNull(
            "切回前台后新帧到达必须撤掉遮罩",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                runtime.surfaceBufferPresent.value && runtime.coverFrame.value == null
            },
        )
    }

    /**
     * 开孔区域与期望画面的逐像素不符比例（百分比）。
     *
     * 期望画面取「保留帧」（切后台时回读的那一帧，有则优先）或终端底色：
     * 两者都是终端自己的画面，都不是后方桌面。保留帧是 RGBA8 像素，坐标与
     * surface 一一对应（帧的左上角即 surface 的左上角）。
     *
     * 色彩管理会让屏幕取色与帧内像素差一两级，故单像素偏差在阈值内即算一致。
     */
    private fun coverMismatchRatio(
        bitmap: android.graphics.Bitmap,
        location: IntArray,
        surface: View,
        cover: terminal.emulator.bridge.Bridge.CoverFrame?,
    ): Int {
        val left = location[0].coerceIn(0, bitmap.width - 1)
        val top = location[1].coerceIn(0, bitmap.height - 1)
        val right = (left + surface.width).coerceIn(left + 1, bitmap.width)
        val bottom = (top + surface.height / 2).coerceIn(top + 1, bitmap.height)
        var mismatched = 0
        var total = 0
        var y = top
        while (y < bottom) {
            var x = left
            while (x < right) {
                total++
                val actual = bitmap.getPixel(x, y)
                val expected = expectedHolePixel(x - location[0], y - location[1], cover)
                if (expected == null || colorDistance(actual, expected) > COVER_PIXEL_TOLERANCE) {
                    mismatched++
                }
                x += INK_SAMPLE_STEP_PX
            }
            y += INK_SAMPLE_STEP_PX
        }
        return if (total == 0) 100 else mismatched * 100 / total
    }

    /** 期望画面在 (x, y) 处的颜色：保留帧优先，其次终端底色；取不到即 null。 */
    private fun expectedHolePixel(x: Int, y: Int, cover: terminal.emulator.bridge.Bridge.CoverFrame?): Int? {
        if (cover != null) {
            val index = (y * cover.width + x) * 4
            if (x in 0 until cover.width && y in 0 until cover.height && index + 3 < cover.pixels.size) {
                val red = cover.pixels[index].toInt() and 0xFF
                val green = cover.pixels[index + 1].toInt() and 0xFF
                val blue = cover.pixels[index + 2].toInt() and 0xFF
                return 0xFF000000.toInt() or (red shl 16) or (green shl 8) or blue
            }
            return null
        }
        return terminalBackgroundArgb
    }

    /** 两个颜色的通道差之和：色彩管理带来的一两级偏差在容忍范围内。 */
    private fun colorDistance(actual: Int, expected: Int): Int {
        var distance = 0
        for (shift in intArrayOf(16, 8, 0)) {
            distance += kotlin.math.abs(((actual shr shift) and 0xFF) - ((expected shr shift) and 0xFF))
        }
        return distance
    }

    /** 轮询至选择菜单的「复制」项出现在无障碍树中（返回耗时，null 表示超时）。 */
    private fun awaitCopyAction(): Long? =
        UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
            device.hasObject(By.text("复制"))
        }

    /**
     * 首帧提交只把缓冲排入队列，SurfaceFlinger 要到下一次 vsync 才锁存上屏。
     *
     * 若在同一拍就撤掉开孔遮罩，回前台会有一拍「既无旧画面、也无新画面」的底色
     * ——就是用户看到的闪烁。故 true 的发布必须延后一个显示帧；false（surface
     * 被拆）必须立即生效，遮罩马上回来。
     */
    @Test
    fun bufferPresentIsPublishedOneDisplayFrameLate() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.activity.runOnUiThread {
            runtime.publishSurfaceBufferPresent(false)
            assertTrue("surface 被拆时必须立即报「无缓冲区」", !runtime.surfaceBufferPresent.value)
            runtime.publishSurfaceBufferPresent(true)
            // 同一主线程任务内帧回调还没机会执行：遮罩必须仍然在。
            assertTrue(
                "首帧提交后同一拍不得撤遮罩，否则开孔会露出一拍底色",
                !runtime.surfaceBufferPresent.value,
            )
        }
        assertNotNull(
            "下一个显示帧后必须发布「已有缓冲区」",
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 16) {
                runtime.surfaceBufferPresent.value
            },
        )
    }
}
