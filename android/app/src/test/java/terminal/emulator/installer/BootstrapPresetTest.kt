package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 引导包预设的判定与地址拼装。
 *
 * 判据只有包名后缀一处来源：变种之间除包名外没有别的标记，预设若认错了包名，
 * 用户点下的就是另一套引导包（apt 与 nix 的包模型不兼容，装错即坏）。
 */
class BootstrapPresetTest {
    @Test
    fun `nix package name selects the nix on droid preset`() {
        assertEquals(BootstrapPreset.NIX_ON_DROID, bootstrapPresetFor("com.termux.nix"))
    }

    @Test
    fun `termux package name selects the termux preset`() {
        assertEquals(BootstrapPreset.TERMUX, bootstrapPresetFor("com.termux"))
    }

    @Test
    fun `nix on droid url carries release and architecture`() {
        assertEquals(
            "https://nix-on-droid.unboiled.info/bootstrap-release-24.05/bootstrap-x86_64.zip",
            NixOnDroidBootstrap.downloadUrl("x86_64"),
        )
        assertEquals(
            "https://nix-on-droid.unboiled.info/bootstrap-release-24.05/bootstrap-aarch64.zip",
            NixOnDroidBootstrap.downloadUrl("aarch64"),
        )
    }
}
