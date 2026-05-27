# Torvox 开发指南

> 构建 Torvox、测试 Torvox、发布 Torvox 的内部工作流文档。

---

## 前置条件

### 必需

| 工具 | 版本 | 用途 |
|------|------|------|
| Rust | stable (固定在 `rust-toolchain.toml`) | 核心引擎, Edition 2024 |
| Android Studio / SDK | Meerkat (2025.3.4+) | Android 构建 |
| Android NDK | r29 | 交叉编译 Rust 为 Android |
| cargo-ndk | v4 (重大变更: 与 v3 不兼容) | `cargo ndk` 命令 |
| JDK | 25+ | Kotlin/JVM 编译 |
| Gradle | 9.4.1+ | Android 构建系统 |

### 可选 (推荐)

| 工具 | 用途 |
|------|------|
| cargo-nextest | 增强测试运行器 (比 cargo test 快 3x) |
| cargo-fuzz | 模糊测试 (libFuzzer) |
| cargo-geiger | unsafe 代码检测 |
| MIRI | 未定义行为检测 |
| proptest | 属性测试 |
| cargo-flamegraph | 性能分析 |

### Rust 目标

```bash
rustup target add aarch64-linux-android   # 主要 Android 目标
rustup target add x86_64-linux-android    # 模拟器
# no_std 测试目标
rustup target add thumbv6m-none-eabi
```

### Android SDK

```bash
# 通过 ANDROID_SDK_ROOT 环境变量或 local.properties 设置
echo "sdk.dir=$HOME/Android/Sdk" > android/local.properties
echo "ndk.dir=$HOME/Android/Sdk/ndk/29.0.14206865" >> android/local.properties
```

## 快速开始

```bash
# 克隆
git clone https://github.com/${{ github.actor }}/torvox && cd torvox

# Rust — 验证核心编译
cargo build
cargo nextest --workspace

# Android — 构建 debug APK
cd android && ./gradlew assembleDebug

# 安装到设备
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

## 开发工作流

### 规范驱动开发 (SDD)

Torvox 采用规范驱动开发——来自 Bun (1M 行/9 天) 和 Google Ads (500M 行) 的验证方法论：

```
1. 阅读 ADR + ARCHITECTURE.md ← 理解设计
2. 阅读当前阶段 ROADMAP.md ← 我们在构建什么？
3. 编写精确规范 ← 在写代码前思考
4. 先定义类型 ← no_std 类型, 然后行为
5. 实现 core → terminal → renderer → bridge
6. 每层测试 ← 核心测试优先, 然后集成
7. cargo clippy + nextest ← 自动化质量门
8. 更新文档 ← AGENTS.md, ADR 如决策变更
```

### 渐进构建流程

```bash
# 1. 核心类型 (快速编译, <30s)
cargo build -p torvox-core
cargo nextest -p torvox-core

# 2. 终端引擎 (中等编译, ~2min)
cargo build -p torvox-terminal
cargo nextest -p torvox-terminal

# 3. 渲染器 (长编译, 需要 GPU, ~5min)
cargo build -p torvox-renderer

# 4. 完整 workspace (最长编译)
cargo build --workspace
cargo nextest --workspace
cargo clippy --deny warnings

# 5. Android (交叉编译 + Gradle, ~10min)
cd android
./gradlew :app:assembleDebug
```

### 运行测试

```bash
# 所有 Rust 测试 (nextest 更快)
cargo nextest --workspace

# 单个 crate
cargo nextest -p torvox-core

# 属性测试
cargo test --workspace -- proptest

# 包含忽略的 (慢/模糊)
cargo nextest --workspace --run-ignored all

# 带输出
cargo nextest --workspace --nocapture

# Android 单元测试
cd android && ./gradlew test

# Android 设备测试
cd android && ./gradlew connectedCheck
```

### 代码质量门

```bash
# 快速质量门 (每次提交, ~5min)
cargo fmt --check && cargo clippy --deny warnings && cargo nextest --workspace

# 完整质量门 (包含 Android, ~15min)
nu scripts/quality-gate.nu
# 等价于:
cargo fmt --check \
  && cargo clippy --deny warnings \
  && cargo nextest --workspace \
  && cargo test --workspace -- proptest \
  && cargo geiger --all-features \
  && cd android && ./gradlew lint \
  && cd android && ./gradlew test
```

### 模糊测试 (夜间)

```bash
# VT 解析器模糊 (1 小时)
cargo fuzz run vt_parser -- -max_total_time=3600

# OSC 转义模糊 (1 小时)
cargo fuzz run osc_parser -- -max_total_time=3600

# UTF-8 边缘情况模糊 (1 小时)
cargo fuzz run utf8_parser -- -max_total_time=3600

# 长时间模糊 (1B 迭代, ~12 小时)
cargo fuzz run vt_parser -- -max_total=1000000000
```

### 安全检查

```bash
# unsafe 代码统计 (torvox-core + torvox-terminal 应为零)
cargo geiger --all-features

# 未定义行为检测 (MIRI, 仅 Linux)
cargo miri test -p torvox-core
cargo miri test -p torvox-terminal

# 依赖审计
cargo audit
```

## CI/CD

### GitHub Actions 工作流

| 工作流 | 触发 | 检查 | 超时 |
|--------|------|------|------|
| `ci.yml` | 每个 PR 到 `main` | cargo fmt, clippy, nextest, proptest, geiger, gradle lint, gradle test | 30 分钟 |
| `nightly.yml` | 每日 UTC 03:00 | 模糊测试 (3 目标×1h), MIRI, cargo bench, cargo audit | 4 小时 |
| `release.yml` | 标签 `v*` | cargo ndk build → assembleRelease → 签名 → GitHub Release | 60 分钟 |
| `android-ci.yml` | PR (仅 android/ 变更) | gradle lint, gradle test, assembleDebug | 20 分钟 |

### CI 管线

实际工作流定义在 `.github/workflows/` 目录。摘要:

| 工作流 | 触发 | 关键步骤 |
|--------|------|----------|
| `ci.yml` | PR → main | cargo fmt/clippy/nextest/proptest/geiger + gradle lint/test |
| `nightly.yml` | 每日 UTC 03:00 | cargo fuzz (3 目标×1h) + MIRI + bench + audit |
| `release.yml` | 标签 `v*` | cargo ndk build → assembleRelease → 签名 → GitHub Release |

## 构建架构

### Rust → Android NDK

```
[Rust 源码]
↓ cargo ndk v4 (通过 scripts/build-android-libs.nu)
[Rust 交叉编译为 aarch64-linux-android / x86_64-linux-android]
↓ libtorvox_android.so
[Android jniLibs/arm64-v8a/libtorvox_android.so]
↓ APK 打包
[Android 应用通过 System.loadLibrary("torvox_android") 加载]
```

### Gradle 集成

`cargo-ndk v4` 脚本协调:
1. `scripts/build-android-libs.nu` 交叉编译步骤
2. 输出 `.so` 放置到 `jniLibs/` 目录
3. APK 打包 Rust 原生库

```bash
# scripts/build-android-libs.nu 中
cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build --release
```

## 性能分析

### Android

```bash
# 帧时间 (关键: 目标 <8ms/帧)
adb shell dumpsys gfxinfo io.torvox

# 内存 (关键: 空闲 <10MB)
adb shell dumpsys meminfo io.torvox

# CPU (关键: 空闲 <0.5%)
adb shell top -n 1 | grep torvox

# GPU (Vulkan 状态)
adb shell dumpsys gpu

# Systrace (详细帧分析)
python $ANDROID_SDK/platform-tools/systrace/systrace.py \
  --app io.torvox gfx view input
```

### Rust

```bash
# 基准
cargo bench

# Flamegraph
cargo flamegraph --bin torvox-bench

# Perf (仅 Linux)
cargo build --release
perf record ./target/release/torvox-bench
perf report

# Heap 分析 (valgrind massif)
valgrind --tool=massif ./target/release/torvox-bench
ms_print massif.out.*
```

## 发布流程

```bash
# 1. 版本升级 (semver)
# 更新版本于:
# - android/app/build.gradle.kts (versionCode + versionName)
# - torvox-core/Cargo.toml
# - torvox-terminal/Cargo.toml
# - torvox-renderer/Cargo.toml
# - torvox-gui-android/Cargo.toml

# 2. 质量门
nu scripts/quality-gate.nu

# 3. 构建 + 签名
cd android && ./gradlew assembleRelease

# 4. 测试发布 APK
adb install -r android/app/build/outputs/apk/release/app-release.apk

# 5. 标签 + 推送
git tag -a v0.1.0 -m "v0.1.0 — MVP: 基础终端渲染"
git push origin v0.1.0

# 6. CI 构建签名 APK + 创建 GitHub Release
```

---

*本文档随项目演进更新。*
