#!/usr/bin/env -S nix develop --command nu
# Usage: scripts/setup-emulator.nu [--boot_timeout 360]
# Starts the emulator and exits after boot. Emulator survives script exit.
# Uses setsid --fork to detach the emulator process from the script.

let sdk_root = "/usr/local/lib/android/sdk"

def wait-for-boot [--boot_timeout: int = 360] {
    let start = (date now)
    loop {
        let boot = (try { ^adb shell getprop sys.boot_completed } catch { "" } | str trim)
        if $boot == "1" {
            print "Emulator booted"
            return
        }
        if ((date now) - $start) > ($boot_timeout * 1sec) {
            let log_path = ($env.HOME | path join ".android" "avd" "torvox_test.avd" "emulator.log")
            if ($log_path | path exists) {
                print "=== EMULATOR LOG (last 30 lines) ==="
                open $log_path | lines | last 30 | each { print $in }
            }
            error make { msg: $"Emulator did not boot within ($boot_timeout)s" }
        }
        sleep 5sec
    }
}

def main [--boot_timeout: int = 360] {
    $env.ANDROID_AVD_HOME = ($env.HOME | path join ".android")

    let boot = (try { ^adb shell getprop sys.boot_completed } catch { "" } | str trim)
    if $boot == "1" {
        print "Emulator already running and booted"
        ^adb shell echo "ready"
        return
    }

    let avdmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "avdmanager")
    let avd_ini = ($env.ANDROID_AVD_HOME | path join "avd" "torvox_test.ini")
    let avd_dir = ($env.ANDROID_AVD_HOME | path join "avd")
    mkdir $avd_dir

    if not ($avd_ini | path exists) {
        let system_image = "system-images;android-35;google_apis;x86_64"
        let sdkmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "sdkmanager")
        let images_dir = ($sdk_root | path join "system-images")
        if not ($images_dir | path exists) {
            ^($sdkmanager_path) "--install" $system_image
        }
        ^($avdmanager_path) create avd --name torvox_test --package $system_image --device "pixel_6" --force
    }

    let emulator_path = ($sdk_root | path join "emulator" "emulator")
    ^setsid --fork ($emulator_path) -avd torvox_test -no-window -gpu swiftshader_indirect -no-audio -no-boot-anim -port 5554 -no-snapshot -no-metrics -wipe-data -memory 2048
    wait-for-boot --boot_timeout $boot_timeout
    let sdk = (^adb shell getprop ro.build.version.sdk | str trim)
    if $sdk != "35" {
        error make { msg: $"Expected SDK 35, got: ($sdk)" }
    }
    print $"Emulator ready, SDK: ($sdk)"
}
