package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.drawable.AdaptiveIconDrawable
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.math.abs
import kotlin.math.hypot

/**
 * 桌面自适应图标必须把整幅图案留在平台蒙版的安全区内。
 *
 * 平台几何（`AdaptiveIconDrawable`）：图层区域是图标边界的 1.5 倍且居中，
 * 蒙版圆半径是图标边界的一半，安全区再乘 `66/72`。前景里的图案按图层区域
 * 摆放时，一单位图案 = 1.5×边界/108；图案墨迹半径 48.5 单位即 0.674×边界，
 * 超出 0.5×边界的蒙版圆——星形因此被裁掉，只剩终端窗口的一角。
 *
 * 判据落在前景层自身：把它按图层区域（1.5×图标边长）渲染，墨迹半径占图层区域
 * 的比例必须不超过安全区占图层区域的比例。修复前该比例是 0.449（越界），
 * 修复后是 0.282（图案已按 0.65 缩进安全区）。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w360dp-h640dp-160dpi")
class LauncherAdaptiveIconRobolectricTest {

    @Test
    fun `adaptive icon keeps the whole artwork inside the safe zone`() {
        for (iconSizeDp in intArrayOf(LAUNCHER_ICON_SIZE_DP, CANONICAL_ICON_SIZE_DP, LARGE_ICON_SIZE_DP)) {
            val ratio = foregroundInkRatio(iconSizeDp)
            assertTrue(
                "图标 ${iconSizeDp}dp 下图案墨迹占图层区域 $ratio，必须不超过安全区 $SAFE_ZONE_RATIO，" +
                    "否则会被平台蒙版裁掉",
                ratio <= SAFE_ZONE_RATIO,
            )
        }
    }

    /**
     * 图案里最大的星形必须完整画出来。星形很细，只在大尺寸下才有足够的
     * 纯色像素可供判别。
     */
    @Test
    fun `the star of the artwork survives the icon mask`() {
        val foreground = requireNotNull(adaptiveIcon().foreground) { "自适应图标缺少前景层" }
        val size = (LARGE_ICON_SIZE_DP * LAYER_REGION_SCALE).toInt()
        val orange = countOrange(foreground, size)
        assertTrue(
            "橙色星形必须完整出现在前景层里（修复前整颗星被裁掉），实际纯色像素 $orange",
            orange > 0,
        )
    }

    /**
     * 单色层（themed icon）走同一套蒙版几何，图案同样必须留在安全区内。
     */
    @Test
    fun `the monochrome layer also stays inside the safe zone`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val monochrome = requireNotNull(context.resources.getDrawable(R.drawable.launcher_monochrome, null)) {
            "launcher_monochrome 资源缺失"
        }
        val layerRegion = (CANONICAL_ICON_SIZE_DP * LAYER_REGION_SCALE).toInt()
        val ratio = inkRadius(monochrome, layerRegion) / layerRegion
        assertTrue(
            "单色层墨迹占图层区域 $ratio，必须不超过安全区 $SAFE_ZONE_RATIO",
            ratio <= SAFE_ZONE_RATIO,
        )
    }

    private fun adaptiveIcon(): AdaptiveIconDrawable {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val drawable = requireNotNull(context.resources.getDrawable(R.mipmap.ic_launcher, null)) {
            "ic_launcher 资源缺失"
        }
        return drawable as? AdaptiveIconDrawable
            ?: error("ic_launcher 必须是自适应图标，实际 ${drawable.javaClass.simpleName}")
    }

    /** 前景层按图层区域渲染后，墨迹半径占图层区域边长的比例。 */
    private fun foregroundInkRatio(iconSizeDp: Int): Float {
        val foreground = requireNotNull(adaptiveIcon().foreground) { "自适应图标缺少前景层" }
        val layerRegion = (iconSizeDp * LAYER_REGION_SCALE).toInt()
        return inkRadius(foreground, layerRegion) / layerRegion
    }

    /** 最远非透明像素到画布中心的距离。Robolectric 160dpi 下 1px = 1dp。 */
    private fun inkRadius(drawable: android.graphics.drawable.Drawable, size: Int): Float {
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, size, size)
        drawable.draw(Canvas(bitmap))
        val centerX = (size - 1) / 2f
        val centerY = (size - 1) / 2f
        var maxRadius = 0f
        for (y in 0 until bitmap.height) {
            for (x in 0 until bitmap.width) {
                if ((bitmap.getPixel(x, y) ushr 24) < ALPHA_FLOOR) continue
                val distance = hypot(x + 0.5f - centerX, y + 0.5f - centerY)
                if (distance > maxRadius) maxRadius = distance
            }
        }
        return maxRadius
    }

    private fun countOrange(drawable: android.graphics.drawable.Drawable, size: Int): Int {
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, size, size)
        drawable.draw(Canvas(bitmap))
        var count = 0
        for (y in 0 until bitmap.height) {
            for (x in 0 until bitmap.width) {
                val pixel = bitmap.getPixel(x, y)
                if ((pixel ushr 24) < ALPHA_FLOOR) continue
                if (
                    abs(Color.red(pixel) - Color.red(STAR_FILL)) <= COLOR_FUZZ &&
                    abs(Color.green(pixel) - Color.green(STAR_FILL)) <= COLOR_FUZZ &&
                    abs(Color.blue(pixel) - Color.blue(STAR_FILL)) <= COLOR_FUZZ
                ) {
                    count++
                }
            }
        }
        return count
    }

    private companion object {
        /** 启动器图标槽的实际边长（dp）。 */
        const val LAUNCHER_ICON_SIZE_DP = 48

        /** 自适应图标的标称边长（dp）。 */
        const val CANONICAL_ICON_SIZE_DP = 108

        /** 放大到足以判别细笔画的边长（dp）。 */
        const val LARGE_ICON_SIZE_DP = 216

        /** 平台把图层区域撑到图标边界的倍数。 */
        const val LAYER_REGION_SCALE = 1.5f

        /** 安全区占图标边界的比例：蒙版半径的 `66/72`。 */
        const val SAFE_ZONE_RATIO = 66f / 72f / LAYER_REGION_SCALE

        /** 星形的填充色（logo_art 里的橙）。 */
        val STAR_FILL = 0xffb467.toInt()

        /** 颜色匹配余量（抗锯齿边缘会混色）。 */
        const val COLOR_FUZZ = 24

        /** 非透明像素门槛。 */
        const val ALPHA_FLOOR = 0x70
    }
}
