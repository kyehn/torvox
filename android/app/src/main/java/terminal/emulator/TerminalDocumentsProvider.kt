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
import android.util.Log
import java.io.File

class TerminalDocumentsProvider : DocumentsProvider() {
    companion object {
        const val AUTHORITY = "terminal.emulator.documents"
        const val ROOT_ID = "terminal_home"
        private const val TAG = "TerminalDocumentsProvider"

        // 搜索结果上限，与 Termux 的 MAX_SEARCH_RESULTS 对齐。
        private const val MAX_SEARCH_RESULTS = 50

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
            // For symlinks, encode the link's own path rather than its
            // canonical target: SAF clients then address the link entry
            // itself, so deleteDocument removes only the link — never the
            // target's whole tree. Containment is still checked against the
            // canonical path (a link pointing outside the home is skipped
            // below, as before).
            val filePath =
                if (java.nio.file.Files.isSymbolicLink(file.toPath())) {
                    file.path
                } else {
                    file.canonicalPath
                }
            val fileCanonical = file.canonicalPath
            // Symlinks pointing outside the home dir are common in a
            // terminal (e.g. ln -s /sdcard/x ~/link). Skip them rather
            // than throwing — require() would abort the whole SAF
            // directory listing on every browse.
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
            // Containment is checked on the link's OWN path, never its
            // canonical target: rawFile may contain ".." segments (a
            // hostile docId), and File does not normalize them. Resolve
            // the parent canonically and re-append the name so the check
            // covers the actual entry being touched. A null parent (a bare
            // name with no directory part) cannot escape the root.
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
            require(mode.isNotEmpty() && mode.length <= 4) { "Unsupported mode '$mode'" }
            require(mode.all { it in "rwatsd" }) { "Unsupported mode '$mode'" }
            val read = mode.contains('r')
            val write = mode.contains('w')
            require(read || write) { "Unsupported mode '$mode'" }
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

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val cols = projection ?: ROOT_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = queries.rootDir()
        cursor.newRow().apply {
            add(Root.COLUMN_ROOT_ID, ROOT_ID)
            add(Root.COLUMN_DOCUMENT_ID, encodeDocId(rootDir, rootDir))
            add(Root.COLUMN_TITLE, "Terminal Home")
            add(Root.COLUMN_SUMMARY, rootDir.absolutePath)
            add(
                Root.COLUMN_FLAGS,
                Root.FLAG_SUPPORTS_CREATE or Root.FLAG_SUPPORTS_SEARCH or Root.FLAG_SUPPORTS_IS_CHILD,
            )
            add(Root.COLUMN_ICON, R.mipmap.ic_launcher)
            add(Root.COLUMN_MIME_TYPES, "*/*")
            add(Root.COLUMN_AVAILABLE_BYTES, rootDir.freeSpace)
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
        queries.addDocRow(cursor, queries.resolveLinkEntry(documentId, rootDir), rootDir)
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
            queries.addDocRow(cursor, child, rootDir)
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
        return ParcelFileDescriptor.open(file, fileMode)
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
            Log.w(TAG, "isChildDocument: docId outside root", error)
            false
        } catch (error: IllegalArgumentException) {
            Log.w(TAG, "isChildDocument: malformed docId", error)
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
        while (pending.isNotEmpty() && cursor.count < MAX_SEARCH_RESULTS) {
            val current = pending.removeFirst()
            val canonical = queries.canonicalOrNull(current) ?: continue
            if (!(canonical.startsWith(rootPath + File.separator) || canonical == rootPath)) {
                continue
            }
            if (!java.nio.file.Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                if (canonical != rootPath && current.name.lowercase().contains(needle)) {
                    queries.addDocRow(cursor, current, rootDir)
                }
                current.listFiles()?.forEach { pending.addLast(it) }
            } else if (current.name.lowercase().contains(needle)) {
                queries.addDocRow(cursor, current, rootDir)
            }
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
