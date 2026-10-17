package terminal.emulator

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * STATUS ①「Shell 启动前偶发黑屏」的判据：系统启动屏必须在终端首帧呈现前继续遮挡
 * （默认的「内容视图首次绘制」早于原生首帧，其间的 SurfaceView 开孔区是黑底），
 * 首帧呈现后立刻撤下，且首帧迟迟不来时有上限兜底——不得永久遮挡界面。
 *
 * 上限取 5s 字面量而非引用私有常量：这条断言的作用正是钉住该上限，改动它必须同时改测试。
 */
class ShouldKeepSplashOnScreenTest {

    @Test
    fun keepsUntilFirstFramePresented() {
        assertTrue("首帧未呈现时必须继续遮挡", shouldKeepSplashOnScreen(framePresented = false, waitedMs = 0))
        assertTrue(
            "首帧未呈现且未到上限时继续遮挡",
            shouldKeepSplashOnScreen(framePresented = false, waitedMs = 4_999),
        )
        assertFalse(
            "首帧呈现即撤下，无需等待",
            shouldKeepSplashOnScreen(framePresented = true, waitedMs = 0),
        )
        assertFalse(
            "首帧迟到但已呈现：撤下优先于等待上限",
            shouldKeepSplashOnScreen(framePresented = true, waitedMs = 60_000),
        )
    }

    @Test
    fun givesUpAtDeadline() {
        assertFalse(
            "到达上限必须撤下，避免引导缺失时永久遮挡界面",
            shouldKeepSplashOnScreen(framePresented = false, waitedMs = 5_000),
        )
        assertFalse(
            "超过上限同样撤下",
            shouldKeepSplashOnScreen(framePresented = false, waitedMs = 60_000),
        )
    }
}
