#!/usr/bin/env -S nix develop --command nu
# Clone ghostty source and apply VT correctness patch (idempotent)
# Patch features: cursor_style save/restore, DECAWM wraparound, scroll N=0→1
# --forward skips already-applied hunks
# Output: absolute path to ghostty source directory

def main [] {
    let ghostty_directory = $env.PWD | path join "vendor" "ghostty"
    let patch_file = $env.PWD | path join "patches" "libghostty-vt-correctness.patch"

    if not ($ghostty_directory | path exists) {
        ^git clone --depth 1 --branch main https://github.com/ghostty-org/ghostty.git $ghostty_directory
        ^patch --directory $ghostty_directory --strip 1 --forward --input $patch_file
    }

    print $ghostty_directory
}
