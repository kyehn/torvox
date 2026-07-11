package io.torvox.bridge

import android.util.Log
import android.view.Surface

object NativeWindow {
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("torvox_android")
            nativeLoaded = true
        } catch (exception: UnsatisfiedLinkError) {
            Log.w("NativeWindow", "libtorvox_android not loaded: ${exception.message}")
        }
    }

    fun isNativeLoaded(): Boolean = nativeLoaded

    @JvmStatic
    external fun getNativeWindowPtr(surface: Surface): Long
}
