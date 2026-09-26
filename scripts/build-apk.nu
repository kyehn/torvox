#!/usr/bin/env -S nix develop --command nu

const JNILIBS = "android/app/src/main/jniLibs"

# APK 必须至少含一个 libnative.so，否则 minify 或打包配置把原生库弄丢了（见 docs/specification/BUILD.md）。
def assert-apk-has-native-lib [apk_path: string] {
    let entries = (^zipinfo -1 $apk_path | lines | where {|e| $e starts-with "lib/"})
    if ($entries | is-empty) {
        print $"ERROR: ($apk_path) 内没有任何 lib/ 下的原生库"
        exit 1
    }
}

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
        $build_debug = false
    }

    for abi in ["arm64-v8a", "x86_64"] {
        let so = $env.PWD | path join $JNILIBS $abi "libnative.so"
        if not ($so | path exists) {
            print $"ERROR: jniLibs 未填充，缺少 ($so)"
            exit 1
        }
    }

    if $build_release {
        ^./gradlew ":app:assembleRelease"
        let apks = (glob $"($apk_base)/release/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/release/ after assembleRelease"
            exit 1
        }
        for apk in $apks {
            assert-apk-has-native-lib $apk
        }
    }

    if $build_debug {
        ^./gradlew ":app:assembleDebug"
        let apks = (glob $"($apk_base)/debug/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/debug/ after assembleDebug"
            exit 1
        }
        for apk in $apks {
            assert-apk-has-native-lib $apk
        }
    }
}
