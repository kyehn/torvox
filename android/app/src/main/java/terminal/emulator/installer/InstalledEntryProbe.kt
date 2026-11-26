package terminal.emulator.installer

import terminal.emulator.runtime.LogUtil
import java.io.File

/**
 * [file] 是否以 ELF 魔数（0x7f 'E' 'L' 'F'）开头。用于安装结果校验：只有真实 ELF 锚点
 * （`usr/bin/login`、`usr/bin/bash`）才算引导成功，linker-wrapper 的 spawn 路径只能加载
 * 真正的 ELF 二进制。
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
 * 桥接；指向应用私有目录的脚本不计入（其解释器本身尚不可用）。供安装器判断通用 bootstrap
 * 启动器脚本是否可用，不针对特定发行版。
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
