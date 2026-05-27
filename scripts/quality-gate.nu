# Torvox 质量门 (nushell)
# 使用: nu scripts/quality-gate.nu

print "=== Torvox 质量门 ==="
print $"Java: (java -version 2>&1 | complete | get stdout | lines | get 0)"

let failed = (mutable [])

# [1/8] cargo fmt --check
print "[1/8] cargo fmt --check"
if not (cargo fmt --check | complete | get exit_code == 0) {
    $failed | append "cargo fmt"
    print "FAIL: cargo fmt --check"
}

# [2/8] cargo clippy
print "[2/8] cargo clippy -- -D warnings"
if not (cargo clippy --workspace --all-targets -- -D warnings | complete | get exit_code == 0) {
    $failed | append "cargo clippy"
    print "FAIL: cargo clippy"
}

# [3/8] cargo test
print "[3/8] cargo test --workspace"
if not (cargo test --workspace | complete | get exit_code == 0) {
    $failed | append "cargo test"
    print "FAIL: cargo test"
}

# [4/8] proptest
print "[4/8] proptest"
if not (cargo test --workspace -- proptest | complete | get exit_code == 0) {
    $failed | append "proptest"
    print "FAIL: proptest"
}

# [5/8] cargo geiger
print "[5/8] cargo geiger"
if (which cargo-geiger | length) > 0 {
    let result = (cargo geiger --all-features 2>/dev/null | complete)
    if $result.exit_code != 0 {
        $failed | append "cargo geiger"
        print "FAIL: cargo geiger"
    }
} else {
    print "SKIP: cargo-geiger not installed"
}

# [6/8] Android lint
print "[6/8] Android lint"
if ("android" | path exists) and ($env.JAVA_HOME? | default "") != "" {
    cd android
    if not (./gradlew lint --quiet | complete | get exit_code == 0) {
        $failed | append "Android lint"
        print "FAIL: Android lint"
    }
    cd ..
} else {
    print "SKIP: Android lint (no JAVA_HOME)"
}

# [7/8] Android test
print "[7/8] Android test"
if ("android" | path exists) and ($env.JAVA_HOME? | default "") != "" {
    cd android
    if not (./gradlew test --quiet | complete | get exit_code == 0) {
        $failed | append "Android test"
        print "FAIL: Android test"
    }
    cd ..
} else {
    print "SKIP: Android test (no JAVA_HOME)"
}

# [8/8] cargo audit
print "[8/8] cargo audit"
if (which cargo-audit | length) > 0 {
    if not (cargo audit | complete | get exit_code == 0) {
        print "WARN: cargo audit found vulnerabilities"
    }
} else {
    print "SKIP: cargo-audit not installed"
}

print ""
if ($failed | length) == 0 {
    print "=== 质量门通过 ==="
} else {
    print $"=== 质量门失败: ($failed | str join ', ') ==="
    exit 1
}
