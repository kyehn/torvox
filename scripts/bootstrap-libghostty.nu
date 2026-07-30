#!/usr/bin/env -S nix develop --command nu
# Bootstrap patches for torvox.
#
# 1. Clone ghostty C source to vendor/ghostty/ (for C compilation)
# 2. Generate patched libghostty-vt / libghostty-vt-sys crates from upstream
#    libghostty-rs + local patches (for Rust FFI)
#
# Output (last line): absolute path to ghostty source directory.
# The Nix shellHook sources this to set $GHOSTTY_SOURCE_DIR.

def main [] {
    let root = $env.PWD
    let ghostty_dir = ($root | path join "vendor" "ghostty")
    let upstream_rs_dir = ($root | path join ".cache" "libghostty-rs")
    let output_dir = ($root | path join "generated-patches")
    let patch_dir = ($root | path join "patches")
    let correctness_patch = ($patch_dir | path join "libghostty-vt-correctness.patch")

    # ── Step 1: Ghostty C source ──────────────────────────────────────
    if not ($ghostty_dir | path exists) {
        print $"Cloning ghostty to ($ghostty_dir)..."
        ^git clone --depth 1 --branch main https://github.com/ghostty-org/ghostty.git $ghostty_dir
        if ($correctness_patch | path exists) {
            ^patch --directory $ghostty_dir --strip 1 --forward --input $correctness_patch
        }
    } else {
        print "Ghostty source already exists, skipping clone."
    }

    # ── Step 2: Patched Rust crates ───────────────────────────────────
    mkdir $output_dir

    # Clone libghostty-rs upstream if needed
    if not ($upstream_rs_dir | path exists) {
        print $"Cloning libghostty-rs to ($upstream_rs_dir)..."
        ^git clone --depth 1 https://github.com/Uzaaft/libghostty-rs.git $upstream_rs_dir
    }

    # Copy original crates
    rm -rf ($output_dir | path join "libghostty-vt-sys")
    rm -rf ($output_dir | path join "libghostty-vt")
    cp -r ($upstream_rs_dir | path join "crates" "libghostty-vt-sys") ($output_dir | path join "libghostty-vt-sys")
    cp -r ($upstream_rs_dir | path join "crates" "libghostty-vt") ($output_dir | path join "libghostty-vt")

    # Apply local patches
    for patch_file in [($patch_dir | path join "libghostty-vt-sys.patch") ($patch_dir | path join "libghostty-vt.patch")] {
        if ($patch_file | path exists) {
            print $"Applying ($patch_file)..."
            ^patch --directory $output_dir --strip 1 --forward --input $patch_file
        }
    }

    # ── Output: ghostty source path (for shellHook) ──────────────────
    print $ghostty_dir
}
