#!/usr/bin/env -S nix develop --command nu
# Run Android Kotlin/Gradle checks
# Note: connectedAndroidTest requires an emulator — use test-emulator.nu instead.
# Usage:
#   scripts/test-gradle.nu  # lint + unit tests + dokka docs
#
# Roborazzi screenshots live in androidTest (device-side, via
# test-emulator.nu); there are no local unit tests calling captureRoboImage,
# so app:recordRoborazziDebug is intentionally not invoked here.

def main [] {
    # Host-built libnative.so feeds NativeBridgeSmokeTest (JVM JNI round-trip
    # tests). Without it those tests skip; with it they catch JNI-boundary
    # regressions that would otherwise need the emulator.
    cargo build --package native
    cd android
    # assembleDebugAndroidTest compiles the androidTest sources (TestUtils,
    # cucumber steps, instrumented suites). testDebugUnitTest does NOT touch
    # them — without this task, androidTest compile errors surface only in
    # the 40-minute emulator run in the Release workflow.
    ./gradlew spotlessCheck detekt app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:testReleaseUnitTest baselineprofile:testDebugUnitTest -Dorg.gradle.internal.test.results.binary.enabled=false
}
