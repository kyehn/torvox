package io.torvox.exec

import android.content.Context
import java.io.File

object ExecInstaller {
    private const val BIN_DIR = "bin"
    private const val EXEC_NAME = "torvox-exec"

    fun install(context: Context): File {
        val binDir = File(context.filesDir, BIN_DIR)
        binDir.mkdirs()

        val execFile = File(binDir, EXEC_NAME)
        val abi = detectAbi()

        val assetPath = "bin/$abi/$EXEC_NAME"
        if (!execFile.exists() || !execFile.canExecute()) {
            context.assets.open(assetPath).use { input ->
                execFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
            execFile.setExecutable(true, false)
        }

        return execFile
    }

    fun binDir(context: Context): File = File(context.filesDir, BIN_DIR)

    private fun detectAbi(): String =
        when (
            android.os.Build.SUPPORTED_ABIS
                .first()
        ) {
            "arm64-v8a" -> "arm64-v8a"
            "x86_64" -> "x86_64"
            else -> "arm64-v8a"
        }
}
