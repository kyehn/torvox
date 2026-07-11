package io.torvox

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

class TorvoxDocumentsProvider : DocumentsProvider() {
    companion object {
        const val AUTHORITY = "io.torvox.documents"
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
        ): String {
            val rootPath = rootDir.canonicalPath
            val filePath = file.canonicalPath
            require(filePath.startsWith(rootPath + File.separator) || filePath == rootPath) {
                "encodeDocId: $filePath is outside root $rootPath"
            }
            return if (filePath == rootPath) {
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
            require(target.path.startsWith(root.path + File.separator) || target == root) {
                "Access denied: ${target.path} is outside the terminal home directory"
            }
        }
    }

    override fun onCreate(): Boolean = true

    private fun getRootDir(): File = java.io.File(requireNotNull(context).filesDir, "home").also { dir ->
        if (!dir.mkdirs()) {
            Log.w("TorvoxDocumentsProvider", "Failed to create home directory: $dir")
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
        val isWrite = mode.contains("w")
        val fileMode =
            if (isWrite) {
                ParcelFileDescriptor.MODE_READ_WRITE or ParcelFileDescriptor.MODE_CREATE
            } else {
                ParcelFileDescriptor.MODE_READ_ONLY
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
        val safeName = displayName.replace(Regex("[/\\\\]"), "_").replace("..", "_")
        val isDir = mimeType == Document.MIME_TYPE_DIR
        val child = File(parent, safeName)
        if (isDir) {
            child.mkdirs()
        } else {
            child.createNewFile()
        }
        return encodeDocId(child, rootDir)
    }

    override fun deleteDocument(documentId: String) {
        val rootDir = getRootDir()
        val file = decodeDocId(documentId, rootDir)
        requireInsideRoot(file, rootDir)
        if (file.isDirectory) {
            file.deleteRecursively()
        } else {
            file.delete()
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
        val docId = encodeDocId(file, rootDir)
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
