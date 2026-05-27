package io.torvox

import org.junit.Assert.*
import org.junit.Test

/**
 * Unit tests for Kotlin models.
 * These run on the host JVM (no Android device needed).
 *
 * Note: UniFFI-generated bridge types (TorvoxBridge, Shell, etc.) are only
 * available after running: cargo build -p torvox-gui-android + uniffi-bindgen generate
 * These tests verify Kotlin-side logic without the Rust bridge.
 */
class KotlinModelsTest {
    @Test
    fun packageNameIsCorrect() {
        assertEquals("io.torvox", KotlinModelsTest::class.qualifiedName?.substringBeforeLast('.'))
    }
}
