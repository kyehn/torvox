#!/usr/bin/env -S nix develop --command nu
# verify-emulator.nu — 模拟器验证脚本
# 用法：nu scripts/verify-emulator.nu
# 前提：模拟器已启动，adb 已连接

use std [print, printerr]

def main [] {
    print "=== Emulator Verification ==="
    let mut pass = 0
    let mut fail = 0

    # 1. Check ADB connection
    let adb_devices = (adb devices 2>&1 | complete)
    if not ($adb_devices.stdout | str contains "emulator") {
        print "  ❌ No emulator connected"
        $fail += 1
        print ""
        print $"Results: ($pass) passed, ($fail) failed"
        exit 1
    }
    print "  ✅ Emulator connected"
    $pass += 1

    # 2. Check API level
    let api_level = (adb shell getprop ro.build.version.sdk 2>&1 | complete | get stdout | str trim)
    print $"  API level: ($api_level)"
    if ($api_level | into int) >= 33 {
        print "  ✅ API level >= 33"
        $pass += 1
    } else {
        print $"  ❌ API level ($api_level) < 33"
        $fail += 1
    }

    # 3. Launch activity
    print "  Launching TerminalActivity..."
    let start_output = (adb shell am start -n com.terminal.emulator/.TerminalActivity 2>&1 | complete)
    if ($start_output.stdout | str contains "Error") {
        print "  ❌ Failed to launch activity"
        $fail += 1
    } else {
        print "  ✅ Activity launched"
        $pass += 1
    }

    # 4. Wait for rendering
    print "  Waiting 5s for rendering..."
    sleep 5sec

    # 5. Collect frame stats
    let frame_file = "/tmp/framestats.txt"
    adb shell dumpsys gfxinfo com.terminal.emulator framestats | save --force $frame_file

    let frame_content = (open $frame_file | to text)
    if ($frame_content | str contains "Total frames rendered") {
        print "  ✅ Frame stats collected"
        $pass += 1

        # Parse total frames
        let total_match = ($frame_content | parse "Total frames rendered: {count}" | first | get count)
        print $"  Total frames: ($total_match)"

        # Check for Janky frames
        if ($frame_content | str contains "Janky frames") {
            let janky_line = ($frame_content | lines | where ($it | str contains "Janky frames") | first)
            print $"  ($janky_line | str trim)"
        }
    } else {
        print "  ❌ Could not collect frame stats (activity may not be running)"
        $fail += 1
    }

    # 6. Check Vulkan availability
    let vulkan = (adb shell getprop ro.hardware.vulkan 2>&1 | complete | get stdout | str trim)
    print $"  Vulkan: ($vulkan | if $it == '' {'not available'} else {$it})"
    if ($vulkan | str length) > 0 {
        print "  ✅ Vulkan available"
        $pass += 1
    } else {
        print "  ⚠️  Vulkan not detected (may use software renderer)"
    }

    # 7. Memory usage
    let mem_output = (adb shell dumpsys meminfo com.terminal.emulator 2>&1 | complete)
    if ($mem_output.stdout | str contains "TOTAL") {
        let total_line = ($mem_output.stdout | lines | where ($it | str contains "TOTAL") | first)
        print $"  ($total_line | str trim)"
    }

    # Summary
    print ""
    print $"Results: ($pass) passed, ($fail) failed"
    if $fail > 0 {
        printerr "❌ EMULATOR VERIFICATION FAILED"
        exit 1
    } else {
        print "✅ EMULATOR VERIFICATION PASSED"
    }
}
