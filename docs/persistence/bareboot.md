# Bareboot 深度审查

- 来源：https://github.com/rfc9849/Bareboot
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：在已 root 的 Android 上直接启动未修改的 Linux 发行版，不是终端库。终端只是其桌面/控制界面的一部分。

### 分层

- `rust/crates/`：守护进程与协议层。`daemon/src/linux_impl.rs` 负责挂载命名空间、`pivot_root`、seccomp 与进程存活；`daemon/src/dns.rs` 与 `desktop.rs` 提供 DNS 与桌面 companion 通道；`protocol/src/ahb_wire.rs` 定义线协议；`display/src/` 处理 GPU 与远程显示。
- `android/app/src/main/java/ch/ausfuhrli/bareboot/`：Kotlin 控制面。`BarebootViewModel.kt` 串联安装、发行版选择和终端；`ipc/BarebootClient.kt` 与守护进程通信；`terminal/TabConnection.kt` 维护 shell 会话；`ui/ExtraKeysBar.kt`、`ui/TerminalTheme.kt` 提供附加键与主题；`ssh/SshSetup.kt` 配置 USB/Wi-Fi SSH。
- `ksu-module/`、`tools/bench/`、`tools/desktop/`：Magisk/KernelSU 模块、基准与桌面端。
- 终端核心使用 `libghostty`，与本地选型的 VT 解析方向一致，但 Bareboot 的复用点在“会话存活与进程隔离”，不在渲染。

### 关键机制

- 发行版在普通进程外运行：shell 在应用关闭后仍存活，应用只是 attach 到一个已有会话；这让终端 UI 可以自由重建而不需要保留 PTY 所有者。
- 需要 Linux 6.5+、`SO_PEERPIDFD`、root 与 SELinux enforcing；能力检查集中在 `SystemChecks.kt`，不满足条件的设备不会进入安装流程。
- 基准数据覆盖计算、IPC、进程创建和文件 I/O，说明其隔离层开销追求接近原生 Termux；这些数字是设备相关的参考值，不作为本仓性能指标。

### 验证与风险

- 仓库没有独立测试目录；大量 `unsafe` 来自直接系统调用与命名空间操作，正确性依赖内核特性与 root 环境。
- 发行版镜像、模块脚本和桌面 companion 构成较大信任面；任何复刻都必须把镜像来源校验与权限提示放在首位。

### 本地取舍

不采纳其启动链路；可借鉴“终端 UI 与 PTY 所有者解耦”的思路，但本地守护进程边界已由自有规范约束，不能引入 root 依赖。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/ghostty_terminal/public_api.rs` | 对照 `sanitize_vt_input` 的边界：只剔除 NUL，不引入 root/容器进程模型 |
| `native/src/terminal/pty.rs` | 对照 `PtyPair::spawn` 的 fork/exec 失败回报与 FD_CLOEXEC，确认本地不依赖 root 或命名空间 |
| `native/src/android/ffi/input.rs` | 对照会话句柄与 key/paste 的 JNI 输入边界 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` | 对照前后台 Surface 暂停账本与会话切换 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.github/release.yml` | TEXT | 271 | 16 | `aa8c614126c9` |
| `.github/workflows/ci.yml` | TEXT | 4150 | 120 | `da360abcaab3` |
| `.github/workflows/release.yml` | TEXT | 14869 | 420 | `e3e69168f3c5` |
| `.gitignore` | TEXT | 199 | 13 | `48cfe33e1278` |
| `LICENSE` | DECLARED_EXCLUSION | 11352 | 202 | `af7fe7e19003` |
| `README.md` | TEXT | 3904 | 88 | `a67178e4573a` |
| `android/app/build.gradle.kts` | TEXT | 2495 | 81 | `f02077c9219b` |
| `android/app/src/main/AndroidManifest.xml` | TEXT | 2690 | 62 | `c6969ccc1fd8` |
| `android/app/src/main/assets/fonts/GoogleSansCode.ttf` | BINARY | 265764 | 1193 | `e6815145fc59` |
| `android/app/src/main/assets/fonts/OFL.txt` | TEXT | 4396 | 93 | `ff8f4fbc4f1a` |
| `android/app/src/main/assets/ssh-setup.sh` | TEXT | 2506 | 77 | `cca4becf1013` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/BarebootViewModel.kt` | TEXT | 30847 | 831 | `99512ee97bdf` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/MainActivity.kt` | TEXT | 3111 | 86 | `366bbf3219a2` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/RootManager.kt` | TEXT | 762 | 20 | `a700d73428d8` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/SettingsStore.kt` | TEXT | 4671 | 125 | `667b075e8c6a` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/SystemChecks.kt` | TEXT | 6812 | 130 | `ce7dc108821e` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/files/SharedFolderAccess.kt` | TEXT | 4367 | 109 | `20607221cfbb` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/files/SharedFolderProvider.kt` | TEXT | 7823 | 184 | `e441685e5282` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/install/InstallService.kt` | TEXT | 14872 | 347 | `6d4733c81631` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ipc/BarebootClient.kt` | TEXT | 17017 | 567 | `5ec7c3664772` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ipc/DistroCatalog.kt` | TEXT | 6759 | 172 | `3d04b1f9f8a1` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/notify/RunningNotifier.kt` | TEXT | 3641 | 89 | `32955cbcd5a4` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/notify/ShellTileService.kt` | TEXT | 2094 | 54 | `f09561333d01` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ssh/SshSetup.kt` | TEXT | 2228 | 55 | `b8ee5adacbe2` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/terminal/TabConnection.kt` | TEXT | 3988 | 123 | `a465bf2d288e` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/AppIcons.kt` | TEXT | 5142 | 56 | `cae98216a57d` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/BarebootApp.kt` | TEXT | 3789 | 87 | `91a616f21bd2` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/Components.kt` | TEXT | 4312 | 114 | `d67ad29f890c` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/DiagnosticsScreen.kt` | TEXT | 7386 | 166 | `1ed051a092a3` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/ExtraKeysBar.kt` | TEXT | 8735 | 193 | `98964d227ac6` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/HomeScreen.kt` | TEXT | 32065 | 720 | `e139010dbe25` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/ImageSettingsSheet.kt` | TEXT | 18353 | 416 | `0c18d72df455` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/InstallForm.kt` | TEXT | 13782 | 315 | `eecaa456d8d4` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/OnboardingScreen.kt` | TEXT | 19659 | 462 | `34798e898e52` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/SettingsScreen.kt` | TEXT | 31064 | 655 | `23f94ee014a8` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/SshSheet.kt` | TEXT | 9661 | 193 | `47ab42ae3495` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/SystemCheckScreen.kt` | TEXT | 13306 | 311 | `68d9e161b239` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/TerminalScreen.kt` | TEXT | 14902 | 347 | `049c210df65b` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/TerminalTheme.kt` | TEXT | 5134 | 124 | `76fd7cef07e1` |
| `android/app/src/main/java/ch/ausfuhrli/bareboot/ui/theme/Theme.kt` | TEXT | 5175 | 133 | `24c2ac73b42c` |
| `android/app/src/main/res/drawable/ic_add.xml` | TEXT | 638 | 9 | `12f3dd76a233` |
| `android/app/src/main/res/drawable/ic_admin_panel_settings.xml` | TEXT | 1572 | 9 | `30029ee6db34` |
| `android/app/src/main/res/drawable/ic_arrow_back.xml` | TEXT | 642 | 10 | `0f88419ce961` |
| `android/app/src/main/res/drawable/ic_backup.xml` | TEXT | 843 | 11 | `c9f4c768df7e` |
| `android/app/src/main/res/drawable/ic_check_circle.xml` | TEXT | 753 | 9 | `b68d2f55e08e` |
| `android/app/src/main/res/drawable/ic_close.xml` | TEXT | 606 | 9 | `7962f8496787` |
| `android/app/src/main/res/drawable/ic_coffee.xml` | TEXT | 742 | 11 | `82eefc5f8d79` |
| `android/app/src/main/res/drawable/ic_content_paste.xml` | TEXT | 959 | 9 | `19e2703babc6` |
| `android/app/src/main/res/drawable/ic_delete.xml` | TEXT | 1202 | 9 | `1de341599e7a` |
| `android/app/src/main/res/drawable/ic_done.xml` | TEXT | 486 | 9 | `8d1253026a4f` |
| `android/app/src/main/res/drawable/ic_download.xml` | TEXT | 724 | 11 | `fadb0205af93` |
| `android/app/src/main/res/drawable/ic_edit.xml` | TEXT | 599 | 11 | `ed8bf69180e1` |
| `android/app/src/main/res/drawable/ic_error.xml` | TEXT | 874 | 9 | `875f65247032` |
| `android/app/src/main/res/drawable/ic_error_outline.xml` | TEXT | 1148 | 9 | `c7c28992d22b` |
| `android/app/src/main/res/drawable/ic_expand_less.xml` | TEXT | 514 | 9 | `cbe85e2f5026` |
| `android/app/src/main/res/drawable/ic_expand_more.xml` | TEXT | 514 | 9 | `9278235ad82a` |
| `android/app/src/main/res/drawable/ic_extension.xml` | TEXT | 1475 | 9 | `be44162fec35` |
| `android/app/src/main/res/drawable/ic_folder.xml` | TEXT | 942 | 9 | `4ef11a3ba6b5` |
| `android/app/src/main/res/drawable/ic_folder_shared.xml` | TEXT | 1196 | 9 | `13ac98e2b851` |
| `android/app/src/main/res/drawable/ic_format_size.xml` | TEXT | 802 | 9 | `6d064dd67e65` |
| `android/app/src/main/res/drawable/ic_help_outline.xml` | TEXT | 1432 | 10 | `2be1a961418d` |
| `android/app/src/main/res/drawable/ic_info.xml` | TEXT | 1148 | 9 | `ee5cc2210f11` |
| `android/app/src/main/res/drawable/ic_key.xml` | TEXT | 1283 | 9 | `e21794f63fd5` |
| `android/app/src/main/res/drawable/ic_keyboard_arrow_down.xml` | TEXT | 514 | 9 | `15bc1cb324f5` |
| `android/app/src/main/res/drawable/ic_keyboard_arrow_left.xml` | TEXT | 546 | 10 | `9b03d7087cdd` |
| `android/app/src/main/res/drawable/ic_keyboard_arrow_right.xml` | TEXT | 546 | 10 | `c23d56920e1c` |
| `android/app/src/main/res/drawable/ic_keyboard_arrow_up.xml` | TEXT | 478 | 9 | `4d0604c50d72` |
| `android/app/src/main/res/drawable/ic_keyboard_tab.xml` | TEXT | 806 | 10 | `0f3941c03f8f` |
| `android/app/src/main/res/drawable/ic_launcher_background.xml` | TEXT | 684 | 19 | `12ddb87e49ac` |
| `android/app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 3326 | 21 | `d1708ea32067` |
| `android/app/src/main/res/drawable/ic_launcher_monochrome.xml` | TEXT | 620 | 18 | `5a0568f573dd` |
| `android/app/src/main/res/drawable/ic_memory.xml` | TEXT | 1666 | 9 | `dc01065c61be` |
| `android/app/src/main/res/drawable/ic_menu.xml` | TEXT | 784 | 9 | `6e52d2a9036d` |
| `android/app/src/main/res/drawable/ic_more_vert.xml` | TEXT | 736 | 9 | `8c72092c944c` |
| `android/app/src/main/res/drawable/ic_open_in_full.xml` | TEXT | 726 | 9 | `30f1e15ca499` |
| `android/app/src/main/res/drawable/ic_open_in_new.xml` | TEXT | 1031 | 10 | `7f89c4a0d1b4` |
| `android/app/src/main/res/drawable/ic_play_arrow.xml` | TEXT | 518 | 9 | `f752fb7dfe3b` |
| `android/app/src/main/res/drawable/ic_play_circle.xml` | TEXT | 963 | 9 | `cf3f4a80a181` |
| `android/app/src/main/res/drawable/ic_power_settings_new.xml` | TEXT | 767 | 11 | `1e1fafbfbe45` |
| `android/app/src/main/res/drawable/ic_priority_high.xml` | TEXT | 599 | 9 | `1bdf105184f5` |
| `android/app/src/main/res/drawable/ic_refresh.xml` | TEXT | 830 | 9 | `d2fb7d09208a` |
| `android/app/src/main/res/drawable/ic_remove_circle_outline.xml` | TEXT | 995 | 9 | `38f9eb6ad277` |
| `android/app/src/main/res/drawable/ic_restore.xml` | TEXT | 1007 | 9 | `afb5dc15d03b` |
| `android/app/src/main/res/drawable/ic_settings.xml` | TEXT | 2061 | 9 | `6fdb1e928afa` |
| `android/app/src/main/res/drawable/ic_shortcut_terminal.xml` | TEXT | 1127 | 14 | `17cf09e2e618` |
| `android/app/src/main/res/drawable/ic_stop_circle.xml` | TEXT | 1011 | 9 | `a372e1d071d7` |
| `android/app/src/main/res/drawable/ic_storage.xml` | TEXT | 1243 | 9 | `e730293ace79` |
| `android/app/src/main/res/drawable/ic_terminal.xml` | TEXT | 941 | 9 | `615236fcbed2` |
| `android/app/src/main/res/drawable/ic_text_fields.xml` | TEXT | 802 | 9 | `e6ed1ccdfe03` |
| `android/app/src/main/res/drawable/ic_upload_file.xml` | TEXT | 794 | 11 | `0bbf837f99ea` |
| `android/app/src/main/res/drawable/ic_visibility.xml` | TEXT | 1153 | 9 | `3288de2bc57a` |
| `android/app/src/main/res/drawable/ic_warning.xml` | TEXT | 898 | 9 | `d677545ae08b` |
| `android/app/src/main/res/mipmap-anydpi/ic_launcher.xml` | TEXT | 344 | 6 | `ada31da9e23f` |
| `android/app/src/main/res/mipmap-anydpi/ic_launcher_round.xml` | TEXT | 344 | 6 | `ada31da9e23f` |
| `android/app/src/main/res/values-ru/strings.xml` | TEXT | 38174 | 382 | `27f7c32830ba` |
| `android/app/src/main/res/values/strings.xml` | TEXT | 28879 | 383 | `d380694f1956` |
| `android/app/src/main/res/values/themes.xml` | TEXT | 277 | 7 | `baf764873350` |
| `android/build.gradle.kts` | TEXT | 149 | 4 | `8f1ec0a822da` |
| `android/gradle.properties` | TEXT | 146 | 4 | `244a4f86bc5e` |
| `android/settings.gradle.kts` | TEXT | 328 | 18 | `a477f549b48c` |
| `build.sh` | TEXT | 4199 | 105 | `f0969b6953c1` |
| `docs/architecture.md` | TEXT | 1958 | 34 | `772148660839` |
| `docs/benchmarks.md` | TEXT | 3764 | 63 | `d7fcad107a88` |
| `docs/desktop.md` | TEXT | 2986 | 51 | `955d8ad51f9a` |
| `docs/development.md` | TEXT | 1459 | 36 | `8397358b4355` |
| `docs/docker.md` | TEXT | 905 | 27 | `85b87f9de07a` |
| `docs/images/docker.png` | BINARY | 123688 | 365 | `517836d97a55` |
| `docs/images/home.png` | BINARY | 109233 | 355 | `56e28226b428` |
| `docs/images/icon.png` | BINARY | 14996 | 75 | `4959a8477fc0` |
| `docs/images/readme-desktop.gif` | BINARY | 2244397 | 12693 | `35ea27815803` |
| `docs/images/readme-home.png` | BINARY | 145629 | 585 | `25dd53a0348d` |
| `docs/images/readme-settings.png` | BINARY | 210386 | 752 | `5695505f18d7` |
| `docs/images/readme-terminal.png` | BINARY | 452544 | 1684 | `096d67518896` |
| `docs/images/terminal.png` | BINARY | 135210 | 561 | `4d6661f95043` |
| `docs/install.md` | TEXT | 1127 | 24 | `9846d90d8952` |
| `docs/kernel.md` | TEXT | 902 | 17 | `a5dc9225934c` |
| `docs/security.md` | TEXT | 524 | 10 | `fe0ee868d469` |
| `docs/usage.md` | TEXT | 1432 | 50 | `0662f01205ef` |
| `ksu-module/action.sh` | TEXT | 792 | 28 | `ee325bd4ff76` |
| `ksu-module/customize.sh` | TEXT | 1098 | 25 | `9dbc97868a56` |
| `ksu-module/hotinstall.sh` | TEXT | 1191 | 39 | `1a2e414e3990` |
| `ksu-module/module.prop` | TEXT | 131 | 6 | `f20763a4b3d7` |
| `ksu-module/sepolicy.rule` | TEXT | 28837 | 474 | `026641cbfdad` |
| `ksu-module/service.sh` | TEXT | 1505 | 41 | `57696857ac44` |
| `ksu-module/uninstall.sh` | TEXT | 122 | 6 | `2d8ba4e121a0` |
| `rust/.cargo/config.toml` | TEXT | 138 | 3 | `e091107d2ec8` |
| `rust/Cargo.lock` | DECLARED_EXCLUSION | 6038 | 250 | `f500d2a8582d` |
| `rust/Cargo.toml` | TEXT | 472 | 27 | `918321d85f5a` |
| `rust/crates/cli/Cargo.toml` | TEXT | 259 | 15 | `f31999a5fe85` |
| `rust/crates/cli/src/main.rs` | TEXT | 29627 | 836 | `613536659adb` |
| `rust/crates/daemon/Cargo.toml` | TEXT | 240 | 12 | `94a3d578f10a` |
| `rust/crates/daemon/src/desktop.rs` | TEXT | 8805 | 227 | `23be87ba9af8` |
| `rust/crates/daemon/src/dns.rs` | TEXT | 2670 | 85 | `b5b698b06725` |
| `rust/crates/daemon/src/gpu.rs` | BINARY | 15017 | 395 | `710e8e3cd2ea` |
| `rust/crates/daemon/src/lib.rs` | TEXT | 922 | 40 | `4aaa7cee240d` |
| `rust/crates/daemon/src/linux_impl.rs` | TEXT | 53001 | 1402 | `c960a8a28aab` |
| `rust/crates/display/Cargo.toml` | TEXT | 463 | 15 | `7135925cc8e0` |
| `rust/crates/display/bareboot-ahb-v1.xml` | TEXT | 701 | 20 | `927fcf67a4f8` |
| `rust/crates/display/src/direct.rs` | TEXT | 18222 | 581 | `0e7f1a2bcf74` |
| `rust/crates/display/src/linux.rs` | TEXT | 61302 | 1804 | `c2d5568a60dc` |
| `rust/crates/display/src/main.rs` | TEXT | 169 | 10 | `105f84a0ba41` |
| `rust/crates/display/src/remote.rs` | TEXT | 41226 | 1119 | `14cdebdbb840` |
| `rust/crates/protocol/Cargo.toml` | TEXT | 141 | 8 | `0efde7487d79` |
| `rust/crates/protocol/src/ahb_wire.rs` | TEXT | 3389 | 101 | `67dcaac89979` |
| `rust/crates/protocol/src/codec.rs` | TEXT | 32431 | 1122 | `5549664ede5f` |
| `rust/crates/protocol/src/fdpass.rs` | TEXT | 7957 | 238 | `79e2e85562c4` |
| `rust/crates/protocol/src/lib.rs` | TEXT | 595 | 15 | `7f7363ce0271` |
| `rust/crates/runtime/Cargo.toml` | TEXT | 166 | 9 | `78d9214769a8` |
| `rust/crates/runtime/src/auth.rs` | TEXT | 5129 | 164 | `82d8ca0dec91` |
| `rust/crates/runtime/src/caps.rs` | TEXT | 2874 | 107 | `6c1894eb4825` |
| `rust/crates/runtime/src/holder.rs` | TEXT | 44706 | 1217 | `fbe5673453ff` |
| `rust/crates/runtime/src/image.rs` | TEXT | 29532 | 829 | `f2f4e26760b2` |
| `rust/crates/runtime/src/kernel.rs` | TEXT | 5071 | 161 | `4fd582bf7314` |
| `rust/crates/runtime/src/lib.rs` | TEXT | 1100 | 44 | `910ec60eba4a` |
| `rust/crates/runtime/src/loopdev.rs` | TEXT | 4842 | 158 | `97997864d640` |
| `rust/crates/runtime/src/mount.rs` | TEXT | 17753 | 563 | `575f9f97aff0` |
| `rust/crates/runtime/src/pty.rs` | TEXT | 2276 | 70 | `834b9a91783c` |
| `rust/crates/runtime/src/scheduler.rs` | TEXT | 5931 | 202 | `8a4a0c3f356c` |
| `rust/crates/runtime/src/uplink.rs` | TEXT | 11814 | 336 | `ccfb6c10f971` |
| `rust/crates/runtime/src/validate.rs` | TEXT | 13783 | 414 | `290fd5460aa1` |
| `tools/bench/.cargo/config.toml` | TEXT | 138 | 3 | `e091107d2ec8` |
| `tools/bench/Cargo.lock` | DECLARED_EXCLUSION | 158 | 7 | `361a1f3b8f9f` |
| `tools/bench/Cargo.toml` | TEXT | 199 | 13 | `f5cacfeecf8b` |
| `tools/bench/report.py` | TEXT | 3030 | 89 | `62a7f19bcb0d` |
| `tools/bench/src/main.rs` | TEXT | 31925 | 1028 | `96763c08d0be` |
| `tools/check_16k.py` | TEXT | 2247 | 57 | `bbfd7dde0c14` |
| `tools/check_kernel.py` | TEXT | 1687 | 40 | `72143c1c4f8c` |
| `tools/desktop/Cargo.lock` | DECLARED_EXCLUSION | 196367 | 7995 | `5408bd834e4b` |
| `tools/desktop/Cargo.toml` | TEXT | 929 | 29 | `723eb2e6883f` |
| `tools/desktop/package-macos.sh` | TEXT | 3646 | 101 | `f2708837ba54` |
| `tools/desktop/src/adb.rs` | TEXT | 41034 | 1219 | `8d2a5487323b` |
| `tools/desktop/src/app.rs` | TEXT | 30239 | 861 | `2cfae9855cf8` |
| `tools/desktop/src/backend.rs` | TEXT | 36439 | 999 | `d6e60c93de78` |
| `tools/desktop/src/chrome.rs` | TEXT | 1447 | 43 | `f187752b7732` |
| `tools/desktop/src/design/assets.rs` | TEXT | 13883 | 116 | `46deee3fe8b2` |
| `tools/desktop/src/design/mod.rs` | TEXT | 170 | 7 | `e6cdfc767e04` |
| `tools/desktop/src/design/theme.rs` | TEXT | 4812 | 145 | `1233a1916c60` |
| `tools/desktop/src/design/ui.rs` | TEXT | 9625 | 317 | `a92e9d261c76` |
| `tools/desktop/src/dialogs.rs` | TEXT | 26869 | 665 | `388657e45412` |
| `tools/desktop/src/display/gl.rs` | TEXT | 18130 | 437 | `28881258d5f7` |
| `tools/desktop/src/display/keys.rs` | TEXT | 2673 | 120 | `e190a58fd3df` |
| `tools/desktop/src/display/mac.rs` | TEXT | 7730 | 220 | `305ab2fd5a10` |
| `tools/desktop/src/display/mod.rs` | TEXT | 13867 | 413 | `c8e647fda4fa` |
| `tools/desktop/src/display/net.rs` | TEXT | 13244 | 414 | `a4c91801540f` |
| `tools/desktop/src/lib.rs` | TEXT | 286 | 16 | `6f5c798fbd15` |
| `tools/desktop/src/main.rs` | TEXT | 5905 | 171 | `74cedac3f7d9` |
| `tools/desktop/src/model.rs` | TEXT | 6439 | 290 | `75d59d42f7ad` |
| `tools/desktop/src/pages.rs` | TEXT | 79979 | 1957 | `b7ff5ddfd31d` |
| `tools/desktop/src/prefs.rs` | TEXT | 5566 | 202 | `1f4873b661fb` |
| `tools/desktop/src/single.rs` | TEXT | 1957 | 68 | `28d2bf79c62a` |
| `tools/device_smoke.py` | TEXT | 15317 | 347 | `860b869415f4` |
| `tools/gpu/Cargo.lock` | DECLARED_EXCLUSION | 26979 | 1084 | `ec52065655ed` |
| `tools/gpu/ahb_host.rs` | TEXT | 6702 | 181 | `ed4bd66a8b65` |
| `tools/gpu/build-guest.sh` | TEXT | 5722 | 116 | `0a04e27b8936` |
| `tools/gpu/build-host.sh` | TEXT | 4295 | 93 | `fc8f5fade455` |
| `tools/gpu/compute.c` | TEXT | 7003 | 106 | `e4e135758b31` |
| `tools/gpu/compute.comp` | TEXT | 186 | 4 | `8c5a70dd304e` |
| `tools/gpu/gbm_backend.c` | TEXT | 13151 | 458 | `77e8bbd39315` |
| `tools/gpu/gfxstream.patch` | TEXT | 22800 | 540 | `86289c0c3db8` |
| `tools/gpu/hwdec_avcodec.c` | TEXT | 22275 | 598 | `0380fa7583a6` |
| `tools/gpu/media_host.c` | TEXT | 55049 | 1437 | `843f38073473` |
| `tools/gpu/mesa.patch` | TEXT | 37818 | 869 | `77bd461c4664` |
| `tools/gpu/nativewindow/Cargo.toml` | TEXT | 119 | 8 | `6ed2e441064c` |
| `tools/gpu/nativewindow/src/lib.rs` | TEXT | 6053 | 173 | `4970cb5ab3e5` |
| `tools/gpu/run` | TEXT | 2500 | 70 | `4f9817584b1d` |
| `tools/gpu/rutabaga.patch` | TEXT | 6302 | 161 | `fd1ab503c612` |
| `tools/gpu/sepolicy.rule` | TEXT | 17278 | 263 | `a07dd2bbc8b0` |

## 复算命令

```sh
git clone --depth 1 https://github.com/rfc9849/Bareboot /tmp/opencode/external/Bareboot
git -C /tmp/opencode/external/Bareboot ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/Bareboot ls-files | wc -l
```
