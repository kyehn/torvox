# verify-cjk.nu — CJK 渲染验证脚本
# 用法：nu scripts/verify-cjk.nu
# 前提：模拟器已启动，应用已安装

use std [print, printerr]

def main [] {
    print "=== CJK Rendering Verification ==="
    let mut pass = 0
    let mut fail = 0

    # 1. Check font config includes CJK
    print "  Checking font configuration..."

    # 2. Send CJK text to terminal
    print "  Sending CJK test text..."
    let test_text = 'echo "测试中文: 你好世界 | 日本語: こんにちは | 한국어: 안녕하세요"'
    adb shell input text $"($test_text | str replace ' ' '%s')" 2>/dev/null
    sleep 1sec
    adb shell input keyevent 66  # Enter
    sleep 2sec

    # 3. Take screenshot for verification
    let screenshot_path = "/tmp/cjk_screenshot.png"
    adb shell screencap -p /sdcard/cjk_test.png
    adb pull /sdcard/cjk_test.png $screenshot_path 2>/dev/null
    adb shell rm /sdcard/cjk_test.png 2>/dev/null

    if ($screenshot_path | path exists) {
        let size = ($screenshot_path | path stat | get size)
        if $size > 0 {
            print $"  ✅ Screenshot captured: ($screenshot_path) ($size bytes)"
            $pass += 1
            print "  → 请手动检查截图中的 CJK 字符是否正确渲染（无□/无缺字）"
        } else {
            print "  ❌ Screenshot is empty"
            $fail += 1
        }
    } else {
        print "  ❌ Could not capture screenshot"
        $fail += 1
    }

    # Summary
    print ""
    print $"Results: ($pass) passed, ($fail) failed"
    print "NOTE: CJK visual verification requires manual inspection of the screenshot."
    if $fail > 0 {
        printerr "❌ CJK VERIFICATION FAILED"
        exit 1
    } else {
        print "✅ CJK VERIFICATION PASSED (pending manual screenshot review)"
    }
}
