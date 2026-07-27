# Build/CI 经验教训

## 1. Ghostty Android 动态链接的反复尝试

### 问题背景
将 Ghostty 的 libghostty-vt 集成到 Android APK 需要解决交叉编译和动态链接问题。整个过程经历了多次反复尝试，约 10+ 个修复提交才稳定。

### 尝试历程

| 步骤 | 尝试方案 | 结果 | 原因 |
|------|----------|------|------|
| 1 | `--whole-archive` 静态链接 | ❌ 失败 | Zig 安装包只发布 `.o` 文件，没有 `.a` 归档 |
| 2 | 动态链接 (dylib) + 手动 bundle | ❌ 部分失败 | `libghostty-vt.so.0` SONAME 有版本号，被 Gradle/AGP 过滤掉 |
| 3 | build.rs 加 SONAME strip | ✅ 工作 | 用 patchelf 重写 NEEDED，去掉 `.0` 后缀 |
| 4 | 再次尝试 `--whole-archive` | ❌ 再次失败 | 同上，Zig 安装包只有 `.o` |
| 5 | 最终稳定方案 | ✅ 稳定 | 动态链接 + build.rs SONAME strip + bundle libc++_shared.so |

### 最终方案
1. 动态链接 (dylib) — 这是 Zig 构建输出格式，也是唯一可行的方式
2. build.rs 在链接后用 `patchelf --replace-needed` 去掉 `.so.0` 版本后缀
3. 同时 bundle `libc++_shared.so`（NDK 的 C++ 标准库，因为 Zig 编译的代码链接了 C++）
4. 构建脚本将最终的 `.so` 文件复制到 `jniLibs/` 目录供 AGP 打包

### 关键教训
- **Zig 的构建产物主要是 `.o` 而不是 `.a`** — 静态链接不可行
- **AGP/Gradle 过滤版本化 `.so` 文件** — 所有 `.so.0`、`.so.1` 后缀都会被排除在 APK 外
- **patchelf 是 Android Native 开发的必备工具** — 用于修改 ELF 的 NEEDED 条目和 SONAME
- **交叉编译的动态库需要同时处理所有传递依赖** — Ghostty 依赖 C++，所以需要 bundle libc++_shared
- **静态和动态链接方案不要反复横跳** — 每种方案都有不同的问题，确定方案后坚持解决其中的问题

### 相关提交
- `86b01d0fb`: fix(build): use --whole-archive for ghostty-vt static linking on Android
- `ce4dc68e3b`: fix(build): add android-gui/build.rs with --whole-archive
- `6a0d9b3132`: fix(build): bundle libghostty-vt.so instead of static linking
- `3eac6629`: fix(android): restore ghostty dynamic linking with patchelf + libc++_shared bundling
- `88cd1b51`: fix(build): add patchelf and ghostty .so copy to Android build script
- `2cfe7916`: fix(ci): SONAME strip in cargo ndk step
- `2f4ad394`: fix(ci): two-step ndk build strips ghostty SONAME before final link
- `d82b36a6`: fix(libghostty-vt-sys): properly handle ghostty .so symlinks in SONAME strip
- `2509a46f`: fix(patches): regenerate ghostty patch to keep dylib=ghostty-vt link

## 2. CARGO_TARGET 环境变量命名 — Nushell str replace 非全局替换

### 问题
`build-android-libs.nu` 脚本中有一段代码生成 `CARGO_TARGET_${ARCH}_LINUX_ANDROID_LINKER` 环境变量名。代码使用了 Nushell 的 `str replace` 命令将架构名中的 `-` 替换为 `_`：

```nushell
# 错误: 只替换了第一个 -
$arch | str replace '-' '_' | str upcase
# aarch64-linux-android → AARCH64LINUXANDROID (而不是 AARCH64_LINUX_ANDROID)
```

Nushell 的 `str replace` 默认只替换**第一个**匹配，不像 Bash 的 `${var//pattern/replacement}` 替换**全部**匹配。

结果生成的是 `CARGO_TARGET_AARCH64LINUXANDROID_LINKER`，但 cargo 期望的是 `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`。

### 修复
```nushell
# 正确: 使用 --all 全局替换
$arch | str replace -a '-' '_' | str upcase
```

### 教训
- Nushell 的 `str replace` 默认只替换第一个匹配，全局替换需要 `--all` 或 `-a` 参数
- Bash 惯用的 `${var//pattern/replacement}` 没有直接对应，需要注意
- 环境变量名拼写错误导致的错误很难调试（cargo 静默忽略未识别的环境变量）
- 测试 CI 脚本时，应验证生成的环境变量名是否正确

### 相关提交
- `04ae14c0`: fix: address FULL-AUDIT-2026-05-31 findings (S1: build-android-libs.nu env var bug)

## 3. Mesa Lavapipe 替代 SwiftShader — 30分钟构建降到即时

### 问题
SwiftShader needs to be built from source (about 30 minutes), severely impacting CI iteration speed. Mesa's Lavapipe is a software Vulkan implementation that is pre-built on Nix cache.

### 修复
Remove swiftshader from flake.nix, use mesa (Lavapipe) as the sole headless Vulkan implementation. `VK_ICD_FILENAMES` now points to `lvp_icd.x86_64.json`. `try_create_headless_env()` returns `Option` to gracefully skip when no Vulkan adapter is available.

### 教训
Prefer pre-built `cache.nixos.org` packages over source builds where possible. Mesa Lavapipe works identically to SwiftShader for headless Vulkan testing. When a test depends on optional hardware, make it fail gracefully with `Option` rather than panicking.

### 相关提交
- `14b565ba`: fix(build): replace swiftshader with mesa lavapipe for headless Vulkan
