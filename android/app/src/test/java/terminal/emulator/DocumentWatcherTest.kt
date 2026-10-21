// ! Robolectric unit tests for DocumentWatcher's notification mapping.
// !
// ! The inotify plumbing itself is covered by the instrumented suite
// ! (DocumentsProviderInstrumentedTest#terminal_side_change_notifies_open_listing);
// ! here the kernel events are injected directly, so the mapping
// ! (event → child-listing URI + document URI) and the coalescing window
// ! are asserted on the JVM without a device.

package terminal.emulator

import android.content.Context
import android.net.Uri
import android.os.FileObserver
import android.os.Handler
import android.os.Looper
import android.provider.DocumentsContract
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows
import org.robolectric.shadows.ShadowLooper
import java.io.File
import java.util.Collections

@RunWith(RobolectricTestRunner::class)
class DocumentWatcherTest {
    private val authority = "com.termux.documents"
    private lateinit var context: Context
    private lateinit var home: File
    private lateinit var watcher: DocumentWatcher

    @Before
    fun setUp() {
        context = RuntimeEnvironment.getApplication()
        home = File(context.filesDir, "watch-home").apply { mkdirs() }
        watcher = DocumentWatcher(context) { home }
    }

    /** 记录被通知 URI 的观察者，按标签计数（合并窗口的断言要看「几次」而非「有没有」）。 */
    private class CountingObserver : android.database.ContentObserver(Handler(Looper.getMainLooper())) {
        val hits = Collections.synchronizedList(mutableListOf<String>())

        override fun onChange(selfChange: Boolean) {
            hits.add("notify")
        }
    }

    private fun observe(uri: Uri): CountingObserver {
        val observer = CountingObserver()
        context.contentResolver.registerContentObserver(uri, true, observer)
        return observer
    }

    /**
     * 跑空全部 Looper 队列让合并窗口到期。
     *
     * 事件先进入 watcher 的专线线程（延迟 [DocumentWatcher] 的合并窗口），广播再经
     * 各 Looper 派发；Robolectric 默认暂停 Looper，不推进虚拟时钟延迟任务永不执行。
     * 每轮推进 1 秒（窗口的宽松上界），直到预期到齐或轮次耗尽。
     */
    private fun advanceUntil(expected: List<CountingObserver>, maxRounds: Int = 8) {
        repeat(maxRounds) {
            if (expected.all { it.hits.isNotEmpty() }) return
            ShadowLooper.getAllLoopers().forEach { looper ->
                Shadows.shadowOf(looper).idleFor(java.time.Duration.ofSeconds(1))
            }
        }
    }

    @Test
    fun `create in the root notifies the root listing and the new document`() {
        val children = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))
        val document = observe(DocumentsContract.buildDocumentUri(authority, "terminal.txt"))

        watcher.onDirectoryEvent("terminal_home", FileObserver.CREATE, "terminal.txt")
        advanceUntil(listOf(children, document))

        assertTrue("列表必须收到通知（幽灵行的根治点）", children.hits.isNotEmpty())
        assertTrue("新条目自身也必须收到通知", document.hits.isNotEmpty())
    }

    @Test
    fun `delete in a nested directory notifies that directory, not the root`() {
        val nested = observe(DocumentsContract.buildChildDocumentsUri(authority, "sub"))
        val root = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))
        val document = observe(DocumentsContract.buildDocumentUri(authority, "sub/gone.txt"))

        watcher.onDirectoryEvent("sub", FileObserver.DELETE, "gone.txt")
        advanceUntil(listOf(nested, document))

        assertTrue("被列的目录要收到通知", nested.hits.isNotEmpty())
        assertTrue("条目 docId 需要带上父路径", document.hits.isNotEmpty())
        assertEquals("根目录列表不受子目录内变动影响", emptyList<String>(), root.hits)
    }

    @Test
    fun `attrib notifies the document without reloading the listing`() {
        val children = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))
        val document = observe(DocumentsContract.buildDocumentUri(authority, "f.txt"))

        watcher.onDirectoryEvent("terminal_home", FileObserver.ATTRIB, "f.txt")
        advanceUntil(listOf(document))

        assertTrue("元数据变化应通知条目自身", document.hits.isNotEmpty())
        assertEquals("属性变化不改变条目列表，不应触发列表重载", emptyList<String>(), children.hits)
    }

    @Test
    fun `events inside the coalescing window collapse into one notification`() {
        val children = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))

        // 批量操作形态：窗口内连续多条事件
        watcher.onDirectoryEvent("terminal_home", FileObserver.CREATE, "a.txt")
        watcher.onDirectoryEvent("terminal_home", FileObserver.CREATE, "b.txt")
        watcher.onDirectoryEvent("terminal_home", FileObserver.MOVED_TO, "b.txt")
        advanceUntil(listOf(children))

        assertEquals("同一目录的多条事件必须合并成一次列表通知", 1, children.hits.size)
    }

    /**
     * 只改内容的写入（终端侧 `>>`、`sed -i`）内核只报 MODIFY，不改 inode 元数据。
     * 监视集必须含它，否则列表的大小/时间列停在旧值。
     */
    @Test
    fun `modify notifies the document without reloading the listing`() {
        val children = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))
        val document = observe(DocumentsContract.buildDocumentUri(authority, "appended.log"))

        watcher.onDirectoryEvent("terminal_home", FileObserver.MODIFY, "appended.log")
        advanceUntil(listOf(document))

        assertTrue("内容变化必须通知条目自身", document.hits.isNotEmpty())
        assertEquals("改内容不改变条目列表，不应触发列表重载", emptyList<String>(), children.hits)
    }

    /**
     * 提供者被系统停止后不得继续广播旧事件，但重新监视后必须恢复——否则一次
     * `shutdown` 就让文件管理器再也看不到终端侧改动。
     */
    @Test
    fun `shutdown drops pending events and later events still notify`() {
        val children = observe(DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"))

        watcher.onDirectoryEvent("terminal_home", FileObserver.CREATE, "before-shutdown.txt")
        watcher.shutdown()
        advanceUntil(listOf(children), maxRounds = 3)
        assertEquals("停止时待播报的事件必须丢弃", emptyList<String>(), children.hits)

        watcher.onDirectoryEvent("terminal_home", FileObserver.CREATE, "after-shutdown.txt")
        advanceUntil(listOf(children))
        assertTrue("停止后重新监视必须继续广播", children.hits.isNotEmpty())
    }
}
