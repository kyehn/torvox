package terminal.emulator

import android.content.res.AssetFileDescriptor
import android.database.Cursor
import android.database.MatrixCursor
import android.graphics.Point
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import terminal.emulator.runtime.LogUtil
import java.io.File

class TerminalDocumentsProvider : DocumentsProvider() {
    companion object {
        const val AUTHORITY = "com.termux.documents"
        const val ROOT_ID = "terminal_home"
        private const val TAG = "TerminalDocumentsProvider"

        // 搜索结果上限，与 Termux 的 MAX_SEARCH_RESULTS 对齐。
        private const val MAX_SEARCH_RESULTS = 50

        /**
         * 搜索遍历的目录项上限（[querySearchDocuments] 的工作量上限）。
         *
         * 结果上限挡不住工作量：一个不含匹配项的 home 目录会被**完整**遍历——无深度上限、
         * 无已访问集合、无取消检查，全程占用调用方的 binder 线程。已装满的 SDK 目录轻易
         * 达到数十万项，于是文档选择器里的搜索既不返回也不响应关闭。
         *
         * 故结果与遍历量各自设限。触顶即停止并如实告知调用方（见 `truncated`），不谎称
         * 「这就是全部结果」。
         */
        private const val MAX_SEARCH_VISITS = 20_000

        /** SAF mode 字符串的最大长度（`ParcelFileDescriptor.parseMode` 的上界）。 */
        private const val MAX_OPEN_MODE_LENGTH = 4

        /** 列举超过该耗时即落日志：正常规模在毫秒级，出现即说明目录规模异常。 */
        private const val SLOW_LISTING_MILLIS = 250L

        /** mode 字符串允许的字符（与 `parseOpenModeFallback` 的语义集合一致）。 */
        private const val OPEN_MODE_CHARACTERS = "rwatsd"

        private val ROOT_PROJECTION =
            arrayOf(
                Root.COLUMN_ROOT_ID,
                Root.COLUMN_DOCUMENT_ID,
                Root.COLUMN_TITLE,
                Root.COLUMN_SUMMARY,
                Root.COLUMN_FLAGS,
                Root.COLUMN_ICON,
                Root.COLUMN_MIME_TYPES,
                Root.COLUMN_AVAILABLE_BYTES,
            )

        private val DOC_PROJECTION =
            arrayOf(
                Document.COLUMN_DOCUMENT_ID,
                Document.COLUMN_DISPLAY_NAME,
                Document.COLUMN_MIME_TYPE,
                Document.COLUMN_SIZE,
                Document.COLUMN_LAST_MODIFIED,
                Document.COLUMN_FLAGS,
            )

        fun encodeDocId(file: File, rootDir: File): String? {
            val rootPath = rootDir.canonicalPath
            // 对符号链接，编码链接自身的路径而非其规范目标：
            // SAF 客户端于是寻址该链接条目本身，
            // deleteDocument 只删除链接——绝不会删除目标的整棵目录树。
            // 包含性仍针对规范路径检查（指向 home 之外的链接在下文被跳过，同之前）。
            val filePath =
                if (java.nio.file.Files.isSymbolicLink(file.toPath())) {
                    file.path
                } else {
                    file.canonicalPath
                }
            val fileCanonical = file.canonicalPath
            // 终端中指向 home 目录之外的符号链接很常见
            // （如 ln -s /sdcard/x ~/link）。跳过它们而非抛异常
            // ——require() 会使每次浏览都中断整个 SAF 目录列举。
            if (!(fileCanonical.startsWith(rootPath + File.separator) || fileCanonical == rootPath)) {
                return null
            }
            return if (fileCanonical == rootPath) {
                ROOT_ID
            } else {
                filePath.removePrefix(rootPath + File.separator)
            }
        }

        fun decodeDocId(docId: String, rootDir: File): File {
            if (docId == ROOT_ID) return rootDir.canonicalFile
            // 树 URI 线形：外部文件管理器经树 URI 寻址时，传进来的 docId 会带上树根前缀
            // （`terminal_home/nix.conf`），而本提供者行内只发行无前缀的相对路径。
            // 真机 logcat 实证：该前缀形 delete/rename 落到 `home/terminal_home/…`（不存在）而失败。
            // 精确路径优先（家目录里真有一个 `terminal_home` 子目录时，其行内地址不受影响），
            // 仅当精确路径不存在、去前缀后仍在根内时接受别名；两者都不存在时按原样解码，
            // 错误形态与此前一致。返回值恒为无前缀的规范地址，调用方回传的 docId 自愈为行内形式。
            val exact = File(rootDir, docId)
            val rootPrefix = "$ROOT_ID/"
            if (docId.startsWith(rootPrefix) && !exact.exists() &&
                !java.nio.file.Files.isSymbolicLink(exact.toPath())
            ) {
                // 去前缀别名只收无链接的普通路径：前缀形恰好命中一条家目录内的链接时，
                // 交给原样解码走链接自身的读写分支（重命名只动链接、删除只删链接）。
                val stripped = File(rootDir, docId.removePrefix(rootPrefix))
                if (!java.nio.file.Files.isSymbolicLink(stripped.toPath())) {
                    val resolved = stripped.canonicalFile
                    requireInsideRoot(resolved, rootDir)
                    return resolved
                }
            }
            val resolved = exact.canonicalFile
            requireInsideRoot(resolved, rootDir)
            return resolved
        }

        fun isHomeLink(rawFile: File, rootDir: File): Boolean {
            // 包含性针对链接*自身*路径检查，绝不针对其规范目标：
            // rawFile 可能含 ".." 段（恶意 docId），而 File 不会规范化它们。
            // 规范解析父目录后重新追加文件名，使检查覆盖实际被触及的条目。
            // 父目录为 null（无目录部分的裸文件名）无法逃出根目录。
            if (!java.nio.file.Files.isSymbolicLink(rawFile.toPath())) return false
            val parentCanonical = rawFile.parentFile?.canonicalFile ?: return true
            val linkPath = File(parentCanonical, rawFile.name).canonicalPath
            val rootPath = rootDir.canonicalPath
            return linkPath.startsWith(rootPath + File.separator) || linkPath == rootPath
        }

        internal fun requireInsideRoot(file: File, rootDir: File) {
            val root = rootDir.canonicalFile
            val target = file.canonicalFile
            if (!(target.path.startsWith(root.path + File.separator) || target == root)) {
                throw java.io.FileNotFoundException(
                    "Access denied: ${target.path} is outside the terminal home directory",
                )
            }
        }

        internal fun parseOpenModeFallback(mode: String): Int {
            // 抛 FileNotFoundException 而非 require() 的 IllegalArgumentException：
            // openDocument 跨 Binder 返回，后者在调用方进程里是未捕获异常（崩溃），
            // 前者才是 SAF 契约里「这个文档打不开」的可处理失败。
            fun unsupported(reason: String): Nothing =
                throw java.io.FileNotFoundException("Unsupported open mode '$mode': $reason")
            if (mode.isEmpty() || mode.length > MAX_OPEN_MODE_LENGTH) {
                unsupported("length must be 1..$MAX_OPEN_MODE_LENGTH")
            }
            if (!mode.all { it in OPEN_MODE_CHARACTERS }) {
                unsupported("allowed characters are $OPEN_MODE_CHARACTERS")
            }
            val read = mode.contains('r')
            val write = mode.contains('w')
            if (!read && !write) {
                unsupported("needs at least one of r/w")
            }
            var parsed = 0
            parsed =
                parsed or
                if (read && write) {
                    ParcelFileDescriptor.MODE_READ_WRITE
                } else if (write) {
                    ParcelFileDescriptor.MODE_WRITE_ONLY
                } else {
                    ParcelFileDescriptor.MODE_READ_ONLY
                }
            if (write) parsed = parsed or ParcelFileDescriptor.MODE_CREATE
            if (mode.contains('a')) parsed = parsed or ParcelFileDescriptor.MODE_APPEND
            if (mode.contains('t')) parsed = parsed or ParcelFileDescriptor.MODE_TRUNCATE
            return parsed
        }
    }

    /**
     * 提供者创建即开始监视家目录：客户端看得见哪一层，查询哪一层时再登记
     * （见 [DocumentWatcher]）。根目录在此登记，`queryChildDocuments` 登记其余。
     */
    override fun onCreate(): Boolean {
        watcher.watch(queries.rootDir())
        return true
    }

    private val queries: DocumentQueries by lazy {
        DocumentQueries(requireNotNull(context) { "TerminalDocumentsProvider requires a Context" })
    }

    private val watcher: DocumentWatcher by lazy {
        DocumentWatcher(
            requireNotNull(context) { "TerminalDocumentsProvider requires a Context" },
            { queries.rootDir() },
        )
    }

    private val mutations: DocumentMutations by lazy {
        DocumentMutations(
            requireNotNull(context) { "TerminalDocumentsProvider requires a Context" },
            { queries.rootDir() },
        )
    }

    /**
     * 写回通知的回调线程。`openDocument` 跑在 Binder 线程上，而
     * `ParcelFileDescriptor.OnCloseListener` 需要一个带 Looper 的 Handler。
     *
     * 用专用线程而非主 Looper：回调要读盘（`rootDir()` 的 `mkdirs`/`isDirectory`）
     * 并广播，写在主线程既触发 StrictMode 的磁盘写告警，也会让通知排在整帧
     * 渲染之后；进程被切后台/回收时主 Looper 排空不完，通知就永久丢了。
     * 专用线程只做这一次 I/O，不与渲染争主线程；provider 的生命周期等于进程，
     * 线程随进程消亡，无需单独回收（`ContentProvider` 也没有可覆写的销毁钩子）。
     */
    private val closeNotifyThread: android.os.HandlerThread by lazy {
        android.os.HandlerThread("terminal-documents-writeback").apply { start() }
    }

    private val closeNotifyHandler: android.os.Handler by lazy {
        android.os.Handler(closeNotifyThread.looper)
    }

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val cols = projection ?: ROOT_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        cursor.newRow().apply {
            if (Root.COLUMN_ROOT_ID in cols) add(Root.COLUMN_ROOT_ID, ROOT_ID)
            if (Root.COLUMN_DOCUMENT_ID in cols) add(Root.COLUMN_DOCUMENT_ID, encodeDocId(rootDir, rootDir))
            if (Root.COLUMN_TITLE in cols) add(Root.COLUMN_TITLE, "Terminal Home")
            if (Root.COLUMN_SUMMARY in cols) add(Root.COLUMN_SUMMARY, rootDir.absolutePath)
            if (Root.COLUMN_FLAGS in cols) {
                add(
                    Root.COLUMN_FLAGS,
                    Root.FLAG_SUPPORTS_CREATE or Root.FLAG_SUPPORTS_SEARCH or Root.FLAG_SUPPORTS_IS_CHILD,
                )
            }
            if (Root.COLUMN_ICON in cols) add(Root.COLUMN_ICON, R.mipmap.ic_launcher)
            if (Root.COLUMN_MIME_TYPES in cols) add(Root.COLUMN_MIME_TYPES, "*/*")
            if (Root.COLUMN_AVAILABLE_BYTES in cols) add(Root.COLUMN_AVAILABLE_BYTES, rootDir.freeSpace)
        }
        context?.contentResolver?.let { resolver ->
            cursor.setNotificationUri(
                resolver,
                android.provider.DocumentsContract.buildRootsUri(AUTHORITY),
            )
        }
        return cursor
    }

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor {
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        val file = queries.resolveLinkEntry(documentId, rootDir)
        queries.addDocRow(cursor, file, rootDir, cols)
        context?.contentResolver?.let { resolver ->
            // 通知 URI 取由文件反推的行内 docId，而非客户端传来的原样字符串。
            // 树 URI 寻址传的是 `terminal_home/nix.conf`，广播只发行 `nix.conf`，
            // 两者构成兄弟 URI——注册成前缀形式就永远收不到广播，客户端于是
            // 停在旧快照上，拿幽灵行把创建/改名拒成「路径已存在」。
            // 站外链接（`ln -s /sdcard/x ~/link`）编码得 null：addDocRow 已按同一
            // 判断跳过该行，这里同样不设通知 URI，而不是把合法输入当不变量破坏。
            encodeDocId(file, rootDir)?.let { docId ->
                cursor.setNotificationUri(
                    resolver,
                    android.provider.DocumentsContract.buildDocumentUri(AUTHORITY, docId),
                )
            }
        }
        return cursor
    }

    override fun queryChildDocuments(
        parentDocumentId: String,
        projection: Array<out String>?,
        sortOrder: String?,
    ): Cursor {
        val started = android.os.SystemClock.elapsedRealtime()
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        requireInsideRoot(parent, rootDir)
        watcher.watch(parent)
        val children = parent.listFiles() ?: emptyArray()
        val sorted =
            children.sortedWith(
                compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() },
            )
        for (child in sorted) {
            queries.addDocRow(cursor, child, rootDir, cols)
        }
        // 逐条目 stat 在巨大目录里会拖慢整次列举；「保存超时」一类反馈需要一条
        // 「慢在哪一层」的证据，正常规模（毫秒级）不落日志。
        val elapsed = android.os.SystemClock.elapsedRealtime() - started
        if (elapsed >= SLOW_LISTING_MILLIS) {
            LogUtil.w(TAG, "slow listing '$parentDocumentId': ${sorted.size} entries in ${elapsed}ms")
        }
        context?.contentResolver?.let { resolver ->
            // 同 queryDocument：通知 URI 必须是行内无前缀形式，广播也只发这种，
            // 客户端带树根前缀寻址时两者才是同一个 URI。parent 已过
            // requireInsideRoot，编码失败即不变量被破坏。
            val parentId =
                requireNotNull(encodeDocId(parent, rootDir)) {
                    "queryChildDocuments: cannot encode docId for ${parent.path}"
                }
            cursor.setNotificationUri(
                resolver,
                android.provider.DocumentsContract.buildChildDocumentsUri(AUTHORITY, parentId),
            )
        }
        return cursor
    }

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        // 本重写直接落地（基类对 openDocument 无实现），取消检查得自己做：
        // 客户端取消后不再等句柄，此时继续开文件等于白做 I/O。
        signal?.throwIfCanceled()
        val rootDir = queries.rootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        // 不要求条目已存在：目录在 queryDocument 里声明了 FLAG_DIR_SUPPORTS_CREATE，
        // 平台据此允许客户端经 openDocument 直接落盘（编辑器 SAVE、文件管理器
        // 覆盖写都走这条）。此前「只打开已存在条目」的理由是防止客户端凭空造出
        // .mkshrc 等点文件——该前提不成立：createDocument 的 sanitize 只拒空名、
        // "."、".." 与路径分隔符，并不拦点文件，客户端一次 createDocument 即可造出
        // 同样的文件。那条检查因此不增加任何安全性，只让按平台惯例用 MODE_CREATE
        // 的客户端拿到 FileNotFoundException，正是「无法修改文件」的直接成因。
        // 是否创建交由模式位决定：只读模式不含 CREATE，文件不存在时
        // ParcelFileDescriptor.open 自然抛 FileNotFoundException。
        // 显式映射而非委托 parseMode：实测本平台 parseMode("w") 不含
        // TRUNCATE，会破坏 SAF 的“w 截断”语义（单测已锁定）。
        // 已知模式走精确映射；未知但合法的组合（如编辑器偶发的 "rwa"）按语义派生，
        // 仅完全非法的模式才抛错，避免“无法修改”。
        val fileMode =
            when (mode) {
                "r" -> ParcelFileDescriptor.MODE_READ_ONLY

                "w",
                "wt",
                ->
                    ParcelFileDescriptor.MODE_WRITE_ONLY or
                        ParcelFileDescriptor.MODE_CREATE or
                        ParcelFileDescriptor.MODE_TRUNCATE

                "wa" ->
                    ParcelFileDescriptor.MODE_WRITE_ONLY or
                        ParcelFileDescriptor.MODE_CREATE or
                        ParcelFileDescriptor.MODE_APPEND

                "rw" -> ParcelFileDescriptor.MODE_READ_WRITE or ParcelFileDescriptor.MODE_CREATE

                "rwt" ->
                    ParcelFileDescriptor.MODE_READ_WRITE or
                        ParcelFileDescriptor.MODE_CREATE or
                        ParcelFileDescriptor.MODE_TRUNCATE

                "rws",
                "rwd",
                -> ParcelFileDescriptor.MODE_READ_WRITE or ParcelFileDescriptor.MODE_CREATE

                else -> parseOpenModeFallback(mode)
            }
        // 句柄关闭即视为外部写回：广播文档与父目录，文件选择器才能看到大小/时间变化。裸 open() 时外部编辑"看起来没反应"。
        return ParcelFileDescriptor.open(file, fileMode, closeNotifyHandler) {
            mutations.notifyWritten(file)
        }
    }

    override fun openDocumentThumbnail(
        documentId: String,
        sizeHint: Point?,
        signal: CancellationSignal?,
    ): AssetFileDescriptor {
        // 仅图片行声明 FLAG_SUPPORTS_THUMBNAIL，缩略图即原文件只读句柄：
        // 平台约定「不能生成缩略图的提供者直接交整档，由选择器缩放」，故不按
        // sizeHint 另造小图；取消检查仍须自己做（本重写不走基类实现）。
        signal?.throwIfCanceled()
        val rootDir = queries.rootDir()
        val file = queries.resolveLinkEntry(documentId, rootDir)
        val parcelFileDescriptor = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        return AssetFileDescriptor(parcelFileDescriptor, 0, file.length())
    }

    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String =
        trace("createDocument '$displayName'") {
            mutations.createDocument(parentDocumentId, mimeType, displayName)
        }

    override fun renameDocument(documentId: String, displayName: String): String? =
        trace("renameDocument '$documentId' -> '$displayName'") {
            mutations.renameDocument(documentId, displayName)
        }

    override fun copyDocument(sourceDocumentId: String, targetParentDocumentId: String): String =
        trace("copyDocument '$sourceDocumentId'") {
            mutations.copyDocument(sourceDocumentId, targetParentDocumentId)
        }

    override fun moveDocument(
        sourceDocumentId: String,
        sourceParentDocumentId: String,
        targetParentDocumentId: String,
    ): String = trace("moveDocument '$sourceDocumentId'") {
        mutations.moveDocument(sourceDocumentId, targetParentDocumentId)
    }

    override fun deleteDocument(documentId: String) {
        trace("deleteDocument '$documentId'") { mutations.deleteDocument(documentId) }
    }

    /**
     * 变更入口的统一取证：每次调用的耗时与结果都进 logcat。
     *
     * 真机反馈里「保存超时」「重命名卡死」两种形态都只有客户端一侧的现象——是提供者
     * 慢、根本没被调用，还是客户端自己等待，从现有日志分不出来。落一条「进来了、多久
     * 出去、结果如何」就能把下一份反馈直接归因，不必再靠猜。
     */
    private fun <T> trace(operation: String, block: () -> T): T {
        val started = android.os.SystemClock.elapsedRealtime()
        try {
            val result = block()
            LogUtil.i(TAG, "$operation ok (${android.os.SystemClock.elapsedRealtime() - started}ms)")
            return result
        } catch (error: Exception) {
            LogUtil.w(
                TAG,
                "$operation failed (${android.os.SystemClock.elapsedRealtime() - started}ms)",
                error,
            )
            throw error
        }
    }

    override fun isChildDocument(parentDocumentId: String, documentId: String): Boolean {
        val rootDir = queries.rootDir()
        return try {
            val parent = decodeDocId(parentDocumentId, rootDir)
            val child = decodeDocId(documentId, rootDir)
            child.canonicalPath.startsWith(parent.canonicalPath + File.separator)
        } catch (error: java.io.FileNotFoundException) {
            LogUtil.w(TAG, "isChildDocument: docId outside root", error)
            false
        } catch (error: IllegalArgumentException) {
            LogUtil.w(TAG, "isChildDocument: malformed docId", error)
            false
        }
    }

    override fun getDocumentType(documentId: String): String {
        val rootDir = queries.rootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        return if (file.isDirectory) Document.MIME_TYPE_DIR else queries.getMimeType(file.name)
    }

    override fun querySearchDocuments(rootId: String, query: String, projection: Array<out String>?): Cursor {
        // 与 Termux 一致的按文件名搜索：迭代遍历、上限截断、符号链接
        // 不得跳出 home。查询词双向小写，修正 Termux 仅小写文件名的遗漏。
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        val rootPath = rootDir.canonicalPath
        val needle = query.lowercase()
        val pending = ArrayDeque<File>()
        pending.addLast(decodeDocId(rootId, rootDir))
        var visits = 0
        var truncated = false
        while (pending.isNotEmpty() && cursor.count < MAX_SEARCH_RESULTS) {
            if (visits >= MAX_SEARCH_VISITS) {
                truncated = true
                break
            }
            visits++
            val current = pending.removeFirst()
            val canonical = queries.canonicalOrNull(current) ?: continue
            if (!(canonical.startsWith(rootPath + File.separator) || canonical == rootPath)) {
                continue
            }
            if (!java.nio.file.Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                if (canonical != rootPath && current.name.lowercase().contains(needle)) {
                    queries.addDocRow(cursor, current, rootDir, cols)
                }
                current.listFiles()?.forEach { pending.addLast(it) }
            } else if (current.name.lowercase().contains(needle)) {
                queries.addDocRow(cursor, current, rootDir, cols)
            }
        }
        if (truncated) {
            // 静默截断会让用户以为「文件不存在」：只搜到部分目录时必须留证据。
            LogUtil.i(
                "TerminalDocumentsProvider",
                "querySearchDocuments: traversal hit $MAX_SEARCH_VISITS entries; results truncated",
            )
        }
        context?.contentResolver?.let { resolver ->
            cursor.setNotificationUri(
                resolver,
                android.provider.DocumentsContract.buildSearchDocumentsUri(AUTHORITY, rootId, query),
            )
        }
        return cursor
    }
}
