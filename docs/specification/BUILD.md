# 构建

## 环境

- 使用 `nix develop && nix print-dev-env > result-dev.sh` 创建开发环境，使用 `source result-dev.sh` 进入开发环境，工具由 `flake.nix` 声明。
- 禁止使用 `apt` 等包管理器安装工具。
- 禁止使用 `which` 进行运行时路径探测。
- 环境是确定性的：Zig 版本以 `flake.nix` 中声明的为准，禁止进行 Zig 版本检查。
- 可以从外部下载且能自动处理的文件不得 vendor，也不得作为代码片段或文档段落引入。

## 构建

- 使用 `cargo ndk` 进行 Android 构建，不使用 `cargo zigbuild`
- 构建顺序：先构建 `.so`，再构建 APK。APK 构建阶段要求 `jniLibs/` 已填充。
- compileOptions 包括 `JavaVersion.VERSION_17`。
- `minSdkVersion` 为 31，`compileSdkVersion` 为 37，`targetSdkVersion` 为 28。
- 提供 `applicationIdSuffix = ".nix"` 和 `app_name` 为 `Nix` 的变种，使用 `/data/data/com.termux.nix` 目录，对其禁用如 termux bootstrap 等不兼容功能
- `versionCode` 为 2000，`versionName` 为 0.1.0。
- Rust 版本最低为 1.99，`edition` 最低为 2024。
- 不得保留未使用依赖，所有依赖使用最新稳定版本；如果目前已使用不稳定版本且无更高版本的稳定版本则使用最新不稳定版本，不得降级依赖。尽力处理破坏性更新。
- 代码、Gradle 配置等不得出现 `cargo build`、`cargo test`、`./gradlew`、`git clone` 等未受管理的构建和外部源引入。
- 只支持 `arm64-v8a`、`x86_64` 架构；`x86_64` 用于构建、测试与模拟器测试。
- Cargo/Gradle 等配置必须保持简洁和效率，Gradle 配置不进行 `jniLibs/` 检查（避免影响并发构建）
- release 构建日志等级 `INFO`，需尽可能设置，避免输出调试信息，避免影响性能
- release 构建必须尽可能确保性能，正确设置构建参数

## 产物

- 构建产物不得提交到仓库。
- baselineprofile 文件保存到 assets 分支
