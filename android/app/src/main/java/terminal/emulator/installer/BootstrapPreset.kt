package terminal.emulator.installer

/**
 * 引导包预设的种类。
 *
 * 变种之间只有引导包不同：Termux 变种用官方 apt 引导包，Nix 变种用
 * Nix-on-Droid 引导包（apt 与 nix 的包模型不兼容，两套引导包不能混用）。
 */
internal enum class BootstrapPreset {
    /** Termux 官方引导包（`apt.android-7` 变体）。 */
    TERMUX,

    /** Nix-on-Droid 引导包（Nix 变种用）。 */
    NIX_ON_DROID,
}

/** Nix 变种的包名后缀（对应构建里的 `applicationIdSuffix = ".nix"`）。 */
internal const val NIX_VARIANT_PACKAGE_SUFFIX = ".nix"

/**
 * 按应用包名判定引导包预设。
 *
 * 判据只有包名后缀这一处来源：变种之间除包名外没有别的标记，多一个构建标记
 * 就多一处可能与包名不同步的地方（包名改了、标记没改，预设就会指错引导包）。
 */
internal fun bootstrapPresetFor(applicationId: String): BootstrapPreset =
    if (applicationId.endsWith(NIX_VARIANT_PACKAGE_SUFFIX)) {
        BootstrapPreset.NIX_ON_DROID
    } else {
        BootstrapPreset.TERMUX
    }
