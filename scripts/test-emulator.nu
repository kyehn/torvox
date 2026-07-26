#!/usr/bin/env -S nix develop --command nu
# Usage: scripts/test-emulator.nu
# Runs all Gradle Android instrumentation tests + Maestro flows.
# Prerequisites: emulator must be booted (scripts/setup-emulator.nu).

def main [] {
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($env.PWD | path join "android")
    cd $android_dir

    print "=== Running instrumentation tests ==="
    try {
        ^./gradlew ":app:connectedDebugAndroidTest" "-Pandroid.testInstrumentationRunnerArguments.notPackage=io.term.benchmark"
    } catch {|e|
        print $"WARNING: Instrumentation tests failed: ($e)"
    }

    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }

    print "=== Installing release APK ==="
    try {
        ^./gradlew ":app:installRelease"
    } catch {|e|
        print $"WARNING: Release APK install failed: ($e)"
    }

    print "=== Reconnecting emulator ==="
    try { ^adb reconnect } catch {|e| print $"WARNING: adb reconnect failed: ($e)" }
    sleep 2sec

    try { ^adb wait-for-device } catch {|e| print $"WARNING: adb wait-for-device failed: ($e)" }
    try { ^adb shell true } catch {|e| print $"WARNING: adb shell check failed: ($e)" }

    print "=== Verifying release APK installation ==="
    let pkg_check = (^adb shell pm list packages com.termux | complete)
    if not ($pkg_check.stdout | str contains "package:com.termux") {
        print "WARNING: com.termux not found after install, retrying install..."
        try {
            ^./gradlew ":app:installRelease"
        } catch {|e|
            print $"WARNING: Retry install also failed: ($e)"
        }
        sleep 3sec
    }

    print "=== Running benchmarks ==="
    try { ^./gradlew "benchmark:lockClocks" } catch {|e| print $"WARNING: lockClocks failed: ($e)" }
    try { ^./gradlew ":benchmark:connectedReleaseAndroidTest" } catch {|e| print $"WARNING: Benchmark tests failed: ($e)" }
    try { ^./gradlew ":baselineprofile:generateBaselineProfile" } catch {|e| print $"WARNING: Baseline profile generation failed: ($e)" }

    cd $env.PWD
    let maestro_dir = ($env.PWD | path join "maestro")
    let all_flows = [
        (glob $"($maestro_dir)/flows/*.yml")
        (glob $"($maestro_dir)/suites/*.yml")
    ] | flatten
    for flow in $all_flows {
        try {
            ^maestro test $flow
        } catch {|e|
            print $"WARNING: Maestro flow ($flow) failed: ($e)"
        }
    }
}
