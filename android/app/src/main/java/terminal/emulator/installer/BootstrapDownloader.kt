package terminal.emulator.installer

import android.content.Context
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.TimeUnit

class BootstrapDownloader(
    private val context: Context,
    private val onProgress: BootstrapProgressCallback? = null,
    internal val client: OkHttpClient = defaultClient(),
) {
    companion object {
        private const val NETWORK_CONNECT_TIMEOUT_MS = 30_000L
        private const val NETWORK_READ_TIMEOUT_MS = 300_000L
        private const val MIN_BOOTSTRAP_SIZE_BYTES = 1_048_576L
        private const val MAX_BOOTSTRAP_SIZE_BYTES = 1_073_741_824L // 1 GiB hard cap
        private const val DOWNLOAD_BUFFER_SIZE = 8192
        private const val PROGRESS_PERCENT_STEP = 2

        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(NETWORK_CONNECT_TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .readTimeout(NETWORK_READ_TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .build()
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
        val request = Request.Builder().url(url).build()
        try {
            client.newCall(request).execute().use { response ->
                // Redirect bypass guard: okhttp follows cross-protocol
                // redirects (https -> http) by default, which would defeat
                // the initial https check above. The zip is executed
                // (postinst), so the FINAL URL must also be https.
                val finalScheme = response.request.url.scheme
                if (!finalScheme.equals("https", ignoreCase = true)) {
                    return@withContext Result.failure(
                        // Log only the final protocol, never the URL itself:
                        // it may carry token/query parameters that would end up
                        // in the persistent log via the orchestrator (round-102).
                        Exception("Bootstrap redirect to non-https URL rejected (final protocol: $finalScheme)"),
                    )
                }
                if (!response.isSuccessful) {
                    return@withContext Result.failure(
                        Exception("HTTP ${response.code}: ${response.message}"),
                    )
                }
                val contentLength = response.body.contentLength()
                if (contentLength > 0 && contentLength < MIN_BOOTSTRAP_SIZE_BYTES) {
                    return@withContext Result.failure(Exception("File too small: $contentLength bytes"))
                }
                if (contentLength > MAX_BOOTSTRAP_SIZE_BYTES) {
                    return@withContext Result.failure(Exception("File too large: $contentLength bytes"))
                }
                val body = response.body
                val cachedDir = File(context.cacheDir, "bootstrap-$arch.zip")
                cachedDir.delete()
                body.source().use { input ->
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
            }
        } catch (exception: Exception) {
            // Log the exception class only, not the exception itself: HTTP
            // error messages embed the full URL (with any token/query) and
            // this log can be captured by crash reporters (round-103).
            Log.e("BootstrapDownloader", "Download failed: ${exception.javaClass.simpleName}")
            Result.failure(exception)
        }
    }
}
