package io.torvox.test

import android.os.Bundle
import androidx.test.runner.AndroidJUnitRunner

/**
 * Custom test runner that delegates directly to AndroidJUnitRunner without
 * Cucumber bundle hijacking. Used for running non-Cucumber tests.
 */
class DirectTestRunner : AndroidJUnitRunner() {
    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
    }
}
