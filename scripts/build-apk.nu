#!/usr/bin/env -S nix develop --command nu

# 从 assets 分支取回 baseline profile 规则。
#
# 规则不进主仓（BUILD.md：baselineprofile 文件保存到 assets 分支），release 变体的
# 采集产物落在 `android/app/src/release/generated/baselineProfiles/`，该路径已被
# .gitignore 忽略，所以从 assets 分支还原后工作区仍是干净的。
def restore_baseline_profile [repo_root: path] {
    let profile_dir = $repo_root | path join "android" "app" "src" "release" "generated" "baselineProfiles"
    let fetch = (
        ^git -C $repo_root fetch origin "+refs/heads/assets:refs/remotes/origin/assets"
        | complete
    )
    if $fetch.exit_code != 0 {
        error make {msg: $"cannot fetch assets branch: ($fetch.stderr)"}
    }
    mkdir $profile_dir
    for name in ["baseline-prof.txt" "startup-prof.txt"] {
        let blob = (
            ^git -C $repo_root show $"origin/assets:baselineprofile/($name)"
            | complete
        )
        if $blob.exit_code != 0 {
            error make {msg: $"cannot read ($name) from origin/assets: ($blob.stderr)"}
        }
        $blob.stdout | save --force ($profile_dir | path join $name)
    }
}

def main [--release, --debug] {
    let repo_root = $env.PWD
    cd android
    let apk_base = $env.PWD | path join "app" "build" "outputs" "apk"
    for variant in ["release", "debug"] {
        let dir = $apk_base | path join $variant
        if ($dir | path exists) {
            for apk in (glob $"($dir)/*.apk") {
                rm -f $apk
            }
        }
    }
    mut build_release = $release
    mut build_debug = $debug
    if (not $release) and (not $debug) {
        $build_release = true
        $build_debug = false
    }

    if $build_release {
        restore_baseline_profile $repo_root
        ^./gradlew ":app:assembleRelease"
        let apks = (glob $"($apk_base)/release/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/release/ after assembleRelease"
            exit 1
        }
    }

    if $build_debug {
        ^./gradlew ":app:assembleDebug"
        let apks = (glob $"($apk_base)/debug/*.apk")
        if ($apks | is-empty) {
            print $"ERROR: no APK found in ($apk_base)/debug/ after assembleDebug"
            exit 1
        }
    }
}
