package io.torvox.bridge

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class NativeWindowTest {
    @Test
    fun nativeWindowObjectIsNotNull() {
        assertNotNull(NativeWindow)
    }

    @Test
    fun nativeLoadedWithoutNativeLibIsFalse() {
        assertFalse(NativeWindow.isNativeLoaded())
    }
}
