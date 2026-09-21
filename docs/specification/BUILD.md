# 构建指南

## 环境

- 使用 `nix develop` 进入开发环境，所有工具由 `flake.nix` 声明。
- 禁止使用 `sdkmanager` 或其他包管理器安装工具。
- `ANDROID_NDK_HOME` 已预设，无需回退查找。
- 禁止使用 `which` 进行运行时路径探测，全部工具由 `nix develop` 保证可用。
- 环境是确定性的：Zig 版本以 `flake.nix` 中声明的为准，禁止进行 Zig 版本检查。

## 构建

- 使用 `cargo ndk` 进行 Android 构建，禁止使用 `cargo zigbuild`。
- 构建顺序：先构建 `.so`，再构建 APK。APK 构建阶段要求 `jniLibs/` 已填充。
- 校验 `libnative.so` 是否包含 `libghostty-vt.so` 的 `NEEDED` 条目：若为动态链接，需将 `libghostty-vt.so` 复制到 `jniLibs/<abi>/`；若为静态链接则跳过。
- 检查 APK 至少包含一个 `.so`。
- release/dev `.so` 文件大小必须合理，如果较大必须找出原因解决
- compileOptions 为 `JavaVersion.VERSION_17`
- minSdkVersion 为 33，compileSdkVersion 为 37，targetSdkVersion 为 28
- ndkVersion 为 r30，不在代码中固定版本
- versionCode 为 2000，versionName 为 0.1.0
- Rust 版本最低为 1.98，edition 最低为 2024
- 不得保留未使用依赖，所有依赖使用最新稳定版本，如果目前已使用不稳定版本且无更高版本的稳定版本则使用最新不稳定版本，不得降级依赖
- rust/kotlin/gradle 代码中绝不能出现使用 `cargo build` `cargo test` `./gradlew ":app:assembleDebug"` 或其他进行构建的情况，禁止不受管理的构建
- 只支持 arm64-v8a x86_64 架构，x86_64 在构建/测试/模拟器测试时使用
