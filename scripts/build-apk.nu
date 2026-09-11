#!/usr/bin/env -S nix develop --command nu
# Build APKs: release + debug, verify .so bundling
# Requires: build-android-libs.nu has populated jniLibs
# Usage: scripts/build-apk.nu [--release] [--debug]

const MINIMUM_APK_SIZE_BYTES = 5_000_000

def main [--release, --debug] {
    cd android

    let apk_base = $env.PWD | path join "app" "build" "outputs" "apk"

    # Clean old APK artifacts
    for variant in ["release", "debug"] {
        let dir = $apk_base | path join $variant
        if ($dir | path exists) {
            for apk in (glob $"($dir)/*.apk") {
                rm --force $apk
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
        let apk = $apks | first
        let size = (stat --format=%s $apk | str trim | into int)
        if $size < $MINIMUM_APK_SIZE_BYTES {
            print $"ERROR: ($apk) size ($size) below minimum ($MINIMUM_APK_SIZE_BYTES)"
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
        let apk = $apks | first
        let size = (stat --format=%s $apk | str trim | into int)
        if $size < $MINIMUM_APK_SIZE_BYTES {
            print $"ERROR: ($apk) size ($size) below minimum ($MINIMUM_APK_SIZE_BYTES)"
            exit 1
        }
    }

    # Verify .so bundling in all built APKs
    let variants = if $build_release and $build_debug { ["release", "debug"] } else if $build_release { ["release"] } else { ["debug"] }
    for variant in $variants {
        let apk_dir = $apk_base | path join $variant
        for apk_path in (glob $"($apk_dir)/*.apk") {
            let so_lines = (unzip -l $apk_path | lines | find ".so")
            if ($so_lines | is-empty) {
                print $"ERROR: ($apk_path) contains no .so files"
                exit 1
            }
        }
    }
}
