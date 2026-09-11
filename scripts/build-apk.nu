#!/usr/bin/env -S nix develop --command nu

def main [--release, --debug] {
    cd android
    let apk_base = $env.PWD | path join "app" "build" "outputs" "apk"
    for variant in ["release", "debug"] {
        let dir = $apk_base | path join $variant
        if ($dir | path exists) {
            for apk in (glob $"($dir)/*.apk") {
                rm -f $apk
            }
        }
    }
    mut build_release = $release
    mut build_debug = $debug
    if (not $release) and (not $debug) {
        $build_release = true
        $build_debug = true
    }

    if $build_release {
        ^./gradlew ":app:assembleRelease"
        let apks = (glob $"($apk_base)/release/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/release/ after assembleRelease"
            exit 1
        }
    }

    if $build_debug {
        ^./gradlew ":app:assembleDebug"
        let apks = (glob $"($apk_base)/debug/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/debug/ after assembleDebug"
            exit 1
        }
    }
}
