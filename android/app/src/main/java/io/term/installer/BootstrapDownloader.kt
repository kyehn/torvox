package io.term.installer

import android.content.Context
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL

class BootstrapDownloader(
    private val context: Context,
    private val onProgress: BootstrapProgressCallback? = null,
) {
    internal var internalConnectionFactory: ((String) -> HttpURLConnection)? = null

    private fun openConnection(url: String): HttpURLConnection = internalConnectionFactory?.invoke(url)
        ?: (URL(url).openConnection() as HttpURLConnection)

    companion object {
        private const val NETWORK_CONNECT_TIMEOUT_MS = 30_000
        private const val NETWORK_READ_TIMEOUT_MS = 300_000
        private const val MIN_BOOTSTRAP_SIZE_BYTES = 1_048_576L
        private const val DOWNLOAD_BUFFER_SIZE = 8192
        private const val PROGRESS_PERCENT_STEP = 2
    }

    suspend fun download(
        url: String,
        arch: String,
    ): Result<File> = withContext(Dispatchers.IO) {
        try {
            val cachedDir = File(context.cacheDir, "bootstrap-$arch.zip")
            cachedDir.delete()
            val connection = openConnection(url)
            connection.connectTimeout = NETWORK_CONNECT_TIMEOUT_MS
            connection.readTimeout = NETWORK_READ_TIMEOUT_MS
            connection.instanceFollowRedirects = true
            connection.connect()
            if (connection.responseCode !in 200..299) {
                return@withContext Result.failure(
                    Exception("HTTP ${connection.responseCode}: ${connection.responseMessage}"),
                )
            }
            connection.contentLength.let { length ->
                if (length > 0 && length < MIN_BOOTSTRAP_SIZE_BYTES) {
                    return@withContext Result.failure(Exception("File too small: $length bytes"))
                }
            }
            val contentLength = connection.contentLength.toLong().coerceAtLeast(0L)
            connection.inputStream.use { input ->
                FileOutputStream(cachedDir).use { output ->
                    val buffer = ByteArray(DOWNLOAD_BUFFER_SIZE)
                    var total = 0L
                    var lastReportedPct = -100
                    while (true) {
                        if (!isActive) {
                            cachedDir.delete()
                            return@withContext Result.failure(Exception("Download cancelled"))
                        }
                        val bytesRead = input.read(buffer)
                        if (bytesRead == -1) break
                        output.write(buffer, 0, bytesRead)
                        total += bytesRead
                        val pct =
                            if (contentLength > 0L) {
                                (total * 100L / contentLength).toInt()
                            } else {
                                -1
                            }
                        if (pct != lastReportedPct) {
                            lastReportedPct = pct
                            if (lastReportedPct % PROGRESS_PERCENT_STEP == 0 || lastReportedPct >= 99) {
                                onProgress?.onProgress(
                                    BootstrapProgress.Downloading(total, contentLength),
                                )
                            }
                        }
                    }
                    if (total < MIN_BOOTSTRAP_SIZE_BYTES) {
                        cachedDir.delete()
                        return@withContext Result.failure(Exception("Download too small: $total bytes"))
                    }
                }
            }
            Result.success(cachedDir)
        } catch (exception: Exception) {
            Log.e("BootstrapDownloader", "Download failed", exception)
            Result.failure(exception)
        }
    }
}
