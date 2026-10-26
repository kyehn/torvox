package terminal.emulator

import android.content.Context
import android.provider.DocumentsContract
import terminal.emulator.util.runCatchingCancellable
import java.io.File
import java.io.IOException
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.attribute.PosixFilePermission

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

        /** 待继承权限的条目上限：替换是一次「改名让位 → 建同名新文件 → 删让位文件」的连续动作，几条足够。 */
        const val REPLACED_PERMISSION_LIMIT = 8
    }

    /**
     * 被改名让位的条目权限：`(父目录规范路径, 原名) -> 权限位`。
     *
     * SAF 没有 chmod 通道。编辑器保存（系统文件管理器、Material Files 等）的「替换」流程是：
     * 把原文件改名让位 → 用 `createDocument` 建同名新文件 → 删除让位文件。新 inode 只带
     * 进程 umask 的默认权限，执行位必丢——`openDocument` 截断保留 inode 只覆盖「就地覆写」
     * 那一条路，对这条流程无效（真机 logcat：`renameDocument c -> <id>.rollback`、
     * `createDocument c`、`deleteDocument <id>.rollback`）。故建同名文件时把让位条目的
     * 权限位继承回来。上限按最近使用淘汰，防止长期进程里无限增长。
     */
    private val replacedPermissions =
        object : LinkedHashMap<String, Set<PosixFilePermission>>(16, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Set<PosixFilePermission>>): Boolean =
                size > REPLACED_PERMISSION_LIMIT
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
        applyReplacedPermissions(parent, child)
        val docId =
            TerminalDocumentsProvider.encodeDocId(child, root)
                ?: throw IOException("Failed to encode docId for '${child.name}'")
        notifyChildren(parent, root)
        notifyDocument(child, root)
        return docId
    }

    fun renameDocument(documentId: String, displayName: String): String? {
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
        if (safeName == source.name) {
            // 名字没变：框架约定「原 docId 仍有效时返回 null」。少了这一守卫，
            // uniqueChild 会把源文件本身当冲突，把 `x` 静默改成 `x (2)`。
            return null
        }
        // 重名冲突与 create/copy/move 同一策略（`uniqueChild`）：框架合约明确允许
        // 提供者为避开重名而改写名字。硬失败只会让「先新建、后改名」的客户端
        // （文件管理器的建文件/建文件夹、另存为）拿到跨 Binder 的运行时异常，
        // 界面报「路径已存在」而操作全丢。
        val target = uniqueChild(parent, safeName)
        // 权限位必须在改名之前读：改名后原路径已不存在，读不到源 inode。
        val replacedName = source.name
        val replacedMode = posixPermissionsOrNull(source)
        if (!source.renameTo(target)) {
            throw IOException("Failed to rename '$displayName'")
        }
        if (replacedMode != null) {
            rememberReplacedPermissions(parent, replacedName, replacedMode)
        }
        val newId =
            TerminalDocumentsProvider.encodeDocId(target, root)
                ?: throw IOException("Failed to encode docId for '$safeName'")
        notifyChildren(parent, root)
        notifyDocument(target, root)
        notifyDocument(source, root)
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
            deleteOrFail(rawFile, "Failed to delete symlink")
            notifyParentOf(rawFile, root)
            return
        }
        val file = TerminalDocumentsProvider.decodeDocId(documentId, root)
        if (file.canonicalFile == root.canonicalFile) {
            // "" 或 "." 解码回根目录自身：拒绝，否则一次调用清空整个家目录。
            throw java.io.FileNotFoundException("Refusing to delete the root document")
        }
        deleteWithoutFollowingSymlinks(file)
        notifyParentOf(file, root)
        notifyDocument(file, root)
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
        if (!Files.exists(source.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            throw java.io.FileNotFoundException("Source document does not exist: '$sourceDocumentId'")
        }
        val targetParent = TerminalDocumentsProvider.decodeDocId(targetParentDocumentId, root)
        requireDirectory(targetParent)
        // 读源用跟随语义（复制链接目标内容）还是链接自身？与浏览一致：
        // 行地址是链接自身时复制链接 inode，避免把站外目标整棵树吸入家目录。
        val rawSource = File(root, sourceDocumentId)
        val effectiveSource =
            if (TerminalDocumentsProvider.isHomeLink(rawSource, root)) rawSource else source
        // 与 moveDocument 同一守卫：复制目录进自己的子孙时 copyTreeInto 会把刚
        // 复制的子树再当作源继续遍历，产出 a/b/a/b/… 直至 ENAMETOOLONG 或磁盘写满。
        // 守卫只认真目录：copyTreeInto 遇到符号链接只复制 inode 自身、不展开目标树，
        // 故「链接的解析目标在源之下」并无风险；而 canonicalFile 正是把链接解析到
        // 目标，据此拒绝会误伤「把链接拷进其目标子目录」这一正常操作。
        if (effectiveSource.isDirectory && !Files.isSymbolicLink(effectiveSource.toPath())) {
            val sourceCanonical = effectiveSource.canonicalFile.path
            val targetParentCanonical = targetParent.canonicalFile.path
            if (
                targetParentCanonical == sourceCanonical ||
                targetParentCanonical.startsWith(sourceCanonical + File.separator)
            ) {
                throw IOException("Refusing to copy a directory into its own descendant")
            }
        }
        val target = copyTree(effectiveSource, targetParent)
        notifyChildren(targetParent, root)
        val newId =
            TerminalDocumentsProvider.encodeDocId(target, root)
                ?: throw IOException("Failed to encode docId for '${target.name}'")
        notifyDocument(target, root)
        return newId
    }

    fun moveDocument(
        sourceDocumentId: String,
        targetParentDocumentId: String,
        sourceParentDocumentId: String? = null,
    ): String {
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
        // AOSP 契约：sourceParentDocumentId 是客户端声称的源父目录。忽略它，
        // 跨父移动时客户端手里的过期列表不会暴露，源会从真实父目录悄悄搬走。
        // 空串表示未提供（部分客户端如此），此时无从校验。
        if (!sourceParentDocumentId.isNullOrEmpty()) {
            val claimedParent = TerminalDocumentsProvider.decodeDocId(sourceParentDocumentId, root)
            val actualParent = source.parentFile ?: root
            if (claimedParent.canonicalFile != actualParent.canonicalFile) {
                throw java.io.FileNotFoundException(
                    "sourceParentDocumentId '$sourceParentDocumentId' is not the parent of '$sourceDocumentId'",
                )
            }
        }
        val targetParent = TerminalDocumentsProvider.decodeDocId(targetParentDocumentId, root)
        requireDirectory(targetParent)
        // 禁止把目录搬进自己的子孙：否则复制/删除在环上打转。
        // 与 copyDocument 同一守卫，且同样只认真目录：move 对符号链接只是改个名，
        // 不遍历子树，故「链接的解析目标在源之下」并无 a/b/a/b 打转风险；而
        // canonicalFile 正是把链接解析到目标，据此拒绝会误伤「把链接搬进其目标
        // 子目录」这一正常操作——copyDocument 早已按此放行并有用例钉住。
        if (source.isDirectory && !Files.isSymbolicLink(source.toPath())) {
            val sourceCanonical = source.canonicalFile.path
            val targetParentCanonical = targetParent.canonicalFile.path
            if (
                targetParentCanonical == sourceCanonical ||
                targetParentCanonical.startsWith(sourceCanonical + File.separator)
            ) {
                throw IOException("Refusing to move a directory into its own descendant")
            }
        }
        val sourceParentFile = source.parentFile ?: root
        // 移到同一目录：语义上就是「不动」。AOSP 的 moveDocument 在源父目录与目标父目录
        // 相同时原样返回源 docId——用户没要求改名，就不该被 uniqueChild 改成「name (1)」
        // （源文件自身与目标同名，会被当成冲突）。
        if (sourceParentFile.canonicalFile == targetParent.canonicalFile) {
            return TerminalDocumentsProvider.encodeDocId(source, root)
                ?: throw IOException("Failed to encode docId for '${source.name}'")
        }
        val target = uniqueChild(targetParent, source.name)
        if (!source.renameTo(target)) {
            // 同文件系统 rename 极少失败：目标已占位已在上一步排除，
            // 剩下的是 IO 错误；退化为复制加删除，保证语义完整。
            val copied = copyTree(source, targetParent, source.name)
            deleteWithoutFollowingSymlinks(source)
            notifyChildren(sourceParentFile, root)
            notifyChildren(targetParent, root)
            val copiedId =
                TerminalDocumentsProvider.encodeDocId(copied, root)
                    ?: throw IOException("Failed to encode docId for '${copied.name}'")
            notifyDocument(copied, root)
            notifyDocument(source, root)
            return copiedId
        }
        notifyChildren(sourceParentFile, root)
        notifyChildren(targetParent, root)
        val newId =
            TerminalDocumentsProvider.encodeDocId(target, root)
                ?: throw IOException("Failed to encode docId for '${target.name}'")
        notifyDocument(target, root)
        notifyDocument(source, root)
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
            if (!Files.isSymbolicLink(current.toPath()) && current.isDirectory) {
                directories.add(current)
                current.listFiles()?.forEach { stack.addLast(it) }
            } else {
                deleteOrFail(current, "Failed to delete")
            }
        }
        for (directoryIndex in directories.indices.reversed()) {
            deleteOrFail(directories[directoryIndex], "Failed to delete directory")
        }
    }

    /**
     * 删除单个条目；条目已不在时视为成功。
     *
     * 真机 logcat 实证：文件管理器「覆盖」＝先删目标再拷入，而它缓存的列表里常有
     * 终端侧 `rm` 掉的**幽灵行**——删幽灵时目标已不存在，此前的实现抛 IOException，
     * 跨 Binder 变成 RuntimeException（`exists=false` 那两条未捕获异常即此），
     * 客户端只能报操作失败。删除的后置条件是「不再存在」，已满足就没有失败可言。
     * 确实删不掉（权限、目录非空）时抛 [java.io.FileNotFoundException]：框架
     * `DocumentsProvider.call` 只把这一种类型原样转交，其余一律包成 RuntimeException。
     */
    private fun deleteOrFail(target: File, message: String) {
        if (target.delete()) return
        if (!Files.exists(target.toPath(), LinkOption.NOFOLLOW_LINKS)) return
        throw java.io.FileNotFoundException(
            "$message '${target.path}' " +
                "(exists=${target.exists()}, isDirectory=${target.isDirectory()}, " +
                "canWrite=${target.canWrite()}, isSymlink=${Files.isSymbolicLink(target.toPath())})",
        )
    }

    private fun copyTree(source: File, targetParent: File, baseName: String = source.name): File {
        val target = uniqueChild(targetParent, baseName)
        try {
            copyTreeInto(source, target, source, target)
        } catch (failure: IOException) {
            // 半截复制产物不能留给客户端：尽力清掉再把原错抛出去。
            runCatchingCancellable { deleteWithoutFollowingSymlinks(target) }
            throw failure
        }
        return target
    }

    /**
     * 复制符号链接本身，不跟随目标。
     *
     * 相对目标的基准是**链接所在目录**：原样复制会把基准留在旧目录，
     * 复制到别处后同一串相对路径解析到完全不同的文件（甚至悬空）。故按新位置重算：
     *
     * - 目标在被复制的这棵树**内部**：映射到副本里的同一相对位置。整棵树
     *   `~/p` → `~/q` 时 `p/d/link -> ../f` 变成 `q/d/link -> ../f`，副本自包含，
     *   删掉原树也不会悬空。
     * - 目标在树**外**：仍指向原来那个文件（用绝对路径表达，因为相对深度在新位置
     *   未必成立）。
     * - 显式绝对目标原样保留：用户就是要它指向那个位置。
     */
    private fun copySymbolicLink(source: File, target: File, sourceRoot: File, destRoot: File) {
        val originalTarget = Files.readSymbolicLink(source.toPath())
        val sourceParent =
            source.parentFile
                ?.toPath()
                ?.toAbsolutePath()
                ?.normalize()
        val targetParent =
            target.parentFile
                ?.toPath()
                ?.toAbsolutePath()
                ?.normalize()
        if (sourceParent == null || targetParent == null) {
            Files.createSymbolicLink(target.toPath(), originalTarget)
            return
        }
        // 显式绝对目标原样保留：用户就是要它指向那个位置，重算反而改变语义。
        if (originalTarget.isAbsolute) {
            Files.createSymbolicLink(target.toPath(), originalTarget)
            return
        }
        val resolved = sourceParent.resolve(originalTarget).normalize()
        val sourceRootPath = sourceRoot.toPath().toAbsolutePath().normalize()
        val destRootPath = destRoot.toPath().toAbsolutePath().normalize()
        // 链接目标在**被复制的树内**时，按树内相对位置重映射到副本：
        // 复制 p 到 q 后 p/d/link -> ../f 必须变成 q/d/link -> ../f，
        // 而不是指向原树（副本不自治，且泄漏私有绝对路径）。
        val rebased =
            if (resolved.startsWith(sourceRootPath)) {
                destRootPath.resolve(sourceRootPath.relativize(resolved))
            } else {
                resolved
            }
        Files.createSymbolicLink(target.toPath(), targetParent.relativize(rebased))
    }

    private fun copyTreeInto(source: File, target: File, sourceRoot: File, destRoot: File) {
        if (Files.isSymbolicLink(source.toPath())) {
            // 只复制链接 inode：跟随会把站外目标整棵树吸入家目录。
            copySymbolicLink(source, target, sourceRoot, destRoot)
            return
        }
        if (!source.isDirectory) {
            copyRegularFile(source, target)
            return
        }
        if (!target.mkdirs() && !target.isDirectory) {
            throw IOException("Failed to create directory '${target.path}'")
        }
        // 目录权限最后再搬：源目录若是只读（如 0555），先设权限会让随后创建子项
        // 立刻 EACCES，复制一棵只读目录树必然失败。
        val pendingDirectoryPermissions = mutableListOf(source to target)
        val stack = ArrayDeque<Pair<File, File>>()
        source.listFiles()?.forEach { stack.addLast(it to target) }
        while (stack.isNotEmpty()) {
            val (source, destinationParent) = stack.removeLast()
            if (Files.isSymbolicLink(source.toPath())) {
                copySymbolicLink(source, File(destinationParent, source.name), sourceRoot, destRoot)
            } else if (source.isDirectory) {
                val destination = File(destinationParent, source.name)
                if (!destination.mkdirs() && !destination.isDirectory) {
                    throw IOException("Failed to create directory '${destination.path}'")
                }
                pendingDirectoryPermissions += source to destination
                source.listFiles()?.forEach { stack.addLast(it to destination) }
            } else {
                copyRegularFile(source, File(destinationParent, source.name))
            }
        }
        pendingDirectoryPermissions.asReversed().forEach { (source, destination) ->
            copyPermissions(source, destination)
        }
    }

    /**
     * 复制普通文件并保留权限位。
     *
     * 新建 inode 只带进程 umask 给的默认权限（实测 0644），执行位必丢——脚本拷过去
     * 就不可执行，DESIGN「可执行文件被修改后不丢失权限」因此不成立。故内容搬完后
     * 把源文件的权限位原样写到目标上。
     */
    private fun copyRegularFile(source: File, target: File) {
        source.copyTo(target)
        copyPermissions(source, target)
    }

    /**
     * 把源条目的 POSIX 权限位搬到目标条目。
     *
     * 目标已存在时按「就地改权限」处理，不重建 inode——重建会丢硬链接与属主。
     * 源或目标不落在支持 POSIX 权限的文件系统上时静默跳过：那是平台限制，
     * 不是本次复制引入的错误。
     */
    private fun copyPermissions(source: File, target: File) {
        val permissions = posixPermissionsOrNull(source) ?: return
        runCatchingCancellable {
            Files.setPosixFilePermissions(target.toPath(), permissions)
        }
    }

    /** 条目的 POSIX 权限位；符号链接不参与（Linux 上恒为 0777，lchmod 无从设置）。 */
    private fun posixPermissionsOrNull(source: File): Set<PosixFilePermission>? =
        if (Files.isSymbolicLink(source.toPath())) {
            null
        } else {
            runCatchingCancellable {
                Files.getPosixFilePermissions(source.toPath(), LinkOption.NOFOLLOW_LINKS)
            }.getOrNull()
        }

    private fun rememberReplacedPermissions(parent: File, name: String, permissions: Set<PosixFilePermission>) {
        val key = replacedPermissionKey(parent, name)
        synchronized(replacedPermissions) { replacedPermissions[key] = permissions }
    }

    /**
     * 新建的条目若顶替了一个刚被改名让位的同名条目，继承其权限位。
     *
     * 只认「同一父目录 + 同一名字」的让位记录，用掉即删；普通新建（没有让位记录）
     * 保持 umask 默认权限，不会凭空多出执行位。
     */
    private fun applyReplacedPermissions(parent: File, child: File) {
        val key = replacedPermissionKey(parent, child.name)
        val permissions = synchronized(replacedPermissions) { replacedPermissions.remove(key) } ?: return
        runCatchingCancellable {
            Files.setPosixFilePermissions(child.toPath(), permissions)
        }
    }

    private fun replacedPermissionKey(parent: File, name: String): String = "${parent.canonicalFile.path}\u0000$name"

    private fun uniqueChild(parent: File, baseName: String): File {
        // 与 Termux/桌面/AOSP 一致的冲突策略：已存在则追加序号而非报错。
        // 序号插在扩展名之前且从 1 起（`a.txt` → `a (1).txt`）：与 AOSP
        // `FileUtils.buildUniqueFileWithExtension`（`ncounters n = 0` 先自增，
        // 首个冲突即 `'example (1).txt'`）同型；从 2 起会让第一次冲突显示成
        // `a (2).txt`，客户端与用户都以为已经有同名兄弟（重复复制到第三次才「对上」）。
        var child = File(parent, baseName)
        var conflictId = 1
        while (child.exists() || Files.isSymbolicLink(child.toPath())) {
            child = File(parent, deconflictedName(baseName, conflictId))
            conflictId++
        }
        return child
    }

    /**
     * 冲突名序号：有扩展名把序号插在最后一个点前（`a.txt` → `a (1).txt`）；
     * 无扩展名（`lastIndexOf` 得 -1）或前导点（`.gitignore`，点在首位不算
     * 扩展名）则整体作为基础名尾部追加 ` (n)`。目录与文件共用同一规则，
     * 含点的目录名同样点前插序号（桌面/SAF 惯例一致）。
     */
    private fun deconflictedName(baseName: String, index: Int): String {
        val dot = baseName.lastIndexOf('.')
        return if (dot > 0) {
            "${baseName.substring(0, dot)} ($index)${baseName.substring(dot)}"
        } else {
            "$baseName ($index)"
        }
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
        val extension =
            android.webkit.MimeTypeMap
                .getSingleton()
                .getExtensionFromMimeType(mimeType)
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
     *
     * 广播只决定「什么时候刷新」，不决定「看到什么」：每次 `queryDocument` 与
     * `queryChildDocuments` 都直读文件系统。所以即使某次广播随进程被杀而丢失，
     * 数据依然正确，至多让选择器晚一次刷新——不值得为它加重试或落盘兜底。
     */
    fun notifyWritten(written: File) {
        // 广播一律由文件反推行内 docId（见 notifyDocument/notifyChildren），
        // 调用方传进来的可能带树根前缀（`terminal_home/nix.conf`），而那与本提供者
        // 发行的 `nix.conf` 是兄弟 URI，广播过去收不到。
        notifyDocument(written, rootDir())
        notifyParentOf(written, rootDir())
    }

    /** 父目录列表的变更广播；目录无法编码（根外）时静默跳过，不是可报的错误。 */
    private fun notifyParentOf(file: File, root: File) {
        val parent = file.parentFile ?: return
        notifyChildren(parent, root)
    }

    private fun notifyChildren(parent: File, root: File) {
        // 广播与「列表查询注册的通知 URI」必须同一个：客户端带树根前缀寻址时
        // parent 的 docId 是 `terminal_home/sub`，而行内只发行 `sub`，两者是
        // 兄弟 URI。故这里一律由目录反推行内 docId，绝不透传调用方的字符串。
        val parentId = TerminalDocumentsProvider.encodeDocId(parent, root) ?: return
        context.contentResolver.notifyChange(
            DocumentsContract.buildChildDocumentsUri(
                TerminalDocumentsProvider.AUTHORITY,
                parentId,
            ),
            null,
        )
    }

    private fun notifyDocument(file: File, root: File) {
        val documentId = TerminalDocumentsProvider.encodeDocId(file, root) ?: return
        context.contentResolver.notifyChange(
            DocumentsContract.buildDocumentUri(
                TerminalDocumentsProvider.AUTHORITY,
                documentId,
            ),
            null,
        )
    }
}
