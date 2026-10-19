package terminal.emulator

import android.content.Context
import android.os.FileObserver
import android.os.Handler
import android.os.HandlerThread
import android.provider.DocumentsContract
import java.io.File

/**
 * 家目录变更监视：把**绕过提供者**的改动（终端里的 `rm`/`mv`/解压/git 等）转成
 * `notifyChange`，文件管理器的列表才不会停在旧快照上。
 *
 * 平台契约要求「数据变化即广播，否则界面必然过期」，而提供者此前只广播自己的
 * 变更操作，磁盘被 shell 直接改动时一无所知。真机实证：系统 Files 打开家目录，
 * 终端侧 `rm ghost.txt` 后列表 4 秒后仍显示 `ghost.txt`（幽灵行）。客户端拿这类
 * 幽灵行当已占用的名字，创建/重命名/粘贴就被它自己拒成「路径已存在」，而选择
 * 「覆盖」绕过预检后又能成功——报障的形态与此完全一致。
 *
 * 只监视「曾被 [TerminalDocumentsProvider.queryChildDocuments] 查询过」的目录：
 * 客户端看得见哪一层，就守哪一层，查询本身即是登记。每个受监视目录占一个
 * inotify 实例（应用级上限约 128），故设 [MAX_WATCHED_DIRECTORIES] 上限并按最近
 * 使用淘汰：宁可放弃最久未看的目录，也不让 inotify 实例耗尽后整体静默失效。
 */
internal class DocumentWatcher(private val context: Context, private val rootDir: () -> File) {
    private companion object {
        /** 每个受监视目录占一个 inotify 实例，应用级上限约 128，留足余量。 */
        const val MAX_WATCHED_DIRECTORIES = 64

        /**
         * 合并窗口。批量操作（解压、`git checkout`）会为每个条目产生一条事件，
         * 逐条广播会让客户端的列表重载上百次；窗口内的事件合并成一轮通知。
         */
        const val COALESCE_MILLIS = 250L

        /**
         * 关注的事件：条目的出现、消失、改名、移动与属性变化，加上目录自身的
         * 删除/移动（用于注销观察者）。不含 OPEN/ACCESS/CLOSE_*：查询自身就要读
         * 目录，逐次广播等于自激。
         *
         * 含 [FileObserver.MODIFY]：终端侧 `>>`、`sed -i` 一类只改内容的写入不改变
         * inode 元数据，内核只报 MODIFY；缺了它，列表的大小/时间列停在旧值。
         */
        const val EVENTS =
            FileObserver.CREATE or
                FileObserver.DELETE or
                FileObserver.MOVED_FROM or
                FileObserver.MOVED_TO or
                FileObserver.ATTRIB or
                FileObserver.MODIFY or
                FileObserver.DELETE_SELF or
                FileObserver.MOVE_SELF

        /** 命名空间事件：只有它们改变目录的**条目列表**，属性事件只改单个条目的元数据。 */
        const val NAMESPACE_EVENTS =
            FileObserver.CREATE or FileObserver.DELETE or FileObserver.MOVED_FROM or FileObserver.MOVED_TO
    }

    /** 监视中的目录：绝对路径 → 观察者；按访问顺序淘汰最久未用者。 */
    private val watched =
        object : LinkedHashMap<String, FileObserver>(16, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, FileObserver>): Boolean {
                if (size <= MAX_WATCHED_DIRECTORIES) return false
                eldest.value.stopWatching()
                return true
            }
        }

    private val lock = Any()

    /** 合并窗口内累积的待播报：目录 docId（条目列表变化）与条目 docId（元数据变化）。 */
    private val pendingChildren = mutableSetOf<String>()
    private val pendingDocuments = mutableSetOf<String>()
    private var flushScheduled = false

    /**
     * 事件在 FileObserver 自己的线程上到达，广播另起专线：inotify 回调里直接做
     * Binder 调用会让同一观察者后续事件排队等待，批量操作时更慢。线程随进程消亡。
     */
    private val thread by lazy { HandlerThread("terminal-documents-watch").apply { start() } }
    private val handler by lazy { Handler(thread.looper) }
    private val flushRunnable = Runnable { flushPending() }

    /** 登记一个客户端看得见的目录；已登记则只刷新其活跃度。 */
    fun watch(directory: File) {
        val path = directory.path
        synchronized(lock) {
            // 经 `get` 而非 `containsKey`：容器的访问序只被 `get` 刷新，`containsKey`
            // 不算访问。用后者时，一个正在被客户端反复查询的目录位置停在首次插入处，
            // 容量满时先被淘汰——客户端看得见的目录失去监视，幽灵行就此回归。
            if (watched[path] != null) return
        }
        // 根目录自身与根外目录（encodeDocId 返回 null）不监视：
        // 根外目录本就不该出现在选择器里，越界条目在查询时已被跳过。
        val docId = TerminalDocumentsProvider.encodeDocId(directory, rootDir()) ?: return
        val observer =
            object : FileObserver(directory, EVENTS) {
                override fun onEvent(event: Int, name: String?) {
                    if (event and (FileObserver.DELETE_SELF or FileObserver.MOVE_SELF) != 0) {
                        // 目录被删或被改名：路径键与 docId 都已失效，原地注销；
                        // 它在父目录列表里的增删由父目录的观察者播报。
                        forget(path)
                    } else {
                        onDirectoryEvent(docId, event, name)
                    }
                }
            }
        // 创建 → 启用 → 登记 收进同一把锁：binder 线程并发对同一目录登记时，
        // 锁外窗口会让两个观察者并存（各收一份事件，后者覆盖前者后 stop 永不
        // 发生，白耗一个 inotify 槽位）。比对结果在同一锁内确认，避免并发二写。
        synchronized(lock) {
            if (watched[path] != null) return
            watched[path] = observer
        }
        observer.startWatching()
    }

    /**
     * 目录内某条目发生变化：列出该目录的客户端要刷新条目列表，持有该条目的
     * 客户端要刷新其元数据（大小、修改时间）。条目名缺失（个别内核事件不带名字）
     * 时只播报列表。
     */
    fun onDirectoryEvent(directoryDocId: String, event: Int, name: String?) {
        synchronized(lock) {
            if (event and NAMESPACE_EVENTS != 0) pendingChildren += directoryDocId
            if (name != null) {
                pendingDocuments += if (directoryDocId == TerminalDocumentsProvider.ROOT_ID) {
                    name
                } else {
                    "$directoryDocId/$name"
                }
            }
            if (!flushScheduled) {
                flushScheduled = true
                handler.postDelayed(flushRunnable, COALESCE_MILLIS)
            }
        }
    }

    private fun forget(path: String) {
        val observer = synchronized(lock) { watched.remove(path) } ?: return
        observer.stopWatching()
    }

    private fun flushPending() {
        val children: List<String>
        val documents: List<String>
        synchronized(lock) {
            // 先结算再广播：广播期间到达的新事件要能重新排上下一轮。
            children = pendingChildren.toList()
            documents = pendingDocuments.toList()
            pendingChildren.clear()
            pendingDocuments.clear()
            flushScheduled = false
        }
        val resolver = context.contentResolver
        for (docId in children) {
            resolver.notifyChange(
                DocumentsContract.buildChildDocumentsUri(TerminalDocumentsProvider.AUTHORITY, docId),
                null,
            )
        }
        for (docId in documents) {
            resolver.notifyChange(DocumentsContract.buildDocumentUri(TerminalDocumentsProvider.AUTHORITY, docId), null)
        }
    }
}
