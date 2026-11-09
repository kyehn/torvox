package terminal.emulator

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.util.TerminalDispatchers

/**
 * 终端主题设置的持久化与应用。
 *
 * 从 [TerminalViewModel] 拆出：五个主题设置器共用同一条「先落盘、再把主题重新
 * 应用到运行期」的路径，与会话、选区、字体等职责无关。
 */
internal class ThemeManager(
    private val settingsRepository: SettingsRepository,
    private val runtime: TerminalRuntime,
    private val scope: CoroutineScope,
) {
    fun setThemeName(name: String) = applyThemeSettings { settingsRepository.setThemeName(name) }

    fun setDayThemeName(name: String) = applyThemeSettings {
        settingsRepository.setDayThemeName(name)
    }

    fun setNightThemeName(name: String) = applyThemeSettings {
        settingsRepository.setNightThemeName(name)
    }

    fun setThemeMode(mode: String) = applyThemeSettings { settingsRepository.setThemeMode(mode) }

    fun setAppThemeMode(mode: String) = applyThemeSettings {
        settingsRepository.setAppThemeMode(mode)
    }

    /**
     * 持久化一项主题设置，然后把整个主题重新应用到 bridge。由五个主题设置器共用。
     *
     * 存储里失效的主题标识由运行期一处清完（`clearUnknownThemeNames`，三个键一并
     * 查），此处 MUST NOT 再兜一层异常重试：那一层捕获不到真正会抛的那个异常类型，
     * 反而会在原生字节布局出问题时把用户的主题重写掉——把契约破裂藏成「设置已修复」。
     */
    private fun applyThemeSettings(persist: suspend () -> Unit) {
        scope.launch(TerminalDispatchers.inputOutput) {
            persist()
            runtime.applySettings()
        }
    }
}
