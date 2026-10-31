package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Termux 引导程序包的下载 URL 形状。
 *
 * 断言的是**构造规则**而不是某一版版本号：版本号本就应当随 termux-app 上游更新，
 * 把某个具体字符串钉死只会让每次跟进上游都要改测试，而这里真正会出错的是拼法
 * ——`+` 未转义成 `%2B` 时 GitHub 直接 404，症状只是安装进度条停住。
 */
class TermuxBootstrapTest {

    @Test
    fun `url points at the release tag with the variant escaped`() {
        val url = TermuxBootstrap.downloadUrl("aarch64")
        assertEquals(
            "https://github.com/termux/termux-packages/releases/download/" +
                "bootstrap-${TermuxBootstrap.RELEASE}%2B${TermuxBootstrap.VARIANT}" +
                "/bootstrap-aarch64.zip",
            url,
        )
        // 未转义的 `+` 在 URL 路径里是空格，GitHub 会 404——必须逐字断言。
        assertTrue("变体分隔符必须转义为 %2B，实际 $url", url.contains("%2B"))
        assertTrue("路径中不得出现未转义的 +，实际 $url", !url.contains("+"))
    }

    @Test
    fun `every supported abi maps to an arch the release publishes`() {
        // 设置页只会传这两种 arch（见 detectDeviceArchitecture）；URL 的末段必须原样带出，
        // 否则会下载到不存在的归档。
        listOf("aarch64", "x86_64").forEach { arch ->
            assertTrue(
                "arch=$arch 的 URL 末段不正确",
                TermuxBootstrap.downloadUrl(arch).endsWith("/bootstrap-$arch.zip"),
            )
        }
    }

    @Test
    fun `variant is the one design mandates`() {
        // DESIGN：只用 apt.android-7，不提供其它变体。
        assertEquals("apt.android-7", TermuxBootstrap.VARIANT)
    }

    /**
     * 两个受支持架构都必须钉住归档摘要。
     *
     * 归档解压后会执行其中的 postinst，摘要是唯一的信任根（https 只防链路篡改）。
     * 漏钉一个架构＝该架构的安装退化成「只查 https」，正是本能力要堵的口子。
     */
    @Test
    fun `every supported abi has a pinned sha256`() {
        listOf("aarch64", "x86_64").forEach { arch ->
            val digest = TermuxBootstrap.sha256(arch)
            assertTrue("arch=$arch 未钉住摘要", digest != null)
            assertTrue(
                "arch=$arch 的摘要不是 64 位十六进制：$digest",
                digest!!.matches(Regex("[0-9a-f]{64}")),
            )
        }
        assertEquals("未受支持的架构不得有摘要", null, TermuxBootstrap.sha256("arm"))
    }
}
