- use nix develop. All tools declared in `flake.nix`. — Never `sdkmanager`, or any others package manager. 
— `ANDROID_NDK_HOME` 被设置. No fallback search.
— Use `cargo ndk`. No `cargo zigbuild`.
- No `which`, All tools guaranteed by `nix develop`. No runtime path discovery.
- No zig version checks, Environment is deterministic. Zig version from nix is correct.
- Build order, `.so` first, then APK. APK step expects populated `jniLibs/` and `assets/bin/`.
- Verify `libnative.so` has no `libghostty-vt.so` NEEDED entry. If dynamic-linked, copy `libghostty-vt.so` to `jniLibs/<abi>/`. If static-linked, skip.
— Check APK contains at least one `.so`.
