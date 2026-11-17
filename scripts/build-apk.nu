#!/usr/bin/env -S nix develop --command nu

def main [--release, --debug, --nix] {
    cd android
    let apk_base = $env.PWD | path join "app" "build" "outputs" "apk"

    mut build_release = $release
    mut build_debug = $debug
    mut build_nix = $nix

    if (not $release) and (not $debug) and (not $nix) {
        $build_release = true
    }

    let targets = [
        { name: "release", build: $build_release, task: ":app:assembleRelease" },
        { name: "debug",   build: $build_debug,   task: ":app:assembleDebug"   },
        { name: "nix",     build: $build_nix,     task: ":app:assembleNix"     },
    ]

    for target in $targets {
        if not $target.build {
            continue
        }

        let dir = $apk_base | path join $target.name
        if ($dir | path exists) {
            for apk in (glob $"($dir)/*.apk") {
                rm -f $apk
            }
        }

        ^./gradlew $target.task

        let apks = (glob $"($apk_base)/($target.name)/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/($target.name)/ after ($target.task)"
            exit 1
        }
    }
}
