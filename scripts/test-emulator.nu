#!/usr/bin/env -S nix develop --command nu
# Usage: scripts/test-emulator.nu
# Runs all Gradle Android instrumentation tests + Maestro flows.
# Prerequisites: emulator must be booted (scripts/setup-emulator.nu).

def main [] {
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($env.PWD | path join "android")
    cd $android_dir
    ^./gradlew :app:connectedDebugAndroidTest
    try { ^adb uninstall com.termux } catch { null }
    ^./gradlew benchmark:lockClocks
    ^./gradlew :benchmark:connectedReleaseAndroidTest
    ^./gradlew :baselineprofile:generateBaselineProfile

    cd $env.PWD
    let maestro_dir = ($env.PWD | path join "maestro")
    let all_flows = [
        (glob $"($maestro_dir)/flows/*.yml")
        (glob $"($maestro_dir)/suites/*.yml")
    ] | flatten
    for flow in $all_flows {
        ^maestro test $flow
    }
}
