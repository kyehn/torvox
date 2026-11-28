package terminal.emulator.benchmark

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
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
        private const val OPEN_SETTINGS_EXTRA = "terminal.emulator.open_settings"

        /** 设置页的 LazyColumn 与引导分区资源 id（app 以 testTagsAsResourceId 暴露）。 */
        private const val SETTINGS_LIST_ID = "SettingsLazyColumn"
        private const val NIX_PRESET_ID = "BootstrapPreset_NixOnDroid"
        private const val INSTALL_BUTTON_ID = "BootstrapInstallButton"
    }

    @Test
    fun nixVariantInstallsNixOnDroidBootstrap() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val installed = device.executeShellCommand("pm path $NIX_PACKAGE")
        assertTrue(
            "Nix 变种未安装（$NIX_PACKAGE），本用例必须在装了它的设备上跑：$installed",
            installed.isNotBlank(),
        )
        // Nix 变种是 release 构建：MainActivity 只在 debug 构建接受引导安装 extra，
        // 故这里走设置页的真实入口——点预设填入官方地址，再点安装。
        device.executeShellCommand("am force-stop $NIX_PACKAGE")
        // 启动时 MainActivity 会请求通知权限，系统弹窗会盖住设置页。
        device.executeShellCommand("pm grant $NIX_PACKAGE android.permission.POST_NOTIFICATIONS")
        val sinceMillis = System.currentTimeMillis()
        device.executeShellCommand(
            "am start -n $NIX_PACKAGE/$NIX_ACTIVITY" +
                " --es $LOG_LEVEL_EXTRA DEBUG" +
                " --ez $OPEN_SETTINGS_EXTRA true",
        )
        val settings =
            device.wait(Until.findObject(By.res(SETTINGS_LIST_ID)), 30_000)
                ?: device.wait(Until.findObject(By.scrollable(true)), 5_000)
                ?: error("设置页未打开")
        scrollTo(device, settings, By.res(NIX_PRESET_ID)).click()
        scrollTo(device, settings, By.res(INSTALL_BUTTON_ID)).click()

        val outcomes = awaitOutcome(device, sinceMillis)
        assertTrue(
            "引导包安装未成功（结论行缺失或为失败）：$outcomes",
            outcomes.any { it.message.contains(OUTCOME_SUCCESS) },
        )
    }

    /** 在设置页里向下滚动直到目标节点出现。 */
    private fun scrollTo(device: UiDevice, list: UiObject2, selector: BySelector): UiObject2 {
        repeat(12) {
            device.wait(Until.findObject(selector), 500)?.let { return it }
            list.scroll(Direction.DOWN, 0.8f)
        }
        return device.wait(Until.findObject(selector), 5_000)
            ?: error("设置页中找不到 $selector")
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
}
