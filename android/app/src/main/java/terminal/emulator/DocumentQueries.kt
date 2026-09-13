package terminal.emulator

import android.content.Context
import android.database.MatrixCursor
import android.provider.DocumentsContract.Document
import android.util.Log
import android.webkit.MimeTypeMap
import java.io.File

/**
 * 查询侧辅助：根目录解析、链接条目归一、行组装、MIME 判定。
 *
 * 从 [TerminalDocumentsProvider] 拆分出来，使提供者主体只剩 DocumentsProvider override 入口，函数数回到 detekt
 * TooManyFunctions 阈值内。变更操作见 [DocumentMutations]。
 */
internal class DocumentQueries(private val context: Context) {
    fun rootDir(): File = java.io
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

    // 链接条目返回链接自身而非目标：与 queryChildDocuments 给出的行
    // 保持同一 docId，否则客户端按浏览结果回查会拿到另一个 id。
    // Termux 无此区分（绝对路径即 id）；此处在 Termux 行为之上补齐
    // 链接身份一致性。normalize 只做词法处理不跟随链接，配合根内
    // 校验挡住 ".." 逃逸。
    fun resolveLinkEntry(documentId: String, rootDir: File): File {
        val linkCandidate = File(rootDir, documentId)
        if (!java.nio.file.Files.isSymbolicLink(linkCandidate.toPath())) {
            val decoded = TerminalDocumentsProvider.decodeDocId(documentId, rootDir)
            TerminalDocumentsProvider.requireInsideRoot(decoded, rootDir)
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

    fun canonicalOrNull(file: File): String? = try {
        file.canonicalPath
    } catch (error: java.io.IOException) {
        Log.w("TerminalDocumentsProvider", "query skipping unreadable entry", error)
        null
    }

    fun addDocRow(
        cursor: MatrixCursor,
        file: File,
        rootDir: File,
    ) {
        val docId = TerminalDocumentsProvider.encodeDocId(file, rootDir) ?: return
        val mime = if (file.isDirectory) Document.MIME_TYPE_DIR else getMimeType(file.name)
        var flags = 0
        if (file.isDirectory) flags = flags or Document.FLAG_DIR_SUPPORTS_CREATE
        flags = flags or Document.FLAG_SUPPORTS_DELETE or Document.FLAG_SUPPORTS_WRITE
        // 重命名/复制/移动均已实现并单测覆盖：不声明客户端就不提供入口，
        // “无法重命名/复制”报障即源于此。
        flags =
            flags or
            Document.FLAG_SUPPORTS_RENAME or
            Document.FLAG_SUPPORTS_COPY or
            Document.FLAG_SUPPORTS_MOVE
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

    fun getMimeType(fileName: String): String {
        val ext = fileName.substringAfterLast('.', "").lowercase()
        if (ext.isEmpty()) return "application/octet-stream"
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }
}
