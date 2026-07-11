#!/usr/bin/env -S nix develop --command nu
# Run Android Kotlin/Gradle checks
# Note: connectedAndroidTest requires an emulator — use test-emulator.nu instead.
# Usage:
#   scripts/test-android-gradle.nu  # lint + unit tests + verify screenshots

def main [] {
    cd android
    ./gradlew spotlessCheck detekt app:dokkaGenerate lintDebug lintVitalRelease testDebugUnitTest benchmark:testReleaseUnitTest baselineprofile:testDebugUnitTest app:recordRoborazziDebug -Dorg.gradle.internal.test.results.binary.enabled=false
}
