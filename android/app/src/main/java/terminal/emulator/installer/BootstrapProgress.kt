package terminal.emulator.installer

/** Progress of a bootstrap install, surfaced on the settings screen.
 *  All user-facing text is formatted in the UI layer from the raw
 *  numbers below (string resources), so this file stays text-free. */
sealed class BootstrapProgress {
    abstract fun overallProgress(): Float

    data class Downloading(
        val bytesWritten: Long,
        val contentLength: Long,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = if (contentLength > 0) {
            (bytesWritten.toFloat() / contentLength) * 0.85f
        } else {
            0f
        }
    }

    data class Extracting(
        val entriesExtracted: Int,
        val totalEntries: Int,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.85f +
            if (totalEntries > 0) {
                // Capped at 0.97 so CreatingSymlinks (0.99) and
                // RunningPostInstall (0.97..1.0) never regress the bar
                //
                (entriesExtracted.toFloat() / totalEntries) * 0.12f
            } else {
                0f
            }
    }

    data class RunningPostInstall(
        val scriptsCompleted: Int,
        val totalScripts: Int,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.99f +
            if (totalScripts > 0) {
                // Starts at 0.99 (range 0.99..1.0) so the bar never regresses
                // from CreatingSymlinks (0.99); Complete (1.0) is the final
                // step.
                (scriptsCompleted.toFloat() / totalScripts) * 0.01f
            } else {
                0f
            }
    }

    data object CreatingSymlinks : BootstrapProgress() {
        override fun overallProgress(): Float = 0.99f
    }

    data object Complete : BootstrapProgress() {
        override fun overallProgress(): Float = 1f
    }

    data class Error(
        val message: String,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0f
    }
}

fun interface BootstrapProgressCallback {
    fun onProgress(progress: BootstrapProgress)
}
