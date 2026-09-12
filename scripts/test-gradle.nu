#!/usr/bin/env -S nix develop --command nu

def main [] {
    cd android
    ./gradlew spotlessCheck detekt app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:testReleaseUnitTest baselineprofile:testDebugUnitTest -Dorg.gradle.internal.test.results.binary.enabled=false
}
