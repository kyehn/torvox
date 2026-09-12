#!/usr/bin/env -S nix develop --command nu

let sdk_root = "/usr/local/lib/android/sdk"
let avd_home = ($env.HOME | path join ".android")
let avd_ini = ($avd_home | path join "test_avd.ini")
let avd_dir = ($avd_home | path join "test_avd.avd")
let emulator_log = ($avd_dir | path join "emulator.log")
let emulator_log_err = ($avd_dir | path join "emulator.err.log")

def emulator-alive [] {
    let ids = (try { ^pgrep -f qemu-system } catch { "" } | str trim)
    not ($ids | is-empty)
}

def print-log-tail [path: path, lines: int] {
    if ($path | path exists) {
        print "=== EMULATOR LOG (last ($lines) lines) ==="
        open $path | lines | last $lines | each { print $in }
    } else {
        print $"(ansi yellow)no emulator log at ($path)(ansi reset)"
    }
}

def print-emulator-logs [] {
    print-log-tail $emulator_log_err 25
    print-log-tail $emulator_log 30
}

def wait-for-boot [--boot_timeout: int] {
    let start = (date now)
    loop {
        let boot = (try { ^adb shell getprop sys.boot_completed } catch { "" } | str trim)
        if $boot == "1" {
            print "Emulator booted"
            return
        }
        if not (emulator-alive) {
            print $"(ansi red)EMULATOR PROCESS DIED during boot(ansi reset)"
            print-emulator-logs
            error make { msg: "Emulator process exited before boot completed (likely a host-side SEGV)" }
        }
        if ((date now) - $start) > ($boot_timeout * 1sec) {
            print-emulator-logs
            error make { msg: $"Emulator did not boot within ($boot_timeout)s" }
        }
        sleep 5sec
    }
}

def main [--boot_timeout: int = 360, --gpu: string = "swiftshader_indirect", --keep-data] {
    $env.ANDROID_AVD_HOME = $avd_home

    let boot = (try { ^adb shell getprop sys.boot_completed } catch { "" } | str trim)
    if $boot == "1" {
        print "Emulator already running and booted"
        ^adb shell echo "ready"
        return
    }

    let avdmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "avdmanager")
    let sdkmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "sdkmanager")
    let system_image = "system-images;android-35;google_apis;x86_64"
    let system_image_dir = ($sdk_root | path join "system-images" "android-35" "google_apis" "x86_64")

    if not ($system_image_dir | path exists) {
        ^($sdkmanager_path) "--install" $system_image
    }
    let emulator_pkg = ($sdk_root | path join "emulator" "emulator")
    if not ($emulator_pkg | path exists) {
        ^($sdkmanager_path) "--install" "emulator"
    }
    if not ($avd_ini | path exists) {
        ^($avdmanager_path) create avd --name test_avd --package $system_image --device "pixel_6" --force
    }

    if (emulator-alive) {
        print "Emulator process already running but not yet booted — waiting for boot instead of spawning a second instance"
        wait-for-boot --boot_timeout $boot_timeout
        let sdk = (^adb shell getprop ro.build.version.sdk | str trim)
        if $sdk != "35" {
            error make { msg: $"Expected SDK 35, got: ($sdk)" }
        }
        let gles = (^adb shell "dumpsys SurfaceFlinger | grep 'GLES:'" | str trim)
        print $"Emulator ready, SDK: ($sdk)"
        print $"GLES backend: ($gles)"
        return
    }

    let emulator_path = ($sdk_root | path join "emulator" "emulator")
    mkdir $avd_dir
    rm -f $emulator_log $emulator_log_err
    rm -f ($avd_home | path join "emu-update-last-check.ini")
    let wipe_flag = (if $keep_data { [] } else { ["-wipe-data"] })
    $env.QT_QPA_PLATFORM = "offscreen"
    ^setsid --fork ($emulator_path) -avd test_avd -no-window -gpu $gpu -no-audio -no-boot-anim -port 5554 -no-snapshot -no-metrics ...$wipe_flag -memory 2048 o> $emulator_log e> $emulator_log_err
    wait-for-boot --boot_timeout $boot_timeout
    let sdk = (^adb shell getprop ro.build.version.sdk | str trim)
    if $sdk != "35" {
        error make { msg: $"Expected SDK 35, got: ($sdk)" }
    }
    let gles = (^adb shell "dumpsys SurfaceFlinger | grep 'GLES:'" | str trim)
    print $"Emulator ready, SDK: ($sdk)"
    print $"GLES backend: ($gles)"
}