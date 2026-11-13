#!/usr/bin/env -S nix develop --command nu

def main [] {
    let repo_dir = $env.PWD
    try { ^adb shell pm uninstall --user 0 com.termux } catch { null }
    let android_dir = $repo_dir | path join "android"
    cd $android_dir
    ^./gradlew ":app:connectedDebugAndroidTest"
    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest"
    # Nix 变种（包名 com.termux.nix）的引导包安装：Nix-on-Droid 引导包与 Termux
    # 官方包不兼容，变种装不上只有这条用例能发现（主包的仪器测试只跑 debug 变种）。
    ^./gradlew ":benchmark:connectedBenchmarkNixAndroidTest" "-Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.benchmark.NixBootstrapTest"
    cd $repo_dir
}
