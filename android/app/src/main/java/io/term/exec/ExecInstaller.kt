package io.term.exec

import android.content.Context
import io.term.R
import java.io.File

object ExecInstaller {
    private const val EXEC_NAME = "exec-bin"

    fun install(context: Context): File {
        val binDir = context.getDir("bin", Context.MODE_PRIVATE)
        binDir.mkdirs()

        val execFile = File(binDir, EXEC_NAME)
        val abi = detectAbi(context)

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

    fun binDir(context: Context): File = context.getDir("bin", Context.MODE_PRIVATE)

    private fun detectAbi(context: Context): String {
        val abi =
            android.os.Build.SUPPORTED_ABIS
                .firstOrNull()
                ?: error("No supported ABIs found on device")
        return when (abi) {
            "arm64-v8a" -> {
                "arm64-v8a"
            }

            "x86_64" -> {
                "x86_64"
            }

            else -> {
                error(
                    context.getString(
                        R.string.unsupported_abi,
                        abi,
                    ),
                )
            }
        }
    }
}
