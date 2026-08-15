package terminal.emulator.installer

import android.content.Context
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.runBlocking
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.File

class BootstrapDownloaderTest {
    private lateinit var httpsServer: MockWebServer
    private lateinit var plainServer: MockWebServer
    private lateinit var tempDir: File

    @Before
    fun setUp() {
        tempDir = File.createTempFile("bootstrap-test", "").apply {
            delete()
            mkdirs()
        }
        httpsServer = MockWebServer()
        plainServer = MockWebServer()
    }

    @After
    fun tearDown() {
        httpsServer.close()
        plainServer.close()
        tempDir.deleteRecursively()
    }

    private fun context(): Context = mockk {
        every { cacheDir } returns tempDir
    }

    /** Configures httpsServer with an okhttp-tls HeldCertificate (2048-bit,
     *  SAN=localhost) and returns a client that trusts that exact certificate.
     *  A JDK ephemeral self-signed cert would be rejected by
     *  jdk.tls.disabledAlgorithms (RSA < 2048) with handshake_failure. */
    private fun httpsSetup(): OkHttpClient {
        val heldCertificate =
            HeldCertificate.Builder()
                .addSubjectAlternativeName("localhost")
                .build()
        val serverCerts =
            HandshakeCertificates.Builder()
                .heldCertificate(heldCertificate)
                .build()
        val clientCerts =
            HandshakeCertificates.Builder()
                .addTrustedCertificate(heldCertificate.certificate)
                .build()
        httpsServer.useHttps(serverCerts.sslSocketFactory())
        return OkHttpClient.Builder()
            .sslSocketFactory(clientCerts.sslSocketFactory(), clientCerts.trustManager)
            .build()
    }

    @Test
    fun nonHttpsUrlRejectedWithoutRequest() = runBlocking {
        plainServer.start()
        val downloader = BootstrapDownloader(context(), client = OkHttpClient())
        val result = downloader.download(plainServer.url("/bootstrap.zip").toString(), "test")
        assertTrue(result.isFailure)
        assertEquals("Bootstrap URL must be https (got non-https URL)", result.exceptionOrNull()?.message)
        // The URL gate must reject before any network I/O happens.
        assertEquals(0, plainServer.requestCount)
    }

    @Test
    fun httpsDownloadSuccessWithProgress() = runBlocking {
        val client = httpsSetup()
        httpsServer.start()
        val payload = ByteArray(1_050_000) { 0x41 } // above MIN_BOOTSTRAP_SIZE_BYTES
        httpsServer.enqueue(
            MockResponse.Builder()
                .code(200)
                .body(Buffer().write(payload))
                .build(),
        )
        var progressCalls = 0
        val downloader = BootstrapDownloader(
            context(),
            onProgress = BootstrapProgressCallback { progressCalls++ },
            client = client,
        )
        val result = downloader.download(httpsServer.url("/bootstrap.zip").toString(), "test")
        assertTrue("download failed: ${result.exceptionOrNull()}", result.isSuccess)
        val file = result.getOrNull()
        assertNotNull(file)
        assertEquals(payload.size.toLong(), requireNotNull(file).length())
        assertTrue("progress callback never invoked", progressCalls > 0)
        assertEquals(1, httpsServer.requestCount)
    }

    @Test
    fun redirectToNonHttpsRejected() = runBlocking {
        val client = httpsSetup()
        plainServer.start()
        httpsServer.start()
        plainServer.enqueue(
            MockResponse.Builder()
                .code(200)
                .body(Buffer().write(ByteArray(1_050_000) { 0x42 }))
                .build(),
        )
        httpsServer.enqueue(
            MockResponse.Builder()
                .code(302)
                .addHeader("Location", plainServer.url("/bootstrap.zip").toString())
                .build(),
        )
        val downloader = BootstrapDownloader(context(), client = client)
        val result = downloader.download(httpsServer.url("/bootstrap.zip").toString(), "test")
        assertTrue(result.isFailure)
        val message = result.exceptionOrNull()?.message.orEmpty()
        assertTrue("unexpected message: $message", message.contains("non-https"))
    }

    @Test
    fun tooSmallDownloadRejected() = runBlocking {
        val client = httpsSetup()
        httpsServer.start()
        httpsServer.enqueue(
            MockResponse.Builder()
                .code(200)
                .body(Buffer().writeUtf8("tiny"))
                .build(),
        )
        val downloader = BootstrapDownloader(context(), client = client)
        val result = downloader.download(httpsServer.url("/bootstrap.zip").toString(), "test")
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("too small") == true)
    }

    @Test
    fun httpErrorStatusRejected() = runBlocking {
        val client = httpsSetup()
        httpsServer.start()
        httpsServer.enqueue(
            MockResponse.Builder()
                .code(500)
                .body(Buffer().writeUtf8("boom"))
                .build(),
        )
        val downloader = BootstrapDownloader(context(), client = client)
        val result = downloader.download(httpsServer.url("/bootstrap.zip").toString(), "test")
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("500") == true)
    }
}
