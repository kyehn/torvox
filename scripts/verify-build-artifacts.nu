#!/usr/bin/env -S nix develop --command nu
# verify-build-artifacts.nu — 构建产物验证脚本
# 用法：nu scripts/verify-build-artifacts.nu

use std [print, printerr]

def main [] {
    print "=== Build Artifact Verification ==="
    let mut pass = 0
    let mut fail = 0

    # 1. Check .so exists and size
    let so_path = "target/aarch64-linux-android/release/libnative.so"
    if ($so_path | path exists) {
        let size_bytes = ($so_path | path stat | get size)
        let size_mb = ($size_bytes | into float) / 1048576.0
        print $"  .so size: ($size_mb | format number)M"
        if $size_mb >= 14.0 and $size_mb <= 18.0 {
            print "  ✅ .so size in range [14M, 18M]"
            $pass += 1
        } else {
            print $"  ❌ .so size out of range: ($size_mb)M (expected 14-18M)"
            $fail += 1
        }
    } else {
        print $"  ❌ .so not found at ($so_path)"
        $fail += 1
    }

    # 2. Check readelf for ghostty NEEDED
    if ($so_path | path exists) {
        let readelf_output = (readelf --dynamic $so_path 2>&1 | complete)
        if ($readelf_output.stdout | str contains "ghostty") {
            print "  ❌ NEEDED ghostty found in .so"
            $fail += 1
        } else {
            print "  ✅ No NEEDED ghostty"
            $pass += 1
        }
    }

    # 3. Check APK contains .so
    let apk_files = (glob "android/app/build/outputs/apk/debug/*.apk")
    if ($apk_files | length) > 0 {
        let apk = ($apk_files | first)
        let unzip_output = (unzip -l $apk 2>&1 | complete)
        if ($unzip_output.stdout | str contains "libnative.so") {
            print $"  ✅ APK contains libnative.so: ($apk | path basename)"
            $pass += 1
        } else {
            print "  ❌ APK does not contain libnative.so"
            $fail += 1
        }
    } else {
        print "  ⚠️  No APK found (run ./gradlew :app:assembleDebug first)"
    }

    # Summary
    print ""
    print $"Results: ($pass) passed, ($fail) failed"
    if $fail > 0 {
        printerr "❌ BUILD ARTIFACT VERIFICATION FAILED"
        exit 1
    } else {
        print "✅ BUILD ARTIFACT VERIFICATION PASSED"
    }
}
