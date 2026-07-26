package io.term.bridge

import android.util.Log
import android.view.Surface

/**
 * Native JNI window operations — ANativeWindow pointer acquisition.
 *
 * @see FR-051 Surface: JNI/NDK ANativeWindow
 */
object NativeWindow {
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("android_gui_lib")
            nativeLoaded = true
        } catch (exception: UnsatisfiedLinkError) {
            Log.w("NativeWindow", "libtorvox_android not loaded: ${exception.message}")
        }
    }

    fun isNativeLoaded(): Boolean = nativeLoaded

    @JvmStatic
    external fun getNativeWindowPtr(surface: Surface): Long
}
