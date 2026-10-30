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
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
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
        prefsDir.deleteRecursively()
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
            Thread.sleep(800)
            assertEquals("最后一次编辑必须最终落盘", urls.last(), gatedRepository.bootstrapUrl.first())
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
        var failNextWrite = true
        coEvery { gated.updateData(any()) } coAnswers {
            if (failNextWrite) {
                failNextWrite = false
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
            Thread.sleep(800)
            assertEquals(
                "失败的写入不得留下半截值",
                "",
                gatedRepository.bootstrapUrl.first(),
            )
            gatedRepository.recordBootstrapUrlEdit(secondUrl)
            Thread.sleep(800)
            assertEquals(
                "首次写入失败后收集器仍须存活",
                secondUrl,
                gatedRepository.bootstrapUrl.first(),
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
            Thread.sleep(700)
            assertEquals(rearmedUrl, repository.bootstrapUrl.first())
        }
    }
}
