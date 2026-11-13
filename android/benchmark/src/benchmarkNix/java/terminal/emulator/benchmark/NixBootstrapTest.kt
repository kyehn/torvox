package terminal.emulator.benchmark

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Nix 变种的引导包安装：装的是 Nix-on-Droid 引导包，不是 Termux 官方包。
 *
 * 两套引导包的包模型不兼容（apt 与 nix），混用即坏。故 Nix 变种必须能装上
 * 自己那套——这条路径此前没有任何验证：Nix 变种是 release 构建，而仪器测试默认
 * 只跑 debug 变种，装不上也无人知道。
 *
 * 跑法：`:benchmark:connectedBenchmarkNixAndroidTest`（被测包 com.termux.nix）。
 */
@RunWith(AndroidJUnit4::class)
class NixBootstrapTest {
    companion object {
        /** Nix 变种的包名：与 `android/app/build.gradle.kts` 的 `applicationIdSuffix` 一致。 */
        private const val NIX_PACKAGE = "com.termux.nix"

        /** 被测 Activity 的类名：applicationId 是 `com.termux.nix`，类名不在该包下。 */
        private const val NIX_ACTIVITY = "terminal.emulator.MainActivity"

        /** 安装超时：模拟器上 39MB 的下载与解压要几分钟。 */
        private const val INSTALL_TIMEOUT_MILLIS = 15 * 60 * 1000L

        /** 轮询间隔。 */
        private const val POLL_INTERVAL_MILLIS = 2_000L

        /**
         * 结论行的匹配：正文形如 `bootstrap outcome: success`。
         *
         * 空白用 `.` 代替——设备侧过滤词不经 shell，含空白会被按空白切词。
         * 只匹配「有结论」这一事实，成功与否在本地按正文判定（失败也是结论）。
         */
        private val OUTCOME_PATTERN = Regex("""bootstrap.outcome:""")

        /** 成功结论的正文。 */
        private const val OUTCOME_SUCCESS = "bootstrap outcome: success"

        /** 与 app 模块的 `MainActivity.EXTRA_*` 一致；基准模块不依赖 app 模块的内部，故本地声明。 */
        private const val LOG_LEVEL_EXTRA = "terminal.emulator.log_level"
        private const val BOOTSTRAP_URL_EXTRA = "terminal.emulator.bootstrap_url"

        /** Nix-on-Droid 引导包地址（与 app 模块的预设同源）。 */
        private const val NIX_BOOTSTRAP_BASE =
            "https://nix-on-droid.unboiled.info/bootstrap-release-24.05"
    }

    @Test
    fun nixVariantInstallsNixOnDroidBootstrap() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val installed = device.executeShellCommand("pm path $NIX_PACKAGE")
        assertTrue(
            "Nix 变种未安装（$NIX_PACKAGE），本用例必须在装了它的设备上跑：$installed",
            installed.isNotBlank(),
        )
        val url = "$NIX_BOOTSTRAP_BASE/bootstrap-${deviceArchitecture()}.zip"
        val sinceMillis = System.currentTimeMillis()

        // 启动被测应用并带上两个 extra：日志等级提到 debug（结论行是 info 级），
        // 以及官方引导包地址（应用只认本变种的官方地址）。
        device.executeShellCommand(
            "am start -n $NIX_PACKAGE/$NIX_ACTIVITY" +
                " --es $LOG_LEVEL_EXTRA DEBUG" +
                " --es $BOOTSTRAP_URL_EXTRA $url",
        )

        val outcomes = awaitOutcome(device, sinceMillis)
        assertTrue(
            "引导包安装未成功（结论行缺失或为失败）：$outcomes",
            outcomes.any { it.message.contains(OUTCOME_SUCCESS) },
        )
    }

    /** 轮询 logcat 直到出现安装结论行（成功或失败都算有结论）。 */
    private fun awaitOutcome(device: UiDevice, sinceMillis: Long): List<LogcatRecord> {
        val deadline = System.currentTimeMillis() + INSTALL_TIMEOUT_MILLIS
        var records = readLogcatSince(device, sinceMillis, OUTCOME_PATTERN)
        while (records.isEmpty() && System.currentTimeMillis() < deadline) {
            Thread.sleep(POLL_INTERVAL_MILLIS)
            records = readLogcatSince(device, sinceMillis, OUTCOME_PATTERN)
        }
        return records
    }

    /** 设备架构（引导包按架构分文件）：与 app 模块的 `detectDeviceArchitecture` 同口径。 */
    private fun deviceArchitecture(): String = when (android.os.Build.SUPPORTED_ABIS.firstOrNull()) {
        "arm64-v8a" -> "aarch64"
        "x86_64" -> "x86_64"
        else -> error("不支持的 ABI：${android.os.Build.SUPPORTED_ABIS.firstOrNull()}")
    }
}
