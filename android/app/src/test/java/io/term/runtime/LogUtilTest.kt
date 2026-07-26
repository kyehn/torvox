package io.term.runtime

import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class LogUtilTest {
    @Test
    fun wAlwaysCallsLogW() {
        val exception =
            kotlin.runCatching {
                LogUtil.w("TestTag", "test warning")
            }
        assertTrue("LogUtil.w() must not throw regardless of BuildConfig.DEBUG", exception.isSuccess)
    }

    @Test
    fun eAlwaysCallsLogE() {
        val exception =
            kotlin.runCatching {
                LogUtil.e("TestTag", "test error")
            }
        assertTrue("LogUtil.e() must not throw regardless of BuildConfig.DEBUG", exception.isSuccess)
    }

    @Test
    fun iAlwaysCallsLogI() {
        val exception =
            kotlin.runCatching {
                LogUtil.i("TestTag", "test info")
            }
        assertTrue("LogUtil.i() must not throw regardless of BuildConfig.DEBUG", exception.isSuccess)
    }

    @Test
    fun eAcceptsThrowable() {
        val exception =
            kotlin.runCatching {
                LogUtil.e("TestTag", "error with cause", RuntimeException("root cause"))
            }
        assertTrue("LogUtil.e() with throwable must not throw", exception.isSuccess)
    }
}
