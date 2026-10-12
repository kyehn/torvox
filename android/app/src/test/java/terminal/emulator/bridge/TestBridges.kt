package terminal.emulator.bridge

/**
 * 单测用的 [Bridge] 构造夹具。
 *
 * [Bridge] 构造只保存配置并建一个 lambda，无 JNI 副作用，故单测可直接构造。
 * 主题标识取默认夜间主题名（单源在原生），断言目标不涉及真实配色。
 */
internal object TestBridges {
    fun create(rows: Int = 24, cols: Int = 80): Bridge = Bridge(
        TerminalConfig(
            shell = Shell.SystemDefault,
            rows = rows,
            cols = cols,
            themeName = "",
            home = "",
            workingDirectory = "",
            prefix = "",
            mkshrcPath = "",
            fontSizeTenths = 140,
        ),
    )
}
