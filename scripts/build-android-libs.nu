#!/usr/bin/env -S nix develop --command nu

const JNILIBS = "android/app/src/main/jniLibs"

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

    for abi in $abis {
        let jni_dir = $env.PWD | path join $JNILIBS $abi
        if ($jni_dir | path exists) {
            rm -rf $jni_dir
        }
    }

    for profile in $profiles {
        let ndk_args = ($abis | each { |a| ["--target", $a] } | flatten)
        cargo ndk ...$ndk_args --platform 21 build --package native --profile $profile
    }

    let deploy_profile = if "release" in $profiles { "release" } else { $profiles | first }
    let deploy_outdir = profile-to-out-dir $deploy_profile
    for abi in $abis {
        let triple = abi-to-target-triple $abi
        let lib_dir = $env.PWD | path join $JNILIBS $abi
        mkdir $lib_dir
        let so_path = $env.PWD | path join "target" $triple $deploy_outdir "libnative.so"
        if not ($so_path | path exists) {
            print $"ERROR: libnative.so not found at ($so_path)"
            exit 1
        }
        cp $so_path ($lib_dir | path join "libnative.so")
    }
}
