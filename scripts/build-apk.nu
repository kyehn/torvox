#!/usr/bin/env -S nix develop --command nu

def main [--release, --debug, --nix] {
    cd android
    let apk_base = $env.PWD | path join "app" "build" "outputs" "apk"
    for variant in ["release", "debug", "nix"] {
        let dir = $apk_base | path join $variant
        if ($dir | path exists) {
            for apk in (glob $"($dir)/*.apk") {
                rm -f $apk
            }
        }
    }
    mut build_release = $release
    mut build_debug = $debug
    mut build_nix = $nix
    if (not $release) and (not $debug) and (not $nix) {
        $build_release = true
        $build_debug = false
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

    if $build_nix {
        ^./gradlew ":app:assembleNix"
        let apks = (glob $"($apk_base)/nix/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/nix/ after assembleNix"
            exit 1
        }
    }
}
