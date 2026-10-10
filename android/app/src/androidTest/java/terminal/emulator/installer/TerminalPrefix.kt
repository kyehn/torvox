package terminal.emulator.installer

import android.content.Context
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull

/**
 * 测试前置：保证 termux prefix（`files/usr`）就位。
 *
 * 为什么每个依赖真实 shell 的用例都要自己铺：`:app:connectedAndroidTest` 开跑前会
 * 全新安装应用，应用数据随之清空，prefix 也就没了；而 prefix 只由
 * `installer.*` 的用例装回来，而类名按字典序排在 `terminal.emulator.*` 之后——
 * 于是「先跑的用例没有 shell」纯属字母序的巧合。实测本地全量套件里
 * `PasteButtonInstrumentedTest` 等 8 个用例因此报「标记未落格」，装上 prefix 后单跑即过。
 *
 * 幂等：已装好时只做一次文件存在性检查，不重复下载解压。
 */
object TerminalPrefix {
    private const val TAG = "TerminalPrefix"

    /** 安装前置的单次上限，与安装器自身的下载/解压上限同量级。 */
    private const val INSTALL_TIMEOUT_MS = 10 * 60_000L

    /**
     * 夹具刻意用一份已知可下载的归档，与产品预设指向的版本
     * （[terminal.emulator.installer.TermuxBootstrap.RELEASE]）无关：安装器测试
     * 要验证的是「装得起来」，不是「预设指向哪个版本」。
     */
    private const val OFFICIAL_URL =
        "https://github.com/termux/termux-packages/releases/download/" +
            "bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-x86_64.zip"

    fun ensureInstalled() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val prefixDir = java.io.File(context.filesDir, "usr")
        val bash = java.io.File(prefixDir, "bin/bash")
        if (bash.isFile) {
            java.io.File(context.filesDir, "home").mkdirs()
            return
        }
        val url =
            InstrumentationRegistry.getArguments().getString("test.bootstrapUrl") ?: OFFICIAL_URL
        Log.i(TAG, "installing terminal prefix from $url")
        // 有界等待：下载与安装都可能停在网络或子进程上，无上限的阻塞只会把
        // 整轮仪器化套件挂死，届时无人知道卡在哪一步。
        val result =
            runBlocking {
                withTimeoutOrNull(INSTALL_TIMEOUT_MS) {
                    BootstrapOrchestrator(
                        BootstrapDownloader(context),
                        BootstrapInstaller(prefixDir, java.io.File(context.filesDir, "home"), stagingDir(context)),
                        SecondStageRunner(prefixDir, java.io.File(context.filesDir, "home")),
                    ).ensureBootstrap(url)
                }
            }
        checkNotNull(result) { "terminal prefix install timed out after ${INSTALL_TIMEOUT_MS}ms (url=$url)" }
        check(result.isSuccess) { "terminal prefix install failed: $result" }
        check(bash.isFile) { "prefix installed but $bash is missing" }
    }

    /**
     * staging 必须与 prefix 同文件系统：原子换入的 `rename` 跨挂载会失败
     * （`cacheDir` 与 `filesDir` 在本设备并非同一挂载，已实测）。
     */
    private fun stagingDir(context: Context): java.io.File = java.io.File(context.filesDir, "bootstrap-staging")
}
