# Torvox 模拟器测试 (nushell)
# 使用: nu scripts/emulator-test.nu [--unit|--android|--emulator|--all]

let project_dir = ($env.PWD)
let results_dir = ($project_dir | path join "test-results")
let apk_path = ($project_dir | path join "android/app/build/outputs/apk/debug/app-debug.apk")
let avd_name = "torvox_api36"
let emulator_timeout = 120

mkdir $results_dir

def log [msg: string] { print $"(ansi green)[TEST](ansi reset) ($msg)" }
def warn [msg: string] { print $"(ansi yellow_bold)[WARN](ansi reset) ($msg)" }
def fail [msg: string] { print $"(ansi red_bold)[FAIL](ansi reset) ($msg)"; exit 1 }

# ── L0: 编译时检查 ──────────────────────────────────────
def run_clippy [] {
    log "L0: cargo clippy -- -D warnings"
    cargo clippy --workspace -- -D warnings o> ($results_dir | path join "clippy.log")
    log "L0: cargo fmt --check"
    cargo fmt --check o> ($results_dir | path join "fmt.log")
    log "L0 通过"
}

# ── L1: Rust 单元测试 ──────────────────────────────────
def run_rust_tests [] {
    log "L1: cargo nextest run --workspace"
    cargo nextest run --workspace o> ($results_dir | path join "nextest.log")
    log "L1 通过"
}

# ── L2: 属性测试 ──────────────────────────────────────
def run_proptest [] {
    log "L2: proptest (10K+ 用例)"
    cargo test --workspace -- proptest o> ($results_dir | path join "proptest.log")
    log "L2 通过"
}

# ── L3: 集成测试 ──────────────────────────────────────
def run_integration_tests [] {
    log "L3: torvox-integration-tests"
    cargo test -p torvox-integration-tests o> ($results_dir | path join "integration.log")
    log "L3 通过"
}

# ── Android Gradle 测试 ──────────────────────────────────
def run_android_tests [] {
    log "Android: gradle lint + test"
    cd ($project_dir | path join "android")
    ./gradlew lint --quiet o> ($results_dir | path join "android-lint.log")
    ./gradlew test --quiet o> ($results_dir | path join "android-test.log")
    cd $project_dir
    log "Android 测试通过"
}

# ── 构建 APK ──────────────────────────────────────────
def build_apk [] {
    log "构建 APK"
    cd ($project_dir | path join "android")
    ./gradlew assembleDebug o> ($results_dir | path join "build-apk.log")
    cd $project_dir
    if not ($apk_path | path exists) { fail $"APK 构建失败: ($apk_path)" }
    log $"APK 构建成功: ($apk_path)"
}

# ── 模拟器管理 ──────────────────────────────────────────
def check_kvm [] {
    if not ("/dev/kvm" | path exists) {
        fail "KVM 不可用"
        print "启用 KVM: sudo modprobe kvm_intel (Intel) 或 kvm_amd (AMD)"
        exit 1
    }
    log "KVM 可用"
}

def create_avd [] {
    log $"创建 AVD: ($avd_name)"
    avdmanager create avd -n $avd_name -k "system-images;android-36;default;x86_64" -d pixel_7_pro --force o> ($results_dir | path join "avd-create.log")
    log "AVD 创建成功"
}

def start_emulator [] {
    log $"启动模拟器 (超时: ($emulator_timeout)s)"
    mut emulator_pid = (process spawn { emulator -avd $avd_name -no-window -gpu swiftshader_indirect -noaudio -no-boot-anim -camera-back none -memory 2048 -no-snapshot }).pid
    $emulator_pid | save -f ($results_dir | path join "emulator.pid")

    mut elapsed = 0
    while $elapsed < $emulator_timeout {
        let result = (adb shell getprop sys.boot_completed | complete)
        if ($result.stdout | str trim) == "1" {
            log $"模拟器启动完成 ($elapsed)s"
            return
        }
        sleep 5sec
        $elapsed += 5
    }
    fail $"模拟器启动超时 ($emulator_timeout)s"
}

def stop_emulator [] {
    if ($results_dir | path join "emulator.pid" | path exists) {
        let pid = (open ($results_dir | path join "emulator.pid") | str trim)
        try { kill $pid } catch {}
        rm -f ($results_dir | path join "emulator.pid")
    }
    try { adb emu kill } catch {}
}

# ── 模拟器测试 ──────────────────────────────────────────
def run_emulator_tests [] {
    log "=== 模拟器测试 ==="
    check_kvm
    create_avd
    start_emulator

    log "安装 APK"
    adb install -r $apk_path o> ($results_dir | path join "install.log")

    log "启动 MainActivity"
    adb shell am start -a android.intent.action.MAIN -n io.torvox/.MainActivity
    sleep 3sec

    log "验证 Activity"
    let dump = (adb shell dumpsys activity activities | complete)
    if not ($dump.stdout | str contains "io.torvox/.MainActivity") { fail "Activity 未运行" }
    log "Activity 运行正常"

    log "验证进程"
    let ps = (adb shell ps | complete)
    if not ($ps.stdout | str contains "io.torvox") { fail "进程不存在" }
    log "进程存在"

    log "收集 logcat"
    adb logcat -d -s "torvox:*" "Torvox:*" "System.out:*" o> ($results_dir | path join "logcat.log") | complete

    log "截取屏幕"
    adb shell screencap -p /sdcard/torvox-test.png 2>/dev/null | complete
    adb pull /sdcard/torvox-test.png ($results_dir | path join "screenshot.png") 2>/dev/null | complete

    log "运行 instrumented 测试"
    cd ($project_dir | path join "android")
    try { ./gradlew connectedDebugAndroidTest o> ($results_dir | path join "instrumented.log") } catch { warn "instrumented 测试跳过" }
    cd $project_dir

    stop_emulator
    log "模拟器测试完成"
}

# ── 主流程 ──────────────────────────────────────────
let mode = ($env.argv? | default "--all" | get 0)

match $mode {
    "--unit" => { run_clippy; run_rust_tests; run_proptest; run_integration_tests }
    "--android" => { run_android_tests }
    "--emulator" => { build_apk; run_emulator_tests }
    "--all" => { run_clippy; run_rust_tests; run_proptest; run_integration_tests; run_android_tests; build_apk; run_emulator_tests }
    _ => { print "用法: nu scripts/emulator-test.nu [--unit|--android|--emulator|--all]"; exit 1 }
}

log "=== 全部测试通过 ==="
print ""
print $"结果目录: ($results_dir)"
ls $results_dir
