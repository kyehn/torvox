package terminal.emulator.bridge

/**
 * 单测用的 [Bridge] 构造夹具。
 *
 * [Bridge] 构造只保存配置并建一个 lambda，无 JNI 副作用，故单测可直接构造。
 * 全零色值足以满足「需要一个实例」的用途：断言目标不涉及真实配色。
 */
internal object TestBridges {
    private val blankTheme =
        BridgeTheme(
            name = "test",
            background = 0,
            foreground = 0,
            cursor = 0,
            ansi0 = 0,
            ansi1 = 0,
            ansi2 = 0,
            ansi3 = 0,
            ansi4 = 0,
            ansi5 = 0,
            ansi6 = 0,
            ansi7 = 0,
            ansi8 = 0,
            ansi9 = 0,
            ansi10 = 0,
            ansi11 = 0,
            ansi12 = 0,
            ansi13 = 0,
            ansi14 = 0,
            ansi15 = 0,
        )

    fun create(rows: Int = 24, cols: Int = 80): Bridge = Bridge(
        TerminalConfig(
            shell = Shell.SystemDefault,
            rows = rows,
            cols = cols,
            theme = blankTheme,
            home = "",
            workingDirectory = "",
            prefix = "",
            mkshrcPath = "",
            fontSizeTenths = 140,
        ),
    )
}
