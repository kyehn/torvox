package terminal.emulator.installer

import android.content.Context
import android.util.Log
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.asResponseBody
import okio.Buffer
import okio.BufferedSource
import okio.Source
import okio.Timeout
import okio.buffer
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLog

/**
 * 下载中途被取消属**正常路径**（用户中止安装），MUST NOT 留下 error 级日志
 * （见 `logcat-hygiene`「应用自身的正常路径不得产生警告与错误」）。
 *
 * 回归钉：`download` 的外层 `catch (Exception)` 曾把内层重抛的
 * `CancellationException` 一并吞下，再以「Download failed: …」记 error。取消本身由
 * `withContext` 的收尾抛回调用方，故行为层看不出分叉——只有日志能暴露它，本用例
 * 因此断言日志等级。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BootstrapDownloaderCancellationTest {
    @Test
    fun `cancelling an in-flight download logs no error`() = runBlocking {
        val client = OkHttpClient
            .Builder()
            .addInterceptor { chain -> endlessResponse(chain.request()) }
            .build()
        val context = ApplicationProvider.getApplicationContext<Context>()
        val downloader = BootstrapDownloader(context, client = client)
        ShadowLog.clear()
        val result = withTimeoutOrNull(CANCEL_AFTER_MS) {
            downloader.download(BOOTSTRAP_URL, "test")
        }
        assertNull("计时到点必须让下载中止，而不是返回成功/失败结果", result)
        val errors = ShadowLog.getLogs().filter { it.tag == TAG && it.type >= Log.ERROR }
        assertTrue("取消不得记 error：${errors.map { it.msg }}", errors.isEmpty())
    }

    /** 每次读出一块就返回、永不结束，给取消留出确定的窗口。 */
    private fun endlessResponse(request: Request): Response = Response
        .Builder()
        .request(request)
        .protocol(Protocol.HTTP_1_1)
        .code(200)
        .message("OK")
        .body(endlessSource().asResponseBody("application/zip".toMediaType(), ENDLESS_LENGTH))
        .build()

    private fun endlessSource(): BufferedSource {
        val chunk = ByteArray(CHUNK_BYTES)
        return object : Source {
            override fun read(sink: Buffer, byteCount: Long): Long {
                val size = minOf(byteCount, CHUNK_BYTES.toLong()).toInt()
                sink.write(chunk, 0, size)
                return size.toLong()
            }

            override fun timeout(): Timeout = Timeout.NONE

            override fun close() = Unit
        }.buffer()
    }

    private companion object {
        const val TAG = "BootstrapDownloader"
        const val BOOTSTRAP_URL = "https://example.invalid/bootstrap.zip"
        const val CANCEL_AFTER_MS = 200L
        const val CHUNK_BYTES = 8192

        /** 无界响应体的声明长度，用来验证取消在拷贝途中生效。 */
        const val ENDLESS_LENGTH = 512L * 1024 * 1024
    }
}
