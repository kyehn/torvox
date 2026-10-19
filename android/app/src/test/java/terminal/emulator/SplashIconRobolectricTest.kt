package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.Drawable
import android.graphics.drawable.LayerDrawable
import android.util.TypedValue
import android.view.ContextThemeWrapper
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.math.abs
import kotlin.math.hypot

/**
 * 启动屏图标的行为判据（STATUS ⑥「启动动画图案不完整，被限制到图标形状」）：主题解析出的
 * 启动屏 drawable 只能是普通图形；包装层墨迹必须完整落进平台蒙版且足够醒目
 * （详见 openspec/specs/render-stability 的「启动屏图标为单一图案源的收缩包装」）。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w360dp-h640dp-160dpi")
class SplashIconRobolectricTest {

    @Test
    fun splashIconWrapsSharedArtworkInsideMaskCircle() {
        val themed = themedContext()

        val splashValue = TypedValue()
        assertTrue(
            "启动屏主题必须能解析出 windowSplashScreenAnimatedIcon",
            themed.theme.resolveAttribute(
                android.R.attr.windowSplashScreenAnimatedIcon,
                splashValue,
                true,
            ),
        )
        assertEquals(
            "启动屏图标必须就是 splash_icon 资源本身",
            R.drawable.splash_icon,
            splashValue.resourceId,
        )
        val splashIcon = requireNotNull(themed.getDrawable(splashValue.resourceId)) {
            "windowSplashScreenAnimatedIcon 指向的 drawable 必须存在"
        }
        assertFalse(
            "启动屏图标不得是自适应图标：平台会沿用桌面蒙版把图案裁成图标形状。" +
                "实际 ${splashIcon.javaClass.simpleName}",
            splashIcon is AdaptiveIconDrawable,
        )

        // 图标后不得垫不透明的圆形底板（STATUS ⑪「完整图案」）。父主题
        // Theme.SplashScreen 解析出全透明底色；一旦换成
        // Theme.SplashScreen.IconBackground 或本主题自声明颜色，底板就回来了。
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
            "图标后不得垫圆形底板，实际底色 type=${iconBackground.type} alpha=$backgroundAlpha",
            backgroundAlpha == 0,
        )

        // 结构：两处 wrapper 都必须单层引用 logo_art（保证图案只此一份）。
        val splashChild = (splashIcon as LayerDrawable).getDrawable(0)
            ?: error("splash_icon 的 layer item 必须引用 logo_art")
        val launcherIcon = requireNotNull(
            themed.resources.getDrawable(R.drawable.launcher_foreground, themed.theme),
        )
        val launcherChild = (launcherIcon as LayerDrawable).getDrawable(0)
            ?: error("launcher_foreground 的 layer item 必须引用 logo_art")

        // 同一份图案：两个 wrapper 的子层都在自身固有尺寸（108dp=108px，density 1）
        // 的画布上渲染，像素完全一致，否则就是两份副本漂移了。
        val artPx = requireNotNull(splashChild.intrinsicWidth) {
            "splash_icon 的 item 必须带 android:width/height 固定尺寸"
        }
        val splashChildBits = renderBits(splashChild, artPx)
        val launcherChildBits = renderBits(launcherChild, artPx)
        assertTrue(
            "启动屏与桌面引用的必须是同一份 logo_art 像素（" +
                "splash=${splashChildBits.size} launcher=${launcherChildBits.size}）",
            splashChildBits.contentEquals(launcherChildBits),
        )

        // 桌面 1:1：前景图案墨迹半径应 = logo_art 原始 ≈47.6~50.3 单位（= dp）。
        val launcherInk = inkRadius(renderBitmap(launcherChild, artPx))
        assertTrue(
            "桌面图案必须是原始 1:1 大小（墨迹半径应落在 $ARTWORK_MIN.." +
                "$ARTWORK_MAX dp），实际 ${"%.1f".format(launcherInk)}dp",
            launcherInk in ARTWORK_MIN..ARTWORK_MAX,
        )

        // 平台管线几何：启动屏把这一 wrapper 画进 192dp 单元并按 88.9dp
        // 蒙版裁剪；96dp 包装下图案墨迹必须完整落进蒙版圆（尺寸上不封顶地
        // 接近蒙版，说明收缩包装没生效——旧实现就在 0.95 上被裁）。
        val cellPx = SPLASH_CELL_DP
        val cell = Bitmap.createBitmap(cellPx, cellPx, Bitmap.Config.ARGB_8888)
        splashIcon.setBounds(0, 0, cellPx, cellPx)
        splashIcon.draw(Canvas(cell))
        val splashInkOnCell = inkRadius(cell)
        assertTrue(
            "启动屏图案墨迹半径必须是蒙版的 ${"%.0f".format(SPLASH_ART_MIN_RATIO * 100)}%" +
                "..${"%.0f".format(SPLASH_ART_MAX_RATIO * 100)}%，实际 ${"%.1f".format(splashInkOnCell)}dp",
            splashInkOnCell in SPLASH_ART_MIN_RATIO * MASK_RADIUS_DP..SPLASH_ART_MAX_RATIO * MASK_RADIUS_DP,
        )
    }

    /** 三个点迹（红/黄/绿）必须按各自 fill 色渲染成小圆，不得渲染成大色块。 */
    @Test
    fun dotsRenderAsSmallCircles() {
        val themed = themedContext()
        val art = requireNotNull(themed.getDrawable(R.drawable.logo_art)) {
            "logo_art 资源缺失"
        }
        val artPx = requireNotNull(art.intrinsicWidth) {
            "logo_art 的固有尺寸必须是矢量声明的 108"
        }
        val artBitmap = renderBitmap(art, artPx)
        intArrayOf(DOT_RED, DOT_YELLOW, DOT_GREEN).forEach { fillColor ->
            var ink = 0
            for (y in 0 until artPx) {
                for (x in 0 until artPx) {
                    val pixel = artBitmap.getPixel(x, y)
                    if ((pixel ushr 24) >= ALPHA_FLOOR &&
                        abs(Color.red(pixel) - Color.red(fillColor)) <= DOT_COLOR_FUZZ &&
                        abs(Color.green(pixel) - Color.green(fillColor)) <= DOT_COLOR_FUZZ &&
                        abs(Color.blue(pixel) - Color.blue(fillColor)) <= DOT_COLOR_FUZZ
                    ) {
                        ink++
                    }
                }
            }
            // 正确形态（实测校准：红4/黄5/绿5px）与缺首段 M 的坏形态（实测 21px）
            // 之间取安全带 [2,14]，两侧都比校准值离边界远。
            assertTrue(
                "点迹必须渲染成 ${DOT_MIN_INK}..$DOT_MAX_INK px 的小圆，fillColor=${"%06X".format(fillColor)} 露出墨迹 $ink px",
                ink in DOT_MIN_INK..DOT_MAX_INK,
            )
        }
    }

    private fun themedContext(): ContextThemeWrapper = ContextThemeWrapper(
        ApplicationProvider.getApplicationContext<Context>(),
        R.style.Theme_Terminal_Splash,
    )

    /** 最远非透明像素到画布中心的距离。Robolectric 160dpi 下 1px = 1dp。 */
    private fun inkRadius(bitmap: Bitmap): Float {
        val centerX = (bitmap.width - 1) / 2f
        val centerY = (bitmap.height - 1) / 2f
        var maxRadius = 0f
        for (y in 0 until bitmap.height) {
            for (x in 0 until bitmap.width) {
                val argb = bitmap.getPixel(x, y)
                if ((argb ushr 24) < ALPHA_FLOOR) continue
                val distance = hypot(x + 0.5f - centerX, y + 0.5f - centerY)
                if (distance > maxRadius) maxRadius = distance
            }
        }
        return maxRadius
    }

    private fun renderBitmap(drawable: Drawable, px: Int): Bitmap {
        val bitmap = Bitmap.createBitmap(px, px, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, px, px)
        drawable.draw(Canvas(bitmap))
        return bitmap
    }

    private fun renderBits(drawable: Drawable, px: Int): IntArray {
        val bitmap = renderBitmap(drawable, px)
        val bits = IntArray(px * px)
        bitmap.getPixels(bits, 0, px, 0, 0, px, px)
        return bits
    }

    private companion object {
        /** logo_art 的视口（108 单位）。 */
        const val VIEWPORT_UNITS = 108

        /** 平台启动屏单元：160dp × 1.2（no_background_scale_factor）= 192dp。 */
        const val SPLASH_CELL_DP = 192

        /** 平台默认蒙版圆半径：192dp 单元的 50/108 ≈ 88.9dp。 */
        const val MASK_RADIUS_DP: Float = SPLASH_CELL_DP * 50f / VIEWPORT_UNITS

        /** 启动屏图案墨迹半径区间（相对蒙版）：下限 = 完整图案要醒目
         *  （Robolectric ≈0.50、平台实测 ≈0.87）；上限 = 不触碰蒙版
         *  （直接给裸图案的旧实现 ≈1.0，星尖恰好出圆被裁）。 */
        const val SPLASH_ART_MIN_RATIO = 0.40f
        const val SPLASH_ART_MAX_RATIO = 0.95f

        /** 1:1 图案（launcher）墨迹半径范围（logo_art 原始 ≈47.6~50.3dp）。 */
        const val ARTWORK_MIN = 44f
        const val ARTWORK_MAX = 54f

        /** 点迹 fill 色（logo_art 的红/黄/绿三圆）。 */
        const val DOT_RED = 0xff8a6c.toInt()
        const val DOT_YELLOW = 0xffdb67.toInt()
        const val DOT_GREEN = 0x86e96d.toInt()

        // 点迹颜色匹配余量（抗锯齿边缘会混色，只计大部分覆盖本色的像素）。
        const val DOT_COLOR_FUZZ = 24

        /** 点迹最小墨迹（抗锯齿边缘的整圆至少有这么多纯色像素）。 */
        const val DOT_MIN_INK = 2

        /** 点迹墨迹上限（缺首段 M 的坏形态实测墨迹 21px，此处取区间安全上限）。 */
        const val DOT_MAX_INK = 14

        /** 非透明像素门槛（抗锯齿边缘也算墨迹）。 */
        const val ALPHA_FLOOR = 0x70
    }
}
