package terminal.emulator

import android.content.res.Configuration
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 配置变化的重推判据：夜间模式、系统「字体大小」与显示密度任一变化都要重推渲染契约。
 *
 * manifest 已声明这些限定符，Activity 不重建而运行期是 @Singleton 存活下来，
 * 不重推就停在旧系数上：Kotlin 按新系数算字号与网格、原生按旧值光栅，字形被拉伸、
 * 度量与网格整体偏掉。显示密度此前不在判据里，是同一类漂移。
 */
@RunWith(RobolectricTestRunner::class)
class ConfigurationChangeContractTest {

    private fun configuration(
        nightMode: Int = Configuration.UI_MODE_NIGHT_NO,
        fontScale: Float = 1f,
        densityDpi: Int = 420,
    ): Configuration = Configuration().apply {
        uiMode = nightMode
        this.fontScale = fontScale
        this.densityDpi = densityDpi
    }

    @Test
    fun `density change requires a new render contract`() {
        assertTrue(
            "显示密度变了必须重推渲染契约",
            shouldApplySettingsOnConfigurationChange(
                previous = configuration(),
                current = configuration(densityDpi = 560),
            ),
        )
    }

    @Test
    fun `font scale change requires a new render contract`() {
        assertTrue(
            "系统字体大小变了必须重推渲染契约",
            shouldApplySettingsOnConfigurationChange(
                previous = configuration(),
                current = configuration(fontScale = 1.3f),
            ),
        )
    }

    @Test
    fun `night mode change requires a new render contract`() {
        assertTrue(
            "夜间模式变了必须重推渲染契约",
            shouldApplySettingsOnConfigurationChange(
                previous = configuration(),
                current = configuration(nightMode = Configuration.UI_MODE_NIGHT_YES),
            ),
        )
    }

    @Test
    fun `unrelated configuration change does not require a new render contract`() {
        val current =
            configuration().apply {
                orientation = Configuration.ORIENTATION_LANDSCAPE
            }
        assertFalse(
            "与渲染契约无关的限定符变化不得触发重推",
            shouldApplySettingsOnConfigurationChange(
                previous = configuration(),
                current = current,
            ),
        )
    }
}
