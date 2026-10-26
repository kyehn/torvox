#!/usr/bin/env -S nix develop --command nu

const CRATE = "native"
const JNILIBS = "android/app/src/main/jniLibs"

def abi-to-target-triple [abi: string] {
    match $abi {
        x86_64 => "x86_64-linux-android"
        "arm64-v8a" => "aarch64-linux-android"
        _ => {
            print $"ERROR: unsupported ABI ($abi)"
            exit 1
        }
    }
}

def main [...abis: string] {
    mut abis = $abis
    if ($abis | is-empty) {
        $abis = ["x86_64"]
    }

    let repo = $env.PWD

    for abi in $abis {
        let jni_dir = $repo | path join $JNILIBS $abi
        if ($jni_dir | path exists) {
            rm -rf $jni_dir
        }
    }

    let ndk_args = $abis | each {|a| ["--target", $a] } | flatten
    # `cargo ndk` 不接受 `--manifest-path`（它在当前目录解析清单），故进入包根执行。
    cd $CRATE
    cargo ndk ...$ndk_args --platform 33 build --package native --profile release

    for abi in $abis {
        let triple = abi-to-target-triple $abi
        let lib_dir = $repo | path join $JNILIBS $abi
        mkdir $lib_dir
        let so_path = $repo | path join $CRATE "target" $triple "release" "libnative.so"
        if not ($so_path | path exists) {
            print $"ERROR: libnative.so not found at ($so_path)"
            exit 1
        }
        cp $so_path ($lib_dir | path join "libnative.so")
    }
}
