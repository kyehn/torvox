package io.torvox

import android.content.ComponentName
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AppInstrumentedTest {
    companion object {
        private val TARGET_PKG = "com.termux"
        private val MAIN_ACTIVITY = ComponentName(TARGET_PKG, "io.torvox.MainActivity")
    }

    @Test
    fun appContextIsCorrect() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        assertEquals(TARGET_PKG, appContext.packageName)
    }

    @Test
    fun appHasMainActivity() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val pm = appContext.packageManager
        val intent = pm.getLaunchIntentForPackage(TARGET_PKG)
        assertNotNull("App has launch intent", intent)
    }

    @Test
    fun appHasCorrectMinSdk() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val appInfo =
            appContext.packageManager.getApplicationInfo(TARGET_PKG, 0)
        assertTrue("minSdk >= 33", appInfo.minSdkVersion >= 33)
    }
}
