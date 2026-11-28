package terminal.emulator.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext

/**
 * 设置页展示期间那次渲染暂停的领用与归还。
 *
 * 渲染暂停只有一个计数账本（见 `render-stability` 规范）：每次领用都必须有确定的
 * 归属，否则计数永不归零，共享渲染器被永久暂停，终端从此不再出帧（黑屏）。
 *
 * 三个不变式：
 *
 * - **领与存合成一步**：凭证领到即写进状态，中间不留取消点。不可取消段保证段的
 *   出口被取消时凭证仍已落到状态上，由 [DisposableEffect] 的 `onDispose` 归还。
 * - **关闭后不再领**：设置页可能在本 effect 运行期间关掉——此时 `onDispose` 早已
 *   跑过且没拿到凭证。此刻再领一次就没人归还，账本计数停在 1，终端永久黑屏。
 *   故领之前再问一次「设置页此刻还开着吗」。
 * - **只归还自己领的那一次**：归还走账本的凭证去重，重复归还不减别人的持有。
 *
 * @param enabled 设置页是否开着；变化时本 effect 重启（关闭即归还）。
 * @param stillNeeded 读**当前**值的判据。effect 体里捕获的 [enabled] 是启动时的
 *   快照：设置页在 effect 运行期间关掉时它仍是旧值，故判据必须另取。
 * @param awaitReady 等待渲染器就绪（原生暂停标志落在全局渲染器上，渲染器尚未建立
 *   时写入即丢失）。
 * @param acquire 领一次暂停，返回凭证。
 * @param release 归还一次暂停。
 * @param onReleased 归还之后的一次补救动作（如立刻出一帧）。
 */
@Composable
internal fun SettingsRenderPauseEffect(
    enabled: Boolean,
    stillNeeded: () -> Boolean,
    awaitReady: suspend () -> Unit,
    acquire: () -> Long,
    release: (Long) -> Unit,
    onReleased: () -> Unit,
) {
    var token by remember { mutableStateOf<Long?>(null) }
    LaunchedEffect(enabled) {
        if (!enabled) return@LaunchedEffect
        awaitReady()
        withContext(NonCancellable) {
            // 领与存合成一步：不可取消段内没有取消点，段的出口被取消也不会把凭证
            // 丢在无人归还的地方。
            //
            // 段内再问一次「设置页此刻还开着吗」：关着时 onDispose 早已跑过且没拿到
            // 凭证，此刻领了没人还，账本计数永不归零。
            if (!stillNeeded()) return@withContext
            token = acquire()
        }
    }
    DisposableEffect(enabled) {
        onDispose {
            val held = token
            token = null
            if (held == null) return@onDispose
            release(held)
            onReleased()
        }
    }
}
