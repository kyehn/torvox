package terminal.emulator.util

import kotlinx.coroutines.CancellationException

/**
 * 可取消的 [runCatching] 变体：永不吞掉协程取消信号。
 *
 * 标准库 [runCatching] 会捕获 [CancellationException]，在协程中阻断结构化取消 （slack-lint
 * DenyListedApi）。本函数将其重新抛出，其余行为与 [runCatching] 一致； 调用处保持 `runCatching { }.getOrNull()` 形态，不引入
 * try/catch（detekt TooGenericExceptionCaught）。
 */
@Suppress("TooGenericExceptionCaught") // runCatching 对等语义必须捕获 Exception；取消信号已优先重抛。
inline fun <T> runCatchingCancellable(block: () -> T): Result<T> = try {
    Result.success(block())
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (failure: Exception) {
    Result.failure(failure)
}
