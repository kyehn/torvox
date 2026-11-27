# ferrux 深度审查

- 来源：https://github.com/FerraStack/ferrux
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：Rust 工作区 + Kotlin 应用，提供终端与进程/文件 API，带独立 AIDL 客户端样例与 bootstrap 安装器。

### 分层

- `crates/`：`ferrux-term` 管终端状态，`ferrux-pty` 管 PTY，`ferrux-session` 管会话，`ferrux-jni-core`/`ferrux-android` 管 JNI，`ferrux-host`/`ferrux-env` 管宿主与运行环境。
- `android/app/`：Compose UI、`SessionManager.kt`、`BootstrapInstaller.kt`、`FerruxService.kt`、`FerruxSettings.kt`，以及 `api/ApiRegistry.kt`、`FerruxApiService.kt`。
- `android/api/` 与 `android/apiclient-sample/`：对外 AIDL/Kotlin API 契约与跨应用测试；`android/terminal-view/`：`ScreenText.kt`、`TerminalAccessibility.kt`。

### 关键机制

- 终端核心与 Android 边界通过 JNI 会话句柄连接；跨应用 API 由 AIDL 暴露并带 `ApiErrors`、`Ownership`、`FerruxException` 等明确错误模型，`OwnershipTest`/`UnsupportedTest` 覆盖契约行为。
- bootstrap 安装器下载并布置运行环境；服务层管理会话生命周期、前台通知与退出会话。
- 13 个测试文件，含 `ExtraKeysAccessibilityTest`、`ClipboardRequestTest`、`SettingsPreferencesTest` 与 androidTest 的跨应用测试；`unsafe` 计数 41 集中在 Rust/JNI 边界。

### 验证与风险

- bootstrap 下载链路是供应链信任点，校验策略必须随环境包格式演进；AIDL API 扩大了可调用面，`Ownership` 语义一旦松动就会造成跨应用越权。
- 工作区分层清晰但模块数量多；只取终端核心会失去 API 层，只取 API 层又会带入会话所有权模型，不能拆着用。

### 本地取舍

参考其错误模型：跨边界调用返回显式错误类型而不是静默失败；参考 `TerminalAccessibility.kt` 的可访问性组织。AIDL 与 bootstrap 不进入本仓。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 对照显式错误模型与原生缺失降级 |
| `native/src/android/ffi/input.rs` | 对照跨边界输入的句柄与错误处理 |
| `native/src/terminal/output_processor.rs` | 对照错误不静默失败的分派边界 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.cargo/config.toml` | TEXT | 320 | 7 | `e5b8945f3cb7` |
| `.github/pull_request_template.md` | TEXT | 560 | 17 | `a91c505b669d` |
| `.github/workflows/ci.yml` | TEXT | 10954 | 242 | `5d6d4f9ccc23` |
| `.github/workflows/publish.yml` | TEXT | 4048 | 99 | `444c3344a290` |
| `.github/workflows/release.yml` | TEXT | 8222 | 205 | `90f8e801edd0` |
| `.gitignore` | TEXT | 123 | 10 | `c99183d1f4a7` |
| `CONTRIBUTING.md` | TEXT | 1220 | 24 | `7466023b4976` |
| `Cargo.lock` | DECLARED_EXCLUSION | 24919 | 995 | `46fa1935eb41` |
| `Cargo.toml` | TEXT | 1188 | 42 | `fe118d5838d6` |
| `LICENSE` | DECLARED_EXCLUSION | 11357 | 201 | `c71d239df917` |
| `README.md` | TEXT | 15563 | 277 | `33064ff44f0d` |
| `android/.gitignore` | TEXT | 278 | 15 | `65a01d7b5ef4` |
| `android/api-signers.properties` | TEXT | 617 | 12 | `f3fb28338a4b` |
| `android/api/build.gradle.kts` | TEXT | 2684 | 88 | `a23bfdba2cd3` |
| `android/api/consumer-rules.pro` | TEXT | 143 | 2 | `a52961770497` |
| `android/api/src/main/AndroidManifest.xml` | TEXT | 558 | 11 | `b828234a19b7` |
| `android/api/src/main/aidl/dev/ferrux/api/AttachResult.aidl` | TEXT | 223 | 8 | `28f92ee1ed50` |
| `android/api/src/main/aidl/dev/ferrux/api/FerruxInfo.aidl` | TEXT | 848 | 29 | `4293daaa8d29` |
| `android/api/src/main/aidl/dev/ferrux/api/FileStat.aidl` | TEXT | 443 | 19 | `9118e21b1b4e` |
| `android/api/src/main/aidl/dev/ferrux/api/IFerruxApi.aidl` | TEXT | 2298 | 59 | `5218068e0876` |
| `android/api/src/main/aidl/dev/ferrux/api/IProcessCallback.aidl` | TEXT | 439 | 10 | `1766985ee120` |
| `android/api/src/main/aidl/dev/ferrux/api/SessionInfo.aidl` | TEXT | 273 | 13 | `35f8ba0f95f3` |
| `android/api/src/main/aidl/dev/ferrux/api/SpawnRequest.aidl` | TEXT | 1115 | 29 | `0a3ea3463852` |
| `android/api/src/main/aidl/dev/ferrux/api/SpawnResult.aidl` | TEXT | 700 | 20 | `8c3732c0f3a3` |
| `android/api/src/main/kotlin/dev/ferrux/api/ApiErrors.kt` | TEXT | 1109 | 26 | `24845dc3c4ec` |
| `android/api/src/main/kotlin/dev/ferrux/client/FerruxClient.kt` | TEXT | 5702 | 133 | `e707a7de7245` |
| `android/api/src/main/kotlin/dev/ferrux/client/FerruxConnection.kt` | TEXT | 10071 | 241 | `b806349f1612` |
| `android/api/src/main/kotlin/dev/ferrux/client/FerruxException.kt` | TEXT | 3052 | 67 | `1c736603a0d7` |
| `android/api/src/main/kotlin/dev/ferrux/client/FerruxFiles.kt` | TEXT | 4546 | 109 | `9a0077d2666d` |
| `android/api/src/main/kotlin/dev/ferrux/client/FerruxProcess.kt` | TEXT | 5452 | 138 | `cd9e9600681e` |
| `android/api/src/main/kotlin/dev/ferrux/client/Ownership.kt` | TEXT | 2047 | 53 | `fa6ee682ea20` |
| `android/api/src/test/kotlin/dev/ferrux/client/OwnershipTest.kt` | TEXT | 2513 | 71 | `b1439feb450c` |
| `android/api/src/test/kotlin/dev/ferrux/client/UnsupportedTest.kt` | TEXT | 3598 | 89 | `8ed6044da2cb` |
| `android/apiclient-sample/build.gradle.kts` | TEXT | 1332 | 42 | `d635c1d72795` |
| `android/apiclient-sample/src/androidTest/kotlin/dev/ferrux/apiclient/sample/ApiCrossAppTest.kt` | TEXT | 12258 | 306 | `e19819d18b98` |
| `android/apiclient-sample/src/main/AndroidManifest.xml` | TEXT | 790 | 18 | `e3788a35b652` |
| `android/apiclient-sample/src/main/kotlin/dev/ferrux/apiclient/sample/SampleActivity.kt` | TEXT | 5189 | 134 | `340691885f1c` |
| `android/app/build.gradle.kts` | TEXT | 8250 | 210 | `a34084a21f39` |
| `android/app/proguard-rules.pro` | TEXT | 370 | 7 | `bde22b40eff4` |
| `android/app/src/debug/AndroidManifest.xml` | TEXT | 998 | 21 | `bca29b900c78` |
| `android/app/src/debug/kotlin/dev/ferrux/app/debug/DebugReceiver.kt` | TEXT | 7045 | 148 | `e473858a984a` |
| `android/app/src/main/AndroidManifest.xml` | TEXT | 3187 | 67 | `53267432c259` |
| `android/app/src/main/kotlin/dev/ferrux/app/BootstrapInstaller.kt` | TEXT | 7424 | 189 | `51ec1063fee5` |
| `android/app/src/main/kotlin/dev/ferrux/app/Clipboard.kt` | TEXT | 639 | 20 | `663f0e78ea41` |
| `android/app/src/main/kotlin/dev/ferrux/app/ExitedSession.kt` | TEXT | 1973 | 53 | `1ecf955eda50` |
| `android/app/src/main/kotlin/dev/ferrux/app/FerruxAppNative.kt` | TEXT | 2646 | 73 | `f9cf1919c4e4` |
| `android/app/src/main/kotlin/dev/ferrux/app/FerruxApplication.kt` | TEXT | 738 | 26 | `21c7fc8f520d` |
| `android/app/src/main/kotlin/dev/ferrux/app/FerruxService.kt` | TEXT | 11766 | 276 | `3b2d6474bf66` |
| `android/app/src/main/kotlin/dev/ferrux/app/FerruxSettings.kt` | TEXT | 9422 | 233 | `ca35e677553d` |
| `android/app/src/main/kotlin/dev/ferrux/app/MainActivity.kt` | TEXT | 6186 | 156 | `df5c1f228c43` |
| `android/app/src/main/kotlin/dev/ferrux/app/SessionManager.kt` | TEXT | 12162 | 299 | `0588d12b840b` |
| `android/app/src/main/kotlin/dev/ferrux/app/SessionShortcuts.kt` | TEXT | 1641 | 42 | `9d78618dd894` |
| `android/app/src/main/kotlin/dev/ferrux/app/api/ApiNative.kt` | TEXT | 2520 | 70 | `b062ad066902` |
| `android/app/src/main/kotlin/dev/ferrux/app/api/ApiRegistry.kt` | TEXT | 8206 | 208 | `f00ad90fa573` |
| `android/app/src/main/kotlin/dev/ferrux/app/api/FerruxApiService.kt` | TEXT | 8760 | 242 | `c65c1ca444ec` |
| `android/app/src/main/kotlin/dev/ferrux/app/ui/BootstrapScreen.kt` | TEXT | 5695 | 138 | `d5e2f2769c33` |
| `android/app/src/main/kotlin/dev/ferrux/app/ui/ExtraKeys.kt` | TEXT | 16977 | 410 | `fd452872e7c2` |
| `android/app/src/main/kotlin/dev/ferrux/app/ui/FerruxScreen.kt` | TEXT | 29867 | 703 | `cd4e1d9e4932` |
| `android/app/src/main/kotlin/dev/ferrux/app/ui/SettingsScreen.kt` | TEXT | 26222 | 635 | `3d38da2e183d` |
| `android/app/src/main/kotlin/dev/ferrux/app/ui/Theme.kt` | TEXT | 7456 | 180 | `a9e373795545` |
| `android/app/src/main/res/drawable/ic_add.xml` | TEXT | 342 | 10 | `c661de62d477` |
| `android/app/src/main/res/drawable/ic_arrow_back.xml` | TEXT | 404 | 11 | `721a3e9a8f84` |
| `android/app/src/main/res/drawable/ic_check.xml` | TEXT | 360 | 10 | `108a2fb315d2` |
| `android/app/src/main/res/drawable/ic_close.xml` | TEXT | 399 | 10 | `e8aa14834abb` |
| `android/app/src/main/res/drawable/ic_launcher_background.xml` | TEXT | 896 | 22 | `7f0a3a8aa2ec` |
| `android/app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 2886 | 65 | `663343188c29` |
| `android/app/src/main/res/drawable/ic_launcher_monochrome.xml` | TEXT | 1202 | 26 | `24ee6ddcc454` |
| `android/app/src/main/res/drawable/ic_menu.xml` | TEXT | 348 | 10 | `048e0ee24888` |
| `android/app/src/main/res/drawable/ic_more_vert.xml` | TEXT | 494 | 10 | `b79a0c84b2a0` |
| `android/app/src/main/res/drawable/ic_notification.xml` | TEXT | 452 | 13 | `2d68b3f1abf7` |
| `android/app/src/main/res/drawable/ic_settings.xml` | TEXT | 1214 | 10 | `da5774926039` |
| `android/app/src/main/res/mipmap-anydpi/ic_launcher.xml` | TEXT | 456 | 7 | `c277101df2ac` |
| `android/app/src/main/res/values/colors.xml` | TEXT | 168 | 5 | `384b858750aa` |
| `android/app/src/main/res/values/strings.xml` | TEXT | 6832 | 99 | `fac0a7e6ba71` |
| `android/app/src/main/res/values/themes.xml` | TEXT | 376 | 8 | `7fc634dc2eef` |
| `android/app/src/test/kotlin/dev/ferrux/app/ClipboardRequestTest.kt` | TEXT | 657 | 20 | `5354a8c38176` |
| `android/app/src/test/kotlin/dev/ferrux/app/ExitedSessionTest.kt` | TEXT | 1426 | 37 | `f1bffa4529ce` |
| `android/app/src/test/kotlin/dev/ferrux/app/FerruxSettingsTest.kt` | TEXT | 2872 | 84 | `6b7d1f671012` |
| `android/app/src/test/kotlin/dev/ferrux/app/NotificationTextTest.kt` | TEXT | 1521 | 41 | `0a5e5afcb234` |
| `android/app/src/test/kotlin/dev/ferrux/app/SessionListTest.kt` | TEXT | 1387 | 40 | `416003b6b7a0` |
| `android/app/src/test/kotlin/dev/ferrux/app/SessionShortcutsTest.kt` | TEXT | 1645 | 35 | `e3f071380af5` |
| `android/app/src/test/kotlin/dev/ferrux/app/SettingsPreferencesTest.kt` | TEXT | 4068 | 104 | `8d911c980308` |
| `android/app/src/test/kotlin/dev/ferrux/app/ui/ExtraKeysAccessibilityTest.kt` | TEXT | 3958 | 106 | `5e382ba99474` |
| `android/build.gradle.kts` | TEXT | 297 | 7 | `fbca9e4ec83c` |
| `android/gradle.properties` | TEXT | 1122 | 26 | `2104c50acfe1` |
| `android/gradle/libs.versions.toml` | TEXT | 2488 | 39 | `13fe8ca84ab2` |
| `android/gradle/wrapper/gradle-wrapper.jar` | BINARY | 49154 | 185 | `621008afc4f5` |
| `android/gradle/wrapper/gradle-wrapper.properties` | TEXT | 368 | 10 | `0357ed920855` |
| `android/gradlew` | TEXT | 8656 | 248 | `a5a5c199ba02` |
| `android/gradlew.bat` | TEXT | 4888 | 112 | `f17917f8dbe6` |
| `android/settings.gradle.kts` | TEXT | 642 | 29 | `a8ba9ffca717` |
| `android/terminal-view/build.gradle.kts` | TEXT | 4007 | 110 | `c542ae9f2536` |
| `android/terminal-view/consumer-rules.pro` | TEXT | 144 | 2 | `56cdafcd0444` |
| `android/terminal-view/licenses/JetBrainsMono-OFL.txt` | TEXT | 4399 | 93 | `30f0c136e3c8` |
| `android/terminal-view/src/main/AndroidManifest.xml` | TEXT | 52 | 2 | `daa0fb1f32ba` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/ScreenText.kt` | TEXT | 4342 | 99 | `0b1306980a3b` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalAccessibility.kt` | TEXT | 12145 | 290 | `9a881b5a10a4` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalBackend.kt` | TEXT | 3714 | 96 | `e5dab07625c3` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalGestures.kt` | TEXT | 4489 | 121 | `11b5d245553b` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalInputConnection.kt` | TEXT | 5891 | 151 | `253e1e8e4bc7` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalKeys.kt` | TEXT | 7139 | 214 | `4252f437e1c5` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalNative.kt` | TEXT | 7268 | 177 | `b73a6f1d1620` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalSelection.kt` | TEXT | 20931 | 508 | `888eb01577ee` |
| `android/terminal-view/src/main/kotlin/dev/ferrux/terminal/TerminalView.kt` | TEXT | 53816 | 1285 | `1710153534dc` |
| `android/terminal-view/src/main/res/font/jetbrains_mono_bold.ttf` | BINARY | 277828 | 1579 | `5590990c82e0` |
| `android/terminal-view/src/main/res/font/jetbrains_mono_bold_italic.ttf` | BINARY | 279832 | 1807 | `4039d5ce0ed2` |
| `android/terminal-view/src/main/res/font/jetbrains_mono_italic.ttf` | BINARY | 276840 | 1992 | `9d0a1f7a708e` |
| `android/terminal-view/src/main/res/font/jetbrains_mono_regular.ttf` | BINARY | 273900 | 1480 | `a0bf60ef0f83` |
| `android/terminal-view/src/main/res/values/strings.xml` | TEXT | 494 | 10 | `f70d8189cfa0` |
| `android/terminal-view/src/test/kotlin/dev/ferrux/terminal/ScreenTextTest.kt` | TEXT | 3415 | 86 | `bbf3ef039a0e` |
| `android/terminal-view/src/test/kotlin/dev/ferrux/terminal/TerminalGesturesTest.kt` | TEXT | 6484 | 164 | `07f27aabb31d` |
| `branding/icon/ic_launcher_background.svg` | TEXT | 400 | 10 | `100ff67d0c6d` |
| `branding/icon/ic_launcher_foreground.svg` | TEXT | 2015 | 29 | `1e100406fe1b` |
| `branding/icon/ic_launcher_monochrome.svg` | TEXT | 722 | 6 | `37f756440756` |
| `branding/icon/icon-512.png` | BINARY | 30260 | 103 | `52dcf3dedf2d` |
| `branding/icon/icon.svg` | TEXT | 2262 | 34 | `61dbea591e47` |
| `branding/icon/preview.png` | BINARY | 197690 | 633 | `cb3fb8a56ff2` |
| `branding/icon/tools/build.sh` | TEXT | 925 | 20 | `ea7c5a22ce8c` |
| `branding/icon/tools/gen.py` | TEXT | 7605 | 201 | `7327dce22769` |
| `branding/icon/tools/preview.py` | TEXT | 7063 | 152 | `44e243410a41` |
| `branding/icon/tools/svg2vd.py` | TEXT | 4429 | 101 | `4c842d29d91f` |
| `crates/ferrux-android/Cargo.toml` | TEXT | 456 | 23 | `cae79c63dcfa` |
| `crates/ferrux-android/src/api.rs` | TEXT | 14045 | 421 | `e2251c3dd533` |
| `crates/ferrux-android/src/core.rs` | TEXT | 10395 | 320 | `a9041c7af0bf` |
| `crates/ferrux-android/src/lib.rs` | TEXT | 7241 | 227 | `9a3ec818072f` |
| `crates/ferrux-env/Cargo.toml` | TEXT | 319 | 17 | `bc7b10734026` |
| `crates/ferrux-env/src/bootstrap.rs` | TEXT | 22129 | 607 | `c55fdc45b376` |
| `crates/ferrux-env/src/lib.rs` | TEXT | 17336 | 476 | `2f2f712374e3` |
| `crates/ferrux-host/Cargo.toml` | TEXT | 442 | 19 | `c65218ace5aa` |
| `crates/ferrux-host/src/files.rs` | TEXT | 12890 | 371 | `61e1ace259e7` |
| `crates/ferrux-host/src/lib.rs` | TEXT | 22203 | 641 | `cef536a07f32` |
| `crates/ferrux-host/src/tests.rs` | TEXT | 22454 | 680 | `4d25e3511105` |
| `crates/ferrux-jni-core/Cargo.toml` | TEXT | 552 | 22 | `f3141ed19bbf` |
| `crates/ferrux-jni-core/src/core.rs` | TEXT | 16118 | 458 | `e87fd81e8e02` |
| `crates/ferrux-jni-core/src/lib.rs` | TEXT | 18087 | 569 | `24eb9ebd7017` |
| `crates/ferrux-jni-core/src/tests.rs` | TEXT | 1975 | 62 | `3da2988ffd67` |
| `crates/ferrux-pty/Cargo.toml` | TEXT | 299 | 16 | `8f791022761d` |
| `crates/ferrux-pty/src/lib.rs` | TEXT | 27583 | 859 | `52f09ef7c173` |
| `crates/ferrux-pty/src/tests.rs` | TEXT | 11743 | 348 | `1ebd8be353aa` |
| `crates/ferrux-session/Cargo.toml` | TEXT | 311 | 16 | `db8b6fe4cd15` |
| `crates/ferrux-session/src/events.rs` | TEXT | 4692 | 150 | `5c9c83abd960` |
| `crates/ferrux-session/src/lib.rs` | TEXT | 25933 | 748 | `a609beb02387` |
| `crates/ferrux-session/src/tests.rs` | TEXT | 16004 | 454 | `78d2b43c6071` |
| `crates/ferrux-term-jni/Cargo.toml` | TEXT | 387 | 18 | `1d64b830b95d` |
| `crates/ferrux-term-jni/src/lib.rs` | TEXT | 1091 | 27 | `3d6732dfc0e2` |
| `crates/ferrux-term/Cargo.toml` | TEXT | 263 | 14 | `0456a199697b` |
| `crates/ferrux-term/src/color.rs` | TEXT | 8267 | 263 | `48c6d5d73104` |
| `crates/ferrux-term/src/keys.rs` | TEXT | 12401 | 390 | `32f29e2409c6` |
| `crates/ferrux-term/src/lib.rs` | TEXT | 35294 | 935 | `99c96d448b0d` |
| `crates/ferrux-term/src/mouse.rs` | TEXT | 9920 | 355 | `268f48cb82da` |
| `crates/ferrux-term/src/selection.rs` | TEXT | 4200 | 131 | `72d2b7d5cc59` |
| `crates/ferrux-term/src/snapshot.rs` | TEXT | 13833 | 404 | `f6cae8222541` |
| `crates/ferrux-term/src/tests.rs` | TEXT | 25983 | 787 | `955fa721d027` |
| `crates/ferrux-term/src/urls.rs` | TEXT | 6447 | 209 | `ecdeec6e57de` |
| `docs/API.md` | TEXT | 41672 | 825 | `dffe33396345` |
| `docs/PACKAGES.md` | TEXT | 9487 | 178 | `88eb27713cee` |
| `docs/RESEARCH.md` | TEXT | 62718 | 684 | `473e119a6f05` |
| `docs/REUSE.md` | TEXT | 10577 | 203 | `db9668badc55` |
| `rustfmt.toml` | TEXT | 17 | 1 | `f32d2f03b3f4` |
| `scripts/build-native.sh` | TEXT | 903 | 21 | `523826d1977d` |
| `scripts/ci-adb-guard.sh` | TEXT | 1408 | 33 | `e3b7ba9b3bb2` |
| `scripts/ci-api.sh` | TEXT | 6198 | 149 | `6a950229b186` |
| `scripts/ci-bell.sh` | TEXT | 128 | 4 | `afe07f97a2d8` |
| `scripts/ci-colors.sh` | TEXT | 1132 | 31 | `280eedd74e11` |
| `scripts/ci-hyperlink.sh` | TEXT | 324 | 7 | `b0d4685b67d0` |
| `scripts/ci-mouse.sh` | TEXT | 868 | 22 | `c9545d95c8a5` |
| `scripts/ci-osc52.sh` | TEXT | 267 | 5 | `5170a8214bc1` |
| `scripts/ci-post-screenshots.sh` | TEXT | 3695 | 102 | `0588b03bc44c` |
| `scripts/ci-screenshots.sh` | TEXT | 57618 | 1478 | `61603893ded4` |

## 复算命令

```sh
git clone --depth 1 https://github.com/FerraStack/ferrux /tmp/opencode/external/ferrux
git -C /tmp/opencode/external/ferrux ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/ferrux ls-files | wc -l
```
