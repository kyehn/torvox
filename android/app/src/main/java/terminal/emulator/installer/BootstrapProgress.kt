package terminal.emulator.installer

/** 引导安装的进度，在设置界面展示。所有面向用户的文案由 UI 层从下方原始数值
 *  （字符串资源）格式化，故本文件不含任何文案。 */
sealed class BootstrapProgress {
    abstract fun overallProgress(): Float

    data class Downloading(val bytesWritten: Long, val contentLength: Long) : BootstrapProgress() {
        override fun overallProgress(): Float = if (contentLength > 0) {
            (bytesWritten.toFloat() / contentLength) * 0.85f
        } else {
            0f
        }
    }

    data class Extracting(val entriesExtracted: Int, val totalEntries: Int) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.85f +
            if (totalEntries > 0) {
                // 上限 0.97，使 CreatingSymlinks（0.99）与
                // RunningPostInstall（0.97..1.0）永不使进度条回退
                (entriesExtracted.toFloat() / totalEntries) * 0.12f
            } else {
                0f
            }
    }

    data class RunningPostInstall(val scriptsCompleted: Int, val totalScripts: Int) : BootstrapProgress() {
        override fun overallProgress(): Float = 0.99f +
            if (totalScripts > 0) {
                // 起始 0.99（区间 0.99..1.0），使进度条永不从
                // CreatingSymlinks（0.99）回退；Complete（1.0）是最后一步。
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

    data class Error(val message: String) : BootstrapProgress() {
        override fun overallProgress(): Float = 0f
    }
}

fun interface BootstrapProgressCallback {
    fun onProgress(progress: BootstrapProgress)
}
