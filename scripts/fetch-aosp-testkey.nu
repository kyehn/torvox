#!/usr/bin/env -S nix develop --command nu
# Download AOSP test signing key via git clone and convert to PKCS#12.
# Idempotent: skips if aosp-testkey.p12 already exists.
# No fallback — errors propagate.

def main [] {
    let p12 = ($env.PWD | path join "android" "app" "aosp-testkey.p12")
    if ($p12 | path exists) {
        print $"SKIP: ($p12) already exists"
        return
    }

    mkdir ($p12 | path dirname)
    let tmp = (^mktemp -d | str trim)

    ^git clone --depth 1 --quiet https://android.googlesource.com/platform/build ($tmp | path join "repo")

    let pem = ($tmp | path join "repo" "target" "product" "security" "testkey.x509.pem")
    let pk8 = ($tmp | path join "repo" "target" "product" "security" "testkey.pk8")

    let pk8_pem = ($tmp | path join "testkey.pk8.pem")
    ^openssl pkey -inform DER -in $pk8 -out $pk8_pem

    ^openssl pkcs12 -export -in $pem -inkey $pk8_pem -out $p12 -password pass:android -name testkey

    rm -rf $tmp
    print $"OK: ($p12)"
}
