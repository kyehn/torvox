#!/usr/bin/env -S nix develop --command nu

def main [] {
    semgrep scan --error --dataflow-traces --time --config .semgrep/kotlin-deny-patterns.yml --config .semgrep/android-deny-patterns.yml
    pushd android
    ./gradlew --continue spotlessCheck app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest testDebugUnitTest benchmark:testReleaseUnitTest baselineprofile:testDebugUnitTest -Dorg.gradle.internal.test.results.binary.enabled=false
    popd
}
