package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.Drawable
import android.util.TypedValue
import android.view.ContextThemeWrapper
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
 * 启动屏图标的判据（STATUS ⑥「启动动画图案不完整，被限制到图标形状」）。
 *
 * 平台的 [SplashscreenContentDrawer] 只在拿到自适应图标时才沿用它自己的蒙版
 * （See `SplashViewBuilder.processAdaptiveIcon` / `createIconDrawable`）：给
 * `windowSplashScreenAnimatedIcon` 一份普通图形时，平台会套
 * `AdaptiveForegroundDrawable`——把图形缩进内圈 72×72（108 视口），再按圆形蒙版裁；
 * 又因为主题没有声明 `windowSplashScreenIconBackgroundColor`，整块图标区会被
 * `splash_icon_no_background_scale_factor`（1.5）放大，图案的观感尺寸与应用图标一致。
 * 因此「图案完整」的充分必要条件是：**图形是普通图形，且它的图案在内圈里仍落进
 * 半径 36 的可见圆内**。
 *
 * 历史坑：曾把 `windowSplashScreenAnimatedIcon` 指向自适应图标 @mipmap/ic_launcher，
 * 图案沿桌面蒙版被裁成图标形状（用户报「图案不完整」）；也曾为了绕开而把
 * `launcher_foreground` 整体缩到 0.62，结果桌面图标一起变小变样（用户报
 * 「应用图标被意外更改」）。故本测试钉住三件事：启动屏图案是普通图形、图案完整
 * 落进可见圆、且桌面图标的前景保持原始大小（半径 50.3 单位）。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SplashIconRobolectricTest {

    @Test
    fun splashIconIsAPlainDrawableWhoseArtworkFitsTheVisibleCircle() {
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
        assertFalse(
            "启动屏图标不得是自适应图标：平台会沿用桌面蒙版把图案裁成图标形状" +
                "（STATUS「启动动画图案不完整」）。实际 ${icon.javaClass.simpleName}",
            icon is AdaptiveIconDrawable,
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

        // 平台把普通图形缩进内圈 72/108，再因为「没有图标底板」把整块图标区放大
        // NO_BACKGROUND_SCALE 倍。图案要完整，就得在**图标区**里不越出蒙版半径
        // （圆内切于方形图标区，半径 = 图标区半宽），同时不要比桌面图标的可见内容还小。
        val full = render(icon, VIEWPORT_UNITS)
        val iconAreaFraction = full.maxRadius / VIEWPORT_UNITS *
            (ICON_INNER_UNITS.toFloat() / VIEWPORT_UNITS) * NO_BACKGROUND_SCALE
        assertTrue(
            "启动屏图案不得越出图标区（蒙版半径 0.5），实际占图标区 " +
                "${"%.3f".format(iconAreaFraction)}，包围盒 ${full.bounds}",
            iconAreaFraction <= 0.5f,
        )
        assertTrue(
            "启动屏图案不得小于桌面图标的可见内容（${VISIBLE_CIRCLE_RADIUS}/${VIEWPORT_UNITS}），" +
                "实际占图标区 ${"%.3f".format(iconAreaFraction)}",
            iconAreaFraction >= VISIBLE_CIRCLE_RADIUS / VIEWPORT_UNITS,
        )
        assertTrue(
            "启动屏图案必须是可绘制图案，实际不透明像素 ${full.opaquePixels}",
            full.opaquePixels > OPAQUE_PIXELS_FLOOR,
        )

        // 启动屏画的必须是与桌面图标同一份图形（不得另存缩小版，否则两处观感不一致）。
        val appIcon = themed.resources.getDrawable(R.drawable.launcher_foreground, themed.theme)
        val launcherFull = render(appIcon, VIEWPORT_UNITS)
        assertTrue(
            "启动屏图案必须与桌面图标的前景同一份图形（bounds=${VIEWPORT_UNITS}u）",
            full.pixels.contentEquals(launcherFull.pixels),
        )

        // 桌面图标的前景本身不得被调小：原始图案最远半径实测 50.3 单位。
        assertTrue(
            "应用图标前景不得被调小（原始半径约 $ARTWORK_RADIUS_FLOOR 单位以上），" +
                "实际最远 ${launcherFull.maxRadius}，包围盒 ${launcherFull.bounds}",
            launcherFull.maxRadius >= ARTWORK_RADIUS_FLOOR,
        )
        assertTrue(
            "应用图标前景不应远超画布，实际最远 ${launcherFull.maxRadius} 单位",
            launcherFull.maxRadius <= ARTWORK_RADIUS_CEILING,
        )
    }

    private class Rendered(val maxRadius: Float, val opaquePixels: Int, val bounds: String, val pixels: IntArray)

    /** 把 drawable 画进 `boundsUnits`×`boundsUnits`（108 视口单位）的位图并测量图案。 */
    private fun render(drawable: Drawable, boundsUnits: Int): Rendered {
        val px = (boundsUnits * UNITS_TO_PX).toInt()
        val bitmap = Bitmap.createBitmap(px, px, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, px, px)
        drawable.draw(Canvas(bitmap))

        val pixels = IntArray(px * px)
        bitmap.getPixels(pixels, 0, px, 0, 0, px, px)
        var maxRadius = 0f
        var opaque = 0
        var minU = Float.MAX_VALUE
        var minV = Float.MAX_VALUE
        var maxU = -Float.MAX_VALUE
        var maxV = -Float.MAX_VALUE
        val centre = boundsUnits / 2f
        for (y in 0 until px) {
            for (x in 0 until px) {
                if (pixels[y * px + x] ushr 24 == 0) continue
                opaque++
                val u = (x + 0.5f) / UNITS_TO_PX
                val v = (y + 0.5f) / UNITS_TO_PX
                minU = min(minU, u)
                maxU = max(maxU, u)
                minV = min(minV, v)
                maxV = max(maxV, v)
                maxRadius = max(maxRadius, hypot(u - centre, v - centre))
            }
        }
        return Rendered(
            "%.1f".format(maxRadius).toFloat(),
            opaque,
            "[%.1f,%.1f]×[%.1f,%.1f]".format(minU, maxU, minV, maxV),
            pixels,
        )
    }

    private companion object {
        /** 108 单位视口按 4 像素/单位渲染，足够分辨抗锯齿边缘。 */
        const val UNITS_TO_PX = 4f

        /** 自适应图标视口。 */
        const val VIEWPORT_UNITS = 108

        /** 平台给普通启动屏图形用的内圈（108 视口里占 72）。 */
        const val ICON_INNER_UNITS = 72

        /** 内圈对应的圆形可见区半径：72 / 2。 */
        const val VISIBLE_CIRCLE_RADIUS = 36f

        /** 没声明 windowSplashScreenIconBackgroundColor 时平台对图标区的放大倍数。 */
        const val NO_BACKGROUND_SCALE = 1.5f

        /** 原始图案最远半径实测 50.3 单位（含描边）；低于 40 即为「被调小」。 */
        const val ARTWORK_RADIUS_FLOOR = 40f

        /** 画布对角线的一半约 76 单位；超过 60 说明图形被放大到越界。 */
        const val ARTWORK_RADIUS_CEILING = 60f

        /** 图案至少覆盖位图的 1%，用于区分「有图案」与「整块空白」。 */
        const val OPAQUE_PIXELS_FLOOR = ICON_INNER_UNITS * ICON_INNER_UNITS / 100
    }
}
