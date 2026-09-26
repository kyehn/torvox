# 构建指南

## 环境

- 使用 `nix develop` 进入开发环境，所有工具由 `flake.nix` 声明。
- 禁止使用 `sdkmanager` 或其他包管理器安装工具。
- `flake.nix` 的 devShell **不提供** Android NDK。`cargo ndk` 从 `ANDROID_NDK_HOME` 或 `ANDROID_HOME` 读取 SDK，而这两个变量由 CI runner 提供而非本仓声明，因此 NDK 版本随环境变化。**禁止在文档或脚本中假设 NDK 具体版本**，也禁止为找不到 NDK 而做回退查找——找不到就让命令直接失败。
- 禁止使用 `which` 进行运行时路径探测，全部工具由 `nix develop` 保证可用。
- 环境是确定性的：Zig 版本以 `flake.nix` 中声明的为准，禁止进行 Zig 版本检查。

## 构建

- 使用 `cargo ndk` 进行 Android 构建，禁止使用 `cargo zigbuild`。
- 构建顺序：先构建 `.so`，再构建 APK。APK 构建阶段要求 `jniLibs/` 已填充；`scripts/build-apk.nu` 必须先断言 `jniLibs/<abi>/libnative.so` 存在再调用 Gradle。
- 链接模式校验：`libghostty-vt-sys` 默认静态链接 ghostty（其 `Cargo.toml` 的 `default` 不含 `link-dynamic`，`build.rs` 默认 `LinkMode::Static`）。`scripts/build-android-libs.nu` 必须用 `readelf -d` 断言 `libnative.so` 的 `NEEDED` **不含** `libghostty-vt.so`；若出现该条目说明有人开启了 `link-dynamic`，必须报错并要求把 `libghostty-vt.so` 复制到 `jniLibs/<abi>/`，不得静默通过。
- 检查 APK 至少包含一个 `.so`：`scripts/build-apk.nu` 必须解包校验 `lib/<abi>/libnative.so` 存在，不得只 glob `*.apk`。
- release/dev `.so` 文件大小必须合理：dev 超过 release 的 2 倍即报错并要求找出原因（未 strip、缺 `strip`、多余后端均属原因）。
- compileOptions 为 `JavaVersion.VERSION_17`。
- `minSdkVersion` 为 33，`compileSdkVersion` 为 37，`targetSdkVersion` 为 28。
- 不在代码中固定 NDK 版本。
- `versionCode` 为 2000，`versionName` 为 0.1.0。
- Rust 版本最低为 1.98；本仓 crate 的 `edition` 为 2024。依赖各自的 `edition` 与 `rust-version` 不受此约束（已知 `lru 0.18.5` 无 `edition` 键，按 2015 编译）。
- 不得保留未使用依赖。**工作区级 `[workspace.dependencies]` 声明同样受此约束**，`cargo machete` 只扫成员清单，查不到工作区级未使用项，需人工核对。已知无法跟进的依赖必须在 `DESIGN.md` 依赖一节记录原因。
- 所有依赖使用最新稳定版本，如果目前已使用不稳定版本且无更高版本的稳定版本则使用最新不稳定版本，不得降级依赖。
- rust/kotlin/gradle 代码中绝不能出现使用 `cargo build` `cargo test` `./gradlew ":app:assembleDebug"` 或其他进行构建的情况，**注释与文档字符串同样禁止**，禁止不受管理的构建。
- CI 的 `run` 步骤同样禁止绕开 `scripts/` 直接构建，必须调用 `scripts/*.nu`。
- 只支持 `arm64-v8a`、`x86_64` 架构，`x86_64` 在构建/测试/模拟器测试时使用。代码中不得保留其他 ABI 分支。
