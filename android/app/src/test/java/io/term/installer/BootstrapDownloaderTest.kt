package io.term.installer

import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import java.io.ByteArrayInputStream
import java.net.HttpURLConnection

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class BootstrapDownloaderTest {
    private val testDispatcher = StandardTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun teardown() {
        Dispatchers.resetMain()
    }

    private fun createMockConnection(
        responseCode: Int = HttpURLConnection.HTTP_OK,
        responseMessage: String = "OK",
        contentLength: Int = 2_000_000,
        inputBytes: ByteArray? = null,
    ): HttpURLConnection {
        val connection = mockk<HttpURLConnection>(relaxed = true)
        every { connection.responseCode } returns responseCode
        every { connection.responseMessage } returns responseMessage
        every { connection.contentLength } returns contentLength
        if (responseCode in 200..299) {
            val bytes = inputBytes ?: ByteArray(maxOf(contentLength, 0)) { 0x42 }
            every { connection.inputStream } returns ByteArrayInputStream(bytes)
        }
        return connection
    }

    @Test
    fun `HTTP 200 returns success with downloaded file`() = runTest(testDispatcher) {
        val contentLength = 1_050_000
        val mockConnection = createMockConnection(contentLength = contentLength)
        val context = RuntimeEnvironment.getApplication().applicationContext
        val downloader = BootstrapDownloader(context)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isSuccess)
        val file = result.getOrThrow()
        assertTrue(file.exists())
        assertEquals(contentLength.toLong(), file.length())
        verify { mockConnection.connectTimeout = any() }
        verify { mockConnection.readTimeout = any() }
    }

    @Test
    fun `HTTP 404 returns failure`() = runTest(testDispatcher) {
        val mockConnection =
            createMockConnection(
                responseCode = 404,
                responseMessage = "Not Found",
            )
        val context = RuntimeEnvironment.getApplication().applicationContext
        val downloader = BootstrapDownloader(context)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("HTTP 404") == true)
    }

    @Test
    fun `content too small is rejected`() = runTest(testDispatcher) {
        val mockConnection = createMockConnection(contentLength = 10)
        val context = RuntimeEnvironment.getApplication().applicationContext
        val downloader = BootstrapDownloader(context)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isFailure)
        assertTrue(
            result.exceptionOrNull()?.message?.contains("too small") == true ||
                result.exceptionOrNull()?.message?.contains("contentLength") == true,
        )
    }

    @Test
    fun `progress is reported during download`() = runTest(testDispatcher) {
        val contentLength = 2_000_000
        val mockConnection = createMockConnection(contentLength = contentLength)
        val context = RuntimeEnvironment.getApplication().applicationContext
        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val downloader = BootstrapDownloader(context, onProgress)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isSuccess)
        assertTrue(captured.isNotEmpty())
        assertTrue(captured.all { it is BootstrapProgress.Downloading })
        val lastProgress = captured.last() as BootstrapProgress.Downloading
        assertEquals(contentLength.toLong(), lastProgress.contentLength)
        assertTrue(lastProgress.bytesWritten in 1..contentLength)
    }

    @Test
    fun `content length just above minimum succeeds`() = runTest(testDispatcher) {
        val contentLength = 1_500_000
        val mockConnection = createMockConnection(contentLength = contentLength)
        val context = RuntimeEnvironment.getApplication().applicationContext
        val downloader = BootstrapDownloader(context)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isSuccess)
        assertTrue(result.getOrThrow().exists())
    }

    @Test
    fun `HTTP 500 returns failure`() = runTest(testDispatcher) {
        val mockConnection =
            createMockConnection(
                responseCode = 500,
                responseMessage = "Internal Server Error",
            )
        val context = RuntimeEnvironment.getApplication().applicationContext
        val downloader = BootstrapDownloader(context)
        downloader.internalConnectionFactory = { mockConnection }

        val result = downloader.download("https://example.com/test.zip", "x86_64")

        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("HTTP 500") == true)
    }
}
