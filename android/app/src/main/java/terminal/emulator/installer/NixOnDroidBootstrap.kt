package terminal.emulator.installer

/**
 * Nix-on-Droid 引导包（Nix 变种用）。
 *
 * 引导包里的 `bin/login` 以包私有目录（`/data/data/<pkg>/files/...`）为绝对路径，
 * 故它只适用于包名为 `com.termux.nix` 的变种——正是本仓的 Nix 变种。
 *
 * 引导包结构与 Termux 官方包同源（`SYMLINKS.txt` + `EXECUTABLES.txt` + `bin/login`），
 * 故安装流程共用；差别只在符号链接的目标写法（沙箱根绝对路径，见
 * [BootstrapInstaller.resolveAbsoluteSymlinkTarget]）。
 */
internal object NixOnDroidBootstrap {
    /** 引导包发布版本。 */
    const val RELEASE: String = "24.05"

    private const val DOWNLOAD_BASE = "https://nix-on-droid.unboiled.info/bootstrap-release-$RELEASE"

    /**
     * 该版本 + 架构的下载 URL。
     *
     * 架构取 [terminal.emulator.detectDeviceArchitecture] 的取值
     * （`aarch64` / `x86_64`），与引导包里的文件名一致。
     */
    fun downloadUrl(architecture: String): String = "$DOWNLOAD_BASE/bootstrap-$architecture.zip"
}
