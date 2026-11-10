package terminal.emulator

import android.content.Context
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import terminal.emulator.runtime.RuntimeState
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxConfiguration
import terminal.emulator.settings.TermuxPropertiesRepository
import java.util.concurrent.CountDownLatch

/**
 * 构造运行期假件：只有「是否已在运行」与「有哪些会话」对被测行为有影响。
 *
 * 两处用例（外部 Failsafe Intent 处置、桌面通知点击切会话）各自抄了一份，
 * 字段一改就分叉：会话表构造口径不一致时，同一「已有会话」前提在两处
 * 意味着不同的运行期状态，断言便各自为政。
 */
internal fun createTestRuntime(sessionIds: List<Long>): TerminalRuntime {
    val runtime = mockk<TerminalRuntime>(relaxed = true)
    every { runtime.state } returns
        MutableStateFlow(
            RuntimeState(isRunning = sessionIds.isNotEmpty(), sessionIds = sessionIds),
        )
    return runtime
}

/**
 * 构造带假设置仓库的 [TerminalViewModel]：字号与主题对被测行为无影响，只需是合法值。
 *
 * 与 [createTestRuntime] 同理——设置字段一改（新增设置项），两处抄本必须同步改，
 * 漏改的那处会在 relaxed mock 上拿到零值而把断言带偏。
 */
internal fun createTestViewModel(runtime: TerminalRuntime): TerminalViewModel {
    val settingsRepository = mockk<SettingsRepository>(relaxed = true)
    every { settingsRepository.settings } returns
        flowOf(
            SettingsRepository.SettingsState(
                fontSize = 14f,
                themeName = "",
                dayThemeName = "",
                nightThemeName = "",
            ),
        )
    every { settingsRepository.defaultFontSize } returns 14f
    val termuxProperties = mockk<TermuxPropertiesRepository>(relaxed = true)
    every { termuxProperties.configuration } returns MutableStateFlow(TermuxConfiguration())
    return TerminalViewModel(
        context = mockk<Context>(relaxed = true),
        settingsRepository = settingsRepository,
        termuxProperties = termuxProperties,
        runtime = runtime,
        transientMessages = TransientMessageChannel(),
    )
}

/**
 * 记录会话切换调用的运行期假件：返回假件与「已切换」信号。
 *
 * 「切换是否发生」是通知点击用例的唯一判据，两条用例各自抄了一份「装 mock + 挂信号」
 * 的搭建，信号口径一旦分叉（谁在何时 countDown），断言就会等一个不会来的信号。
 */
internal fun createSwitchRecordingRuntime(): Pair<TerminalRuntime, CountDownLatch> {
    val runtime = createTestRuntime(sessionIds = listOf(SWITCH_TARGET_SESSION_ID))
    val switched = CountDownLatch(1)
    coEvery { runtime.switchSession(any(), any(), any(), any()) } coAnswers {
        switched.countDown()
        true
    }
    return runtime to switched
}

/** 通知点击用例的目标会话号：假件与断言共用一份，避免两处各写一个 7L。 */
internal const val SWITCH_TARGET_SESSION_ID = 7L
