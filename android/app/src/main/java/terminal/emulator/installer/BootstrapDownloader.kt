package terminal.emulator.installer

import android.content.Context
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.TerminalDispatchers
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.security.DigestOutputStream
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

class BootstrapDownloader(
    private val context: Context,
    private val onProgress: BootstrapProgressCallback? = null,
    internal val client: OkHttpClient = defaultClient(),
) {
    companion object {
        private const val TAG = "BootstrapDownloader"

        /** 本地引导 zip 的 URL 方案。门控与前缀剥离共用它，避免两处口径漂移。 */
        private const val FILE_URL_SCHEME = "file://"
        private const val NETWORK_CONNECT_TIMEOUT_MS = 30_000L
        private const val NETWORK_READ_TIMEOUT_MS = 300_000L
        private const val MIN_BOOTSTRAP_SIZE_BYTES = 1_048_576L
        private const val PROGRESS_PERCENT_STEP = 2

        /** 归档校验和算法：`java.security` 保证可用的标准名。 */
        private const val SHA_256_ALGORITHM = "SHA-256"

        /**
         * 进程内唯一的默认客户端。
         *
         * 每个 `OkHttpClient` 自带一套 `Dispatcher`（线程池）与 `ConnectionPool`，
         * 都要显式关闭才会立即回收。而 `BootstrapDownloader` 是每次点安装新建的
         * （超时的那个必须随那次安装走完），若这里也现造，连续安装几次就会叠加出
         * 多套线程池与连接池——它们的线程只能在 keep-alive 结束后自退。
         * 超时是常量，无需每次重建，故按进程缓存。
         */
        private val defaultClientHolder =
            lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
                OkHttpClient
                    .Builder()
                    .connectTimeout(NETWORK_CONNECT_TIMEOUT_MS, TimeUnit.MILLISECONDS)
                    .readTimeout(NETWORK_READ_TIMEOUT_MS, TimeUnit.MILLISECONDS)
                    .build()
            }

        fun defaultClient(): OkHttpClient = defaultClientHolder.value
    }

    suspend fun download(url: String, architecture: String): Result<File> =
        withContext(TerminalDispatchers.inputOutput) {
            // 本地 file 链路：测试把已下载的 zip 推到设备，安装器直接用它。
            // 产品 UI 只提供 https URL（见下门控），file 永远到不了用户手里。
            //
            // 必须**复制**到 cacheDir 再返回，而不能把用户的文件本身交出去：本方法的返回值
            // 归调用方所有，`BootstrapOrchestrator` 装完（无论成败）都会 `delete()` 它。
            // 直接返回原文件等于让安装流程删掉用户的 zip——测试里恰好是构建产物，删了不易察觉。
            // 门控与剥离必须同一口径：`String.removePrefix` 区分大小写，此前门控写
            // ignoreCase = true 而剥离没有，于是 `FILE:///sdcard/x.zip` 通过门控却剥不掉
            // 前缀，`File("FILE:///…")` 必然不是文件，落到下面那句「本地文件不可读」——
            // 错误信息与真实原因对不上。file URI 的 scheme 按 RFC 3986 本就小写。
            if (url.startsWith(FILE_URL_SCHEME)) {
                val sourceFile = File(url.removePrefix(FILE_URL_SCHEME).substringBefore("?"))
                if (!sourceFile.isFile || !sourceFile.canRead()) {
                    return@withContext Result.failure(Exception("Local bootstrap file unreadable"))
                }
                if (sourceFile.length() !in 1L..BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES) {
                    return@withContext Result.failure(
                        Exception("File size out of range: ${sourceFile.length()} bytes"),
                    )
                }
                val staged = File(context.cacheDir, "bootstrap-local-$architecture.zip")
                return@withContext try {
                    staged.delete()
                    sourceFile.inputStream().use { input ->
                        FileOutputStream(staged).use { output -> input.copyTo(output) }
                    }
                    Result.success(staged)
                } catch (exception: IOException) {
                    staged.delete()
                    Result.failure(exception)
                }
            }
            // 传输门控：引导 zip 会被解压并执行其 postinst 脚本，
            // 故下载必须经 TLS 防链路篡改；默认 URL 本就是 https，
            // 拒绝 http 对合法用户零成本。https 不构成归档内容真值。
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
                    if (contentLength > BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES) {
                        return@withContext Result.failure(Exception("File too large: $contentLength bytes"))
                    }
                    val body = response.body
                    val cachedFile = File(context.cacheDir, "bootstrap-$architecture.zip")
                    cachedFile.delete()
                    // 完整性门控的第二道：https 只防链路篡改，不保证拿到的就是声明的那个
                    // 归档，而引导 URL 在设置里可由用户编辑，故 https 不构成信任根——
                    // 校验和才是。只对钉住的预设 URL 生效：自定义 URL 的内容本仓无从预知，
                    // 无从比对，但这次跳过必须留下痕迹，不能静默降级。
                    val expectedSha256 =
                        if (url == TermuxBootstrap.downloadUrl(architecture)) {
                            TermuxBootstrap.sha256(architecture)
                        } else {
                            LogUtil.warn(
                                TAG,
                                "bootstrap URL is not the pinned preset; checksum not verified",
                            )
                            null
                        }
                    // 摘要在落盘途中顺带算出，不为校验再读一遍归档。
                    val digest =
                        expectedSha256?.let { MessageDigest.getInstance(SHA_256_ALGORITHM) }
                    body.byteStream().use { input ->
                        FileOutputStream(cachedFile).use { rawOutput ->
                            val output =
                                digest?.let { DigestOutputStream(rawOutput, it) } ?: rawOutput
                            var total = 0L
                            var lastReportedPercent = -100
                            try {
                                copyBootstrapArchive(
                                    input = input,
                                    output = output,
                                    shouldAbort = { !isActive },
                                    onCopied = { copied ->
                                        total = copied
                                        val percent =
                                            if (contentLength > 0L) {
                                                (total * 100L / contentLength).toInt()
                                            } else {
                                                -1
                                            }
                                        if (percent != lastReportedPercent) {
                                            lastReportedPercent = percent
                                            if (percent % PROGRESS_PERCENT_STEP == 0 || percent >= 99) {
                                                onProgress?.onProgress(
                                                    BootstrapProgress.Downloading(total, contentLength),
                                                )
                                            }
                                        }
                                    },
                                )
                            } catch (copyError: Exception) {
                                cachedFile.delete()
                                return@withContext if (!isActive) {
                                    Result.failure(Exception(BootstrapOrchestrator.ERROR_CANCELLED))
                                } else {
                                    Result.failure(copyError)
                                }
                            }
                            if (total < MIN_BOOTSTRAP_SIZE_BYTES) {
                                cachedFile.delete()
                                return@withContext Result.failure(Exception("Download too small: $total bytes"))
                            }
                            val actualSha256 = digest?.digest()?.encodeToHex()
                            if (expectedSha256 != null &&
                                !expectedSha256.equals(actualSha256, ignoreCase = true)
                            ) {
                                cachedFile.delete()
                                // 摘要本身不含凭据，可以安全地带进日志；归档随后会被解压
                                // 并执行其 postinst，故不符即删、绝不交给安装器。
                                return@withContext Result.failure(
                                    Exception(
                                        "Bootstrap checksum mismatch: expected $expectedSha256, got $actualSha256",
                                    ),
                                )
                            }
                        }
                    }
                    Result.success(cachedFile)
                }
            } catch (exception: Exception) {
                // 只记录异常类名而非异常本身：HTTP 错误消息会嵌入完整 URL
                // （含任何 token/查询参数），而此日志可能被崩溃报告器采集。
                LogUtil.error("BootstrapDownloader", "Download failed: ${exception.javaClass.simpleName}")
                Result.failure(exception)
            }
        }
}
