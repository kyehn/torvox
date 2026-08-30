# 构建指南

## 环境

- 使用 `nix develop` 进入开发环境，全部工具由 `flake.nix` 声明。
- 禁止使用 `sdkmanager` 或其他包管理器安装工具。
- `ANDROID_NDK_HOME` 已预设，无需回退查找。
- 禁止使用 `which` 进行运行时路径探测，全部工具由 `nix develop` 保证可用。
- 环境是确定性的：Zig 版本以 `flake.nix` 中声明的为准，禁止进行 Zig 版本检查。

## 构建

- 使用 `cargo ndk` 进行 Android 构建，禁止使用 `cargo zigbuild`。
- 构建顺序：先构建 `.so`，再构建 APK。APK 构建阶段要求 `jniLibs/` 与 `assets/bin/` 已填充。
- 校验 `libnative.so` 是否包含 `libghostty-vt.so` 的 `NEEDED` 条目：若为动态链接，需将 `libghostty-vt.so` 复制到 `jniLibs/<abi>/`；若为静态链接则跳过。
- 检查 APK 至少包含一个 `.so`。
- release/dev/debug `.so` 文件大小必须合理，如果较大必须找出原因解决
