#!/usr/bin/env -S nix develop --command nu
# Deploy the kudzu nix-on-droid flake on the emulator via run-as + proot.
# Transcribes the verified manual procedure (each step executed on-device
# pre-rollback): UID mapping, id-shim, composed switch env. All device
# specifics are discovered at runtime; nothing is hardcoded.
# Requires: bootstrap installed (termux.env present), kudzu flake staged at
#   <prefix>/home/kudzu/flake.nix (see switch attempt logs for staging).
# Usage: scripts/deploy-kudzu-switch.nu [--flake-subdir home/kudzu]
# NOTE: transcribed, not yet re-executed end-to-end post-rollback.

def run_as [cmd: string] {
    ^adb shell $"run-as com.termux sh -c '($cmd)'" | str trim
}

def main [--flake-subdir: string = "home/kudzu"] {
    # 1. Preconditions.
    let prefix = "/data/data/com.termux/files/usr"
    let env = (try { run_as "ls files/usr/etc/termux/termux.env" } catch { "" })
    if ($env | is-empty) { print "ERROR: bootstrap not installed (termux.env missing)"; exit 1 }
    let flake = (try { run_as $"ls files/usr/($flake_subdir)/flake.nix" } catch { "" })
    if ($flake | is-empty) { print $"ERROR: flake not staged at files/usr/($flake_subdir)/flake.nix"; exit 1 }
    let proot = (try { run_as "ls files/usr/nix/store/*-proot-termux-static-*/bin/proot" } catch { "" })
    if ($proot | is-empty) { print "ERROR: static proot not found in store"; exit 1 }

    # 2. UID mapping: guest passwd/group placeholder -> app UID (runtime-discovered).
    let uid = (^adb shell "dumpsys package com.termux | grep userId=" | str trim | parse "userId={uid}" | get 0.uid)
    run_as $"sed -i s/65534/($uid)/g files/usr/etc/passwd files/usr/etc/group"
    print $"UID mapped to ($uid)"

    # 3. id-shim: store bash exists (glob at runtime), coreutils id does not
    #    survive the guest/host ABI wall, and nix-on-droid evaluates `id` at
    #    eval time (users-groups.nix). Shim answers from the mapped UID.
    let bash = (run_as "ls -d files/usr/nix/store/*-bash-interactive-*/bin/bash")
    let shim = $"#!/($bash)\necho 'uid=($uid)(nix-on-droid) gid=($uid)(nix-on-droid) groups=($uid)(nix-on-droid)'\n"
    run_as $"printf '%s' '($shim)' > files/usr/home/bin/id; chmod +x files/usr/home/bin/id"
    print "id-shim installed"

    # 4. Switch with the composed env (each flag earned by a prior failure).
    let guest_flake = $"/home/($flake_subdir)"
    let switch_cmd = (
        $"export PROOT_TMP_DIR=($prefix)/tmp; export HOME=/home; "
        + $"export PATH=($prefix)/home/bin:$PATH; "
        + $"($proot) -r ($prefix) -b /system -b /proc -b /dev -b /sys -w /home "
        + $"($prefix)/nix/store/*-nix-*/bin/nix-on-droid switch --flake ($guest_flake)#default "
        + $"--accept-flake-config --option connect-timeout 60 --option substitute true"
    )
    print "Running switch (long; tail device log on failure)..."
    run_as $switch_cmd
    print "Switch command returned; verify with /tmp/nix-verify-activation.sh"
}
