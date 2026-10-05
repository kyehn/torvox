package terminal.emulator

import android.content.Context
import android.database.MatrixCursor
import android.provider.DocumentsContract.Document
import android.webkit.MimeTypeMap
import terminal.emulator.runtime.LogUtil
import java.io.File

/**
 * 查询侧辅助：根目录解析、链接条目归一、行组装、MIME 判定。
 *
 * 从 [TerminalDocumentsProvider] 拆分出来，使提供者主体只剩 DocumentsProvider override 入口，函数数回到 detekt
 * TooManyFunctions 阈值内。变更操作见 [DocumentMutations]。
 */
internal class DocumentQueries(private val context: Context) {
    /**
     * 暴露给系统文件选择器的家目录，返回**规范形式**：`encodeDocId` 以
     * `rootDir.canonicalPath` 剥前缀，若这里交出词法路径形式（`/data/user/0/…` 之类
     * 经符号链接祖先到达的 `filesDir`），前缀就剥不掉，链接条目的 docId 退化成整条
     * 绝对路径，客户端回查时被重新拼到根目录下而永远打不开。
     */
    fun rootDir(): File = java.io
        .File(
            requireNotNull(context) { "TerminalDocumentsProvider requires a Context" }.filesDir,
            "home",
        )
        .also { dir ->
            // mkdirs 在目录已存在时返回 false，只有目录仍不存在才告警。
            dir.mkdirs()
            if (!dir.isDirectory) {
                LogUtil.w("DocumentsProvider", "Failed to create home directory: $dir")
            }
        }
        .canonicalFile

    // 链接条目返回链接自身而非目标：与 queryChildDocuments 给出的行
    // 保持同一 docId，否则客户端按浏览结果回查会拿到另一个 id。
    // Termux 无此区分（绝对路径即 id）；此处在 Termux 行为之上补齐
    // 链接身份一致性。
    fun resolveLinkEntry(documentId: String, rootDir: File): File {
        val linkCandidate = File(rootDir, documentId)
        // 包含性复用 isHomeLink（规范解析父目录 + 原名，既不跟随链接也真正消解 ".."）：
        // 此处曾用词法 Path.normalize 自行判断，那既不消解 ".."，也与 isHomeLink
        // 重复，同一条规则的两套实现迟早分叉。
        // 非链接：decodeDocId 内部已做根内校验并返回规范化的 File，
        // 此处再判一次只是对同一个已规范化路径多两次文件系统往返。
        if (!TerminalDocumentsProvider.isHomeLink(linkCandidate, rootDir)) {
            return TerminalDocumentsProvider.decodeDocId(documentId, rootDir)
        }
        return linkCandidate
    }

    fun canonicalOrNull(file: File): String? = try {
        file.canonicalPath
    } catch (error: java.io.IOException) {
        LogUtil.w("TerminalDocumentsProvider", "query skipping unreadable entry", error)
        null
    }

    fun addDocRow(cursor: MatrixCursor, file: File, rootDir: File, columns: Array<out String>) {
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
            if (Document.COLUMN_DOCUMENT_ID in columns) add(Document.COLUMN_DOCUMENT_ID, docId)
            if (Document.COLUMN_DISPLAY_NAME in columns) add(Document.COLUMN_DISPLAY_NAME, file.name)
            if (Document.COLUMN_MIME_TYPE in columns) add(Document.COLUMN_MIME_TYPE, mime)
            if (Document.COLUMN_SIZE in columns) add(Document.COLUMN_SIZE, file.length())
            if (Document.COLUMN_LAST_MODIFIED in columns) add(Document.COLUMN_LAST_MODIFIED, file.lastModified())
            if (Document.COLUMN_FLAGS in columns) add(Document.COLUMN_FLAGS, flags)
        }
    }

    fun getMimeType(fileName: String): String {
        val ext = fileName.substringAfterLast('.', "").lowercase(java.util.Locale.ROOT)
        if (ext.isEmpty()) return "application/octet-stream"
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }
}
