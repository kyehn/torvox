package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.Drawable
import android.util.TypedValue
import android.view.ContextThemeWrapper
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertNotNull
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
 * 启动屏图标的判据（STATUS ⑪「启动动画使用图标而不是完整图案」+「应用图标被意外更改」）。
 *
 * 平台对 `windowSplashScreenAnimatedIcon` 的处理：图标绘在
 * `windowSplashScreenBackground` 上，圆形可见区只有 108 视口中心半径 36 单位
 * （=72×72 安全区）；自适应图标只取其前景层，超出的部分与桌面上一样被蒙版裁掉。
 * 图标后是否再垫一层圆形底板，只取决于主题有没有声明
 * `windowSplashScreenIconBackgroundColor`——与 `windowSplashScreenAnimatedIcon`
 * 指到哪个 drawable 无关。
 *
 * 历史坑：曾把「图标带底板」误判成自适应图标所致，先改父主题、又把
 * `launcher_foreground` 整体缩到 0.62，结果桌面图标一起变小变样（用户报
 * 「应用图标被意外更改」）。故本测试钉住两件事：启动屏用的就是应用图标本身
 * （自适应图标，不得另存一份缩小的图形），且它的前景图案保持原始大小
 * （半径 50.3 单位），不得被调小。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SplashIconRobolectricTest {

    @Test
    fun splashIconIsTheAppIconItselfAndItsArtworkIsNotShrunk() {
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
        val icon = requireNotNull(themed.getDrawable(value.resourceId)) {
            "windowSplashScreenAnimatedIcon 指向的 drawable 必须存在"
        }
        val adaptive = icon as? AdaptiveIconDrawable
        assertNotNull(
            "启动屏必须直接用应用图标（mipmap/ic_launcher）：另做一份缩小图形会让" +
                "启动屏与应用图标各画一遍同一图案。实际 ${icon.javaClass.simpleName}",
            adaptive,
        )
        // 图标后垫不垫圆形底板（STATUS ⑪ 的「完整图案」）只由这个属性的透明度决定：
        // Theme.SplashScreen 解析出的底色是全透明，Theme.SplashScreen.IconBackground
        // 才给不透明底色。父主题换成后者、或本主题自己声明一个颜色，都会让底板回来。
        val iconBackground = TypedValue()
        themed.theme.resolveAttribute(
            android.R.attr.windowSplashScreenIconBackgroundColor,
            iconBackground,
            true,
        )
        val backgroundAlpha = when {
            iconBackground.type in TypedValue.TYPE_FIRST_COLOR_INT..TypedValue.TYPE_LAST_COLOR_INT ->
                iconBackground.data ushr 24

            iconBackground.type == TypedValue.TYPE_REFERENCE ->
                themed.resources.getColor(iconBackground.resourceId, themed.theme) ushr 24

            else -> 0
        }
        assertTrue(
            "图标后不得垫圆形底板（STATUS ⑪「完整图案」），" +
                "实际底色 type=${iconBackground.type} alpha=$backgroundAlpha",
            backgroundAlpha == 0,
        )

        // 只画前景层：平台对自适应图标取的就是这一层。
        val rendered = render(requireNotNull(adaptive).foreground)
        assertTrue(
            "图标必须是可绘制图案，实际不透明像素 ${rendered.opaquePixels}",
            rendered.opaquePixels > OPAQUE_PIXELS_FLOOR,
        )
        assertTrue(
            "图案不得被调小（原始半径约 $ARTWORK_RADIUS_FLOOR 单位以上），" +
                "实际最远 ${rendered.maxRadius}，包围盒 ${rendered.bounds}",
            rendered.maxRadius >= ARTWORK_RADIUS_FLOOR,
        )
        assertTrue(
            "图案不应远超画布，实际最远 ${rendered.maxRadius} 单位",
            rendered.maxRadius <= ARTWORK_RADIUS_CEILING,
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

        /** 原始图案最远半径实测 50.3 单位（含描边）；低于 40 即为「被调小」。 */
        const val ARTWORK_RADIUS_FLOOR = 40f

        /** 画布对角线的一半约 76 单位；超过 60 说明图形被放大到越界。 */
        const val ARTWORK_RADIUS_CEILING = 60f

        /** 图案至少覆盖位图的 1%，用于区分「有图案」与「整块空白」。 */
        const val OPAQUE_PIXELS_FLOOR = BITMAP_PX * BITMAP_PX / 100
    }
}
