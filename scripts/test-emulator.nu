#!/usr/bin/env -S nix develop --command nu

def main [] {
    let repo_dir = $env.PWD
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = ($repo_dir | path join "android")
    cd $android_dir
    ^./gradlew ":app:connectedDebugAndroidTest"
    # 修饰键栏行为测试的诊断信息不进 connected-failure-message，必须落盘才能
    # 在 CI 日志里看到根因（屏上文本 dump）。
    try {
        let report_dirs = (glob android/app/build/outputs/androidTest-results/connected/debug/*)
        for dir in $report_dirs {
            let xmls = (glob ($dir | path join "*.xml"))
            for xml in $xmls {
                let content = (open --raw $xml)
                if ($content | str contains "behavior_modifier_bar_visible") {
                    print $content
                }
            }
        }
    } catch { null }
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
