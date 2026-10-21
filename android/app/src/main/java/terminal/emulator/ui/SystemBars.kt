package terminal.emulator.ui

import android.app.Activity
import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.core.view.WindowInsetsControllerCompat

/**
 * 系统栏图标明暗跟随**当前页面的底色**。
 *
 * 窗口是边到边的（`MainActivity.onCreate` 已 `WindowCompat.enableEdgeToEdge`），
 * 系统栏自身透明、只决定**图标明暗**，而 `enableEdgeToEdge` 的默认样式跟随
 * **系统**深浅色——与应用内「日间/夜间」开关以及页面配色都无关：
 * 浅色页面上状态栏图标恒为浅色，在浅背景上不可见
 * （DESIGN：软件主题三种模式；终端配色作用于终端页面与修饰键栏）。
 *
 * 明暗判据取背景自身亮度而非软件主题开关：图标最终画在该背景像素上，
 * 按像素判才不会在浅色页面配夜间开关时判反。
 *
 * 每个页面按**自己的**底色调用：终端页用终端底色，设置页用设置页底色——
 * 否则浅色设置页上会残留深色终端页决定的浅色图标（时钟看不清）。
 */
@Composable
internal fun SystemBarsFollowBackground(view: View, background: Color) {
    val window = (view.context as? Activity)?.window ?: return
    val lightBackground = hasLightBackground(background)
    SideEffect {
        // 显式声明图标明暗：`WindowCompat.enableEdgeToEdge(window)` 按**系统**深浅色
        // 决定，且不接受样式参数；`SystemBarStyle` 那套只挂在已弃用的
        // `ComponentActivity.enableEdgeToEdge` 上。要让图标跟随应用内主题与页面配色，
        // 只能直接写 insets controller 的两枚开关。
        WindowInsetsControllerCompat(window, view).apply {
            isAppearanceLightStatusBars = lightBackground
            isAppearanceLightNavigationBars = lightBackground
        }
    }
}

/**
 * 背景是否为浅色（WCAG 相对亮度中点为界）。浅色背景上系统栏图标取深色。
 *
 * 按背景像素而非软件主题开关判定：图标最终画在该背景上，浅色背景配夜间开关时
 * 按开关判会判反。
 *
 * 名字说的是「背景浅不浅」而不是「图标浅不浅」——后者是结论，会随平台对
 * `isAppearanceLightStatusBars` 的语义理解而反转。
 */
internal fun hasLightBackground(background: Color): Boolean =
    background.luminance() > LIGHT_BACKGROUND_LUMINANCE_THRESHOLD

/** 背景亮度高于此值即按浅色背景处理（图标取深色）。取 WCAG 相对亮度中点。 */
private const val LIGHT_BACKGROUND_LUMINANCE_THRESHOLD = 0.5f
