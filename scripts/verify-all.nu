#!/usr/bin/env -S nix develop --command nu
# verify-all.nu — 一键运行所有验证脚本
# 用法：nu scripts/verify-all.nu

use std [print, printerr]

def main [--skip-emulator] {
    print "========================================"
    print "  Torvox Master Verification Suite v8"
    print "========================================"
    print ""

    let mut total_pass = 0
    let mut total_fail = 0

    # 1. Rust tests
    print ">>> [1/5] Rust Unit Tests"
    let test_result = (cargo test -p native --lib 2>&1 | complete)
    if ($test_result.exit_code == 0) {
        let passed = ($test_result.stdout | lines | where ($it | str contains "passed") | first)
        print $"  ✅ ($passed | str trim)"
        $total_pass += 1
    } else {
        print "  ❌ Rust tests failed"
        $total_fail += 1
    }
    print ""

    # 2. Clippy
    print ">>> [2/5] Clippy Lint"
    let clippy_result = (cargo clippy -p native -- -D warnings 2>&1 | complete)
    if ($clippy_result.exit_code == 0) {
        print "  ✅ Zero warnings"
        $total_pass += 1
    } else {
        print "  ❌ Clippy warnings found"
        $total_fail += 1
    }
    print ""

    # 3. Machete
    print ">>> [3/5] Dependency Audit (cargo-machete)"
    let machete_result = (cargo machete 2>&1 | complete)
    if ($machete_result.exit_code == 0) {
        print "  ✅ No unused dependencies"
        $total_pass += 1
    } else {
        print "  ❌ Unused dependencies found"
        $total_fail += 1
    }
    print ""

    # 4. Build artifacts
    print ">>> [4/5] Build Artifact Verification"
    try {
        nu scripts/verify-build-artifacts.nu
        $total_pass += 1
    } catch {|err|
        print $"  ❌ Build artifact verification failed: ($err.msg)"
        $total_fail += 1
    }
    print ""

    # 5. Emulator (optional)
    if not $skip_emulator {
        print ">>> [5/5] Emulator Verification"
        try {
            nu scripts/verify-emulator.nu
            $total_pass += 1
        } catch {|err|
            print $"  ⚠️  Emulator verification skipped/failed: ($err.msg)"
        }
    } else {
        print ">>> [5/5] Emulator Verification (SKIPPED)"
    }
    print ""

    # Final summary
    print "========================================"
    print $"  TOTAL: ($total_pass) passed, ($total_fail) failed"
    print "========================================"
    if $total_fail > 0 {
        printerr "❌ VERIFICATION SUITE FAILED"
        exit 1
    } else {
        print "✅ ALL VERIFICATIONS PASSED"
    }
}
