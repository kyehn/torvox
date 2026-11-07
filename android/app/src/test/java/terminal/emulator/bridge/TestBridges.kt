package terminal.emulator.bridge

/**
 * 单测用的 [Bridge] 构造夹具。
 *
 * [Bridge] 构造只保存配置并建一个 lambda，无 JNI 副作用，故单测可直接构造。
 */
internal object TestBridges {
    fun create(): Bridge = Bridge(
        TerminalConfig(
            shell = Shell.SystemDefault,
            home = "",
            workingDirectory = "",
            prefix = "",
            mkshrcPath = "",
            fontSizeTenths = 140,
        ),
    )
}
