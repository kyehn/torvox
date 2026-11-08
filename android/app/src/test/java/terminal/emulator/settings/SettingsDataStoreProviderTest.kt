package terminal.emulator.settings

import android.content.Context
import android.content.ContextWrapper
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList

/**
 * 设置存储的装配：目录创建必须推迟到 DataStore 的 IO 线程。
 *
 * 主线程构造期建目录会让 StrictMode 报 DiskWriteViolation，而该告警出现在冷启动
 * logcat 里，与真实缺陷无法区分。用例直接断言「谁在什么时候创建了目录」，比断言
 * 告警条数更贴近事实。
 */
@RunWith(RobolectricTestRunner::class)
class SettingsDataStoreProviderTest {
    @Test
    fun `construction must not create any directory`() {
        val context = RecordingContext(ApplicationProvider.getApplicationContext())

        SettingsDataStoreProvider(context)

        assertTrue("构造期不得创建目录", context.requestedDirs.isEmpty())
    }

    @Test
    fun `prefs directory is created off the constructing thread on first read`() = runBlocking {
        val context = RecordingContext(ApplicationProvider.getApplicationContext())
        val constructingThread = Thread.currentThread()

        val provider = SettingsDataStoreProvider(context)
        provider.dataStore.data.first()

        assertEquals(listOf("prefs"), context.requestedDirs)
        assertNotSame("目录创建不得落在构造线程上", constructingThread, context.creatingThreads.single())
        assertTrue("首次读取后 prefs 目录必须存在", File(context.createdPaths.single()).isDirectory)
    }

    @Test
    fun `settings round-trip through the lazily created store`() = runBlocking {
        val context = RecordingContext(ApplicationProvider.getApplicationContext())
        val provider = SettingsDataStoreProvider(context)
        val probeKey = stringPreferencesKey("probe")

        provider.dataStore.edit { it[probeKey] = "value" }

        assertEquals("value", provider.dataStore.data.first()[probeKey])
        assertTrue(
            "设置文件必须落在 prefs 目录内",
            File(context.createdPaths.single(), "settings.preferences_pb").isFile,
        )
    }

    /** 记录 `getDir` 的调用者与调用线程，用于判定目录创建发生在哪个线程上。 */
    private class RecordingContext(base: Context) : ContextWrapper(base) {
        val requestedDirs = CopyOnWriteArrayList<String>()
        val creatingThreads = CopyOnWriteArrayList<Thread>()
        val createdPaths = CopyOnWriteArrayList<String>()

        override fun getDir(name: String, mode: Int): File {
            requestedDirs += name
            creatingThreads += Thread.currentThread()
            return super.getDir(name, mode).also { createdPaths += it.absolutePath }
        }
    }
}
