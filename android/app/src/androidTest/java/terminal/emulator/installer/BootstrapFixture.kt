package terminal.emulator.installer

import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.Assert
import java.io.File

private const val TAG = "BootstrapFixture"

/**
 * 官方 x86_64 引导包。模拟器无外网时，CI 先 `adb push` 官方 zip 再经
 * `-e test.bootstrapUrl file:///...` 传入；未传参则直连官方发布地址。
 */
internal const val DEFAULT_BOOTSTRAP_URL =
    "https://github.com/termux/termux-packages/releases/download/" +
        "bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-x86_64.zip"

/**
 * 让仪器测试拿到真实可执行的 prefix：已装则复用，未装则走产品同一条
 * [BootstrapOrchestrator] 路径下载安装。
 *
 * 需要真实 shell 的用例（包管理器、预览截图）都经此入口，避免各自复制安装
 * 流程——`prefix/home/staging` 三者必须同挂载的约束只在这里维护一次。
 */
internal fun ensureBootstrapInstalled(): File {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val bash = File(context.filesDir, "usr/bin/bash")
    val home = File(context.filesDir, "home")
    if (bash.exists()) {
        home.mkdirs()
        return bash
    }
    val url =
        InstrumentationRegistry.getArguments().getString("test.bootstrapUrl")
            ?: DEFAULT_BOOTSTRAP_URL
    Log.i(TAG, "installing bootstrap from $url")
    // 路径必须经 context 解析：/data/data 与 /data/user/0 在此设备上并非同一
    // 挂载，硬编码前者会让 staging→prefix 的原子换入跨分区失败。
    val result =
        runBlocking {
            BootstrapOrchestrator(
                BootstrapDownloader(context),
                BootstrapInstaller(
                    File(context.filesDir, "usr"),
                    home,
                    File(context.filesDir, "bootstrap-staging"),
                ),
                SecondStageRunner(File(context.filesDir, "usr"), home),
            ).ensureBootstrap(url)
        }
    Log.i(TAG, "bootstrap result: $result")
    Assert.assertTrue("bootstrap failed: $result", result.isSuccess)
    Assert.assertTrue("bash not found after bootstrap", bash.exists())
    home.mkdirs()
    return bash
}
