package io.torvox.installer

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
) {
    companion object {
        private const val NETWORK_CONNECT_TIMEOUT_MS = 30_000
        private const val NETWORK_READ_TIMEOUT_MS = 300_000
        private const val MIN_BOOTSTRAP_SIZE_BYTES = 1_048_576L
        private const val DOWNLOAD_BUFFER_SIZE = 8192
    }

    suspend fun download(
        url: String,
        arch: String,
    ): Result<File> = withContext(Dispatchers.IO) {
        try {
            val cachedDir = File(context.cacheDir, "bootstrap-$arch.zip")
            cachedDir.delete()
            val connection = URL(url).openConnection() as HttpURLConnection
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
            connection.inputStream.use { input ->
                FileOutputStream(cachedDir).use { output ->
                    val buffer = ByteArray(DOWNLOAD_BUFFER_SIZE)
                    var total = 0L
                    while (true) {
                        if (!isActive) {
                            cachedDir.delete()
                            return@withContext Result.failure(Exception("Download cancelled"))
                        }
                        val bytesRead = input.read(buffer)
                        if (bytesRead == -1) break
                        output.write(buffer, 0, bytesRead)
                        total += bytesRead
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
