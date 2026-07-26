package io.term.bridge

import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class NativeWindowTest {
    @Test
    fun nativeLoadedWithoutNativeLibIsFalse() {
        assertFalse("Native window bindings must not be loaded in a JVM-only unit test", NativeWindow.isNativeLoaded())
    }
}
