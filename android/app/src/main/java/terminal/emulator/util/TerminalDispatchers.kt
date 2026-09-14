package terminal.emulator.util

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers

/**
 * 全应用协程调度器唯一来源：slack-lint RawDispatchersUse 禁止直接引用
 * [Dispatchers]，调用处一律经此中转，语义与标准调度器完全一致。
 */
@Suppress("RawDispatchersUse") // 全应用唯一的直接引用点；集中抑制优于 25 处分散引用。
object TerminalDispatchers {
    val main: CoroutineDispatcher = Dispatchers.Main
    val inputOutput: CoroutineDispatcher = Dispatchers.IO
    val background: CoroutineDispatcher = Dispatchers.Default
}
