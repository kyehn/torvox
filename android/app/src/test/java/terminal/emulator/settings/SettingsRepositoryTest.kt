package terminal.emulator.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.emptyPreferences
import androidx.test.core.app.ApplicationProvider
import app.cash.turbine.test
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkObject
import io.mockk.unmockkObject
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import terminal.emulator.ui.theme.ThemeRepository
import java.io.File

/**
 * DataStore-backed settings: round-trip writes and defaults.
 *
 * The DataStore itself is real (over a per-test temp file); only the Hilt-injected
 * [SettingsDataStoreProvider] is replaced by a MockK stand-in that exposes that store, so tests
 * never touch the app's real prefs.
 */
@RunWith(RobolectricTestRunner::class)
class SettingsRepositoryTest {
    private lateinit var prefsDir: File
    private lateinit var repository: SettingsRepository

    @Before
    fun setUp() {
        // 缺省主题名来自原生主题目录（JNI），纯 JVM 测试取不到；把它替换成一个
        // 可辨认的名字，测试才能断言「缺键时走默认」这条 elvis 分支，
        // 而不是在 UnsatisfiedLinkError 上判红。
        mockkObject(ThemeRepository)
        every { ThemeRepository.defaultDarkName() } returns TEST_DARK_THEME
        every { ThemeRepository.defaultLightName() } returns TEST_LIGHT_THEME
        val context = ApplicationProvider.getApplicationContext<Context>()
        prefsDir = File(context.cacheDir, "settings_test_${System.nanoTime()}")
        prefsDir.mkdirs()
        val dataStore: androidx.datastore.core.DataStore<Preferences> =
            PreferenceDataStoreFactory.create(
                corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            ) {
                File(prefsDir, "settings.preferences_pb")
            }
        val provider = mockk<SettingsDataStoreProvider>()
        every { provider.dataStore } returns dataStore
        every { provider.screenWidthDp } returns 360f
        repository = SettingsRepository(provider)
    }

    @After
    fun tearDown() {
        unmockkObject(ThemeRepository)
        prefsDir.deleteRecursively()
    }

    private companion object {
        const val TEST_DARK_THEME = "test-default-dark"
        const val TEST_LIGHT_THEME = "test-default-light"
    }

    @Test
    fun `font size defaults to device-adaptive value then round-trips`() = runTest {
        repository.fontSize.test {
            // Screen stubbed at 360dp: raw formula 11.54sp lifts to the
            //  14sp floor (user-reported "default too small" fix;
            // floor raised from the  12sp).
            assertEquals(14f, awaitItem(), 0.01f)
            repository.setFontSize(22f)
            assertEquals(22f, awaitItem())
        }
    }

    @Test
    fun `string setting round-trips`() = runTest {
        repository.appThemeMode.test {
            assertEquals(SettingsRepository.DEFAULT_FOLLOW_SYSTEM, awaitItem())
            repository.setAppThemeMode("dark")
            assertEquals("dark", awaitItem())
        }
    }

    /**
     * 主题名的缺省值不在 data class 默认参数里，而在 [SettingsRepository.settings] 的
     * elvis 右侧：Kotlin 构造时求值默认参数会让**每次读设置**都调一次原生主题目录。
     * 本用例钉住「缺键时取默认、有键时取存储值」这条真正的行为。
     */
    @Test
    fun `theme names fall back to the catalog defaults only when unset`() = runTest {
        repository.themeName.test {
            assertEquals(TEST_DARK_THEME, awaitItem())
            repository.setThemeName("Solarized Dark")
            assertEquals("Solarized Dark", awaitItem())
        }
        repository.dayThemeName.test {
            assertEquals(TEST_LIGHT_THEME, awaitItem())
        }
        repository.nightThemeName.test {
            assertEquals(TEST_DARK_THEME, awaitItem())
        }
    }

    @Test
    fun `bootstrap url round-trips`() = runTest {
        repository.bootstrapUrl.test {
            assertEquals("", awaitItem())
            repository.recordBootstrapUrlEdit("https://packages.termux.dev/apt/termux-main/bootstrap.zip")
            assertEquals("https://packages.termux.dev/apt/termux-main/bootstrap.zip", awaitItem())
        }
    }

    @Test
    fun `distinct keys do not clobber each other`() = runTest {
        repository.setFontSize(20f)
        repository.setThemeName("Solarized Dark")
        repository.fontSize.test {
            assertEquals(20f, awaitItem())
        }
        repository.themeName.test {
            assertEquals("Solarized Dark", awaitItem())
        }
    }

    @Test
    fun `first launch default font size adapts to screen width`() {
        // spec default-typography: 14sp floor / 24sp cap
        // (user-reported "default too small"; was 12/18 in ).
        // 断在两端之外取一个中间宽度：那里不触发钳位，断言的是这个默认值**存在的
        // 理由**——手机与平板显示相同的列数（`widthDp / sp / 字形宽比` 恒定）。
        // 直接照抄 52f / 0.6f 来对照则只会把公式复述一遍，改错了照样全绿。
        listOf(520f, 600f, 700f).forEach { widthDp ->
            val size = SettingsRepository.defaultFontSizeFor(widthDp)
            val columns = widthDp / size / SettingsRepository.MONOSPACE_CHAR_ASPECT
            assertEquals(
                "widthDp=$widthDp 的默认字号 $size 给出 $columns 列，目标 52 列",
                52f,
                columns,
                0.01f,
            )
        }
    }

    @Test
    fun `first launch font size is persisted only while unset`() = runTest {
        repository.applyFirstLaunchDefaultFontSize(360f)
        // 360dp lands on the  14sp floor.
        assertEquals(14f, repository.fontSize.first(), 0.01f)
        // An explicit user pick wins over re-applying the default.
        repository.setFontSize(14f)
        repository.applyFirstLaunchDefaultFontSize(800f)
        assertEquals(14f, repository.fontSize.first(), 0.01f)
    }

    @Test
    fun `font family round-trips then clears`() = runTest {
        assertEquals("", repository.fontFamily.first())
        repository.setFontFamily("Noto Sans Mono")
        assertEquals("Noto Sans Mono", repository.fontFamily.first())
        repository.clearFontFamily()
        assertEquals("", repository.fontFamily.first())
    }

    @Test
    fun `corrupted preferences degrade to defaults instead of crashing`() = runTest {
        File(prefsDir, "settings.preferences_pb").writeBytes(byteArrayOf(1, 2, 3, 4))
        assertEquals(14f, repository.fontSize.first(), 0.01f)
    }

    @Test
    fun `shell defaults to empty and round-trips raw`() = runTest {
        assertEquals("", repository.shell.first())
        repository.setShell("/data/data/com.termux/files/usr/bin/bash -l")
        assertEquals("/data/data/com.termux/files/usr/bin/bash -l", repository.shell.first())
    }

    @Test
    fun `clearing settings empties every key`() = runTest {
        repository.setFontSize(30f)
        repository.setShell("/data/data/com.termux/files/usr/bin/bash -l")
        repository.setFontFamily("Noto Sans Mono")

        repository.clearAllSettings()

        assertEquals(14f, repository.fontSize.first(), 0.01f)
        assertEquals("", repository.shell.first())
        assertEquals("", repository.fontFamily.first())
    }

    @Test
    fun `clearing settings survives the prefs directory being deleted`() = runTest {
        repository.setShell("/data/data/com.termux/files/usr/bin/bash -l")

        // 清除应用数据的顺序是「先清内容、再删目录」：DataStore 缓存着已读入的
        // 状态，只删目录的话内存态仍是旧值，界面与后续写入都会把旧值带回来。
        repository.clearAllSettings()
        prefsDir.deleteRecursively()
        prefsDir.mkdirs()

        assertEquals("目录被删后设置仍须为空", "", repository.shell.first())
    }

    @Test
    fun `a write after clearing does not resurrect the cleared values`() = runTest {
        repository.setShell("/bin/cleared-shell-marker")
        repository.clearAllSettings()
        prefsDir.deleteRecursively()
        prefsDir.mkdirs()

        repository.setShell("/bin/after-clear")

        // 直接读落盘字节：进程内的缓存已清空，文件里才真的没有旧值。
        // 不再开第二个 DataStore 读同一文件：DataStore 不允许同一文件存在两个活动实例。
        val persisted = File(prefsDir, "settings.preferences_pb").readBytes().toString(Charsets.UTF_8)
        assertTrue("清除后写入的值必须落盘", persisted.contains("/bin/after-clear"))
        assertFalse("清除前的值不得复活", persisted.contains("/bin/cleared-shell-marker"))
    }

    @Test
    fun `latest bootstrap url survives a blocked write`() {
        val writeStarted = CompletableDeferred<Unit>()
        val releaseWrite = CompletableDeferred<Unit>()
        val durable =
            PreferenceDataStoreFactory.create(
                corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            ) {
                File(prefsDir, "blocked_write.preferences_pb")
            }
        val gated = mockk<DataStore<Preferences>>()
        every { gated.data } returns durable.data
        coEvery { gated.updateData(any()) } coAnswers {
            writeStarted.complete(Unit)
            releaseWrite.await()
            durable.updateData(firstArg())
        }
        val provider = mockk<SettingsDataStoreProvider>()
        every { provider.dataStore } returns gated
        every { provider.screenWidthDp } returns 360f
        val gatedRepository = SettingsRepository(provider)
        val urls = (0..7).map { "https://example.invalid/$it.zip" }

        runBlocking {
            gatedRepository.recordBootstrapUrlEdit(urls.first())
            withTimeout(2_000) { writeStarted.await() }
            urls.drop(1).forEach(gatedRepository::recordBootstrapUrlEdit)
            assertEquals(
                "缓冲饱和时也必须保留最后编辑值",
                urls.last(),
                gatedRepository.latestBootstrapUrlEdit(),
            )
            releaseWrite.complete(Unit)
            // 等到流真的发布该值，而不是睡一个「通常够用」的固定时长：
            // 固定睡眠在并行跑测试/CI 负载高时必然偶发失败。
            assertEquals(
                "最后一次编辑必须最终落盘",
                urls.last(),
                withTimeout(5_000) { gatedRepository.bootstrapUrl.first { it == urls.last() } },
            )
        }
    }

    @Test
    fun `failed bootstrap url write does not kill later edits`() {
        // 回归：`put` 抛异常若逃出 collect，收集器死亡，后续所有编辑静默丢失。
        // 这条用例让第一次写入失败，第二次必须照常落盘。
        val durable =
            PreferenceDataStoreFactory.create(
                corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            ) {
                File(prefsDir, "failed_write.preferences_pb")
            }
        val gated = mockk<DataStore<Preferences>>()
        every { gated.data } returns durable.data
        val firstWriteAttempted = CompletableDeferred<Unit>()
        var failNextWrite = true
        coEvery { gated.updateData(any()) } coAnswers {
            if (failNextWrite) {
                failNextWrite = false
                firstWriteAttempted.complete(Unit)
                throw java.io.IOException("disk full")
            }
            durable.updateData(firstArg())
        }
        val provider = mockk<SettingsDataStoreProvider>()
        every { provider.dataStore } returns gated
        every { provider.screenWidthDp } returns 360f
        val gatedRepository = SettingsRepository(provider)
        val firstUrl = "https://example.invalid/first.zip"
        val secondUrl = "https://example.invalid/second.zip"

        runBlocking {
            gatedRepository.recordBootstrapUrlEdit(firstUrl)
            // 等「第一次写入确实尝试过」这一事实，而不是睡一个期望够长的固定时长：
            // 失败发生在 updateData 内，尝试返回后持久层必然仍是空值。
            withTimeout(5_000) { firstWriteAttempted.await() }
            assertEquals(
                "失败的写入不得留下半截值",
                "",
                gatedRepository.bootstrapUrl.first(),
            )
            gatedRepository.recordBootstrapUrlEdit(secondUrl)
            assertEquals(
                "首次写入失败后收集器仍须存活",
                secondUrl,
                withTimeout(5_000) { gatedRepository.bootstrapUrl.first { it == secondUrl } },
            )
        }
    }

    @Test
    fun `dropped debounce edit never lands, rearmed edit does`() {
        // R16-T6 回归：防抖窗口内（300ms）的编辑若在清除前被丢弃，
        // 绝不能在删除之后落地重建 preferences_pb；清除完成后新编辑照常落盘。
        // 仓库收集器跑在真实 IO 调度器上，故用真实睡眠（2 倍余量），非虚拟时间。
        val droppedUrl = "https://example.invalid/dropped.zip"
        val rearmedUrl = "https://example.invalid/rearmed.zip"
        kotlinx.coroutines.runBlocking {
            repository.recordBootstrapUrlEdit(droppedUrl)
            repository.dropPendingBootstrapUrlEdits()
            assertEquals(null, repository.latestBootstrapUrlEdit())
            Thread.sleep(700)
            assertEquals(
                "dropped edit must not land after clear",
                "",
                repository.bootstrapUrl.first(),
            )
            repository.rearmBootstrapUrlEdits()
            repository.recordBootstrapUrlEdit(rearmedUrl)
            assertEquals(rearmedUrl, repository.latestBootstrapUrlEdit())
            assertEquals(
                rearmedUrl,
                withTimeout(5_000) { repository.bootstrapUrl.first { it == rearmedUrl } },
            )
        }
    }

    @Test
    fun `a dropped edit never lands when writes are rearmed before the debounce expires`() {
        // 清除应用数据要删目录并重建 prefs，耗时可能长于剩余的防抖窗口。
        // 清除一结束写入就被重新武装，而那个待写值此刻才到期——它 MUST NOT
        // 把清除前的 URL 写回去：否则「已清除」的提示是假话，URL 悄悄回来了。
        val droppedUrl = "https://example.invalid/dropped.zip"
        kotlinx.coroutines.runBlocking {
            repository.recordBootstrapUrlEdit(droppedUrl)
            repository.dropPendingBootstrapUrlEdits()
            // 清除完成：写入在此刻就已重新武装，而防抖窗口还没到期。
            repository.rearmBootstrapUrlEdits()
            Thread.sleep(700)
            assertEquals(
                "dropped edit must not land after clear",
                "",
                repository.bootstrapUrl.first(),
            )
        }
    }
}
