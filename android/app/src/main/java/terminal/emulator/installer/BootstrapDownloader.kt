package terminal.emulator.installer

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
        private const val MAX_BOOTSTRAP_SIZE_BYTES = 1_073_741_824L // 1 GiB hard cap
        private const val DOWNLOAD_BUFFER_SIZE = 8192
        private const val PROGRESS_PERCENT_STEP = 2
    }

    suspend fun download(
        url: String,
        arch: String,
    ): Result<File> = withContext(Dispatchers.IO) {
        // Integrity gate: the bootstrap zip is extracted and its .postinst
        // script is executed, so the download must be authenticated.
        // Plain-http is trivially MITM-able; the default URL already ships
        // https, so rejecting http costs nothing for legitimate users.
        if (!url.startsWith("https://", ignoreCase = true)) {
            return@withContext Result.failure(
                Exception("Bootstrap URL must be https (got non-https URL)"),
            )
        }
        val connection = openConnection(url)
        try {
            val cachedDir = File(context.cacheDir, "bootstrap-$arch.zip")
            cachedDir.delete()
            connection.connectTimeout = NETWORK_CONNECT_TIMEOUT_MS
            connection.readTimeout = NETWORK_READ_TIMEOUT_MS
            connection.instanceFollowRedirects = true
            connection.connect()
            // Redirect bypass guard: the initial https check above can be
            // defeated by a 301/302 from the https server to a plain-http
            // URL, which HttpURLConnection follows silently. The zip is
            // executed (postinst), so the FINAL URL must also be https.
            if (!connection.url.protocol.equals("https", ignoreCase = true)) {
                return@withContext Result.failure(
                    // Log only the final protocol, never the URL itself:
                    // it may carry token/query parameters that would end up
                    // in the persistent log via the orchestrator (round-102).
                    Exception("Bootstrap redirect to non-https URL rejected (final protocol: ${connection.url.protocol})"),
                )
            }
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
            if (contentLength > MAX_BOOTSTRAP_SIZE_BYTES) {
                return@withContext Result.failure(Exception("File too large: $contentLength bytes"))
            }
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
                        // Hard cap: a hostile/misconfigured server with no
                        // Content-Length would otherwise fill the app
                        // partition without bound.
                        if (total > MAX_BOOTSTRAP_SIZE_BYTES) {
                            cachedDir.delete()
                            return@withContext Result.failure(Exception("Download exceeds $MAX_BOOTSTRAP_SIZE_BYTES bytes"))
                        }
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
            // Log the exception class only, not the exception itself: HTTP
            // error messages embed the full URL (with any token/query) and
            // this log can be captured by crash reporters (round-103).
            Log.e("BootstrapDownloader", "Download failed: ${exception.javaClass.simpleName}")
            Result.failure(exception)
        } finally {
            // HttpURLConnection keeps the socket alive if not disconnected;
            // this runs once per bootstrap install but the socket would
            // linger until GC otherwise.
            connection.disconnect()
        }
    }
}
