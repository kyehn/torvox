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
    cd android
    ./gradlew spotlessCheck detekt app:dokkaGenerate lintDebug lintVitalRelease testDebugUnitTest benchmark:testReleaseUnitTest baselineprofile:testDebugUnitTest -Dorg.gradle.internal.test.results.binary.enabled=false
}
