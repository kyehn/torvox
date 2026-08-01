#!/usr/bin/env -S nix develop --command nu
# Usage: scripts/test-emulator.nu
# Runs all Gradle Android instrumentation tests + Maestro flows.
# Prerequisites: emulator must be booted (scripts/setup-emulator.nu).

def main [] {
    # Capture the repo root BEFORE any cd: `cd android` mutates $env.PWD
    # in Nushell, so a later `cd $env.PWD` would return to android/ and
    # break relative paths like maestro/flows.
    let repo_dir = $env.PWD
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($repo_dir | path join "android")
    cd $android_dir

    print "=== Running instrumentation tests ==="
    # The benchmark module is a separate Gradle module
    # (android/benchmark), so the app's connectedDebugAndroidTest never
    # contains benchmark classes; the notPackage filter was stale.
    try {
        ^./gradlew ":app:connectedDebugAndroidTest"
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

    cd $repo_dir
    # `maestro test` accepts flow files or a flows folder, not suite
    # files (a suite YAML has no appId/commands section and fails to
    # parse). The suite tags select the same flows the suite YAMLs
    # aggregate, without double execution.
    let maestro_dir = ($repo_dir | path join "maestro")
    try {
        ^maestro test ($maestro_dir | path join "flows") --include-tags smoke,e2e
    } catch {|e|
        print $"WARNING: Maestro flows failed: ($e)"
    }
}
