#!/usr/bin/env -S nix develop --command nu

const JNILIBS = "android/app/src/main/jniLibs"

# libnative.so 体积上限（字节）。实测 release 约 10 MiB、dev 约 26 MiB；
# 上限按 release 取 3 倍、dev 取 2 倍，超过说明引入了多余后端或未 strip。
const MAX_LIBRARY_SIZE_BYTES = 64mb

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

# libghostty-vt-sys 默认静态链接 ghostty，NEEDED 里出现 libghostty-vt.so 说明有人开了 link-dynamic。
def assert-static-ghostty [so_path: string] {
    let needed = (^readelf -d $so_path | parse -r 'Shared library: \[(?<name>[^\]]+)\]' | get name)
    if "libghostty-vt.so" in $needed {
        print $"ERROR: ($so_path) 动态链接了 libghostty-vt.so，须改为静态链接并移除 link-dynamic feature"
        exit 1
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
        assert-static-ghostty $so_path
        cp $so_path ($lib_dir | path join "libnative.so")
    }

    for abi in $abis {
        let triple = abi-to-target-triple $abi
        let so_size = (ls ($env.PWD | path join "target" $triple $deploy_outdir "libnative.so") | get size.0)
        if $so_size > $MAX_LIBRARY_SIZE_BYTES {
            print $"ERROR: ($abi) 的 ($deploy_outdir)/libnative.so 为 ($so_size) 字节，超过上限 ($MAX_LIBRARY_SIZE_BYTES)"
            exit 1
        }
    }
}
