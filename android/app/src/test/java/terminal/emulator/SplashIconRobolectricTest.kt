package terminal.emulator

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.AnimationDrawable
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
 * 启动屏图标的行为判据：主题解析出的启动屏 drawable 必须是 Animatable（平台据此直接
 * 绘制矢量，不再按 108dp 光栅化后放大，见 STATUS「软件启动图模糊」），且必须是普通
 * 图形而非自适应图标；包装层墨迹必须完整落进平台蒙版且足够醒目
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
        val splashIcon =
            requireNotNull(themed.getDrawable(splashValue.resourceId)) {
                "windowSplashScreenAnimatedIcon 指向的 drawable 必须存在"
            }
        assertFalse(
            "启动屏图标不得是自适应图标：平台会沿用桌面蒙版把图案裁成图标形状。" +
                "实际 ${splashIcon.javaClass.simpleName}",
            splashIcon is AdaptiveIconDrawable,
        )
        // 平台对非 Animatable 的自定义图标先按 108dp 光栅化再放大到 192dp（1.78×），
        // 矢量边缘会被重采样成模糊；AnimationDrawable 是拿到直接绘制路径的最小载体。
        assertTrue(
            "启动屏图标必须是 Animatable，否则平台会把矢量按 108dp 光栅化后放大。" +
                "实际 ${splashIcon.javaClass.simpleName}",
            splashIcon is AnimationDrawable,
        )
        val splashAnimation = splashIcon as AnimationDrawable
        assertEquals(
            "splash_icon 必须是单帧动画：多帧会真的动起来",
            1,
            splashAnimation.numberOfFrames,
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
        val backgroundAlpha =
            when {
                iconBackground.type in TypedValue.TYPE_FIRST_COLOR_INT..TypedValue.TYPE_LAST_COLOR_INT -> {
                    iconBackground.data ushr 24
                }

                iconBackground.type == TypedValue.TYPE_REFERENCE -> {
                    themed.resources.getColor(iconBackground.resourceId, themed.theme) ushr 24
                }

                else -> {
                    0
                }
            }
        assertTrue(
            "图标后不得垫圆形底板，实际底色 type=${iconBackground.type} alpha=$backgroundAlpha",
            backgroundAlpha == 0,
        )

        // 结构：两处 wrapper 都必须单层引用 logo_art（保证图案只此一份）。
        // splash_icon 顶层是单帧动画，帧本身是收缩包装 layer-list。
        val splashFrame =
            requireNotNull(splashAnimation.getFrame(0)) {
                "splash_icon 的唯一帧必须存在"
            }
        val splashChild =
            (splashFrame as LayerDrawable).getDrawable(0)
                ?: error("splash_frame 的 layer item 必须引用 logo_art")
        val launcherIcon =
            requireNotNull(
                themed.resources.getDrawable(R.drawable.launcher_foreground, themed.theme),
            )
        val launcherChild =
            (launcherIcon as LayerDrawable).getDrawable(0)
                ?: error("launcher_foreground 的 layer item 必须引用 logo_art")

        // 同一份图案：两个 wrapper 的子层都是 logo_art 本身，在固有尺寸
        // （108dp=108px，density 1）的画布上渲染，像素必须完全一致。
        val artPx =
            requireNotNull(splashChild.intrinsicWidth) {
                "logo_art 必须有矢量声明的固有尺寸"
            }
        val splashChildBits = renderBits(splashChild, artPx)
        val launcherChildBits = renderBits(launcherChild, artPx)
        assertTrue(
            "启动屏与桌面引用的必须是同一份 logo_art 像素（" +
                "splash=${splashChildBits.size} launcher=${launcherChildBits.size}）",
            splashChildBits.contentEquals(launcherChildBits),
        )

        // 图案摆放的判据落在平台蒙版上：平台把前景层按图标边界的 1.5 倍绘制，
        // 圆形蒙版只露出半径 36 单位（72dp 可见区的一半）的圆，超出即被裁掉。
        //
        // 66dp 安全区（半径 33 单位）是设计建议而非平台裁切线：0.75 缩放后星尖
        // 与终端窗口远角都越过这条建议线，但仍在蒙版圆内——用户明确允许内容溢出
        // 以保住观感（STATUS「应用图标较旧版观感下降」）。因此判据取平台真实裁切
        // 边界，并按近乎不透明的主体像素量几何（半透明抗锯齿边缘不参与）。
        //
        // 星形在 108 视口里位于右侧（x ≥ 56），与终端窗口的墨迹互不重叠，故按
        // x 阈值即可单独量出，确保识别点没被裁掉。
        val launcherBitmap = renderBitmap(launcherChild, artPx)
        val launcherInk = inkRadius(launcherBitmap, alphaFloor = SOLID_ALPHA_FLOOR)
        assertTrue(
            "图案墨迹半径不得超过平台蒙版圆半径 $MASK_CIRCLE_RADIUS 单位，" +
                "实际 ${"%.1f".format(launcherInk)}",
            launcherInk <= MASK_CIRCLE_RADIUS,
        )
        val starInk =
            inkRadius(
                launcherBitmap,
                minXUnits = STAR_REGION_MIN_X_UNITS,
                alphaFloor = SOLID_ALPHA_FLOOR,
            )
        assertTrue(
            "星形墨迹半径不得超过平台蒙版圆半径 $MASK_CIRCLE_RADIUS 单位，" +
                "实际 ${"%.1f".format(starInk)}",
            starInk <= MASK_CIRCLE_RADIUS,
        )

        // 平台管线几何：Animatable 分支把这一 wrapper 直接画进 192dp 单元并按
        // 88.9dp 蒙版裁剪，绝对 263dp 的帧让墨迹半径为 ≈76.7dp；上限 95% 用于
        // 发现「收缩包装失效、图案贴到蒙版边」的回归，下限 40% 防止图案过小。
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
        val art =
            requireNotNull(themed.getDrawable(R.drawable.logo_art)) {
                "logo_art 资源缺失"
            }
        val artPx =
            requireNotNull(art.intrinsicWidth) {
                "logo_art 的固有尺寸必须是矢量声明的 108"
            }
        // 图案已按安全区缩放，点迹随之变细：4 倍渲染才留得住足够的纯色像素
        // 供判别（阈值见 DOT_MIN_SIZE / DOT_MAX_SIZE）。
        val artBitmap = renderBitmap(art, artPx * DOT_RENDER_SCALE)
        intArrayOf(DOT_RED, DOT_YELLOW, DOT_GREEN).forEach { fillColor ->
            var minX = Int.MAX_VALUE
            var minY = Int.MAX_VALUE
            var maxX = -1
            var maxY = -1
            for (y in 0 until artBitmap.height) {
                for (x in 0 until artBitmap.width) {
                    val pixel = artBitmap.getPixel(x, y)
                    if ((pixel ushr 24) >= ALPHA_FLOOR &&
                        abs(Color.red(pixel) - Color.red(fillColor)) <= DOT_COLOR_FUZZ &&
                        abs(Color.green(pixel) - Color.green(fillColor)) <= DOT_COLOR_FUZZ &&
                        abs(Color.blue(pixel) - Color.blue(fillColor)) <= DOT_COLOR_FUZZ
                    ) {
                        if (x < minX) minX = x
                        if (x > maxX) maxX = x
                        if (y < minY) minY = y
                        if (y > maxY) maxY = y
                    }
                }
            }
            assertTrue(
                "点迹必须存在，fillColor=${"%06X".format(fillColor)}",
                maxX >= minX,
            )
            // 正确形态是小圆（直径 ≈ 2×1.6 单位）；缺首段 M 的坏形态是一整块色斑。
            // 判据取包围盒边长：与渲染倍率无关，只反映形状本身。
            val width = maxX - minX + 1
            val height = maxY - minY + 1
            assertTrue(
                "点迹必须是小圆（边长 ${DOT_MIN_SIZE}..$DOT_MAX_SIZE px），" +
                    "fillColor=${"%06X".format(fillColor)} 实测 $width×$height px",
                width in DOT_MIN_SIZE..DOT_MAX_SIZE && height in DOT_MIN_SIZE..DOT_MAX_SIZE,
            )
        }
    }

    private fun themedContext(): ContextThemeWrapper = ContextThemeWrapper(
        ApplicationProvider.getApplicationContext<Context>(),
        R.style.Theme_Terminal_Splash,
    )

    /**
     * 最远非透明像素到画布中心的距离。Robolectric 160dpi 下 1px = 1dp。
     *
     * [minXUnits] 把统计限制在视口右侧（108 单位制），用于把星形与终端窗口的
     * 墨迹分开量。[alphaFloor] 可收紧到近乎不透明的主体像素，使量到的是矢量
     * 几何而不是抗锯齿软边。
     */
    private fun inkRadius(bitmap: Bitmap, minXUnits: Float = 0f, alphaFloor: Int = ALPHA_FLOOR): Float {
        val centerX = (bitmap.width - 1) / 2f
        val centerY = (bitmap.height - 1) / 2f
        val minXPx = minXUnits / VIEWPORT_UNITS * bitmap.width
        var maxRadius = 0f
        for (y in 0 until bitmap.height) {
            for (x in 0 until bitmap.width) {
                if (x + 0.5f < minXPx) continue
                val argb = bitmap.getPixel(x, y)
                if ((argb ushr 24) < alphaFloor) continue
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

        /** 平台启动屏蒙版安全半径的下界：192dp 单元按 50/108 缩放 ≈ 88.9dp
         *  （config_icon_mask 是圆角方形，轴线方向恰好是 96dp，取更保守的 88.9dp）。 */
        const val MASK_RADIUS_DP: Float = SPLASH_CELL_DP * 50f / VIEWPORT_UNITS

        /** 启动屏图案墨迹半径区间（相对蒙版）：下限 = 完整图案要醒目
         *  （直接给裸图案的旧实现 ≈0.86）；上限 = 不得越过蒙版圆
         *  （0.75 缩放 + 263dp 帧后 ≈0.99，恰好贴边且完整）。 */
        const val SPLASH_ART_MIN_RATIO = 0.40f
        const val SPLASH_ART_MAX_RATIO = 1.0f

        /**
         * 平台圆形蒙版的裁切半径（108 单位视口内）：72dp 可见区的一半 = 36 单位。
         * 越过这条线的内容会被蒙版裁掉。
         */
        const val MASK_CIRCLE_RADIUS = 36f

        /**
         * 视口右侧分界（108 单位制）：星形与尾迹点位于 x ≥ 56，终端窗口的最远点在
         * 左侧，故 x ≥ 50 即可只量到星形一侧的墨迹。
         */
        const val STAR_REGION_MIN_X_UNITS = 50f

        /** 点迹 fill 色（logo_art 的红/黄/绿三圆）。 */
        const val DOT_RED = 0xff8a6c.toInt()
        const val DOT_YELLOW = 0xffdb67.toInt()
        const val DOT_GREEN = 0x86e96d.toInt()

        // 点迹颜色匹配余量：点迹是实色填充，核心像素即本色，故容差取小值。
        // 大容差会把星形光晕与填充的混合色（#FFC37F 一类）也算进来，
        // 那会让包围盒横跨整颗星。
        const val DOT_COLOR_FUZZ = 6

        /** 点迹包围盒最小边长（正确形态是小圆，直径 ≈ 2×1.6 单位）。 */
        const val DOT_MIN_SIZE = 4

        /** 渲染倍率：图案已按安全区缩放，点迹随之变细，4 倍渲染才留得住纯色像素。 */
        const val DOT_RENDER_SCALE = 4

        /**
         * 点迹包围盒最大边长。缺首段 M 的坏形态是从原点拉出的一整条色斑，
         * 包围盒会横跨整幅画布，远超此限。
         */
        const val DOT_MAX_SIZE = 16

        /** 非透明像素门槛（抗锯齿边缘也算墨迹）。 */
        const val ALPHA_FLOOR = 0x70

        /** 近乎不透明的主体像素门槛：量几何时排除半透明软边。 */
        const val SOLID_ALPHA_FLOOR = 0xF0
    }
}
