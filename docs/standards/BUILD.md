# Android Build Standards

## Overview

Build rules for `scripts/build-android-libs.nu`, `scripts/build-apk.nu`,
and any CI workflow that invokes them.

---

## Environment

1. **Only `nix develop`** — Never `nix shell`, `sdkmanager`, or any package manager.
   All tools declared in `flake.nix`.
2. **NDK from environment only** — `ANDROID_NDK_HOME` set by `nix develop`. No fallback search.
3. **No `cargo zigbuild`** — Use `cargo ndk`. Zig used only by libghostty-vt-sys's build.rs internally.
4. **No `which`** — All tools guaranteed by `nix develop`. No runtime path discovery.
5. **No zig version checks** — Environment is deterministic. Zig version from nix is correct.
6. **Start fresh target directory** — `cargo ndk` builds are clean per invocation.

## Build Process

7. **Clean before build** — Delete `.so` in `jniLibs/` and `.apk` in output dirs before each cycle.
8. **Build order** — `.so` first, then APK. APK step expects populated `jniLibs/` and `assets/bin/`.
9. **Ghostty linkage check** — Verify `libnative.so` has no `libghostty-vt.so` NEEDED entry.
   If dynamic-linked, copy `libghostty-vt.so` to `jniLibs/<abi>/`. If static-linked, skip.
10. **Verify APK** — Check APK contains at least one `.so` and exceeds `minimum_apk_size_bytes`.

## Script Rules

See `docs/standards/STYLE.md` for full Nushell style guide. Key Android-specific rules:

- No abbreviated CLI flags (`--target`, `--package`, not `-t`, `-p`)
- No `| ignore` — let failures propagate naturally
- No `nu scripts/xxx.nu` inside `.nu` scripts — use shebang or call directly
- No non-deterministic conditionals on tool versions or paths

## Prohibited Patterns

| Pattern | Why |
|---------|-----|
| `cargo zigbuild` | Unreliable zig version coupling |
| `sdkmanager "ndk;..."` | Software installation outside nix |
| NDK path fallback search | Environment must be deterministic |
| `^cargo zigbuild --package exec-bin` | Must use `cargo ndk` |
| Abbreviated CLI flags | Style violation per STYLE.md |
