# Torvox Android 库交叉编译 (nushell)
# 使用: nu scripts/build-android-libs.nu

let project_root = ($env.PWD | path dirname | path join "..")
let android_dir = ($project_root | path join "android")
let jni_libs_dir = ($android_dir | path join "app/src/main/jniLibs")
let exec_dir = ($android_dir | path join "app/src/main/assets/bin")
let lib_cargo_toml = ($project_root | path join "torvox-gui-android/Cargo.toml")
let target_dir = ($project_root | path join "target")

if not ($env.ANDROID_NDK_ROOT? | default "") {
    print "ERROR: ANDROID_NDK_ROOT must be set"
    exit 1
}

if (which cargo-ndk | length) == 0 {
    print "Installing cargo-ndk..."
    cargo install cargo-ndk
}

let abis = ["arm64-v8a" "x86_64"]
let triples = ["aarch64-linux-android" "x86_64-linux-android"]

print "=== Cross-compiling torvox-gui-android (cdylib) ==="
cargo ndk -t arm64-v8a -t x86_64 -o $target_dir build --manifest-path $lib_cargo_toml --profile dev

for abi in $abis {
    mkdir -p ($jni_libs_dir | path join $abi)
    cp ($target_dir | path join $abi "libtorvox_android.so") ($jni_libs_dir | path join $abi)
    print $"Copied to ($jni_libs_dir)/($abi)/libtorvox_android.so"
}

print "=== Cross-compiling torvox-exec (PIE binary) ==="
for i in 0..(($abis | length) - 1) {
    let abi = ($abis | get $i)
    let triple = ($triples | get $i)
    let linker = ($env.ANDROID_NDK_ROOT | path join "toolchains/llvm/prebuilt/linux-x86_64/bin" | path join $"($triple)33-clang")
    print $"--- Building torvox-exec for ($triple) ($abi) ---"
    let env_var = ($triple | str upcase | str replace -a "-" "_")
    with-env { ($env_var): $linker } {
        cargo build -p torvox-exec --target $triple --profile dev
    }
    mkdir -p ($exec_dir | path join $abi)
    cp ($target_dir | path join $triple "debug/torvox-exec") ($exec_dir | path join $abi)
    chmod +x ($exec_dir | path join $abi "torvox-exec")
    print $"Copied to ($exec_dir)/($abi)/torvox-exec"
}

print "=== Done ==="
