package terminal.emulator

import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.TerminalThemeMode
import terminal.emulator.ui.theme.ThemeRepository
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

/**
 * 并发的主题下发 MUST 以最新设置为准。
 *
 * 主题设置的每个入口落盘后都触发一次重下发，这些重下发在 IO 协程里并发跑。
 * 若每条下发各自读一次设置快照再各自落地，读到旧快照的那条可能后落地，
 * 终端于是停在用户早已改掉的主题上。
 *
 * ## 判据
 *
 * 断言的是**下发序列**而不是最终画面：最终画面只反映最后一次落地，碰巧正确时
 * 看不出中间的乱序；下发序列里旧主题排在新主题之后，即说明旧快照压过了新快照。
 *
 * 序列取 logcat 的下发行（`Bridge` 的 `setTheme:`）：它是下发**顺序**的直接记录。
 * 像素判据看不到被后续下发盖掉的中间态。
 *
 * ## 判据为何是确定的
 *
 * 写入逐个等落盘，落盘顺序即写入顺序，下发序列因此有一个确定的次序可比：
 * 下发的主题名必须随写入顺序单调前进，且最终停在最后写入的那个主题上。
 * 落盘与下发分离的窗口由「并发下发线程」撑开——它在写入期间反复触发重下发，
 * 与写入触发的重下发互相交错，正是旧实现会乱序的形态。
 */
class ThemeApplyOrderInstrumentedTest : ComposeTerminalLogcatTest() {

    private companion object {
        /** 写入的主题数量：足够多，下发之间才有交错可言。 */
        const val WRITTEN_THEME_COUNT = 10

        /** 并发下发线程的下发次数上限：够撑开交错窗口，又不至于压垮设备。 */
        const val CONCURRENT_APPLY_COUNT = 40

        /** 等待设置落盘的上限（毫秒）。 */
        const val SETTINGS_TIMEOUT_MS = 15_000L

        /** 等待主题下发的上限（毫秒）。 */
        const val APPLY_TIMEOUT_MS = 30_000L

        /** 等待并发下发线程退出的上限（毫秒）。 */
        const val JOIN_TIMEOUT_MS = 10_000L

        /** logcat 里的下发行：`Bridge` 标签的 `setTheme: <主题名>`。 */
        val APPLY_LINE = Regex("""D Bridge\s*:\s*setTheme: (\S+)""")
    }

    private var originalSettings: SettingsRepository.SettingsState? = null

    /** 运行期：由主线程取出后交给并发下发线程用。 */
    private var runtime: TerminalRuntime? = null

    /** 并发下发线程的停机标志。 */
    private val stopConcurrentApply = AtomicBoolean(false)

    /** 并发下发线程的异常：出错即用例失败，不让它静默死掉。 */
    private val concurrentApplyFailure = AtomicReference<Throwable?>(null)

    /** 并发下发线程。 */
    private var concurrentApplyThread: Thread? = null

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            originalSettings = activity.terminalViewModel.settings.value
            runtime = activity.terminalViewModel.runtime
            // 固定主题模式：终端主题只由固定主题名决定，与系统夜览状态无关，
            // 判据因此不受别的用例改系统夜览模式的干扰。
            activity.terminalViewModel.setThemeMode(TerminalThemeMode.FIXED)
        }
        val modeStored =
            UxTestUtils.pollUntilTrue(timeoutMs = SETTINGS_TIMEOUT_MS, intervalMs = 50) {
                readSettings().themeMode == TerminalThemeMode.FIXED
            }
        assertNotNull("终端主题模式没有落盘", modeStored)
    }

    @After
    fun tearDown() {
        stopConcurrentApply.set(true)
        concurrentApplyThread?.join(JOIN_TIMEOUT_MS)
        concurrentApplyThread = null
        val before = originalSettings ?: return
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val viewModel = activity.terminalViewModel
            viewModel.setThemeMode(before.themeMode)
            viewModel.setThemeName(before.themeName)
            viewModel.setDayThemeName(before.dayThemeName)
            viewModel.setNightThemeName(before.nightThemeName)
        }
    }

    @Test
    fun concurrent_theme_applies_keep_the_newest_theme() {
        val original = checkNotNull(originalSettings)
        // 主题取自原生目录：不另存一份名字副本，目录变了用例也跟着变。
        // 排除当前固定主题：窗口里出现它即说明有旧快照在窗口之后落地。
        val catalogThemes = ThemeRepository.all().map { it.name }.filter { it != original.themeName }
        val sentinelTheme = catalogThemes.first()
        val writtenThemes = catalogThemes.drop(1).take(WRITTEN_THEME_COUNT)
        assertTrue("主题目录里的主题不够写：$catalogThemes", writtenThemes.size == WRITTEN_THEME_COUNT)

        // 哨兵：窗口开始前先写它并等它落地，窗口因此从「哨兵已下发」之后算起，
        // 窗口之前触发的下发全被冲在窗口之外。
        val sentinelSince = System.currentTimeMillis()
        writeTheme(sentinelTheme)
        assertNotNull("哨兵主题没有落盘", awaitPersisted(sentinelTheme))
        assertNotNull("哨兵主题没有下发", awaitApplied(sentinelTheme, sentinelSince))

        // 窗口：从这里起，下发序列必须随写入顺序单调前进。
        val windowSince = System.currentTimeMillis()
        startConcurrentApply()
        writtenThemes.forEach { themeName ->
            writeTheme(themeName)
            // 逐个等落盘：落盘顺序即写入顺序，判据因此有一个确定的次序可比。
            assertNotNull("主题 $themeName 没有落盘", awaitPersisted(themeName))
        }
        stopConcurrentApply.set(true)
        concurrentApplyThread?.join(JOIN_TIMEOUT_MS)
        assertNull("并发下发线程出错", concurrentApplyFailure.get())

        val lastTheme = writtenThemes.last()
        assertNotNull("最后写入的主题 $lastTheme 没有下发", awaitApplied(lastTheme, windowSince))

        val sequence = appliedThemes(windowSince)
        val order = listOf(sentinelTheme) + writtenThemes
        val indices = sequence.map { themeName -> order.indexOf(themeName) }
        assertTrue("下发序列里有顺序表之外的主题：$sequence", indices.all { it >= 0 })
        val collapsed = indices.fold(mutableListOf<Int>()) { acc, index ->
            if (acc.lastOrNull() != index) acc.add(index)
            acc
        }
        assertTrue(
            "主题下发序列出现回退（旧快照压过了新快照）：$sequence",
            collapsed == collapsed.sorted() && collapsed.distinct().size == collapsed.size,
        )
        assertEquals("最终下发的主题不是最后写入的主题：$sequence", lastTheme, sequence.last())
    }

    /** 写一次固定主题名（写入触发一次重下发）。 */
    private fun writeTheme(themeName: String) {
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            activity.terminalViewModel.setThemeName(themeName)
        }
    }

    /** 起一个后台线程反复触发重下发，与写入触发的重下发交错。 */
    private fun startConcurrentApply() {
        val target = checkNotNull(runtime)
        stopConcurrentApply.set(false)
        concurrentApplyFailure.set(null)
        concurrentApplyThread =
            Thread {
                runBlocking {
                    var applied = 0
                    while (!stopConcurrentApply.get() && applied < CONCURRENT_APPLY_COUNT) {
                        try {
                            target.applySettings()
                        } catch (throwable: Throwable) {
                            concurrentApplyFailure.compareAndSet(null, throwable)
                            return@runBlocking
                        }
                        applied++
                    }
                }
            }.apply {
                name = "theme-apply-race"
                start()
            }
    }

    /** 等设置里落盘了 [themeName]，返回等待耗时（毫秒），超时返回 null。 */
    private fun awaitPersisted(themeName: String): Long? =
        UxTestUtils.pollUntilTrue(timeoutMs = SETTINGS_TIMEOUT_MS, intervalMs = 50) {
            readSettings().themeName == themeName
        }

    /** 等 [themeName] 出现在下发序列里，返回等待耗时（毫秒），超时返回 null。 */
    private fun awaitApplied(themeName: String, sinceMs: Long): Long? =
        UxTestUtils.pollUntilTrue(timeoutMs = APPLY_TIMEOUT_MS, intervalMs = 100) {
            appliedThemes(sinceMs).contains(themeName)
        }

    /** 当前设置快照（主线程同步读）。 */
    private fun readSettings(): SettingsRepository.SettingsState {
        var settings: SettingsRepository.SettingsState? = null
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            settings = activity.terminalViewModel.settings.value
        }
        return checkNotNull(settings)
    }

    /** 下发序列：logcat 里 `Bridge` 的 `setTheme:` 行，按下发顺序。 */
    private fun appliedThemes(sinceMs: Long): List<String> =
        UxTestUtils.logcatSince(sinceMs).mapNotNull { line -> APPLY_LINE.find(line)?.groupValues?.get(1) }
}
