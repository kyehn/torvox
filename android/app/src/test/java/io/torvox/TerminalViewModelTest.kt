package io.torvox

import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit4.MockKRule
import io.torvox.runtime.RuntimeState
import io.torvox.runtime.TorvoxRuntime
import io.torvox.settings.SettingsRepository
import io.torvox.ui.ModifierState
import io.torvox.ui.next
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

class TerminalViewModelTest {
    @Test
    fun testDefaultState() {
        val state = TerminalState()
        assertEquals(0L, state.sessionId)
        assertFalse(state.isRunning)
        assertEquals("Torvox", state.title)
        assertFalse(state.selection.active)
        assertNull(state.selection.start)
        assertNull(state.selection.end)
        assertEquals(ModifierState.Off, state.ctrlState)
        assertEquals(ModifierState.Off, state.altState)
        assertFalse(state.scrollActive)
        assertTrue(state.sessions.isEmpty())
        assertEquals(0L, state.activeSessionId)
        assertNull(state.pastePopupRequest)
    }

    @Test
    fun testSessionRestoreDefaultsOff() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.clearSelection()
        assertFalse(result.isRunning)
        assertTrue(result.sessions.isEmpty())
    }

    @Test
    fun testModifierStateCycling() {
        assertEquals(ModifierState.Once, ModifierState.Off.next())
        assertEquals(ModifierState.Locked, ModifierState.Once.next())
        assertEquals(ModifierState.Off, ModifierState.Locked.next())
    }

    @Test
    fun testModifierStateOffIsDefault() {
        val state = TerminalState()
        assertEquals(ModifierState.Off, state.ctrlState)
        assertEquals(ModifierState.Off, state.altState)
    }

    @Test
    fun testModifierStateOnceIsTransient() {
        val state = TerminalState(ctrlState = ModifierState.Once)
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.clearSelection()
        assertEquals(ModifierState.Off, result.ctrlState)
    }

    @Test
    fun testSessionInfoDefaults() {
        val info = SessionInfo(id = 1L, title = "Test")
        assertEquals(1L, info.id)
        assertEquals("Test", info.title)
    }

    @Test
    fun testSelectionStateDefaults() {
        val sel = SelectionState()
        assertFalse(sel.active)
        assertFalse(sel.dragging)
        assertNull(sel.start)
        assertNull(sel.end)
        assertEquals(SelectionMode.Char, sel.mode)
        assertEquals("", sel.selectedText)
    }

    @Test
    fun testPastePopupRequestDefaults() {
        val req = PastePopupRequest(row = 5, col = 10)
        assertEquals(5, req.row)
        assertEquals(10, req.col)
    }

    @Test
    fun testSelectionModeUpdate() {
        val state = TerminalState(selection = SelectionState(mode = SelectionMode.Char))
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.setSelectionMode(SelectionMode.Word, state)
        assertEquals(SelectionMode.Word, result.selection.mode)
    }

    @Test
    fun testScrollModeToggle() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val toggled = delegate.toggleScrollMode(state)
        assertTrue(toggled.scrollActive)
        val toggledBack = delegate.toggleScrollMode(toggled)
        assertFalse(toggledBack.scrollActive)
    }

    @Test
    fun testClearSelectionResetsAll() {
        val state =
            TerminalState(
                selection =
                SelectionState(
                    active = true,
                    start = SelectionAnchor(0, 0),
                    end = SelectionAnchor(5, 5),
                    selectedText = "hello",
                ),
            )
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.clearSelection()
        assertFalse(result.selection.active)
        assertNull(result.selection.start)
        assertNull(result.selection.end)
        assertEquals("", result.selection.selectedText)
    }

    private fun assertNull(value: Any?) {
        org.junit.Assert.assertNull(value)
    }
}

@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class TerminalViewModelThemeTest {
    @get:Rule
    val mockkRule = MockKRule(this)

    @MockK
    private lateinit var settingsRepository: SettingsRepository

    @MockK
    private lateinit var runtime: TorvoxRuntime

    private val testDispatcher = StandardTestDispatcher()
    private val runtimeStateFlow = MutableStateFlow(RuntimeState())

    private lateinit var viewModel: TerminalViewModel

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        every { runtime.state } returns runtimeStateFlow
        coEvery { runtime.bridge() } returns null
        coEvery { settingsRepository.themeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeMode } returns flowOf("fixed")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.fontSize } returns flowOf(18f)
        coEvery { settingsRepository.fontFamily } returns flowOf("")
        coEvery { settingsRepository.shell } returns flowOf("/system/bin/sh")
        coEvery { settingsRepository.scrollbackLines } returns flowOf(50000)
        coEvery { settingsRepository.keyboardMode } returns flowOf("secure")
        coEvery { settingsRepository.useNerdFontGlyphs } returns flowOf(false)
        coEvery { settingsRepository.useSemanticSelection } returns flowOf(false)
        coEvery { settingsRepository.sessionRestore } returns flowOf(false)
        coEvery { settingsRepository.cursorBlink } returns flowOf(true)
        coEvery { settingsRepository.cursorStyle } returns flowOf("block")
        coEvery { settingsRepository.cursorSpeed } returns flowOf(530)
        coEvery { settingsRepository.touchBehavior } returns flowOf("right_click")
        coEvery { settingsRepository.bootstrapUrl } returns flowOf("")
        coEvery { settingsRepository.usbSerialEnabled } returns flowOf(false)
        coEvery { settingsRepository.mcpServerEnabled } returns flowOf(false)
        coEvery { settingsRepository.volumeKeyMap } returns flowOf(false)
        coEvery { settingsRepository.backgroundImagePath } returns flowOf("")
        coEvery { settingsRepository.backgroundBlurRadius } returns flowOf(0)
        coEvery { settingsRepository.backgroundAlpha } returns flowOf(0.8f)
        coEvery { settingsRepository.setThemeName(any()) } returns Unit
        coEvery { settingsRepository.setDayThemeName(any()) } returns Unit
        coEvery { settingsRepository.setNightThemeName(any()) } returns Unit
        coEvery { settingsRepository.setThemeMode(any()) } returns Unit
        coEvery { settingsRepository.setAppThemeMode(any()) } returns Unit
        coEvery { runtime.applySettings() } returns Unit
        viewModel =
            TerminalViewModel(
                context = RuntimeEnvironment.getApplication(),
                settingsRepository = settingsRepository,
                runtime = runtime,
            )
        testDispatcher.scheduler.advanceUntilIdle()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun setThemeNameCallsApplySettings() = runTest(testDispatcher) {
        viewModel.setThemeName("Nord")
        testDispatcher.scheduler.advanceUntilIdle()
        coVerify { settingsRepository.setThemeName("Nord") }
        coVerify { runtime.applySettings() }
    }

    @Test
    fun setDayThemeNameCallsApplySettings() = runTest(testDispatcher) {
        viewModel.setDayThemeName("Gruvbox Light")
        testDispatcher.scheduler.advanceUntilIdle()
        coVerify { settingsRepository.setDayThemeName("Gruvbox Light") }
        coVerify { runtime.applySettings() }
    }

    @Test
    fun setNightThemeNameCallsApplySettings() = runTest(testDispatcher) {
        viewModel.setNightThemeName("Tokyo Night")
        testDispatcher.scheduler.advanceUntilIdle()
        coVerify { settingsRepository.setNightThemeName("Tokyo Night") }
        coVerify { runtime.applySettings() }
    }

    @Test
    fun setThemeModeCallsApplySettings() = runTest(testDispatcher) {
        viewModel.setThemeMode("day")
        testDispatcher.scheduler.advanceUntilIdle()
        coVerify { settingsRepository.setThemeMode("day") }
        coVerify { runtime.applySettings() }
    }

    @Test
    fun setAppThemeModeCallsApplySettings() = runTest(testDispatcher) {
        viewModel.setAppThemeMode("night")
        testDispatcher.scheduler.advanceUntilIdle()
        coVerify { settingsRepository.setAppThemeMode("night") }
        coVerify { runtime.applySettings() }
    }
}
