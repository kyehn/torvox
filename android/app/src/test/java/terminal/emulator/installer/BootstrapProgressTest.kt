package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * BootstrapProgress.overallProgress — the settings-screen install bar.
 * Invariants pinned here: monotonic stage bands (download → extract →
 * symlinks → post-install → complete), never-over-1.0 progress, and
 * division-by-zero safety when totals are unknown (0).
 */
class BootstrapProgressTest {
    private val tolerance = 0.001f

    @Test
    fun `download scales linearly to 85 percent`() {
        assertEquals(0f, BootstrapProgress.Downloading(0, 100).overallProgress(), tolerance)
        assertEquals(0.425f, BootstrapProgress.Downloading(50, 100).overallProgress(), tolerance)
        assertEquals(0.85f, BootstrapProgress.Downloading(100, 100).overallProgress(), tolerance)
    }

    @Test
    fun `download with unknown length stays at zero`() {
        assertEquals(0f, BootstrapProgress.Downloading(42, 0).overallProgress(), tolerance)
    }

    @Test
    fun `extract never regresses the bar below the download cap`() {
        // 0 进度时落在 0.85 只是构造使然（0.85 + 0 × ratio），断言不出东西；
        // 有意义的判据是整段提取期都不低于下载段的末端。
        assertTrue(BootstrapProgress.Extracting(0, 100).overallProgress() >= 0.85f)
    }

    @Test
    fun `extract caps at 97 percent`() {
        assertEquals(0.97f, BootstrapProgress.Extracting(100, 100).overallProgress(), tolerance)
        assertEquals(0.91f, BootstrapProgress.Extracting(50, 100).overallProgress(), tolerance)
    }

    @Test
    fun `extract with unknown totals stays inside band`() {
        val progress = BootstrapProgress.Extracting(3, 0).overallProgress()
        assertTrue("unknown totals must not leave the extract band", progress in 0.85f..0.97f)
    }

    @Test
    fun `post install interpolates between 99 and 100 percent`() {
        assertEquals(0.99f, BootstrapProgress.RunningPostInstall(0, 10).overallProgress(), tolerance)
        assertEquals(0.995f, BootstrapProgress.RunningPostInstall(5, 10).overallProgress(), tolerance)
        assertEquals(1f, BootstrapProgress.RunningPostInstall(10, 10).overallProgress(), tolerance)
    }

    @Test
    fun `post install with unknown totals stays inside band`() {
        val progress = BootstrapProgress.RunningPostInstall(1, 0).overallProgress()
        assertTrue("unknown totals must stay in 0.99..1.0", progress in 0.99f..1f)
    }

    @Test
    fun `progress never regresses along the real phase order`() {
        // 逐个阶段断言具体数值等于把生产里的字面量抄一遍（CreatingSymlinks=0.99、
        // Complete=1、Error=0 都只是 `= 0.99f` 的回读）。真正该守的是 KDoc 写下
        // 的那条不变量：**进度条永不回退**——按真实阶段顺序取样，任一步下降即失败。
        val phases =
            listOf(
                BootstrapProgress.Downloading(0, 100),
                BootstrapProgress.Downloading(100, 100),
                BootstrapProgress.Extracting(0, 100),
                BootstrapProgress.Extracting(50, 100),
                BootstrapProgress.Extracting(100, 100),
                BootstrapProgress.CreatingSymlinks,
                BootstrapProgress.RunningPostInstall(0, 10),
                BootstrapProgress.RunningPostInstall(10, 10),
                BootstrapProgress.Complete,
            )
        phases.zipWithNext { previous, next ->
            assertTrue(
                "进度从 ${previous::class.simpleName} 的 ${previous.overallProgress()} " +
                    "回退到 ${next::class.simpleName} 的 ${next.overallProgress()}",
                next.overallProgress() >= previous.overallProgress(),
            )
        }
    }

    @Test
    fun `error clears the bar`() {
        // 失败态必须清零而不是停在某个阶段值——否则用户看到的是一个永远不动的进度条。
        assertTrue(
            "失败态必须清零，实际 ${BootstrapProgress.Error("boom").overallProgress()}",
            BootstrapProgress.Error("boom").overallProgress() == 0f,
        )
    }
}
