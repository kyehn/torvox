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
import android.webkit.MimeTypeMap
import java.io.File

class TerminalDocumentsProvider : DocumentsProvider() {
    companion object {
        const val AUTHORITY = "terminal.emulator.documents"
        private const val ROOT_ID = "terminal_home"

        // 搜索结果上限，与 Termux 的 MAX_SEARCH_RESULTS 对齐。
        private const val MAX_SEARCH_RESULTS = 50

        // 冲突重命名起始序号，与 Termux 的 "name (2)" 策略对齐。
        private const val CONFLICT_SUFFIX_START = 2

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

        fun encodeDocId(
            file: File,
            rootDir: File,
        ): String? {
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

        fun decodeDocId(
            docId: String,
            rootDir: File,
        ): File {
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

        private fun requireInsideRoot(
            file: File,
            rootDir: File,
        ) {
            val root = rootDir.canonicalFile
            val target = file.canonicalFile
            if (!(target.path.startsWith(root.path + File.separator) || target == root)) {
                throw java.io.FileNotFoundException(
                    "Access denied: ${target.path} is outside the terminal home directory",
                )
            }
        }
    }

    override fun onCreate(): Boolean = true

    private fun getRootDir(): File = java.io
        .File(
            requireNotNull(context) { "TerminalDocumentsProvider requires a Context" }.filesDir,
            "home",
        )
        .also { dir ->
            // mkdirs 在目录已存在时返回 false，只有目录仍不存在才告警。
            dir.mkdirs()
            if (!dir.isDirectory) {
                Log.w("DocumentsProvider", "Failed to create home directory: $dir")
            }
        }

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val cols = projection ?: ROOT_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = getRootDir()
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
        return cursor
    }

    override fun queryDocument(
        documentId: String,
        projection: Array<out String>?,
    ): Cursor {
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = getRootDir()
        addDocRow(cursor, resolveLinkEntry(documentId, rootDir), rootDir)
        return cursor
    }

    // 链接条目返回链接自身而非目标：与 queryChildDocuments 给出的行
    // 保持同一 docId，否则客户端按浏览结果回查会拿到另一个 id。
    // Termux 无此区分（绝对路径即 id）；此处在 Termux 行为之上补齐
    // 链接身份一致性。normalize 只做词法处理不跟随链接，配合根内
    // 校验挡住 ".." 逃逸。
    private fun resolveLinkEntry(documentId: String, rootDir: File): File {
        val linkCandidate = File(rootDir, documentId)
        if (!java.nio.file.Files.isSymbolicLink(linkCandidate.toPath())) {
            val decoded = decodeDocId(documentId, rootDir)
            requireInsideRoot(decoded, rootDir)
            return decoded
        }
        val rootPath = rootDir.canonicalPath
        val linkPath = linkCandidate.toPath().normalize().toString()
        if (!(linkPath.startsWith(rootPath + File.separator) || linkPath == rootPath)) {
            throw java.io.FileNotFoundException(
                "Access denied: $linkPath is outside the terminal home directory",
            )
        }
        return linkCandidate
    }

    override fun queryChildDocuments(
        parentDocumentId: String,
        projection: Array<out String>?,
        sortOrder: String?,
    ): Cursor {
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = getRootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        requireInsideRoot(parent, rootDir)
        val children = parent.listFiles() ?: emptyArray()
        val sorted =
            children.sortedWith(
                compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() },
            )
        for (child in sorted) {
            addDocRow(cursor, child, rootDir)
        }
        return cursor
    }

    override fun openDocument(
        documentId: String,
        mode: String,
        signal: CancellationSignal?,
    ): ParcelFileDescriptor {
        val rootDir = getRootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        // 显式映射而非委托 parseMode：实测本平台 parseMode("w") 不含
        // TRUNCATE，会破坏 SAF 的“w 截断”语义（单测已锁定）。
        // "rws"/"rwd" 按读写打开，不截断不追加。
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

                "rw" -> ParcelFileDescriptor.MODE_READ_WRITE

                "rwt" ->
                    ParcelFileDescriptor.MODE_READ_WRITE or
                        ParcelFileDescriptor.MODE_CREATE or
                        ParcelFileDescriptor.MODE_TRUNCATE

                "rws",
                "rwd",
                -> ParcelFileDescriptor.MODE_READ_WRITE or ParcelFileDescriptor.MODE_CREATE

                else -> {
                    // 非法模式是客户端契约违反，大声失败，不静默降级。
                    throw IllegalArgumentException("Unsupported mode '$mode'")
                }
            }
        return ParcelFileDescriptor.open(file, fileMode)
    }

    override fun openDocumentThumbnail(
        documentId: String,
        sizeHint: Point?,
        signal: CancellationSignal?,
    ): AssetFileDescriptor {
        // 仅图片行声明 FLAG_SUPPORTS_THUMBNAIL，缩略图即原文件只读句柄。
        val rootDir = getRootDir()
        val file = resolveLinkEntry(documentId, rootDir)
        val parcelFileDescriptor = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        return AssetFileDescriptor(parcelFileDescriptor, 0, file.length())
    }

    override fun createDocument(
        parentDocumentId: String,
        mimeType: String,
        displayName: String,
    ): String {
        val rootDir = getRootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        requireInsideRoot(parent, rootDir)
        val safeName = displayName.replace(Regex("[/\\\\]"), "_").replace("..", "_").trim()
        // Refuse degenerate names: an empty name or "." resolves File(parent,
        // name) back to the parent directory itself — "creating" it would
        // return the parent's docId as a new document, and a client calling
        // deleteDocument on that id would delete the whole directory tree.
        if (safeName.isEmpty() || safeName == ".") {
            throw IllegalArgumentException("Invalid document name: '$displayName'")
        }
        val isDir = mimeType == Document.MIME_TYPE_DIR
        // 与 Termux 一致的冲突策略：已存在则追加 " (2)" 后缀，而非报错。
        var child = File(parent, safeName)
        var conflictId = CONFLICT_SUFFIX_START
        while (child.exists()) {
            child = File(parent, "$safeName ($conflictId)")
            conflictId++
        }
        if (isDir) {
            if (!child.mkdirs() && !child.isDirectory) {
                throw java.io.IOException("Failed to create directory '${child.name}'")
            }
        } else {
            if (!child.createNewFile()) {
                throw java.io.IOException("Failed to create file '${child.name}'")
            }
        }
        // A failure to encode (canonical path IO error) must not silently
        // return ROOT_ID: the client would treat the new document as the
        // root and deleteDocument would later reject it. Fail loudly so
        // the client can surface the error.
        return encodeDocId(child, rootDir)
            ?: throw java.io.IOException("Failed to encode docId for '${child.name}'")
    }

    override fun renameDocument(
        documentId: String,
        displayName: String,
    ): String {
        val rootDir = getRootDir()
        if (documentId == ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to rename the root document")
        }
        val safeName = displayName.replace(Regex("[/\\\\]"), "_").replace("..", "_").trim()
        if (safeName.isEmpty() || safeName == ".") {
            throw IllegalArgumentException("Invalid document name: '$displayName'")
        }
        // 链接条目重命名链接自身而非目标：decode 会跟随链接到目标，
        // 直接重命名会误改目标文件名，目标内容不受影响但名称被改。
        // 与 deleteDocument 同策略：只动链接 inode，归属校验走链接自身路径。
        // 站外链接不在链接分支处理：decode 会跟随到站外目标并由
        // requireInsideRoot 拒绝，不会误改站外文件。
        val rawFile = File(rootDir, documentId)
        val source =
            if (isHomeLink(rawFile, rootDir)) {
                rawFile
            } else {
                val file = decodeDocId(documentId, rootDir)
                requireInsideRoot(file, rootDir)
                file
            }
        val parent =
            source.parentFile ?: throw java.io.FileNotFoundException("Invalid document id: $documentId")
        val target = File(parent, safeName)
        if (target.exists()) {
            throw java.io.IOException("Target '$safeName' already exists")
        }
        if (!source.renameTo(target)) {
            throw java.io.IOException("Failed to rename '$displayName'")
        }
        return encodeDocId(target, rootDir)
            ?: throw java.io.IOException("Failed to encode docId for '$safeName'")
    }

    override fun deleteDocument(documentId: String) {
        val rootDir = getRootDir()
        if (documentId == ROOT_ID) {
            // Root delete would wipe the entire terminal home in one call.
            // SAF clients are never entitled to that; refuse.
            throw java.io.FileNotFoundException("Refusing to delete the root document")
        }
        val rawFile = File(rootDir, documentId)
        if (isHomeLink(rawFile, rootDir)) {
            // The docId addresses a symlink entry itself (encodeDocId
            // encodes the link path, not its canonical target). Delete
            // only the link inode — deleting the canonical target would
            // wipe the linked directory tree the user did not ask to
            // remove.
            if (rawFile.delete()) {
                return
            }
            throw java.io.FileNotFoundException("Failed to delete symlink $documentId")
        }
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        if (file.canonicalFile == rootDir.canonicalFile) {
            // Defense in depth: a docId of "" or "." decodes back to the
            // root directory itself (File(root, "") / File(root, ".")),
            // which requireInsideRoot permits via the target == root
            // equality. Refuse so deleteDocument can never wipe the whole
            // terminal home.
            throw java.io.FileNotFoundException("Refusing to delete the root document")
        }
        deleteWithoutFollowingSymlinks(file)
    }

    /**
     * Iterative delete that never follows symlinks and never recurses into the JVM stack.
     *
     * A user's `ln -s . loop` in the terminal home makes deleteRecursively() recurse into the link's
     * target (the directory itself) forever → StackOverflowError, which bypasses catch(Exception) and
     * crashes the whole process (all sessions). A symlink is just an inode: delete it, not its
     * destination. A deeply nested directory tree (2000+ levels built with repeated `cd` + mkdir)
     * would likewise overflow the stack with recursion — walk it iteratively instead.
     */
    private fun deleteWithoutFollowingSymlinks(file: File) {
        // Two phases: walk the tree once collecting directories, deleting
        // files/symlinks on the way; then delete directories deepest-first
        // (reverse of the pre-order walk guarantees children come before
        // parents). The previous requeue-until-empty variant could spin
        // forever when a child delete() failed (read-only file, I/O error):
        // the directory would be requeued for every child that could not
        // be removed.
        val directories = ArrayList<File>()
        val stack = ArrayDeque<File>()
        stack.addLast(file)
        while (stack.isNotEmpty()) {
            val current = stack.removeLast()
            if (!java.nio.file.Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                directories.add(current)
                current.listFiles()?.forEach { stack.addLast(it) }
            } else {
                // A failed delete silently leaving a file behind confuses
                // SAF clients (the document appears to be gone but still
                // exists); surface it.
                if (!current.delete()) {
                    throw java.io.IOException("Failed to delete '${current.path}'")
                }
            }
        }
        for (i in directories.indices.reversed()) {
            // Directory deletes can fail if a child delete above failed;
            // since we already throw on the first child failure, a failure
            // here is an unexpected race — still report it.
            if (!directories[i].delete()) {
                throw java.io.IOException("Failed to delete directory '${directories[i].path}'")
            }
        }
    }

    override fun isChildDocument(
        parentDocumentId: String,
        documentId: String,
    ): Boolean {
        val rootDir = getRootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        val child = decodeDocId(documentId, rootDir)
        return child.canonicalPath.startsWith(parent.canonicalPath + File.separator)
    }

    override fun getDocumentType(documentId: String): String {
        val rootDir = getRootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        return if (file.isDirectory) Document.MIME_TYPE_DIR else getMimeType(file.name)
    }

    override fun querySearchDocuments(
        rootId: String,
        query: String,
        projection: Array<out String>?,
    ): Cursor {
        // 与 Termux 一致的按文件名搜索：迭代遍历、上限截断、符号链接
        // 不得跳出 home。查询词双向小写，修正 Termux 仅小写文件名的遗漏。
        val cols = projection ?: DOC_PROJECTION
        val cursor = MatrixCursor(cols)
        val rootDir = getRootDir()
        val rootPath = rootDir.canonicalPath
        val needle = query.lowercase()
        val pending = ArrayDeque<File>()
        pending.addLast(decodeDocId(rootId, rootDir))
        while (pending.isNotEmpty() && cursor.count < MAX_SEARCH_RESULTS) {
            val current = pending.removeFirst()
            val canonical = canonicalOrNull(current) ?: continue
            if (!(canonical.startsWith(rootPath + File.separator) || canonical == rootPath)) {
                continue
            }
            if (!java.nio.file.Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                current.listFiles()?.forEach { pending.addLast(it) }
            } else if (current.name.lowercase().contains(needle)) {
                addDocRow(cursor, current, rootDir)
            }
        }
        return cursor
    }

    private fun canonicalOrNull(file: File): String? = try {
        file.canonicalPath
    } catch (error: java.io.IOException) {
        Log.w("TerminalDocumentsProvider", "query skipping unreadable entry", error)
        null
    }

    private fun addDocRow(
        cursor: MatrixCursor,
        file: File,
        rootDir: File,
    ) {
        val docId = encodeDocId(file, rootDir) ?: return
        val mime = if (file.isDirectory) Document.MIME_TYPE_DIR else getMimeType(file.name)
        var flags = 0
        if (file.isDirectory) flags = flags or Document.FLAG_DIR_SUPPORTS_CREATE
        flags = flags or Document.FLAG_SUPPORTS_DELETE or Document.FLAG_SUPPORTS_WRITE
        // 与 Termux 一致：图片声明缩略图支持，对应 openDocumentThumbnail。
        if (mime.startsWith("image/")) flags = flags or Document.FLAG_SUPPORTS_THUMBNAIL
        cursor.newRow().apply {
            add(Document.COLUMN_DOCUMENT_ID, docId)
            add(Document.COLUMN_DISPLAY_NAME, file.name)
            add(Document.COLUMN_MIME_TYPE, mime)
            add(Document.COLUMN_SIZE, file.length())
            add(Document.COLUMN_LAST_MODIFIED, file.lastModified())
            add(Document.COLUMN_FLAGS, flags)
        }
    }

    private fun getMimeType(fileName: String): String {
        val ext = fileName.substringAfterLast('.', "").lowercase()
        if (ext.isEmpty()) return "application/octet-stream"
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }
}
