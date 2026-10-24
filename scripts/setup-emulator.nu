#!/usr/bin/env -S nix develop --command nu

let sdk_root = "/usr/local/lib/android/sdk"
let avd_home = ($env.HOME | path join ".android")
let avd_ini = ($avd_home | path join "test_avd.ini")
let avd_dir = ($avd_home | path join "test_avd.avd")
let emulator_log = ($avd_dir | path join "emulator.log")
let emulator_log_err = ($avd_dir | path join "emulator.err.log")

def adb_output [args: list<string>] {
    (^adb ...$args | complete).stdout
}

def emulator_alive [] {
    adb_output [devices] | lines | any {|line| $line starts-with "emulator-5554" }
}

def print_log_tail [path: path, lines: int] {
    if ($path | path exists) {
        ^tail -n $lines $path
    } else {
        print $"(ansi yellow)no emulator log at ($path)(ansi reset)"
    }
}

def print_emulator_logs [] {
    print_log_tail $emulator_log_err 25
    print_log_tail $emulator_log 30
}

def wait_for_boot [--boot_timeout: int] {
    let start = (date now)
    loop {
        let boot = (adb_output [-s emulator-5554 shell getprop sys.boot_completed] | str trim)
        if $boot == "1" {
            print "Emulator booted"
            return
        }
        if ((date now) - $start) > 10sec and not (emulator_alive) {
            print $"(ansi red)EMULATOR PROCESS DIED during boot(ansi reset)"
            print_emulator_logs
            error make { msg: "Emulator process exited before boot completed (likely a host-side SEGV)" }
        }
        if ((date now) - $start) > ($boot_timeout * 1sec) {
            print_emulator_logs
            error make { msg: $"Emulator did not boot within ($boot_timeout)s" }
        }
        sleep 5sec
    }
}

def main [--boot_timeout: int = 360, --gpu: string = "swiftshader_indirect", --keep_data] {
    $env.ANDROID_AVD_HOME = $avd_home

    let booted = (adb_output [-s emulator-5554 shell getprop sys.boot_completed] | str trim) == "1"
    if not $booted {
        if (emulator_alive) {
            print "Emulator process already running but not yet booted — waiting for boot instead of spawning a second instance"
        } else {
            let avdmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "avdmanager")
            let sdkmanager_path = ($sdk_root | path join "cmdline-tools" "latest" "bin" "sdkmanager")
            let system_image = "system-images;android-35;google_apis;x86_64"
            let system_image_dir = ($sdk_root | path join "system-images" "android-35" "google_apis" "x86_64")

            if not ($system_image_dir | path exists) {
                ^($sdkmanager_path) "--install" $system_image
            }
            if not (($sdk_root | path join "emulator" "emulator") | path exists) {
                ^($sdkmanager_path) "--install" "emulator"
            }
            if not (($avd_ini | path exists) and ($avd_dir | path exists)) {
                "no\n" | ^($avdmanager_path) create avd --name test_avd --package $system_image --force
            }

            mkdir $avd_dir
            rm -f $emulator_log $emulator_log_err
            rm -f ($avd_home | path join "emu-update-last-check.ini")
            let wipe_flag = (if $keep_data { [] } else { ["-wipe-data"] })
            $env.QT_QPA_PLATFORM = "offscreen"
            ^setsid --fork sh -c 'for f in /proc/self/fd/*; do n=${f##*/}; [ $n -gt 2 ] && eval "exec $n>&-"; done; exec "$@"' -- ($sdk_root | path join "emulator" "emulator") -avd test_avd -no-window -gpu $gpu -no-audio -no-boot-anim -port 5554 -no-snapshot -no-metrics ...$wipe_flag -memory 2048 -feature -Wifi -dns-server 8.8.8.8 o> $emulator_log e> $emulator_log_err
        }
        wait_for_boot --boot_timeout $boot_timeout
    }

    let sdk = (adb_output [-s emulator-5554 shell getprop ro.build.version.sdk] | str trim)
    if $sdk != "35" {
        error make { msg: $"Expected SDK 35, got: ($sdk)" }
    }
    let gles = (adb_output [-s emulator-5554 shell "dumpsys SurfaceFlinger | grep 'GLES:'"] | str trim)
    print $"Emulator ready, SDK: ($sdk)"
    print $"GLES backend: ($gles)"
}
