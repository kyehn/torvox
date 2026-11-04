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
    # 宏基准的指标摘要只进 instrumentation bundle（IDE 可见），CI 控制台看不到；
    # cat 测试 XML 把用例内 println 的实测值（[torvox-bench] 行）与基准摘要带进
    # 构建日志，作为断言预算校准的实测依据。
    let results = (glob $"($android_dir)/benchmark/build/outputs/androidTest-results/**/TEST-*.xml")
    if ($results | is-empty) {
        print "no benchmark test results xml found"
    } else {
        for xml in $results {
            print $"=== benchmark results: ($xml)"
            print (open --raw $xml)
        }
    }
    cd $repo_dir
}
