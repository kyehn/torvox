# Rin 深度审查

- 来源：https://github.com/pavelc4/Rin
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：Kotlin + Compose + Rust/JNI 的 Android 终端，主打兼容 Termux 包生态的 `rpkg`。

### 分层

- Rust 侧：`crates/rin-core` 使用 `termwiz` 的 VT 解析与屏幕模型；`crates/rin-jni` 暴露 session、输入、resize 等 JNI 接口，JNI 句柄管理会话生命周期。
- Android 侧：Compose 终端视图、主题、会话列表与前台服务；`rpkg` 负责 Termux `.deb` 仓库兼容、依赖解析与安装路径补丁；`root helper` 处理提权操作。
- `doc/package-manager.md` 是包管理器行为的权威说明；截图与文档量小，源码是主要信息源。

### 关键机制

- 终端状态在 Rust 核心，Kotlin 只显示屏幕；与本地边界相似，但 parser 选择 `termwiz` 而不是 Ghostty。
- 包管理器会改写 Termux `.deb` 的路径前缀以适配自身安装位置，这是兼容生态的关键，也是最大的维护负担：上游包脚本假设 Termux 目录布局时容易失效。
- `extra keys`、前台服务与会话恢复都是常规 Android 终端做法；仓库没有独立测试目录，只有 `rpkg` 内部两个单元测试，`unsafe` 计数 20 集中在 JNI 边界。

### 验证与风险

- 无自动化测试覆盖 VT 状态机；`termwiz` 兼容性依赖上游版本，缺少本地回归套件。
- root helper 与包安装构成特权边界；README 已给出安全提示，复用时必须保留同等提示与显式授权。

### 本地取舍

不采纳包管理与 root helper。`termwiz` 与本地 Ghostty 解析路线互斥，只把“Rust 核心持有会话、JNI 传输入和 resize”的分层作为对照。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/pty.rs` | 对照 Rust 核心持有 PTY 与 `Pty` trait 的测试替身 |
| `native/src/terminal/ghostty_terminal/public_api.rs` | 对照 Ghostty 与 `termwiz` 的解析路线差异 |
| `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 对照会话句柄与跨语言调用边界 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.github/dependabot.yml` | TEXT | 995 | 47 | `d888c5a06f34` |
| `.github/workflows/android-unified.yml` | TEXT | 10482 | 257 | `40fd91bed84a` |
| `.github/workflows/build.yml` | TEXT | 8211 | 206 | `c00ae7dfccbe` |
| `.github/workflows/release.yml` | TEXT | 10977 | 274 | `7d5d6d0585eb` |
| `.gitignore` | TEXT | 674 | 43 | `d49204aa17ea` |
| `Cargo.lock` | DECLARED_EXCLUSION | 38951 | 1521 | `49cc92cdd623` |
| `Cargo.toml` | TEXT | 824 | 39 | `e4cf1c4ed2cd` |
| `LICENSE` | DECLARED_EXCLUSION | 1063 | 21 | `aba08f66e1b4` |
| `README.md` | TEXT | 2986 | 124 | `250c8e4b42bc` |
| `android/app/build.gradle.kts` | TEXT | 1932 | 74 | `0d6d312e198d` |
| `android/app/lint-baseline.xml` | TEXT | 4444 | 114 | `0b8a94a3201a` |
| `android/app/src/main/AndroidManifest.xml` | TEXT | 1436 | 26 | `e655dce7473c` |
| `android/app/src/main/kotlin/com/rin/MainActivity.kt` | TEXT | 10609 | 273 | `11f96feb2114` |
| `android/app/src/main/kotlin/com/rin/RinLib.kt` | TEXT | 1193 | 25 | `b63e349895e6` |
| `android/app/src/main/kotlin/com/rin/RinPermStorage.kt` | TEXT | 2235 | 62 | `6eb9385af017` |
| `android/app/src/main/kotlin/com/rin/RootHelper.kt` | TEXT | 1537 | 63 | `3d5ddbc6fd28` |
| `android/app/src/main/kotlin/com/rin/permission/StoragePermissionHelper.kt` | TEXT | 1315 | 36 | `96731d1f1f92` |
| `android/app/src/main/kotlin/com/rin/rpkg/RpkgLib.kt` | TEXT | 273 | 13 | `55843fa45c17` |
| `android/app/src/main/kotlin/com/rin/service/TerminalSessionService.kt` | TEXT | 3787 | 114 | `3eeb5af05685` |
| `android/app/src/main/kotlin/com/rin/terminal/SessionManager.kt` | TEXT | 2786 | 96 | `6e72c52d08a6` |
| `android/app/src/main/kotlin/com/rin/terminal/TerminalSession.kt` | TEXT | 212 | 10 | `a0a31db6d45d` |
| `android/app/src/main/kotlin/com/rin/ui/components/ExtraKeysBar.kt` | TEXT | 8175 | 247 | `67aa24b0cb12` |
| `android/app/src/main/kotlin/com/rin/ui/components/HelpDialog.kt` | TEXT | 9089 | 277 | `a70850b26023` |
| `android/app/src/main/kotlin/com/rin/ui/components/SessionDialog.kt` | TEXT | 13002 | 290 | `e0825f79d25a` |
| `android/app/src/main/kotlin/com/rin/ui/components/TerminalSurface.kt` | TEXT | 22906 | 654 | `c1313be1ac45` |
| `android/app/src/main/kotlin/com/rin/ui/screen/SetupScreen.kt` | TEXT | 14022 | 318 | `292683f725cb` |
| `android/app/src/main/kotlin/com/rin/ui/screen/TerminalScreen.kt` | TEXT | 7728 | 201 | `f699909766b1` |
| `android/app/src/main/kotlin/com/rin/ui/theme/Color.kt` | TEXT | 286 | 10 | `1a595adf566f` |
| `android/app/src/main/kotlin/com/rin/ui/theme/TerminalColors.kt` | TEXT | 4192 | 109 | `7c7e7005adff` |
| `android/app/src/main/kotlin/com/rin/ui/theme/Theme.kt` | TEXT | 1215 | 35 | `b879fb1ac438` |
| `android/app/src/main/kotlin/com/rin/ui/theme/Type.kt` | TEXT | 760 | 27 | `04392f9c3e63` |
| `android/app/src/main/res/drawable/ic_launcher_background.xml` | TEXT | 298 | 4 | `0f9990331e99` |
| `android/app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 3779 | 11 | `e96bb51fdd75` |
| `android/app/src/main/res/drawable/ic_launcher_monochrome.xml` | TEXT | 3132 | 8 | `89c8b3ee8fc8` |
| `android/app/src/main/res/drawable/rin.xml` | TEXT | 4395 | 12 | `83d32a0b0e92` |
| `android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml` | TEXT | 344 | 6 | `ada31da9e23f` |
| `android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml` | TEXT | 344 | 6 | `ada31da9e23f` |
| `android/build.gradle.kts` | TEXT | 74 | 3 | `eafc4af9ce6b` |
| `android/gradle.properties` | TEXT | 103 | 3 | `fa078c27c9e4` |
| `android/gradle/wrapper/gradle-wrapper.jar` | BINARY | 47505 | 164 | `7a9ce74cff46` |
| `android/gradle/wrapper/gradle-wrapper.properties` | TEXT | 281 | 9 | `d01b41913834` |
| `android/gradlew` | TEXT | 8656 | 248 | `a5a5c199ba02` |
| `android/gradlew.bat` | TEXT | 2848 | 82 | `d539676c48b5` |
| `android/settings.gradle.kts` | TEXT | 580 | 23 | `4c6437ab463c` |
| `doc/image/ss01.jpg` | BINARY | 69015 | 229 | `79e8e104a447` |
| `doc/image/ss02.jpg` | BINARY | 66716 | 186 | `5ea2a614112d` |
| `doc/image/ss03.jpg` | BINARY | 74732 | 261 | `c16ed9c72f43` |
| `doc/image/ss04.jpg` | BINARY | 140730 | 699 | `87bd38bd983a` |
| `doc/image/ss05.jpg` | BINARY | 135025 | 603 | `bca5d1e5e9df` |
| `doc/image/ss06.jpg` | BINARY | 99080 | 353 | `812e9b15659d` |
| `doc/package-manager.md` | TEXT | 2627 | 98 | `ccb534c6454e` |
| `rpkg/Cargo.toml` | TEXT | 544 | 31 | `e01891cca828` |
| `rpkg/src/bin/rpkg.rs` | TEXT | 8426 | 275 | `c2aeefab43f9` |
| `rpkg/src/core/extract.rs` | TEXT | 9184 | 240 | `72fb60f4d8d3` |
| `rpkg/src/core/index.rs` | TEXT | 6532 | 191 | `8d8d90178f28` |
| `rpkg/src/core/mod.rs` | TEXT | 65 | 4 | `ccda4eebb2fd` |
| `rpkg/src/core/resolver.rs` | TEXT | 1846 | 61 | `e57f08326be7` |
| `rpkg/src/core/types.rs` | TEXT | 3429 | 119 | `bde5262aae94` |
| `rpkg/src/lib.rs` | TEXT | 111 | 5 | `2b62d2b954df` |
| `rpkg/src/manager/install.rs` | TEXT | 7075 | 203 | `6f7dc027b7a7` |
| `rpkg/src/manager/mod.rs` | TEXT | 1744 | 65 | `eea3dfef85e3` |
| `rpkg/src/manager/query.rs` | TEXT | 556 | 15 | `5d7034a4fee2` |
| `rpkg/src/manager/remove.rs` | TEXT | 1923 | 58 | `6481ca325a43` |
| `rpkg/src/manager/upgrade.rs` | TEXT | 1471 | 45 | `3f3a8bdc8a3c` |
| `rpkg/src/platform/android.rs` | TEXT | 3719 | 91 | `9db7b7f75434` |
| `rpkg/src/platform/mod.rs` | TEXT | 45 | 2 | `0c59ad33f19f` |
| `src/core/buffer/alternate.rs` | TEXT | 1458 | 49 | `a8afe32852e9` |
| `src/core/buffer/cursor.rs` | TEXT | 611 | 23 | `4353dc257073` |
| `src/core/buffer/mod.rs` | TEXT | 15053 | 445 | `3fe121ba12c8` |
| `src/core/buffer/screen.rs` | TEXT | 6023 | 202 | `5a942fda0b46` |
| `src/core/cell.rs` | TEXT | 3845 | 160 | `f9c4f6996891` |
| `src/core/grid.rs` | TEXT | 3978 | 145 | `4a7bb3a42dc5` |
| `src/core/mod.rs` | TEXT | 197 | 8 | `cf618529758f` |
| `src/input/handler.rs` | TEXT | 2764 | 116 | `e33c60802372` |
| `src/input/mod.rs` | TEXT | 109 | 4 | `a8e7f2f5716f` |
| `src/lib.rs` | TEXT | 1895 | 81 | `38d8be9a5003` |
| `src/parser/ansi.rs` | TEXT | 20239 | 545 | `fd8c82cdb07d` |
| `src/parser/color.rs` | TEXT | 1508 | 56 | `c189daaf6a60` |
| `src/parser/mod.rs` | TEXT | 113 | 4 | `276e9ebab031` |
| `src/platform/android/jni.rs` | TEXT | 14567 | 525 | `103f0e87ac84` |
| `src/platform/android/mod.rs` | TEXT | 93 | 5 | `a3418f50a4be` |
| `src/platform/android/session.rs` | TEXT | 4550 | 129 | `7160e33a22c6` |
| `src/platform/mod.rs` | TEXT | 17 | 1 | `a0d17afa6efc` |
| `src/pty.rs` | TEXT | 2387 | 87 | `c7986802a4f0` |
| `src/renderer/mod.rs` | TEXT | 357 | 16 | `4876db0a861b` |
| `src/renderer/screen.rs` | TEXT | 2034 | 100 | `f2bd44a56ae3` |

## 复算命令

```sh
git clone --depth 1 https://github.com/pavelc4/Rin /tmp/opencode/external/Rin
git -C /tmp/opencode/external/Rin ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/Rin ls-files | wc -l
```
