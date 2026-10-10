package terminal.emulator.runtime

import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

/**
 * Service 等非 Hilt 组件取运行期单例的入口。
 *
 * `TerminalForegroundService` 由 `Context.startForegroundService` 启动，不是 Hilt
 * 组件；通知的「退出」按钮需要关闭全部会话，只能经 [EntryPoint] 拿同一个
 * `@Singleton`——另建实例会得到一份与真实会话毫无关系的运行期。
 */
@EntryPoint
@InstallIn(SingletonComponent::class)
interface TerminalRuntimeEntryPoint {
    fun terminalRuntime(): TerminalRuntime
}
