package terminal.emulator.bridge

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * 原生库必须由预装载线程装载，不得落在主线程。
 *
 * 装载要读整个 `.so`（debug 构建实测 471ms，StrictMode 记为 `DiskReadViolation`）。
 * 落在首帧前的主线程上，冷启动就得串行等它读完；故 `TerminalApp` 起一条
 * `native-preload` 守护线程去触碰 `NativeBridge`，让装载与 Hilt/Compose 的启动并行。
 *
 * 这个修复曾被整段删掉过而没有任何用例变红，故在此钉住：预装载线程必须是
 * 触碰者，且库必须真的装上了。
 */
@RunWith(AndroidJUnit4::class)
class NativePreloadInstrumentedTest {

    @Test
    fun the_native_library_is_loaded_off_the_main_thread() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        assertTrue(
            "应用进程内原生库必须已装载",
            NativeBridge.isNativeLoaded(),
        )
        val loaderThread = NativeBridge.loaderThreadName
        assertEquals(
            "原生库必须由预装载线程装载，不得落在主线程",
            PRELOAD_THREAD_NAME,
            loaderThread,
        )
    }

    private companion object {
        /** `TerminalApp` 起的预装载线程名。 */
        const val PRELOAD_THREAD_NAME = "native-preload"
    }
}
