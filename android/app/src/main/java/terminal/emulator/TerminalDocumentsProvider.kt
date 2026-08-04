package terminal.emulator

import android.database.Cursor
import android.database.MatrixCursor
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

        private val ROOT_PROJECTION =
            arrayOf(
                Root.COLUMN_ROOT_ID,
                Root.COLUMN_DOCUMENT_ID,
                Root.COLUMN_TITLE,
                Root.COLUMN_SUMMARY,
                Root.COLUMN_FLAGS,
                Root.COLUMN_ICON,
                Root.COLUMN_MIME_TYPES,
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

    private fun getRootDir(): File = java.io.File(requireNotNull(context).filesDir, "home").also { dir ->
        if (!dir.mkdirs()) {
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
            add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE or Root.FLAG_SUPPORTS_IS_CHILD)
            add(Root.COLUMN_ICON, R.mipmap.ic_launcher)
            add(Root.COLUMN_MIME_TYPES, "*/*")
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
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        addDocRow(cursor, file, rootDir)
        return cursor
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
        val sorted = children.sortedWith(compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() })
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
        // Exact match of ParcelFileDescriptor.parseMode semantics: only
        // "w"/"wt"/"rwt" truncate, "wa" appends, "rw" is a plain
        // read-modify-write. Any "contains" heuristic misclassifies at
        // least one of these (round-61 truncated "rw"/"wa"; round-62
        // stopped truncating plain "w", corrupting save-with-shorter-
        // content).
        val fileMode =
            when (mode) {
                "r" -> ParcelFileDescriptor.MODE_READ_ONLY

                "w", "wt" ->
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

                else -> {
                    // An unknown mode string is a client contract violation
                    // (ParcelFileDescriptor.parseMode semantics): fail loudly
                    // instead of silently handing out a read-only fd to a
                    // client that asked for write access.
                    throw IllegalArgumentException("Unsupported mode '$mode'")
                }
            }
        return ParcelFileDescriptor.open(file, fileMode)
    }

    override fun createDocument(
        parentDocumentId: String,
        mimeType: String,
        displayName: String,
    ): String {
        val rootDir = getRootDir()
        val parent = decodeDocId(parentDocumentId, rootDir)
        requireInsideRoot(parent, rootDir)
        val safeName =
            displayName
                .replace(Regex("[/\\\\]"), "_")
                .replace("..", "_")
                .trim()
        // Refuse degenerate names: an empty name or "." resolves File(parent,
        // name) back to the parent directory itself — "creating" it would
        // return the parent's docId as a new document, and a client calling
        // deleteDocument on that id would delete the whole directory tree.
        if (safeName.isEmpty() || safeName == ".") {
            throw IllegalArgumentException("Invalid document name: '$displayName'")
        }
        val isDir = mimeType == Document.MIME_TYPE_DIR
        val child = File(parent, safeName)
        if (isDir) {
            if (!child.mkdirs() && !child.isDirectory) {
                throw java.io.IOException("Failed to create directory '$safeName'")
            }
        } else {
            if (!child.createNewFile()) {
                throw java.io.IOException("Failed to create file '$safeName'")
            }
        }
        // A failure to encode (canonical path IO error) must not silently
        // return ROOT_ID: the client would treat the new document as the
        // root and deleteDocument would later reject it. Fail loudly so
        // the client can surface the error.
        return encodeDocId(child, rootDir)
            ?: throw java.io.IOException("Failed to encode docId for '$safeName'")
    }

    override fun renameDocument(
        documentId: String,
        displayName: String,
    ): String {
        val rootDir = getRootDir()
        if (documentId == ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to rename the root document")
        }
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        val safeName =
            displayName
                .replace(Regex("[/\\\\]"), "_")
                .replace("..", "_")
                .trim()
        if (safeName.isEmpty() || safeName == ".") {
            throw IllegalArgumentException("Invalid document name: '$displayName'")
        }
        val parent = file.parentFile ?: throw java.io.FileNotFoundException("Invalid document id: $documentId")
        val target = File(parent, safeName)
        if (target.exists()) {
            throw java.io.IOException("Target '$safeName' already exists")
        }
        if (!file.renameTo(target)) {
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
        if (java.nio.file.Files.isSymbolicLink(rawFile.toPath())) {
            // The docId addresses a symlink entry itself (encodeDocId
            // encodes the link path, not its canonical target). Delete
            // only the link inode — deleting the canonical target would
            // wipe the linked directory tree the user did not ask to
            // remove.
            // Containment is checked on the link's OWN path, never its
            // canonical target: rawFile may contain ".." segments (a
            // hostile docId), and File does not normalize them. Resolve
            // the parent canonically and re-append the name so the check
            // covers the actual entry being deleted.
            val parentCanonical = rawFile.parentFile?.canonicalFile
            if (parentCanonical == null) {
                throw java.io.FileNotFoundException("Invalid document id: $documentId")
            }
            val linkPath = File(parentCanonical, rawFile.name).canonicalPath
            val rootPath = rootDir.canonicalPath
            if (!(linkPath.startsWith(rootPath + File.separator) || linkPath == rootPath)) {
                throw java.io.FileNotFoundException(
                    "Access denied: $linkPath is outside the terminal home directory",
                )
            }
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
     * Iterative delete that never follows symlinks and never recurses into
     * the JVM stack.
     *
     * A user's `ln -s . loop` in the terminal home makes deleteRecursively()
     * recurse into the link's target (the directory itself) forever →
     * StackOverflowError, which bypasses catch(Exception) and crashes the
     * whole process (all sessions). A symlink is just an inode: delete it,
     * not its destination. A deeply nested directory tree (2000+ levels built
     * with repeated `cd` + mkdir) would likewise overflow the stack with
     * recursion — walk it iteratively instead.
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
        return if (file.isDirectory) Document.MIME_TYPE_DIR else getMimeType(file.name)
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
