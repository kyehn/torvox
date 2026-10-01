package terminal.emulator

import android.content.Context
import android.provider.DocumentsContract
import terminal.emulator.util.runCatchingCancellable
import java.io.File
import java.io.IOException
import java.nio.file.LinkOption

/**
 * 文件变更操作：创建、重命名、删除、复制、移动。
 *
 * 从 [TerminalDocumentsProvider] 拆分出来，使提供者主体只剩查询与打开通道， 函数数回到 detekt TooManyFunctions
 * 阈值内。所有变更落盘后广播通知， 外部文件客户端才能即时刷新（否则复制进入、重命名看起来“没反应”）。
 */
internal class DocumentMutations(private val context: Context, private val rootDir: () -> File) {
    private companion object {
        /** 客户端未给名字时的新建占位名，扩展名按 mimeType 追加。 */
        const val DEFAULT_FILE_NAME = "New Document"
        const val DEFAULT_DIRECTORY_NAME = "New Folder"
    }

    fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String {
        val root = rootDir()
        val parent = TerminalDocumentsProvider.decodeDocId(parentDocumentId, root)
        requireDirectory(parent)
        val isDir = mimeType == android.provider.DocumentsContract.Document.MIME_TYPE_DIR
        val safeName = createName(displayName, mimeType)
        val child = uniqueChild(parent, safeName)
        if (isDir) {
            if (!child.mkdirs() && !child.isDirectory) {
                throw IOException("Failed to create directory '${child.name}'")
            }
        } else {
            if (!child.createNewFile()) {
                throw IOException("Failed to create file '${child.name}'")
            }
        }
        val docId =
            TerminalDocumentsProvider.encodeDocId(child, root)
                ?: throw IOException("Failed to encode docId for '${child.name}'")
        notifyChildren(parentDocumentId)
        notifyDocument(docId)
        return docId
    }

    fun renameDocument(documentId: String, displayName: String): String {
        val root = rootDir()
        val rootCanonical = root.canonicalFile
        if (documentId == TerminalDocumentsProvider.ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to rename the root document")
        }
        val safeName = sanitize(displayName)
        // 链接条目重命名链接自身而非目标：decode 会跟随链接到目标，
        // 直接重命名会误改目标文件名。只动链接 inode，归属校验走链接自身路径。
        val rawFile = File(root, documentId)
        val source =
            if (TerminalDocumentsProvider.isHomeLink(rawFile, root)) {
                rawFile
            } else {
                TerminalDocumentsProvider.decodeDocId(documentId, root)
            }
        val parent =
            source.parentFile ?: throw java.io.FileNotFoundException("Invalid document id: $documentId")
        if (source.canonicalFile == rootCanonical) {
            throw java.io.FileNotFoundException("Refusing to rename the root document")
        }
        val target = File(parent, safeName)
        if (target.exists()) {
            throw IOException("Target '$safeName' already exists")
        }
        if (!source.renameTo(target)) {
            throw IOException("Failed to rename '$displayName'")
        }
        val parentId =
            TerminalDocumentsProvider.encodeDocId(parent, root) ?: TerminalDocumentsProvider.ROOT_ID
        notifyChildren(parentId)
        val newId = TerminalDocumentsProvider.encodeDocId(target, root)
            ?: throw IOException("Failed to encode docId for '$safeName'")
        notifyDocument(newId)
        notifyDocument(documentId)
        return newId
    }

    fun deleteDocument(documentId: String) {
        val root = rootDir()
        if (documentId == TerminalDocumentsProvider.ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to delete the root document")
        }
        val rawFile = File(root, documentId)
        if (TerminalDocumentsProvider.isHomeLink(rawFile, root)) {
            // docId 指向链接自身：只删链接 inode，不断其目标整棵树。
            if (rawFile.delete()) {
                notifyParentOf(rawFile, root)
                return
            }
            throw java.io.FileNotFoundException("Failed to delete symlink $documentId")
        }
        val file = TerminalDocumentsProvider.decodeDocId(documentId, root)
        if (file.canonicalFile == root.canonicalFile) {
            // "" 或 "." 解码回根目录自身：拒绝，否则一次调用清空整个家目录。
            throw java.io.FileNotFoundException("Refusing to delete the root document")
        }
        deleteWithoutFollowingSymlinks(file)
        notifyParentOf(file, root)
        notifyDocument(documentId)
    }

    fun copyDocument(sourceDocumentId: String, targetParentDocumentId: String): String {
        val root = rootDir()
        val rootCanonical = root.canonicalFile
        if (sourceDocumentId == TerminalDocumentsProvider.ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to copy the root document")
        }
        val source = TerminalDocumentsProvider.decodeDocId(sourceDocumentId, root)
        if (source.canonicalFile == rootCanonical) {
            throw java.io.FileNotFoundException("Refusing to copy the root document")
        }
        val targetParent = TerminalDocumentsProvider.decodeDocId(targetParentDocumentId, root)
        requireDirectory(targetParent)
        // 读源用跟随语义（复制链接目标内容）还是链接自身？与浏览一致：
        // 行地址是链接自身时复制链接 inode，避免把站外目标整棵树吸入家目录。
        val rawSource = File(root, sourceDocumentId)
        val effectiveSource =
            if (TerminalDocumentsProvider.isHomeLink(rawSource, root)) rawSource else source
        val target = copyTree(effectiveSource, targetParent)
        notifyChildren(targetParentDocumentId)
        val newId = TerminalDocumentsProvider.encodeDocId(target, root)
            ?: throw IOException("Failed to encode docId for '${target.name}'")
        notifyDocument(newId)
        return newId
    }

    fun moveDocument(sourceDocumentId: String, targetParentDocumentId: String): String {
        val root = rootDir()
        if (sourceDocumentId == TerminalDocumentsProvider.ROOT_ID) {
            throw java.io.FileNotFoundException("Refusing to move the root document")
        }
        val rawSource = File(root, sourceDocumentId)
        val source =
            if (TerminalDocumentsProvider.isHomeLink(rawSource, root)) {
                rawSource
            } else {
                TerminalDocumentsProvider.decodeDocId(sourceDocumentId, root)
            }
        if (source.canonicalFile == root.canonicalFile) {
            throw java.io.FileNotFoundException("Refusing to move the root document")
        }
        val targetParent = TerminalDocumentsProvider.decodeDocId(targetParentDocumentId, root)
        requireDirectory(targetParent)
        // 禁止把目录搬进自己的子孙：否则复制/删除在环上打转。
        val sourceCanonical = source.canonicalFile.path
        val targetParentCanonical = targetParent.canonicalFile.path
        if (
            targetParentCanonical == sourceCanonical ||
            targetParentCanonical.startsWith(sourceCanonical + File.separator)
        ) {
            throw IOException("Refusing to move a directory into its own descendant")
        }
        val sourceParentId =
            TerminalDocumentsProvider.encodeDocId(
                source.parentFile ?: root,
                root,
            ) ?: TerminalDocumentsProvider.ROOT_ID
        val target = uniqueChild(targetParent, source.name)
        if (!source.renameTo(target)) {
            // 同文件系统 rename 极少失败：目标已占位已在上一步排除，
            // 剩下的是 IO 错误；退化为复制加删除，保证语义完整。
            val copied = copyTree(source, targetParent, source.name)
            deleteWithoutFollowingSymlinks(source)
            notifyChildren(sourceParentId)
            notifyChildren(targetParentDocumentId)
            val copiedId = TerminalDocumentsProvider.encodeDocId(copied, root)
                ?: throw IOException("Failed to encode docId for '${copied.name}'")
            notifyDocument(copiedId)
            notifyDocument(sourceDocumentId)
            return copiedId
        }
        notifyChildren(sourceParentId)
        notifyChildren(targetParentDocumentId)
        val newId = TerminalDocumentsProvider.encodeDocId(target, root)
            ?: throw IOException("Failed to encode docId for '${target.name}'")
        notifyDocument(newId)
        notifyDocument(sourceDocumentId)
        return newId
    }

    /**
     * 不跟随链接、不递归栈的删除：`ln -s . loop` 会让 deleteRecursively 在链接目标里打转至 StackOverflow（Error 绕过 catch
     * 让整进程崩溃）； 2000+ 层深目录同样爆栈。链接只删 inode 本身。
     */
    fun deleteWithoutFollowingSymlinks(file: File) {
        // 先序走一遍收目录，文件与链接沿途直删；再按逆序删目录，
        // 保证子先于父。上一步任一子删失败即抛，不重排空转。
        val directories = ArrayList<File>()
        val stack = ArrayDeque<File>()
        stack.addLast(file)
        while (stack.isNotEmpty()) {
            val current = stack.removeLast()
            if (!java.nio.file.Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                directories.add(current)
                current.listFiles()?.forEach { stack.addLast(it) }
            } else {
                if (!current.delete()) {
                    throw IOException("Failed to delete '${current.path}'")
                }
            }
        }
        for (directoryIndex in directories.indices.reversed()) {
            if (!directories[directoryIndex].delete()) {
                throw IOException("Failed to delete directory '${directories[directoryIndex].path}'")
            }
        }
    }

    private fun copyTree(source: File, targetParent: File, baseName: String = source.name): File {
        val target = uniqueChild(targetParent, baseName)
        try {
            copyTreeInto(source, target)
        } catch (failure: IOException) {
            // 半截复制产物不能留给客户端：尽力清掉再把原错抛出去。
            runCatchingCancellable { deleteWithoutFollowingSymlinks(target) }
            throw failure
        }
        return target
    }

    private fun copyTreeInto(source: File, target: File) {
        if (java.nio.file.Files.isSymbolicLink(source.toPath())) {
            // 只复制链接 inode：跟随会把站外目标整棵树吸入家目录。
            java.nio.file.Files.copy(
                source.toPath(),
                target.toPath(),
                LinkOption.NOFOLLOW_LINKS,
            )
            return
        }
        if (!source.isDirectory) {
            source.copyTo(target)
            return
        }
        if (!target.mkdirs() && !target.isDirectory) {
            throw IOException("Failed to create directory '${target.path}'")
        }
        val stack = ArrayDeque<Pair<File, File>>()
        source.listFiles()?.forEach { stack.addLast(it to target) }
        while (stack.isNotEmpty()) {
            val (src, dstParent) = stack.removeLast()
            if (java.nio.file.Files.isSymbolicLink(src.toPath())) {
                java.nio.file.Files.copy(
                    src.toPath(),
                    File(dstParent, src.name).toPath(),
                    LinkOption.NOFOLLOW_LINKS,
                )
            } else if (src.isDirectory) {
                val dst = File(dstParent, src.name)
                if (!dst.mkdirs() && !dst.isDirectory) {
                    throw IOException("Failed to create directory '${dst.path}'")
                }
                src.listFiles()?.forEach { stack.addLast(it to dst) }
            } else {
                src.copyTo(File(dstParent, src.name))
            }
        }
    }

    private fun uniqueChild(parent: File, baseName: String): File {
        // 与 Termux 一致的冲突策略：已存在则追加 " (2)" 后缀，而非报错。
        var child = File(parent, baseName)
        var conflictId = 2
        while (child.exists() || java.nio.file.Files.isSymbolicLink(child.toPath())) {
            child = File(parent, "$baseName ($conflictId)")
            conflictId++
        }
        return child
    }

    /**
     * 只校验、不改写：静默把 `a..b.txt` 改成 `a_b.txt` 会让客户端拿到的
     * docId 与自己请求的名字对不上，文件选择器里表现为"存成了别的名字"。
     * 非法名是客户端契约违反，大声失败。`..` 出现在名字内部是合法文件名
     * （File(parent, name) 不会因此跳出父目录），只有整体等于 "." / ".."
     * 才危险；路径分隔符则任何文件系统都不允许出现在文件名里。
     */
    private fun sanitize(displayName: String): String {
        val safeName = displayName.trim()
        if (safeName.isEmpty() || safeName == "." || safeName == "..") {
            throw java.io.FileNotFoundException("Invalid document name: '$displayName'")
        }
        if (safeName.contains('/') || safeName.contains('\\')) {
            throw java.io.FileNotFoundException(
                "Document name must not contain a path separator: '$displayName'",
            )
        }
        return safeName
    }

    /**
     * 新建时对空名的处理：空名不是「非法名」而是「未命名」——DocumentsUI 在名字
     * 为空时依然放行 SAVE，系统自带 DownloadsProvider 会建出占位文件，而本提供者
     * 若照旧抛异常，异常只进日志、界面上毫无反馈，用户看到的就是「无法创建新文件」。
     * 故仅新建路径取按 mimeType 派生的默认名；非空名与重命名仍走严格校验。
     *
     * 扩展名取自 mimeType（text/plain → .txt），取不到时用无扩展名的占位名。
     */
    private fun createName(displayName: String, mimeType: String): String {
        if (displayName.isNotBlank()) return sanitize(displayName)
        if (mimeType == android.provider.DocumentsContract.Document.MIME_TYPE_DIR) {
            return DEFAULT_DIRECTORY_NAME
        }
        val extension = android.webkit.MimeTypeMap.getSingleton().getExtensionFromMimeType(mimeType)
        return if (extension.isNullOrEmpty()) DEFAULT_FILE_NAME else "$DEFAULT_FILE_NAME.$extension"
    }

    private fun requireDirectory(candidate: File) {
        // 抛 FileNotFoundException 而非 require() 的 IllegalArgumentException：
        // 后者跨 Binder 会让调用方（DocumentsUI / 编辑器）崩溃而非得到可处理的失败。
        if (!candidate.isDirectory) {
            throw java.io.FileNotFoundException("Not a directory: ${candidate.path}")
        }
    }

    /**
     * 外部应用经 SAF 句柄写回后广播：文档本身与其父目录的子文档列表。
     *
     * 提供者自身的变更操作各自已发通知；唯独经 [android.os.ParcelFileDescriptor]
     * 写回的外部修改不经过这里，缺了它文件选择器只在下一次手动刷新才看到变化。
     */
    fun notifyWritten(documentId: String, written: File) {
        notifyDocument(documentId)
        notifyParentOf(written, rootDir())
    }

    private fun notifyChildren(parentDocumentId: String) {
        context.contentResolver.notifyChange(
            DocumentsContract.buildChildDocumentsUri(
                TerminalDocumentsProvider.AUTHORITY,
                parentDocumentId,
            ),
            null,
        )
    }

    private fun notifyDocument(documentId: String) {
        context.contentResolver.notifyChange(
            DocumentsContract.buildDocumentUri(
                TerminalDocumentsProvider.AUTHORITY,
                documentId,
            ),
            null,
        )
    }

    private fun notifyParentOf(file: File, root: File) {
        val parent = file.parentFile ?: return
        val parentId = TerminalDocumentsProvider.encodeDocId(parent, root) ?: return
        notifyChildren(parentId)
    }
}
