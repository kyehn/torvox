package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.Drawable
import android.util.TypedValue
import android.view.ContextThemeWrapper
import androidx.core.content.ContextCompat
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min

/**
 * STATUS ⑪「启动动画使用图标而不是完整图案」的判据。
 *
 * 系统（AOSP `SplashscreenContentDrawer`）对 `windowSplashScreenAnimatedIcon` 的处理：
 * 图标按 `starting_surface_icon_size`(160dp) × 无底色放大系数 1.2 = 192dp 绘制，
 * 非自适应图标再由 `AdaptiveForegroundDrawable` 放大 1.5 倍并裁到圆内，
 * 因此**可见区只有 108 视口中心半径 36 单位（=72×72 安全区）的圆**。
 * 若把自适应应用图标直接交给它，平台会连同底板整块画出（完整图案）且四周被裁；
 * 交给非自适应的前景图形，平台才会把图标画在启动屏底色上。
 *
 * 故一条断言钉住两种情况：解析出的图标不是自适应图标，且渲染出的图案既非空白
 * 又不超出可见圆（超出即被裁切），同时确实铺满可见区（不得缩成一小块）。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SplashIconRobolectricTest {

    @Test
    fun splashIconIsAMarkOnTheSplashBackgroundThatFitsTheVisibleCircle() {
        val themed = ContextThemeWrapper(
            ApplicationProvider.getApplicationContext<Context>(),
            R.style.Theme_Terminal_Splash,
        )
        val value = TypedValue()
        assertTrue(
            "启动屏主题必须能解析出 windowSplashScreenAnimatedIcon",
            themed.theme.resolveAttribute(
                android.R.attr.windowSplashScreenAnimatedIcon,
                value,
                true,
            ),
        )
        val icon = requireNotNull(ContextCompat.getDrawable(themed, value.resourceId)) {
            "windowSplashScreenAnimatedIcon 指向的 drawable 必须存在"
        }
        assertFalse(
            "启动屏必须只画图标本身；交给平台自适应图标会把底板一起画成完整图案",
            icon is AdaptiveIconDrawable,
        )

        val rendered = render(icon)
        assertTrue(
            "图标必须是可绘制图案，实际不透明像素 ${rendered.opaquePixels}",
            rendered.opaquePixels > OPAQUE_PIXELS_FLOOR,
        )
        assertTrue(
            "图标不得超出可见圆（半径 $VISIBLE_RADIUS 单位），实际最远 ${rendered.maxRadius}，" +
                "包围盒 ${rendered.bounds}",
            rendered.maxRadius <= VISIBLE_RADIUS,
        )
        assertTrue(
            "图标应铺满可见区，实际最远只有 ${rendered.maxRadius} 单位",
            rendered.maxRadius >= VISIBLE_RADIUS - 6f,
        )
    }

    private class Rendered(val maxRadius: Float, val opaquePixels: Int, val bounds: String)

    private fun render(drawable: Drawable): Rendered {
        val bitmap = Bitmap.createBitmap(BITMAP_PX, BITMAP_PX, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, BITMAP_PX, BITMAP_PX)
        drawable.draw(Canvas(bitmap))

        val pixels = IntArray(BITMAP_PX * BITMAP_PX)
        bitmap.getPixels(pixels, 0, BITMAP_PX, 0, 0, BITMAP_PX, BITMAP_PX)
        var maxRadius = 0f
        var opaque = 0
        var minU = Float.MAX_VALUE
        var minV = Float.MAX_VALUE
        var maxU = -Float.MAX_VALUE
        var maxV = -Float.MAX_VALUE
        for (y in 0 until BITMAP_PX) {
            for (x in 0 until BITMAP_PX) {
                if (pixels[y * BITMAP_PX + x] ushr 24 == 0) continue
                opaque++
                val u = (x + 0.5f) / UNITS_TO_PX
                val v = (y + 0.5f) / UNITS_TO_PX
                minU = min(minU, u)
                maxU = max(maxU, u)
                minV = min(minV, v)
                maxV = max(maxV, v)
                maxRadius = max(maxRadius, hypot(u - CENTRE, v - CENTRE))
            }
        }
        return Rendered(
            "%.1f".format(maxRadius).toFloat(),
            opaque,
            "[%.1f,%.1f]×[%.1f,%.1f]".format(minU, maxU, minV, maxV),
        )
    }

    private companion object {
        /** 108 单位视口按 4 像素/单位渲染，足够分辨抗锯齿边缘。 */
        const val UNITS_TO_PX = 4f
        const val BITMAP_PX = 432
        const val CENTRE = 54f

        /** 192dp 图标 ÷ 1.5 倍前景放大 = 可见圆直径 72 单位，即半径 36。 */
        const val VISIBLE_RADIUS = 36f

        /** 图案至少覆盖位图的 1%，用于区分「有图案」与「整块空白」。 */
        const val OPAQUE_PIXELS_FLOOR = BITMAP_PX * BITMAP_PX / 100
    }
}
