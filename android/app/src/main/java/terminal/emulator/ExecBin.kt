package terminal.emulator

import android.content.Context
import android.util.Log
import java.io.File

/**
 * Stages the exec-bin trampoline (`assets/bin/<abi>/exec-bin`) into `files/exec-bin` at startup.
 *
 * exec-bin is the INTERP-aware launcher for the Android app-data exec ban (Android 15+ SELinux
 * denies `untrusted_app` direct `execve` of `app_data_file`): it routes prefix binaries through the
 * system linker, chain-loading `/nix/store`-INTERP ELFs (fork Go login) through the prefix's own
 * glibc loader. The terminal and Maestro flows invoke it by absolute path; staging only copies
 * bytes (loud log on failure — the use-site error names the missing file, never a silent fallback).
 */
object ExecBin {
  private const val TAG = "ExecBin"
  const val NAME = "exec-bin"

  fun installedFile(context: Context): File = File(context.filesDir, NAME)

  /**
   * Copy the bundled trampoline to [installedFile] when the bytes differ. Best-effort at startup
   * (must not crash `onCreate` on exotic ROMs); returns the destination file in all cases.
   */
  @Suppress("TooGenericExceptionCaught")
  fun ensureInstalled(context: Context): File {
    val dest = installedFile(context)
    try {
      val abi =
          android.os.Build.SUPPORTED_ABIS.firstOrNull { abi ->
            runCatching { context.assets.list("bin/$abi")?.contains(NAME) }.getOrDefault(false) ==
                true
          } ?: android.os.Build.SUPPORTED_ABIS.firstOrNull()
      if (abi == null) {
        Log.w(TAG, "no supported ABI for $NAME staging")
        return dest
      }
      context.assets.open("bin/$abi/$NAME").use { input ->
        val bytes = input.readBytes()
        if (!dest.isFile || !dest.readBytes().contentEquals(bytes)) {
          dest.writeBytes(bytes)
          dest.setExecutable(true)
        }
      }
    } catch (exception: Exception) {
      Log.w(TAG, "staging $NAME failed", exception)
    }
    return dest
  }
}
