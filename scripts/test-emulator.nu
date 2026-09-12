#!/usr/bin/env -S nix develop --command nu

def main [] {
    let repo_dir = $env.PWD
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($repo_dir | path join "android")
    cd $android_dir
    ^./gradlew ":app:connectedDebugAndroidTest"
    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }
    ^./gradlew ":app:installRelease"
    ^./gradlew "benchmark:lockClocks"
    ^./gradlew ":benchmark:connectedReleaseAndroidTest"
    ^./gradlew ":benchmark:connectedReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#modifierKeyPressAnimation
    ^./gradlew ":benchmark:connectedReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#imeShowAnimation
    ^./gradlew ":baselineprofile:generateBaselineProfile"
}
