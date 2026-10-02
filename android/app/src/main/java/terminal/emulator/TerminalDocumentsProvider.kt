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
            val resolved = File(rootDir, docId).canonicalFile
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

    override fun onCreate(): Boolean = true

    private val queries: DocumentQueries by lazy {
        DocumentQueries(requireNotNull(context) { "TerminalDocumentsProvider requires a Context" })
    }

    private val mutations: DocumentMutations by lazy {
        DocumentMutations(
            requireNotNull(context) { "TerminalDocumentsProvider requires a Context" },
            { queries.rootDir() },
        )
    }

    /**
     * 写回通知的回调线程。`openDocument` 跑在 Binder 线程上，而
     * `ParcelFileDescriptor.OnCloseListener` 需要一个带 Looper 的 Handler；
     * 主 Looper 进程内恒在，是唯一不必在此判空的选择。
     */
    private val closeNotifyHandler: android.os.Handler by lazy {
        android.os.Handler(android.os.Looper.getMainLooper())
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
        queries.addDocRow(cursor, queries.resolveLinkEntry(documentId, rootDir), rootDir, cols)
        context?.contentResolver?.let { resolver ->
            cursor.setNotificationUri(
                resolver,
                android.provider.DocumentsContract.buildDocumentUri(AUTHORITY, documentId),
            )
        }
        return cursor
    }

    override fun queryChildDocuments(
        parentDocumentId: String,
        projection: Array<out String>?,
        sortOrder: String?,
    ): Cursor {
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        requireInsideRoot(parent, rootDir)
        val children = parent.listFiles() ?: emptyArray()
        val sorted =
            children.sortedWith(
                compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() },
            )
        for (child in sorted) {
            queries.addDocRow(cursor, child, rootDir, cols)
        }
        context?.contentResolver?.let { resolver ->
            cursor.setNotificationUri(
                resolver,
                android.provider.DocumentsContract.buildChildDocumentsUri(AUTHORITY, parentDocumentId),
            )
        }
        return cursor
    }

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        val rootDir = queries.rootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
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
            mutations.notifyWritten(documentId, file)
        }
    }

    override fun openDocumentThumbnail(
        documentId: String,
        sizeHint: Point?,
        signal: CancellationSignal?,
    ): AssetFileDescriptor {
        // 仅图片行声明 FLAG_SUPPORTS_THUMBNAIL，缩略图即原文件只读句柄。
        val rootDir = queries.rootDir()
        val file = queries.resolveLinkEntry(documentId, rootDir)
        val parcelFileDescriptor = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        return AssetFileDescriptor(parcelFileDescriptor, 0, file.length())
    }

    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String =
        mutations.createDocument(parentDocumentId, mimeType, displayName)

    override fun renameDocument(documentId: String, displayName: String): String =
        mutations.renameDocument(documentId, displayName)

    override fun copyDocument(sourceDocumentId: String, targetParentDocumentId: String): String =
        mutations.copyDocument(sourceDocumentId, targetParentDocumentId)

    override fun moveDocument(
        sourceDocumentId: String,
        sourceParentDocumentId: String,
        targetParentDocumentId: String,
    ): String = mutations.moveDocument(sourceDocumentId, targetParentDocumentId)

    override fun deleteDocument(documentId: String) {
        mutations.deleteDocument(documentId)
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
                "DocumentsProvider",
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
