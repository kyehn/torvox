package terminal.emulator.installer

import android.content.Context
import android.util.Log
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import terminal.emulator.util.TerminalDispatchers
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

    suspend fun download(url: String, arch: String): Result<File> = withContext(TerminalDispatchers.inputOutput) {
        // 完整性门控：引导 zip 会被解压并执行其 postinst 脚本，
        // 故下载必须经过身份认证。明文 http 极易被中间人篡改；
        // 而默认 URL 本就是 https，故拒绝 http 对合法用户零成本。
        if (!url.startsWith("https://", ignoreCase = true)) {
            return@withContext Result.failure(
                Exception("Bootstrap URL must be https (got non-https URL)"),
            )
        }
        val request = Request.Builder().url(url).build()
        try {
            client.newCall(request).execute().use { response ->
                // 重定向绕过防护：okhttp 默认跟随跨协议重定向（https -> http），
                // 这会架空上方的 https 检查。zip 会被执行（postinst），
                // 故最终 URL 也必须是 https。
                val finalScheme = response.request.url.scheme
                if (!finalScheme.equals("https", ignoreCase = true)) {
                    return@withContext Result.failure(
                        // 只记录最终协议，绝不记录 URL 本身：
                        // 它可能携带 token/查询参数而经编排器进入持久日志。
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
                                return@withContext Result.failure(Exception(BootstrapOrchestrator.ERROR_CANCELLED))
                            }
                            val bytesRead = input.read(buffer)
                            if (bytesRead == -1) break
                            output.write(buffer, 0, bytesRead)
                            total += bytesRead
                            // 硬上限：恶意/配置不当且不返回 Content-Length 的服务器
                            // 否则会无界填满应用分区。
                            if (total > MAX_BOOTSTRAP_SIZE_BYTES) {
                                cachedDir.delete()
                                return@withContext Result.failure(
                                    Exception("Download exceeds $MAX_BOOTSTRAP_SIZE_BYTES bytes"),
                                )
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
            // 只记录异常类名而非异常本身：HTTP 错误消息会嵌入完整 URL
            // （含任何 token/查询参数），而此日志可能被崩溃报告器采集。
            Log.e("BootstrapDownloader", "Download failed: ${exception.javaClass.simpleName}")
            Result.failure(exception)
        }
    }
}
