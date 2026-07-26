package io.term.installer

sealed class BootstrapProgress {
    abstract fun overallProgress(): Float

    abstract fun stepDescription(): String

    data class Downloading(
        val bytesWritten: Long,
        val contentLength: Long,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = if (contentLength > 0) {
            (bytesWritten.toFloat() / contentLength) * 0.85f
        } else {
            0f
        }

        override fun stepDescription(): String {
            val pct = if (contentLength > 0) " (${(bytesWritten * 100 / contentLength)}%)" else ""
            val mb = formatBytes(bytesWritten)
            val total = if (contentLength > 0) " / ${formatBytes(contentLength)}" else ""
            return "Downloading$pct ($mb$total)"
        }
    }

    data class Extracting(
        val entriesExtracted: Int,
        val totalEntries: Int,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.85f +
            if (totalEntries > 0) {
                (entriesExtracted.toFloat() / totalEntries) * 0.15f
            } else {
                0f
            }

        override fun stepDescription(): String {
            val pct = if (totalEntries > 0) " (${(entriesExtracted * 100 / totalEntries)}%)" else ""
            return "Extracting$pct ($entriesExtracted / $totalEntries)"
        }
    }

    data class RunningPostInstall(
        val scriptsCompleted: Int,
        val totalScripts: Int,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.97f +
            if (totalScripts > 0) {
                (scriptsCompleted.toFloat() / totalScripts) * 0.02f
            } else {
                0f
            }

        override fun stepDescription(): String = "Running post-install scripts... ($scriptsCompleted / $totalScripts)"
    }

    data object CreatingSymlinks : BootstrapProgress() {
        override fun overallProgress(): Float = 0.99f

        override fun stepDescription(): String = "Creating symlinks..."
    }

    data object Complete : BootstrapProgress() {
        override fun overallProgress(): Float = 1f

        override fun stepDescription(): String = "Bootstrap complete!"
    }

    data class Error(
        val message: String,
    ) : BootstrapProgress() {
        override fun overallProgress(): Float = 0f

        override fun stepDescription(): String = message
    }

    companion object {
        private const val KB = 1024L
        private const val MB = KB * 1024

        private fun formatBytes(bytes: Long): String = when {
            bytes >= MB -> "${bytes / MB} MB"
            bytes >= KB -> "${bytes / KB} KB"
            else -> "$bytes B"
        }
    }
}

fun interface BootstrapProgressCallback {
    fun onProgress(progress: BootstrapProgress)
}
