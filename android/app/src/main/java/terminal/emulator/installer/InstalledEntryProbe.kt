package terminal.emulator.installer

import terminal.emulator.runtime.LogUtil
import java.io.File

/**
 * [file] 是否以 ELF 魔数（0x7f 'E' 'L' 'F'）开头。用于在 prefix shell 解析中排除 shebang 脚本：
 * linker-wrapper 的 spawn 路径只能加载真正的 ELF 二进制。
 */
internal fun isElf(file: File): Boolean = try {
    file.inputStream().use { input ->
        val magic = ByteArray(4)
        val read = input.read(magic)
        read == 4 &&
            magic[0] == 0x7f.toByte() &&
            magic[1] == 'E'.code.toByte() &&
            magic[2] == 'L'.code.toByte() &&
            magic[3] == 'F'.code.toByte()
    }
} catch (exception: Exception) {
    LogUtil.warn("BootstrapInstaller", "isElf: cannot read ${file.absolutePath}", exception)
    false
}

/**
 * 系统解释器启动脚本判定：首行 shebang 指向系统路径（`/system/bin/`）的脚本可由应用进程直接执行，无需经 linker
 * 桥接；指向应用私有目录的脚本不计入（其解释器本身尚不可用）。与安装器/启动路径共享，供通用 bootstrap 启动器脚本使用，不针对特定发行版。
 */
internal fun isSystemShellScript(file: File): Boolean = try {
    file.inputStream().use { input ->
        val header = ByteArray(64)
        val read = input.read(header)
        if (read <= 0) {
            false
        } else {
            val firstLine = header.decodeToString(0, read).lineSequence().firstOrNull().orEmpty()
            firstLine.startsWith("#!/system/bin/")
        }
    }
} catch (exception: Exception) {
    LogUtil.warn("BootstrapInstaller", "isSystemShellScript: cannot read ${file.absolutePath}", exception)
    false
}
