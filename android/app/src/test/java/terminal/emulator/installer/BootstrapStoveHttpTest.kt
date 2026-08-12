package terminal.emulator.installer

import com.trendyol.stove.extensions.junit.StoveJUnitExtension
import com.trendyol.stove.http.HttpClientSystemOptions
import com.trendyol.stove.http.http
import com.trendyol.stove.http.httpClient
import com.trendyol.stove.system.Stove
import com.trendyol.stove.system.abstractions.ApplicationUnderTest
import com.trendyol.stove.system.stove
import kotlinx.coroutines.runBlocking
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okio.Buffer
import org.junit.jupiter.api.AfterAll
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.BeforeAll
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

/**
 * Stove (Trendyol) end-to-end HTTP test.
 *
 * Stove is a JVM backend e2e DSL (Spring/Ktor + Testcontainers). This
 * Android app has no backend server, so we exercise Stove's HTTP client
 * system against MockWebServer — the same server the bootstrap downloader
 * hits in production — and assert the response with Stove's DSL. This
 * proves the Stove integration runs on the host JVM and keeps the HTTP
 * path honest (real socket I/O, real OkHttp engine).
 *
 * Note: requires JUnit5 (Stove's extension is Jupiter-based); JUnit4
 * tests in this module are unaffected.
 */
@ExtendWith(StoveJUnitExtension::class)
class BootstrapStoveHttpTest {

    companion object {
        private lateinit var server: MockWebServer

        @JvmStatic
        @BeforeAll
        fun setUp() {
            server = MockWebServer()
            server.enqueue(
                MockResponse.Builder()
                    .code(200)
                    .body(Buffer().write("PK\u0003\u0004bootstrap-archive-content".toByteArray()))
                    .setHeader("Content-Type", "application/zip")
                    .build(),
            )
            server.start()
            runBlocking {
                Stove()
                    .with {
                        httpClient {
                            HttpClientSystemOptions(
                                baseUrl = server.url("/").toString().trimEnd('/'),
                            )
                        }
                        applicationUnderTest(
                            object : ApplicationUnderTest<Unit> {
                                override suspend fun start(configurations: List<String>) {}
                                override suspend fun stop() {}
                            },
                        )
                    }
                    .run()
            }
        }

        @JvmStatic
        @AfterAll
        fun tearDown() {
            runBlocking { Stove.stop() }
            server.close()
        }
    }

    @Test
    fun `bootstrap payload is served with 200 over Stove http client`() = runBlocking {
        stove {
            http {
                getBodilessResponse("/bootstrap.zip") { response ->
                    assertEquals(200, response.status)
                }
            }
        }
    }
}
