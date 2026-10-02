#!/usr/bin/env -S nix develop --command nu

def main [] {
    let repo_dir = $env.PWD
    # 仪器化测试与 benchmark 统一使用 release profile 的 native 库：
    # debug 变体的 dev profile（opt-level="z"）在 SwiftShader 软件渲染下过慢，
    # 2 核 1536M 的 CI runner 会被拖垮（run ：
    # connectedDebugAndroidTest 仅 BehaviorInstrumentedTest#behavior_modifier_bar_visible
    # 失败且消息为空，随后 emulator-5554 丢失）。
    # debug apk 产物仍用 dev profile（build.yml 在本步之前完成打包上传）；
    # 规范要求模拟器调试使用 release apk（docs/specification/TESTING.md 环境节）。
    ^nu scripts/build-android-libs.nu --profile release
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($repo_dir | path join "android")
    cd $android_dir
    ^./gradlew ":app:connectedDebugAndroidTest"
    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }
    for scale in ["window_animation_scale", "transition_animation_scale", "animator_duration_scale"] {
        try { ^adb shell settings put global $scale 0 } catch { null }
    }
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest"
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#modifierKeyPressAnimation
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest" -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.InteractionAnimationBenchmark#imeShowAnimation
    ^./gradlew ":app:generateBaselineProfile"
    cd $repo_dir
}
