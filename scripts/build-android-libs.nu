#!/usr/bin/env -S nix develop --command nu
# Build Android native libs: cdylib + torvox-exec
# Uses cargo ndk (no zigbuild, no zig version checks)
# Usage: scripts/build-android-libs.nu [--profile dev] [--profile release] [<abi>...]
#   <abi>: arm64-v8a or x86_64 (positional, defaults to both)

const JNILIBS = "android/app/src/main/jniLibs"
const ASSETS_BIN = "android/app/src/main/assets/bin"
const MINIMUM_SO_SIZE_BYTES = 1_000_000

def abi-to-target-triple [abi: string] {
    match $abi {
        "x86_64" => "x86_64-linux-android"
        "arm64-v8a" => "aarch64-linux-android"
        _ => { print $"ERROR: unsupported ABI ($abi)"; exit 1 }
    }
}

def profile-to-out-dir [name: string] {
    match $name {
        "release" => "release"
        _ => "debug"
    }
}

def main [--profile: string = "", ...abis: string] {
    mut profiles = ["dev", "release"]
    if $profile != "" {
        $profiles = [$profile]
    }
    mut abis = $abis
    if ($abis | is-empty) {
        $abis = ["arm64-v8a", "x86_64"]
    }

    # Clean old artifacts
    for abi in $abis {
        let jni_dir = $env.PWD | path join $JNILIBS $abi
        if ($jni_dir | path exists) {
            rm --force --recursive $jni_dir
        }
        let bin_dir = $env.PWD | path join $ASSETS_BIN $abi
        if ($bin_dir | path exists) {
            rm --force --recursive $bin_dir
        }
    }

    # Build torvox-gui-android (cdylib) for all profiles and ABIs
    for profile in $profiles {
        let ndk_args = ($abis | each { |a| ["--target", $a] } | flatten)
        cargo ndk ...$ndk_args --platform 21 build --package torvox-gui-android --profile $profile
    }

    # Copy release-profile .so to jniLibs (optimized for device)
    let deploy_profile = if "release" in $profiles { "release" } else { $profiles | first }
    let deploy_outdir = profile-to-out-dir $deploy_profile
    for abi in $abis {
        let triple = abi-to-target-triple $abi
        let lib_dir = $env.PWD | path join $JNILIBS $abi
        mkdir $lib_dir
        let so_path = $env.PWD | path join "target" $triple $deploy_outdir "libtorvox_android.so"
        if not ($so_path | path exists) {
            print $"ERROR: libtorvox_android.so not found at ($so_path)"
            exit 1
        }
        cp $so_path ($lib_dir | path join "libtorvox_android.so")

        let size = (stat --format=%s $so_path | str trim | into int)
        if $size < $MINIMUM_SO_SIZE_BYTES {
            print $"ERROR: ($so_path) size ($size) below minimum ($MINIMUM_SO_SIZE_BYTES)"
            exit 1
        }
    }

    # Ghostty linkage check
    for abi in $abis {
        let so_path = $env.PWD | path join $JNILIBS $abi "libtorvox_android.so"
        let readelf_out = (readelf --dynamic $so_path)
        let ghostty_needed = ($readelf_out | lines | where { $in =~ "NEEDED" and $in =~ "ghostty" })
        if not ($ghostty_needed | is-empty) {
            let triple = abi-to-target-triple $abi
            let build_dir = $env.PWD | path join "target" $triple $deploy_outdir "build"
            if not ($build_dir | path exists) {
                print $"ERROR: ghostty NEEDED but build directory not found for ($abi)"
                exit 1
            }
            let candidates = (ls $build_dir | where name !~ ".zig-cache" and name != "build-script-build")
            mut found = ""
            for candidate in $candidates {
                let lib_path = $candidate.name | path join "libghostty-vt.so"
                if ($lib_path | path exists) {
                    $found = $lib_path
                    break
                }
            }
            if $found == "" {
                print $"ERROR: ghostty NEEDED but libghostty-vt.so not found for ($abi)"
                exit 1
            }
            cp $found ($env.PWD | path join $JNILIBS $abi "libghostty-vt.so")
        }
    }

    # Build torvox-exec for all ABIs (always release for on-device)
    let ndk_args = ($abis | each { |a| ["--target", $a] } | flatten)
    cargo ndk ...$ndk_args --platform 21 build --package torvox-exec --profile release
    for abi in $abis {
        let triple = abi-to-target-triple $abi
        let bin_dir = $env.PWD | path join $ASSETS_BIN $abi
        mkdir $bin_dir
        let exec_path = $env.PWD | path join "target" $triple "release" "torvox-exec"
        cp $exec_path ($bin_dir | path join "torvox-exec")
        chmod +x ($bin_dir | path join "torvox-exec")
    }
}
