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
    # Failures must fail the job: swallowing them here produced green
    # Release runs that had actually shipped with broken tests.
    # Retry up to 2 times to mitigate PlayStore emulator cold-start flakiness
    # (permission dialog, drawer animation, swiftshader slowness). Each retry
    # fully re-runs the suite; the first success short-circuits.
    # Use `complete` to avoid Nushell's mutable capture in catch closures.
    mut instrumentation_passed = false
    for attempt in 1..3 {
        print $"=== Instrumentation attempt ($attempt)/3 ==="
        let result = (do { ^./gradlew ":app:connectedDebugAndroidTest" } | complete)
        if $result.exit_code == 0 {
            $instrumentation_passed = true
            break
        }
        print $"WARN: Instrumentation attempt ($attempt) failed with exit code ($result.exit_code)"
        if $attempt == 3 {
            print $"ERROR: Instrumentation tests failed after 3 attempts"
            print $result.stderr
            exit 1
        }
        print "Retrying instrumentation tests after 10s cooldown..."
        sleep 10sec
        try { ^adb shell am force-stop com.termux } catch { null }
        sleep 2sec
    }
    if not $instrumentation_passed {
        print "ERROR: Instrumentation tests did not pass"
        exit 1
    }

    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }

    print "=== Installing release APK ==="
    try {
        ^./gradlew ":app:installRelease"
    } catch {|e|
        print $"ERROR: Release APK install failed: ($e)"
        exit 1
    }

    print "=== Reconnecting emulator ==="
    try { ^adb reconnect } catch {|e| print $"ERROR: adb reconnect failed: ($e)"; exit 1 }
    sleep 2sec

    try { ^adb wait-for-device } catch {|e| print $"ERROR: adb wait-for-device failed: ($e)"; exit 1 }
    try { ^adb shell true } catch {|e| print $"ERROR: adb shell check failed: ($e)"; exit 1 }

    print "=== Verifying release APK installation ==="
    let pkg_check = (^adb shell pm list packages com.termux | complete)
    if not ($pkg_check.stdout | str contains "package:com.termux") {
        print "ERROR: com.termux not found after install, retrying install..."
        try {
            ^./gradlew ":app:installRelease"
        } catch {|e|
            print $"ERROR: Retry install also failed: ($e)"
            exit 1
        }
        sleep 3sec
        let pkg_check2 = (^adb shell pm list packages com.termux | complete)
        if not ($pkg_check2.stdout | str contains "package:com.termux") {
            print "ERROR: com.termux still not installed after retry"
            exit 1
        }
    }

    print "=== Running benchmarks ==="
    try { ^./gradlew "benchmark:lockClocks" } catch {|e| print $"ERROR: lockClocks failed: ($e)"; exit 1 }
    # Run each interaction benchmark as its own instrumentation
    # invocation: the software-rendered emulator exhausts itself during
    # a combined run and the UTP output plugin dies with "Writing local
    # file failed!" on the last test method.
    try { ^./gradlew ":benchmark:connectedReleaseAndroidTest" } catch {|e| print $"ERROR: Benchmark tests failed: ($e)"; exit 1 }
    try { ^./gradlew ":benchmark:connectedReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#modifierKeyPressAnimation } catch {|e| print $"ERROR: modifierKeyPressAnimation failed: ($e)"; exit 1 }
    try { ^./gradlew ":benchmark:connectedReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#imeShowAnimation } catch {|e| print $"ERROR: imeShowAnimation failed: ($e)"; exit 1 }
    try { ^./gradlew ":baselineprofile:generateBaselineProfile" } catch {|e| print $"ERROR: Baseline profile generation failed: ($e)"; exit 1 }

    cd $repo_dir
    # `maestro test` accepts flow files or a flows folder, not suite
    # files (a suite YAML has no appId/commands section and fails to
    # parse). The suite tags select the same flows the suite YAMLs
    # aggregate, without double execution.
    let maestro_dir = ($repo_dir | path join "maestro")
    try {
        ^maestro test ($maestro_dir | path join "flows") --include-tags smoke,e2e
    } catch {|e|
        print $"ERROR: Maestro flows failed: ($e)"
        exit 1
    }
}
