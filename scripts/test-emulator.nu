#!/usr/bin/env -S nix develop --command nu

def main [] {
    let repo_dir = $env.PWD
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = $repo_dir | path join "android"
    cd $android_dir
    ^./gradlew ":app:connectedDebugAndroidTest"
    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest"
    ^./gradlew ":benchmark:connectedBenchmarkNixAndroidTest" "-Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.NixBootstrapTest"
    cd $repo_dir
}
