# Android Build Standards

## Overview

This document defines the strict build rules for the Torvox Android build pipeline.
These rules apply to `scripts/build-android-libs.nu`, `scripts/build-apk.nu`,
and any CI workflow that invokes them.

---

## Environment Rules

1. **Only `nix develop`** — Never `nix shell`. All environment comes from the
   flake's devShell. No system-wide tool installation.

2. **No software installation** — Never call `sdkmanager`, `apt`, `cargo install`,
   `pip install`, or any package manager inside a script. All tools must be
   declared in `flake.nix`.

3. **No `cargo zigbuild`** — Use `cargo ndk` for cross-compilation. Zig is only
   used by libghostty-vt-sys's build.rs (called internally by cargo), never
   invoked directly by scripts.

4. **NDK from environment only** — `ANDROID_NDK_HOME` is set by `nix develop`.
   Never search for NDK through fallback paths or install it via sdkmanager.

5. **No zig version checks** — The environment is deterministic. Zig version
   installed by nix is guaranteed correct. No `if ($zig_version != "0.15.2")`
   conditionals.

6. **No `which` for tool lookup** — All tools are guaranteed by `nix develop`.
   No runtime path discovery.

---

## Build Process Rules

7. **Clean before build** — Delete any `.so` files in `jniLibs/` and `.apk` files
   in `app/build/outputs/apk/` before each build cycle. No incremental mixing of
   old and new artifacts.

8. **Build order** — Native libraries (`.so`) first, then APKs. The APK step
   expects populated `jniLibs/` and `assets/bin/`.

9. **Start fresh target directory** — No assumption of prior `target/` contents.
   `cargo ndk` builds are clean per invocation.

10. **Ghostty linkage check** — After `.so` build, verify `libtorvox_android.so`
    has no `libghostty-vt.so` NEEDED entry. If dynamically linked, copy
    `libghostty-vt.so` to `jniLibs/<abi>/`. If statically linked, skip.

11. **Built artifacts must be verified** — After APK build, verify the APK
    contains at least one `.so` file. Size must exceed `minimum_apk_size_bytes`.

---

## Code Quality Rules

12. **No abbreviated CLI flags** — Use `--target`, `--package`, `--profile`,
     `--dereference` etc. Never `-t`, `-p`, etc.

13. **No non-deterministic code** — No `if` conditionals on tool versions,
     environment variables, or runtime-detected paths. The nix environment is
     the single source of truth.

14. **No fallback behavior** — If a resource is not found, fail. No `try/catch`
     or `else` fallback blocks for expected resources.

15. **No `ignore`** — No `| ignore` to suppress expected failures.

16. **No `which`** — All tool paths guaranteed by nix.

17. **No `nu` script execution inside scripts** — Never call `nu scripts/xxx.nu`
     inside a `.nu` script. Use shebang or `nu scripts/xxx.nu`.

18. **No sdkmanager** — Never invoke `sdkmanager` for NDK or SDK component
     installation.

---

## Prohibited Patterns

| Pattern | Reason |
|---------|--------|
| `cargo zigbuild` | Unreliable, non-deterministic zig version coupling |
| `sdkmanager "ndk;..."` | Software installation outside nix |
| `if ($zig_version != "0.15.2")` | Non-deterministic version check |
| `which zig` | Tool lookup when nix guarantees presence |
| `let zig15_path = ...` | Hardcoded version coupling |
| Abbreviated flags | Style violation per STYLE.md |
| NDK path fallback search | Environment must be deterministic |
| `^cargo zigbuild --package exec-bin` | Must use `cargo ndk` |
