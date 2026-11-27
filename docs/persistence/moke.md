# moke 深度审查

- 来源：https://github.com/briqt/moke
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：Android SSH/mosh 终端，内置 Tailscale、代理、tmux、SFTP、加密备份；终端模块来自 Termux 生态的 vendored 代码。

### 分层

- `app/src/main/java/com/briqt/moke/`：应用与网络层。`data/` 管主机、密钥、网络、设置；`net/` 管 Tailscale 状态、设备选择与 socket；`sync/` 管 WebDAV 加密备份与恢复计划；`terminal/` 把认证、剪贴板、滚动和会话串起来。
- `terminal-emulator/` 与 `terminal-view/`：vendored 的 Termux VT 与绘制代码（Apache-2.0），产品层为 GPL-3.0-or-later；`THIRD_PARTY_NOTICES.md` 记录组件与字体许可。
- `netbridge/`：网络桥接；`app/src/androidTest/` 有大量 UI 与终端交互测试。

### 关键机制

-凭据用 Android Keystore 加密；Tailscale 是应用内 userspace 客户端，不占系统 VPN 槽位，也不给其他 App 提供网络。
- SSH 支持密码、PEM、Tailscale SSH identity、跳板机与每主机启动命令；mosh 要求远端 `mosh-server` 与 UDP 可达，HTTP/HTTPS/SOCKS4 代理不能承载 mosh。
- tmux 面板管理远端会话；SFTP 支持断点续传并把路径送回终端；端口转发只监听设备 loopback。
- 134 个测试文件是这批仓库中最高的，包含 `TerminalInputConnectionTest`、`TerminalRendererTest`、`TerminalClipboardPasteTest` 等真实交互测试；说明其终端模块虽是 vendored，但集成面被认真验证。

### 验证与风险

- 双许可与 vendored 修改使再分发义务复杂；产品层 GPL-3.0 也限制直接复用。
- Tailscale、代理、mosh、备份各自有状态机，整体复杂度远超终端核心；跳板机不承载 mosh UDP 的文档限制说明网络回退并不透明。
- `TODO/FIXME` 14 处，但测试覆盖降低了盲改风险。

### 本地取舍

借鉴其输入连接/剪贴板/滚动手势的 androidTest 组织方式；不引入 SSH/Tailscale/备份。凭据加密与“转发只听 loopback”是安全边界的正确示例，若本地未来增加远程能力必须同等对待。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt` | 对照输入连接、组字差分与批量编辑的测试组织 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | 对照键事件到 VT 序列的编码边界 |
| `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 对照原生库缺失时的显式降级 |
| `native/src/android/ffi/input.rs` | 对照凭据/剪贴板不进入 PTY 写入边界 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.github/ISSUE_TEMPLATE/bug_report.md` | TEXT | 954 | 31 | `d62c0004a481` |
| `.github/ISSUE_TEMPLATE/feature_request.md` | TEXT | 1028 | 30 | `1366072a0186` |
| `.github/workflows/ci.yml` | TEXT | 1832 | 68 | `449c39d636a2` |
| `.github/workflows/release.yml` | TEXT | 5157 | 122 | `74c724c642ca` |
| `.gitignore` | TEXT | 631 | 26 | `bd92dd2effbc` |
| `CHANGELOG.md` | TEXT | 38844 | 352 | `9a080364d67c` |
| `CODE_OF_CONDUCT.md` | TEXT | 453 | 9 | `b6770b0377fb` |
| `CONTRIBUTING.md` | TEXT | 5762 | 90 | `898f385160a6` |
| `CONTRIBUTING.zh-CN.md` | TEXT | 5198 | 90 | `16d7609c4322` |
| `COPYRIGHT.md` | TEXT | 1330 | 29 | `ed46c975f3c9` |
| `LICENSE` | DECLARED_EXCLUSION | 35149 | 674 | `3972dc9744f6` |
| `LICENSE-APACHE-2.0.txt` | DECLARED_EXCLUSION | 6608 | 126 | `9e58c63c26ff` |
| `README.md` | TEXT | 4425 | 59 | `fdbeea9b89f3` |
| `README.zh-CN.md` | TEXT | 4003 | 59 | `b2a090bb1668` |
| `THIRD_PARTY_NOTICES.md` | TEXT | 5410 | 66 | `665832d366e6` |
| `app/build.gradle.kts` | TEXT | 5435 | 147 | `8d5cef5da14e` |
| `app/proguard-rules.pro` | TEXT | 425 | 12 | `e79092226193` |
| `app/src/androidTest/java/com/briqt/moke/data/FontSelectionMigrationTest.kt` | TEXT | 7935 | 167 | `5331156afff7` |
| `app/src/androidTest/java/com/briqt/moke/terminal/AuthenticationOwnerTest.kt` | TEXT | 11248 | 258 | `652e561444f1` |
| `app/src/androidTest/java/com/briqt/moke/terminal/TerminalClipboardPasteTest.kt` | TEXT | 7993 | 210 | `2b7c83fa8551` |
| `app/src/androidTest/java/com/briqt/moke/terminal/TerminalScrollGestureTest.kt` | TEXT | 27052 | 639 | `fa9551998700` |
| `app/src/androidTest/java/com/briqt/moke/ui/GroupHeaderRowTest.kt` | TEXT | 17644 | 363 | `e7aad00ed33a` |
| `app/src/androidTest/java/com/briqt/moke/ui/MokeActivityFixture.kt` | TEXT | 7440 | 149 | `6effb93109ef` |
| `app/src/androidTest/java/com/briqt/moke/ui/TailnetDevicePickerTest.kt` | TEXT | 11738 | 239 | `e1ea31385bd6` |
| `app/src/androidTest/java/com/briqt/moke/ui/TmuxNameDialogInputTest.kt` | TEXT | 9984 | 223 | `f1e3141bd3ba` |
| `app/src/androidTest/java/com/briqt/moke/ui/TmuxSheetRefreshTest.kt` | TEXT | 11193 | 215 | `991d8b89486a` |
| `app/src/androidTest/java/com/termux/view/TerminalInputConnectionTest.kt` | TEXT | 8547 | 181 | `53825ea3d82b` |
| `app/src/androidTest/java/com/termux/view/TerminalInputConnectionWindowTest.kt` | TEXT | 5079 | 96 | `8a65ef5b86d7` |
| `app/src/androidTest/java/com/termux/view/TerminalRendererTest.kt` | TEXT | 43863 | 861 | `c6f786eadb9e` |
| `app/src/main/AndroidManifest.xml` | TEXT | 2777 | 61 | `135506adfadd` |
| `app/src/main/assets/terminfo/a/ansi` | BINARY | 1481 | 2 | `93ec8cb9beb0` |
| `app/src/main/assets/terminfo/d/dumb` | BINARY | 308 | 3 | `123c85a2812a` |
| `app/src/main/assets/terminfo/l/linux` | BINARY | 1740 | 5 | `b70a4941416e` |
| `app/src/main/assets/terminfo/s/screen` | BINARY | 1607 | 3 | `173d3433ab6c` |
| `app/src/main/assets/terminfo/s/screen-256color` | BINARY | 1747 | 3 | `cbac29ca9641` |
| `app/src/main/assets/terminfo/v/vt100` | BINARY | 1282 | 4 | `779a219d6ed2` |
| `app/src/main/assets/terminfo/x/xterm` | BINARY | 3991 | 5 | `83fc97902618` |
| `app/src/main/assets/terminfo/x/xterm-256color` | BINARY | 4071 | 4 | `7f27e57e318f` |
| `app/src/main/assets/terminfo/x/xterm-color` | BINARY | 1551 | 3 | `f74fe619914b` |
| `app/src/main/assets/tmux/observer.sh` | TEXT | 3359 | 85 | `c044f95abcb1` |
| `app/src/main/assets/tmux/probe.sh` | TEXT | 3528 | 71 | `24f7de7a8eaf` |
| `app/src/main/java/com/briqt/moke/LocaleManager.kt` | TEXT | 2795 | 62 | `be365fa4a00d` |
| `app/src/main/java/com/briqt/moke/MainActivity.kt` | TEXT | 5383 | 119 | `2700b4b7b875` |
| `app/src/main/java/com/briqt/moke/MokeApplication.kt` | TEXT | 6377 | 131 | `29d2263c1eaa` |
| `app/src/main/java/com/briqt/moke/data/CredentialCrypto.kt` | TEXT | 2539 | 62 | `c00a35186b5f` |
| `app/src/main/java/com/briqt/moke/data/Host.kt` | TEXT | 6686 | 147 | `eb00826fdafc` |
| `app/src/main/java/com/briqt/moke/data/HostStore.kt` | TEXT | 14158 | 308 | `6cf9dfe1f7f3` |
| `app/src/main/java/com/briqt/moke/data/KeyMigration.kt` | TEXT | 6665 | 152 | `3cdd0205d55c` |
| `app/src/main/java/com/briqt/moke/data/KeyStore.kt` | TEXT | 7960 | 201 | `ec61087b65b3` |
| `app/src/main/java/com/briqt/moke/data/ListOrder.kt` | TEXT | 1177 | 32 | `1611ab47754e` |
| `app/src/main/java/com/briqt/moke/data/NetworkEntry.kt` | TEXT | 3058 | 75 | `eea52924de1a` |
| `app/src/main/java/com/briqt/moke/data/NetworkStore.kt` | TEXT | 7229 | 168 | `cf9fbdfb80aa` |
| `app/src/main/java/com/briqt/moke/data/Route.kt` | TEXT | 6680 | 138 | `15807b87f131` |
| `app/src/main/java/com/briqt/moke/data/SettingsStore.kt` | TEXT | 26473 | 564 | `d17d9a8189a9` |
| `app/src/main/java/com/briqt/moke/data/StoreResult.kt` | TEXT | 4433 | 88 | `46e63e85b84c` |
| `app/src/main/java/com/briqt/moke/data/TmuxTargetStamp.kt` | TEXT | 3434 | 71 | `54fadd5d95b0` |
| `app/src/main/java/com/briqt/moke/data/UiPrefs.kt` | TEXT | 2005 | 48 | `d26dcfde5422` |
| `app/src/main/java/com/briqt/moke/net/NetworkErrors.kt` | TEXT | 2863 | 59 | `8e9b9148500c` |
| `app/src/main/java/com/briqt/moke/net/NetworkManager.kt` | TEXT | 16775 | 305 | `1c441d98769c` |
| `app/src/main/java/com/briqt/moke/net/NetworkRecovery.kt` | TEXT | 2474 | 57 | `b1c3b1d4c3e2` |
| `app/src/main/java/com/briqt/moke/net/NetworkRuntime.kt` | TEXT | 22119 | 481 | `fe62ff44df49` |
| `app/src/main/java/com/briqt/moke/net/NetworkSocket.kt` | TEXT | 5940 | 119 | `0c74f78200c1` |
| `app/src/main/java/com/briqt/moke/net/TailnetDevices.kt` | TEXT | 2727 | 64 | `4b99d22238d4` |
| `app/src/main/java/com/briqt/moke/net/TailnetStateStore.kt` | TEXT | 3061 | 75 | `21b83be5262c` |
| `app/src/main/java/com/briqt/moke/sync/BackupName.kt` | TEXT | 2778 | 55 | `e91bb7aa3885` |
| `app/src/main/java/com/briqt/moke/sync/RestorePlan.kt` | TEXT | 17373 | 361 | `4a4c4028aeb3` |
| `app/src/main/java/com/briqt/moke/sync/SyncBundle.kt` | TEXT | 9604 | 201 | `971ce93de54e` |
| `app/src/main/java/com/briqt/moke/sync/SyncConfigStore.kt` | TEXT | 2682 | 65 | `3c89f49aba6d` |
| `app/src/main/java/com/briqt/moke/sync/SyncController.kt` | TEXT | 14823 | 309 | `613faedad9f0` |
| `app/src/main/java/com/briqt/moke/sync/SyncCrypto.kt` | TEXT | 6643 | 152 | `c120e539c62d` |
| `app/src/main/java/com/briqt/moke/sync/WebDavClient.kt` | TEXT | 10597 | 241 | `987d485a1cae` |
| `app/src/main/java/com/briqt/moke/terminal/BackgroundKeepalive.kt` | TEXT | 10825 | 284 | `22a33fa5e5a9` |
| `app/src/main/java/com/briqt/moke/terminal/EncryptedPkcs8.kt` | TEXT | 2346 | 47 | `357856db246b` |
| `app/src/main/java/com/briqt/moke/terminal/FontCatalog.kt` | TEXT | 10678 | 200 | `0cc9a7262345` |
| `app/src/main/java/com/briqt/moke/terminal/FontRepository.kt` | TEXT | 10848 | 231 | `a9a5219b900f` |
| `app/src/main/java/com/briqt/moke/terminal/HostKeyChangePrompt.kt` | TEXT | 1150 | 38 | `73de95a8f71e` |
| `app/src/main/java/com/briqt/moke/terminal/HostKeyFingerprint.kt` | TEXT | 2531 | 60 | `298ed1c41e41` |
| `app/src/main/java/com/briqt/moke/terminal/HostKeyPrompt.kt` | TEXT | 5529 | 131 | `ac22da8dde6b` |
| `app/src/main/java/com/briqt/moke/terminal/KeySeq.kt` | TEXT | 10746 | 250 | `8eb22ef43234` |
| `app/src/main/java/com/briqt/moke/terminal/KnownHosts.kt` | TEXT | 2212 | 52 | `700d2165a636` |
| `app/src/main/java/com/briqt/moke/terminal/Modifiers.kt` | TEXT | 2282 | 59 | `756ff2bbedc1` |
| `app/src/main/java/com/briqt/moke/terminal/MokeHostKeyVerifier.kt` | TEXT | 3092 | 65 | `c8da5d226546` |
| `app/src/main/java/com/briqt/moke/terminal/MokeSessionService.kt` | TEXT | 13827 | 352 | `0d5b7f39b16d` |
| `app/src/main/java/com/briqt/moke/terminal/MokeTransferService.kt` | TEXT | 5303 | 137 | `708915e08aac` |
| `app/src/main/java/com/briqt/moke/terminal/MoshBootstrap.kt` | TEXT | 3309 | 66 | `c0cd24f75715` |
| `app/src/main/java/com/briqt/moke/terminal/MoshBootstrapOperation.kt` | TEXT | 2851 | 72 | `0340278a19c3` |
| `app/src/main/java/com/briqt/moke/terminal/MoshChildProcess.kt` | TEXT | 9675 | 241 | `b09435d92513` |
| `app/src/main/java/com/briqt/moke/terminal/MoshTransport.kt` | TEXT | 27275 | 554 | `450761939b9e` |
| `app/src/main/java/com/briqt/moke/terminal/PortForwards.kt` | TEXT | 10547 | 242 | `b75222b737a9` |
| `app/src/main/java/com/briqt/moke/terminal/PreviewTransport.kt` | TEXT | 2760 | 52 | `34e97e2ad6df` |
| `app/src/main/java/com/briqt/moke/terminal/RefusedTransport.kt` | TEXT | 788 | 18 | `c2b1e177af57` |
| `app/src/main/java/com/briqt/moke/terminal/SessionManager.kt` | TEXT | 19054 | 385 | `0777082d582b` |
| `app/src/main/java/com/briqt/moke/terminal/SessionMetadata.kt` | TEXT | 7361 | 164 | `7c8eeebcb2d4` |
| `app/src/main/java/com/briqt/moke/terminal/SessionTitlePolicy.kt` | TEXT | 2960 | 55 | `20f527d72ddd` |
| `app/src/main/java/com/briqt/moke/terminal/SshConnector.kt` | TEXT | 11059 | 222 | `449caf21aa64` |
| `app/src/main/java/com/briqt/moke/terminal/SshKeys.kt` | TEXT | 26041 | 612 | `7acccd16cf25` |
| `app/src/main/java/com/briqt/moke/terminal/SshTransport.kt` | TEXT | 15199 | 280 | `04c162b58604` |
| `app/src/main/java/com/briqt/moke/terminal/TailscaleSshAuth.kt` | TEXT | 9814 | 221 | `048b07fd8ad6` |
| `app/src/main/java/com/briqt/moke/terminal/TailscaleSshAuthText.kt` | TEXT | 2562 | 54 | `3c673b3ea826` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalAlerts.kt` | TEXT | 7449 | 172 | `43601024eb6a` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalController.kt` | TEXT | 10949 | 220 | `1696e346d4ae` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalInputQueue.kt` | TEXT | 6041 | 152 | `09d20f0191bd` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalLinks.kt` | TEXT | 5584 | 111 | `1040f640bebc` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalSizeState.kt` | TEXT | 1211 | 38 | `7adaab8f025d` |
| `app/src/main/java/com/briqt/moke/terminal/TerminalThemes.kt` | TEXT | 7482 | 84 | `3f687a0e5f55` |
| `app/src/main/java/com/briqt/moke/terminal/Tmux.kt` | TEXT | 14066 | 280 | `8aafb558d2ed` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxChoiceTickets.kt` | TEXT | 1232 | 34 | `46c0cd0f530d` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxControlOperation.kt` | TEXT | 4348 | 103 | `37f83105cb0b` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxCoordinator.kt` | TEXT | 33412 | 604 | `00ec76a53799` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxDarwin.kt` | TEXT | 23925 | 429 | `80fc7999f3c0` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxIdentity.kt` | TEXT | 5739 | 161 | `7edfd4deca32` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxLaunchPolicy.kt` | TEXT | 2005 | 42 | `e524faa0ef03` |
| `app/src/main/java/com/briqt/moke/terminal/TmuxRemote.kt` | TEXT | 56479 | 877 | `028c00f37232` |
| `app/src/main/java/com/briqt/moke/terminal/TransportResources.kt` | TEXT | 3387 | 109 | `df3449938eb6` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/RemotePath.kt` | TEXT | 4456 | 114 | `07cd27e36769` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/SftpSession.kt` | TEXT | 9465 | 251 | `09f3acaaa9ea` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/SftpStat.kt` | TEXT | 414 | 8 | `e6c205cc6532` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/Transfer.kt` | TEXT | 5396 | 122 | `572e628685f7` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/TransferManager.kt` | TEXT | 33281 | 761 | `71323963ba50` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/TransferStore.kt` | TEXT | 1919 | 45 | `42d47db56050` |
| `app/src/main/java/com/briqt/moke/terminal/sftp/UploadDestination.kt` | TEXT | 2297 | 51 | `948812600231` |
| `app/src/main/java/com/briqt/moke/ui/AboutScreen.kt` | TEXT | 12376 | 278 | `5e69523e3cdb` |
| `app/src/main/java/com/briqt/moke/ui/Adaptive.kt` | TEXT | 4229 | 126 | `330c94100b27` |
| `app/src/main/java/com/briqt/moke/ui/AppearanceScreen.kt` | TEXT | 25733 | 545 | `17945f27912e` |
| `app/src/main/java/com/briqt/moke/ui/Components.kt` | TEXT | 29966 | 693 | `ef3da6140e5c` |
| `app/src/main/java/com/briqt/moke/ui/EditorScaffold.kt` | TEXT | 4522 | 107 | `7c41b53d9302` |
| `app/src/main/java/com/briqt/moke/ui/ExtraKeyLayout.kt` | TEXT | 47591 | 1094 | `4960584e46fc` |
| `app/src/main/java/com/briqt/moke/ui/ExtraKeys.kt` | TEXT | 49204 | 1103 | `e826e91998d3` |
| `app/src/main/java/com/briqt/moke/ui/ExtraKeysSettingsScreen.kt` | TEXT | 72206 | 1593 | `4b647f4e5fab` |
| `app/src/main/java/com/briqt/moke/ui/FilesController.kt` | TEXT | 17268 | 352 | `e06ecb8172ee` |
| `app/src/main/java/com/briqt/moke/ui/FilesScreen.kt` | TEXT | 37098 | 807 | `2df350d68d15` |
| `app/src/main/java/com/briqt/moke/ui/FilesWork.kt` | TEXT | 3761 | 75 | `2a72d77ae4fd` |
| `app/src/main/java/com/briqt/moke/ui/FontsScreen.kt` | TEXT | 13258 | 292 | `9ce22f1a0b9d` |
| `app/src/main/java/com/briqt/moke/ui/FormTextField.kt` | TEXT | 4551 | 121 | `33a8007604a2` |
| `app/src/main/java/com/briqt/moke/ui/FormValidation.kt` | TEXT | 3718 | 81 | `6aa9fdd2b01a` |
| `app/src/main/java/com/briqt/moke/ui/ForwardPanel.kt` | TEXT | 10838 | 238 | `76c126a2e927` |
| `app/src/main/java/com/briqt/moke/ui/HomeScreen.kt` | TEXT | 62872 | 1268 | `e1d7628e73f8` |
| `app/src/main/java/com/briqt/moke/ui/HostEditScreen.kt` | TEXT | 32712 | 487 | `9525d2649dcf` |
| `app/src/main/java/com/briqt/moke/ui/KeyScreens.kt` | TEXT | 46273 | 990 | `93a2ceba4a1a` |
| `app/src/main/java/com/briqt/moke/ui/MokeApp.kt` | TEXT | 48234 | 955 | `487be056ba37` |
| `app/src/main/java/com/briqt/moke/ui/MokeViewModel.kt` | TEXT | 72403 | 1382 | `1754975820c9` |
| `app/src/main/java/com/briqt/moke/ui/NetworkLoginController.kt` | TEXT | 8222 | 178 | `e863c754445e` |
| `app/src/main/java/com/briqt/moke/ui/NetworkPresentation.kt` | TEXT | 1839 | 34 | `22a19f02d397` |
| `app/src/main/java/com/briqt/moke/ui/NetworkScreens.kt` | TEXT | 34447 | 565 | `19b1a3458909` |
| `app/src/main/java/com/briqt/moke/ui/Reorderable.kt` | BINARY | 14902 | 264 | `020ad1eea4cd` |
| `app/src/main/java/com/briqt/moke/ui/RichDropdown.kt` | TEXT | 7578 | 187 | `62b8d9a55edf` |
| `app/src/main/java/com/briqt/moke/ui/SessionCloseDialog.kt` | TEXT | 7535 | 150 | `c65e2e1666f6` |
| `app/src/main/java/com/briqt/moke/ui/SessionGrouping.kt` | TEXT | 1593 | 38 | `c239ac34abc9` |
| `app/src/main/java/com/briqt/moke/ui/SessionTitleDialog.kt` | TEXT | 5135 | 122 | `bd82f38fab55` |
| `app/src/main/java/com/briqt/moke/ui/SessionTitleDraft.kt` | TEXT | 455 | 9 | `2f1c8c1e48cc` |
| `app/src/main/java/com/briqt/moke/ui/SyncScreens.kt` | TEXT | 39998 | 733 | `0dd2318cfc07` |
| `app/src/main/java/com/briqt/moke/ui/TailnetDevicePicker.kt` | TEXT | 8961 | 161 | `f124651abab7` |
| `app/src/main/java/com/briqt/moke/ui/TailscaleSshDialog.kt` | TEXT | 2941 | 59 | `23853ae1998e` |
| `app/src/main/java/com/briqt/moke/ui/TerminalContentLayout.kt` | TEXT | 1527 | 34 | `24ea360f5448` |
| `app/src/main/java/com/briqt/moke/ui/TerminalImeFallback.kt` | TEXT | 2620 | 70 | `31b0800e495a` |
| `app/src/main/java/com/briqt/moke/ui/TerminalInputDialog.kt` | TEXT | 3241 | 75 | `bf7d2b2a9e99` |
| `app/src/main/java/com/briqt/moke/ui/TerminalScreen.kt` | TEXT | 58897 | 1145 | `cc8569336056` |
| `app/src/main/java/com/briqt/moke/ui/TerminalSettingsScreen.kt` | TEXT | 14210 | 320 | `5559ed157d55` |
| `app/src/main/java/com/briqt/moke/ui/TmuxNameDraft.kt` | TEXT | 707 | 22 | `9635e1dcab89` |
| `app/src/main/java/com/briqt/moke/ui/TmuxPanel.kt` | TEXT | 28319 | 552 | `774efd43b872` |
| `app/src/main/java/com/briqt/moke/ui/TmuxPicker.kt` | TEXT | 1115 | 34 | `5664d7ebc5f9` |
| `app/src/main/java/com/briqt/moke/ui/TmuxPresentation.kt` | TEXT | 7989 | 177 | `8109a83856f0` |
| `app/src/main/java/com/briqt/moke/ui/TmuxRowDropdownMenu.kt` | TEXT | 4991 | 106 | `f814460a915f` |
| `app/src/main/java/com/briqt/moke/ui/TmuxRowMenuState.kt` | TEXT | 1073 | 28 | `8534eb20e820` |
| `app/src/main/java/com/briqt/moke/ui/TmuxSheetController.kt` | TEXT | 5238 | 136 | `9678a954723b` |
| `app/src/main/java/com/briqt/moke/ui/theme/Color.kt` | TEXT | 2415 | 54 | `cc1e6602e476` |
| `app/src/main/java/com/briqt/moke/ui/theme/Dimens.kt` | TEXT | 774 | 18 | `5c05e3e028a8` |
| `app/src/main/java/com/briqt/moke/ui/theme/Fonts.kt` | TEXT | 1044 | 21 | `f4d4518dcd72` |
| `app/src/main/java/com/briqt/moke/ui/theme/Shape.kt` | TEXT | 1452 | 35 | `a82637538439` |
| `app/src/main/java/com/briqt/moke/ui/theme/Theme.kt` | TEXT | 8191 | 218 | `6560c3a71b4c` |
| `app/src/main/java/com/briqt/moke/update/UpdateChecker.kt` | TEXT | 7551 | 165 | `63d33ceb1b9d` |
| `app/src/main/res/drawable/ic_launcher_background.xml` | TEXT | 386 | 9 | `3f15aa047471` |
| `app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 1516 | 12 | `ea97c8d36daa` |
| `app/src/main/res/drawable/ic_launcher_monochrome.xml` | TEXT | 1500 | 12 | `0c6ecff7efe2` |
| `app/src/main/res/drawable/ic_notification.xml` | TEXT | 1590 | 14 | `27130c267629` |
| `app/src/main/res/font/jetbrains_mono.ttf` | BINARY | 303128 | 3475 | `6c5c06e39545` |
| `app/src/main/res/font/noto_sans_symbols2.ttf` | BINARY | 1233128 | 16432 | `7d5fb73b7ca6` |
| `app/src/main/res/font/symbols_nerd_font.ttf` | BINARY | 2507556 | 13541 | `f0f624d9b474` |
| `app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml` | TEXT | 344 | 6 | `ada31da9e23f` |
| `app/src/main/res/values-night/colors.xml` | TEXT | 113 | 4 | `cbdcae76e654` |
| `app/src/main/res/values-zh/extra_keys_strings.xml` | TEXT | 4175 | 77 | `b1f0eb0c81d6` |
| `app/src/main/res/values-zh/key_strings.xml` | TEXT | 3876 | 60 | `030117b01019` |
| `app/src/main/res/values-zh/scroll_strings.xml` | TEXT | 543 | 9 | `aedb356ab218` |
| `app/src/main/res/values-zh/strings.xml` | TEXT | 41981 | 658 | `6dba9e6cebd9` |
| `app/src/main/res/values-zh/sync_strings.xml` | TEXT | 7753 | 107 | `85eb31b368cc` |
| `app/src/main/res/values-zh/tailnet_devices.xml` | TEXT | 1455 | 19 | `6eff596ba17b` |
| `app/src/main/res/values-zh/tailscale_ssh.xml` | TEXT | 1018 | 10 | `e8684a4e9146` |
| `app/src/main/res/values/colors.xml` | TEXT | 363 | 7 | `bb0e0ce5fa51` |
| `app/src/main/res/values/extra_keys_strings.xml` | TEXT | 4148 | 77 | `d22ade52cce8` |
| `app/src/main/res/values/key_strings.xml` | TEXT | 4187 | 63 | `afa0fc5dbfad` |
| `app/src/main/res/values/scroll_strings.xml` | TEXT | 572 | 9 | `d3632bfcfc2e` |
| `app/src/main/res/values/strings.xml` | TEXT | 43887 | 669 | `2b25ea8b42e6` |
| `app/src/main/res/values/sync_strings.xml` | TEXT | 8623 | 112 | `71ec5c1469dd` |
| `app/src/main/res/values/tailnet_devices.xml` | TEXT | 1469 | 19 | `945f67d57b96` |
| `app/src/main/res/values/tailscale_ssh.xml` | TEXT | 1097 | 10 | `9f9bf76aec83` |
| `app/src/main/res/values/themes.xml` | TEXT | 481 | 9 | `4e1b62e7d35a` |
| `app/src/main/res/xml/backup_rules.xml` | TEXT | 865 | 15 | `f08386a56555` |
| `app/src/main/res/xml/data_extraction_rules.xml` | TEXT | 1218 | 22 | `a7a6a5075889` |
| `app/src/test/java/com/briqt/moke/LocalizationGuardTest.kt` | TEXT | 4729 | 108 | `41900330c3be` |
| `app/src/test/java/com/briqt/moke/ResourceConsistencyTest.kt` | TEXT | 5705 | 112 | `10a6d739987a` |
| `app/src/test/java/com/briqt/moke/data/HostStartupCommandTest.kt` | TEXT | 1418 | 42 | `81867330decb` |
| `app/src/test/java/com/briqt/moke/data/HostStoreDecodeTest.kt` | TEXT | 1894 | 56 | `23bb21b8a7cc` |
| `app/src/test/java/com/briqt/moke/data/HostUpdateTest.kt` | TEXT | 1625 | 44 | `5d617b839d9b` |
| `app/src/test/java/com/briqt/moke/data/KeyMigrationTest.kt` | TEXT | 7759 | 143 | `1e93431ff43c` |
| `app/src/test/java/com/briqt/moke/data/KeyStoreTest.kt` | TEXT | 5637 | 131 | `88dfc045a5c4` |
| `app/src/test/java/com/briqt/moke/data/NetworkEntryTest.kt` | TEXT | 2701 | 53 | `7ea4b5251f64` |
| `app/src/test/java/com/briqt/moke/data/NetworkImpactTest.kt` | TEXT | 2527 | 49 | `e0ff3366a543` |
| `app/src/test/java/com/briqt/moke/data/RouteTest.kt` | TEXT | 7279 | 130 | `ea6c81409c87` |
| `app/src/test/java/com/briqt/moke/data/SpacingPreferenceTest.kt` | TEXT | 2847 | 63 | `dd11a36c449a` |
| `app/src/test/java/com/briqt/moke/data/StoreEditTest.kt` | TEXT | 3523 | 58 | `1ff360f1c5f7` |
| `app/src/test/java/com/briqt/moke/data/TmuxMemoryStoreTest.kt` | TEXT | 19157 | 318 | `96817d8da186` |
| `app/src/test/java/com/briqt/moke/data/TmuxRememberTargetTest.kt` | TEXT | 4739 | 101 | `bfcea644e2d5` |
| `app/src/test/java/com/briqt/moke/data/TmuxTargetStampTest.kt` | TEXT | 7164 | 96 | `73bad4d38577` |
| `app/src/test/java/com/briqt/moke/net/EncryptedStateFilesTest.kt` | TEXT | 2029 | 50 | `0409e38f93f3` |
| `app/src/test/java/com/briqt/moke/net/NetworkRecoveryTest.kt` | TEXT | 3051 | 60 | `33a642c105c5` |
| `app/src/test/java/com/briqt/moke/net/NetworkRuntimeTest.kt` | TEXT | 34116 | 747 | `ac1b12e562c6` |
| `app/src/test/java/com/briqt/moke/net/NetworkSocketTest.kt` | TEXT | 1309 | 38 | `c18ed755ecc4` |
| `app/src/test/java/com/briqt/moke/net/TailnetDevicesTest.kt` | TEXT | 5568 | 106 | `09647cb0ab38` |
| `app/src/test/java/com/briqt/moke/sync/BackupNameTest.kt` | TEXT | 2148 | 54 | `bfc838392223` |
| `app/src/test/java/com/briqt/moke/sync/RestorePlannerTest.kt` | TEXT | 15026 | 267 | `c4de7ffec852` |
| `app/src/test/java/com/briqt/moke/sync/SyncBundleTest.kt` | TEXT | 9564 | 192 | `e0fbd26b351d` |
| `app/src/test/java/com/briqt/moke/sync/SyncCryptoTest.kt` | TEXT | 2880 | 67 | `16c73c66f0c9` |
| `app/src/test/java/com/briqt/moke/sync/WebDavParseTest.kt` | TEXT | 4167 | 77 | `1ceffddff661` |
| `app/src/test/java/com/briqt/moke/terminal/BackgroundKeepaliveTest.kt` | TEXT | 17283 | 432 | `f2a53e5fef39` |
| `app/src/test/java/com/briqt/moke/terminal/BundledFontsScriptTest.kt` | TEXT | 3953 | 99 | `0795c2ec2368` |
| `app/src/test/java/com/briqt/moke/terminal/EncryptedPkcs8Test.kt` | TEXT | 4082 | 91 | `ba06fcec8751` |
| `app/src/test/java/com/briqt/moke/terminal/FontCatalogBundleTest.kt` | TEXT | 2629 | 66 | `76c96aa2a32b` |
| `app/src/test/java/com/briqt/moke/terminal/HostKeyFingerprintTest.kt` | TEXT | 3990 | 76 | `bcad3ee2abd2` |
| `app/src/test/java/com/briqt/moke/terminal/HostKeyPromptTest.kt` | TEXT | 4302 | 115 | `6deaf4dc58d8` |
| `app/src/test/java/com/briqt/moke/terminal/KeySeqTest.kt` | TEXT | 13932 | 315 | `cbb75b13db52` |
| `app/src/test/java/com/briqt/moke/terminal/ModifiersTest.kt` | TEXT | 4765 | 121 | `66bfff621e05` |
| `app/src/test/java/com/briqt/moke/terminal/MokeScrollTest.kt` | TEXT | 1282 | 35 | `3c18fc3eb862` |
| `app/src/test/java/com/briqt/moke/terminal/MoshBootstrapOperationTest.kt` | TEXT | 6279 | 137 | `05923fa7c109` |
| `app/src/test/java/com/briqt/moke/terminal/MoshBootstrapTest.kt` | TEXT | 2102 | 46 | `ffa86b849995` |
| `app/src/test/java/com/briqt/moke/terminal/MoshChildProcessTest.kt` | TEXT | 8453 | 189 | `d2f289e44004` |
| `app/src/test/java/com/briqt/moke/terminal/MoshOrphanGuardTest.kt` | TEXT | 9736 | 180 | `a9c47e02da82` |
| `app/src/test/java/com/briqt/moke/terminal/MoshPtyTest.kt` | TEXT | 613 | 23 | `7440e501e44a` |
| `app/src/test/java/com/briqt/moke/terminal/PortForwardsTest.kt` | TEXT | 10123 | 226 | `feea63228593` |
| `app/src/test/java/com/briqt/moke/terminal/SessionMetadataTest.kt` | TEXT | 9713 | 170 | `03311b2094c1` |
| `app/src/test/java/com/briqt/moke/terminal/SessionTitlePolicyTest.kt` | TEXT | 7286 | 104 | `3ab35813f649` |
| `app/src/test/java/com/briqt/moke/terminal/SessionTitleTest.kt` | TEXT | 2082 | 51 | `9070750a5d13` |
| `app/src/test/java/com/briqt/moke/terminal/SshConnectedTest.kt` | TEXT | 5165 | 107 | `1762bb579478` |
| `app/src/test/java/com/briqt/moke/terminal/SshKeyPasswordFinderTest.kt` | TEXT | 952 | 26 | `4142b033494f` |
| `app/src/test/java/com/briqt/moke/terminal/SshKeysTest.kt` | TEXT | 17980 | 368 | `51fe090a3953` |
| `app/src/test/java/com/briqt/moke/terminal/TailscaleSshAuthTest.kt` | TEXT | 18956 | 399 | `f6831d6db8a5` |
| `app/src/test/java/com/briqt/moke/terminal/TailscaleSshAuthTextTest.kt` | TEXT | 4104 | 72 | `f40623d16456` |
| `app/src/test/java/com/briqt/moke/terminal/TerminalInputQueueTest.kt` | TEXT | 12404 | 284 | `5801101a81ef` |
| `app/src/test/java/com/briqt/moke/terminal/TerminalLinksTest.kt` | TEXT | 7863 | 173 | `5e45bc3f2eb1` |
| `app/src/test/java/com/briqt/moke/terminal/TerminalSizeStateTest.kt` | TEXT | 10350 | 247 | `3c631ecf0672` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxAttachTest.kt` | TEXT | 4366 | 107 | `7eff2a0daa12` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxChoiceTicketsTest.kt` | TEXT | 7280 | 177 | `74ce29b9400e` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxControlOperationTest.kt` | TEXT | 10450 | 180 | `865a359d64cd` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxCoordinatorTest.kt` | TEXT | 87049 | 1700 | `adc273ab05f3` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxDarwinShellTest.kt` | TEXT | 18426 | 339 | `c6f5ee4406fa` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxDarwinTest.kt` | TEXT | 14225 | 232 | `65bedc4eac4e` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxFallbackShellTest.kt` | TEXT | 3327 | 71 | `94565c692ef6` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxLaunchPolicyTest.kt` | TEXT | 12944 | 200 | `bd39fe8f5770` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxLinuxOwnerTest.kt` | TEXT | 7509 | 129 | `b23ce0f23e70` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxRemoteSocketTest.kt` | TEXT | 15179 | 300 | `253d7b850f64` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxRemoteTest.kt` | TEXT | 48197 | 762 | `8e8416973c02` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxScrollSetupTest.kt` | TEXT | 23773 | 373 | `59fdf6630a03` |
| `app/src/test/java/com/briqt/moke/terminal/TmuxTest.kt` | TEXT | 5286 | 133 | `2100d0b1daf2` |
| `app/src/test/java/com/briqt/moke/terminal/TransportResourcesTest.kt` | TEXT | 8350 | 180 | `3f288878fd13` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/RemotePathTest.kt` | TEXT | 2713 | 76 | `f79aed78abbb` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/SftpAuthenticationRetryTest.kt` | TEXT | 707 | 19 | `1a821347d9a4` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/SftpStatTest.kt` | TEXT | 1631 | 39 | `795172cf7e9e` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/TransferManagerTest.kt` | TEXT | 10493 | 242 | `d5998ee65370` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/TransferResumeTest.kt` | TEXT | 3865 | 104 | `cd58dd4ab1fb` |
| `app/src/test/java/com/briqt/moke/terminal/sftp/UploadDestinationTest.kt` | TEXT | 3689 | 73 | `e6e8ab6c6a7b` |
| `app/src/test/java/com/briqt/moke/ui/AdaptiveLayoutTest.kt` | TEXT | 3974 | 108 | `75ff3b06935d` |
| `app/src/test/java/com/briqt/moke/ui/ExtraKeyGestureTest.kt` | TEXT | 5900 | 122 | `42ea67d4c947` |
| `app/src/test/java/com/briqt/moke/ui/ExtraKeyLayoutTest.kt` | TEXT | 33945 | 649 | `4bf6a4b0d4cd` |
| `app/src/test/java/com/briqt/moke/ui/FilesWorkTest.kt` | TEXT | 9401 | 196 | `c686082e2000` |
| `app/src/test/java/com/briqt/moke/ui/FormValidationTest.kt` | TEXT | 7839 | 139 | `1220c5d5d642` |
| `app/src/test/java/com/briqt/moke/ui/HostKeyTargetLabelTest.kt` | TEXT | 938 | 19 | `74c26e13d9ec` |
| `app/src/test/java/com/briqt/moke/ui/KeySectionsTest.kt` | TEXT | 5505 | 121 | `656aa19fdb48` |
| `app/src/test/java/com/briqt/moke/ui/NetworkLoginControllerTest.kt` | TEXT | 22117 | 467 | `4b69885277fc` |
| `app/src/test/java/com/briqt/moke/ui/NetworkPresentationTest.kt` | TEXT | 3530 | 72 | `931340e22f9f` |
| `app/src/test/java/com/briqt/moke/ui/SessionDialogSourceTest.kt` | TEXT | 6315 | 107 | `13180d02559a` |
| `app/src/test/java/com/briqt/moke/ui/SessionGroupingTest.kt` | TEXT | 2817 | 55 | `150fe18769ec` |
| `app/src/test/java/com/briqt/moke/ui/SessionTitleDraftTest.kt` | TEXT | 2643 | 51 | `e8196844fdbc` |
| `app/src/test/java/com/briqt/moke/ui/TailnetPhaseTest.kt` | TEXT | 3304 | 63 | `651db940f332` |
| `app/src/test/java/com/briqt/moke/ui/TerminalImeFallbackTest.kt` | TEXT | 6134 | 138 | `8a0271ea4b41` |
| `app/src/test/java/com/briqt/moke/ui/TerminalPathProbeTest.kt` | TEXT | 5920 | 99 | `44a875cf8186` |
| `app/src/test/java/com/briqt/moke/ui/TmuxNameDraftTest.kt` | TEXT | 2918 | 87 | `e271d1d2dff7` |
| `app/src/test/java/com/briqt/moke/ui/TmuxObservationRecoveryTest.kt` | TEXT | 1544 | 44 | `79c73c8ce9bd` |
| `app/src/test/java/com/briqt/moke/ui/TmuxPresentationTest.kt` | TEXT | 24524 | 428 | `42b62dd467b8` |
| `app/src/test/java/com/briqt/moke/ui/TmuxRefreshFeedbackTest.kt` | TEXT | 11862 | 243 | `b72bc942aaac` |
| `app/src/test/java/com/briqt/moke/ui/TmuxRowDropdownMenuTest.kt` | TEXT | 6083 | 98 | `b9d96931215a` |
| `app/src/test/java/com/briqt/moke/ui/TmuxRowMenuStateTest.kt` | TEXT | 4216 | 84 | `487ee2992c7e` |
| `app/src/test/java/com/briqt/moke/ui/TmuxSheetControllerTest.kt` | TEXT | 20311 | 439 | `da7df6f02bba` |
| `app/src/test/java/com/briqt/moke/ui/TmuxUiResourcesTest.kt` | TEXT | 6887 | 119 | `660fd6356183` |
| `app/src/test/java/com/briqt/moke/update/PrereleasePickTest.kt` | TEXT | 2538 | 68 | `3beb27435ba0` |
| `app/src/test/java/com/briqt/moke/update/UpdateCheckerTest.kt` | TEXT | 1340 | 39 | `56db32f9a799` |
| `build.gradle.kts` | TEXT | 500 | 14 | `84ebbfffbcf5` |
| `docs/screenshot-appearance.png` | BINARY | 134914 | 567 | `287164e14357` |
| `docs/screenshot-connections.png` | BINARY | 99446 | 488 | `3c91b449a829` |
| `docs/screenshot-files.png` | BINARY | 76687 | 407 | `ab187b346cd5` |
| `docs/screenshot-terminal.png` | BINARY | 184115 | 550 | `23047f1fda8a` |
| `docs/screenshot-tmux.png` | BINARY | 96673 | 362 | `4228dfe6e2cf` |
| `gradle.properties` | TEXT | 224 | 9 | `d77c3c221357` |
| `gradle/libs.versions.toml` | TEXT | 3147 | 51 | `cc08a210a045` |
| `gradle/wrapper/gradle-wrapper.jar` | BINARY | 47505 | 164 | `7a9ce74cff46` |
| `gradle/wrapper/gradle-wrapper.properties` | TEXT | 368 | 10 | `d0bebe8133ba` |
| `gradlew` | TEXT | 8656 | 248 | `a5a5c199ba02` |
| `gradlew.bat` | TEXT | 2848 | 82 | `d539676c48b5` |
| `netbridge/bridge.go` | TEXT | 20970 | 749 | `53304eec6aa0` |
| `netbridge/bridge_test.go` | TEXT | 30538 | 971 | `938030a7078e` |
| `netbridge/config.go` | TEXT | 3656 | 113 | `abc43dfa1bf8` |
| `netbridge/defaultroute_android.go` | TEXT | 324 | 9 | `8d67d0dedc28` |
| `netbridge/defaultroute_other.go` | TEXT | 150 | 6 | `b3487b36c108` |
| `netbridge/go.mod` | TEXT | 2750 | 62 | `b367e695fab3` |
| `netbridge/go.sum` | TEXT | 23569 | 247 | `5e1e5ca6040e` |
| `netbridge/internal/codes/codes.go` | TEXT | 3912 | 131 | `3d7a51affec4` |
| `netbridge/internal/codes/codes_test.go` | TEXT | 1739 | 63 | `7f12055ce77c` |
| `netbridge/internal/netstate/snapshot.go` | TEXT | 4463 | 160 | `dccea1c36e2e` |
| `netbridge/internal/netstate/snapshot_test.go` | TEXT | 2774 | 86 | `f2af18b712a4` |
| `netbridge/internal/proxynet/proxynet.go` | TEXT | 20911 | 673 | `36fd8dbd631e` |
| `netbridge/internal/proxynet/proxynet_test.go` | TEXT | 27006 | 855 | `8955c699878e` |
| `netbridge/internal/relay/moshlab_test.go` | TEXT | 5296 | 159 | `87e05846fb97` |
| `netbridge/internal/relay/relay.go` | TEXT | 20237 | 691 | `c9f97df0da82` |
| `netbridge/internal/relay/relay_test.go` | TEXT | 31786 | 872 | `d20673225e94` |
| `netbridge/internal/socks/socks.go` | TEXT | 6615 | 272 | `38c9c09634bd` |
| `netbridge/internal/socks/socks_test.go` | TEXT | 5156 | 188 | `fb18ab766f52` |
| `netbridge/internal/tailnet/node.go` | TEXT | 9474 | 337 | `07e6ae7c7e49` |
| `netbridge/internal/tailnet/peers.go` | TEXT | 3462 | 117 | `d89f6c0d9b82` |
| `netbridge/internal/tailnet/peers_read_test.go` | TEXT | 2104 | 69 | `ff11cc8dccbe` |
| `netbridge/internal/tailnet/peers_test.go` | TEXT | 9984 | 234 | `715d664ed568` |
| `netbridge/internal/tailnet/tailnet_test.go` | TEXT | 8283 | 252 | `308a6100921d` |
| `netbridge/internal/tailnet/target.go` | TEXT | 3012 | 90 | `a4275aec4e46` |
| `netbridge/netbridge.go` | TEXT | 8120 | 228 | `577bc902c911` |
| `netbridge/tailnetlab_test.go` | TEXT | 5991 | 193 | `946ca0505aff` |
| `scripts/build-mosh-native.sh` | TEXT | 4101 | 86 | `c5f45e5bfeda` |
| `scripts/build-netbridge.sh` | TEXT | 3313 | 69 | `f12204dbf5fe` |
| `scripts/test-verify-netbridge.sh` | TEXT | 7440 | 154 | `2dad70b69aa1` |
| `scripts/verify-bundled-fonts.sh` | TEXT | 1941 | 55 | `ea06dedad0e7` |
| `scripts/verify-netbridge.sh` | TEXT | 3200 | 73 | `639ac4e9bd6f` |
| `settings.gradle.kts` | TEXT | 586 | 27 | `f3e80ee588bd` |
| `terminal-emulator/README.md` | TEXT | 1237 | 12 | `f29a32f92664` |
| `terminal-emulator/build.gradle.kts` | TEXT | 690 | 29 | `2a0dc1ccb7cd` |
| `terminal-emulator/proguard-rules.pro` | TEXT | 933 | 25 | `3f0600b371d4` |
| `terminal-emulator/src/main/AndroidManifest.xml` | TEXT | 23 | 2 | `21058c692532` |
| `terminal-emulator/src/main/java/com/termux/terminal/ByteQueue.java` | TEXT | 3804 | 108 | `a331e76e385e` |
| `terminal-emulator/src/main/java/com/termux/terminal/JNI.java` | TEXT | 1561 | 36 | `84c0fc3b9f16` |
| `terminal-emulator/src/main/java/com/termux/terminal/KeyHandler.java` | TEXT | 18070 | 373 | `78688caed123` |
| `terminal-emulator/src/main/java/com/termux/terminal/Logger.java` | TEXT | 2477 | 80 | `7ab53c7ee035` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalBuffer.java` | TEXT | 24781 | 508 | `21707f10763b` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalColorScheme.java` | TEXT | 7044 | 126 | `a303825f7992` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalColors.java` | TEXT | 3870 | 96 | `27cc579af682` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalEmulator.java` | TEXT | 141356 | 2814 | `a9a25b4c2347` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalOutput.java` | TEXT | 1621 | 43 | `37f029110111` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalRow.java` | TEXT | 14062 | 283 | `280c12d7f6e7` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalScrollToken.java` | TEXT | 614 | 22 | `de3ebfd7351d` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalSession.java` | TEXT | 16363 | 366 | `493789a32531` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalSessionClient.java` | TEXT | 1791 | 59 | `3aeae2d93598` |
| `terminal-emulator/src/main/java/com/termux/terminal/TerminalTransport.java` | TEXT | 2911 | 56 | `fe924190e2de` |
| `terminal-emulator/src/main/java/com/termux/terminal/TextStyle.java` | TEXT | 3623 | 90 | `234faa9646ac` |
| `terminal-emulator/src/main/java/com/termux/terminal/WcWidth.java` | TEXT | 39885 | 566 | `68fc84a28813` |
| `terminal-emulator/src/main/jni/Android.mk` | TEXT | 134 | 5 | `118312b11e9d` |
| `terminal-emulator/src/main/jni/termux.c` | TEXT | 7918 | 227 | `34ae211d9d8e` |
| `terminal-emulator/src/test/java/com/termux/terminal/ApcTest.java` | TEXT | 1059 | 21 | `7eb7731e2d93` |
| `terminal-emulator/src/test/java/com/termux/terminal/ByteQueueTest.java` | TEXT | 1616 | 54 | `86673645a832` |
| `terminal-emulator/src/test/java/com/termux/terminal/ControlSequenceIntroducerTest.java` | TEXT | 6447 | 131 | `95ec55f2657b` |
| `terminal-emulator/src/test/java/com/termux/terminal/CursorAndScreenTest.java` | TEXT | 13697 | 266 | `e554a7a24ab5` |
| `terminal-emulator/src/test/java/com/termux/terminal/DecSetTest.java` | TEXT | 10704 | 215 | `bd7b30a7ba24` |
| `terminal-emulator/src/test/java/com/termux/terminal/DeviceControlStringTest.java` | TEXT | 2128 | 53 | `73f1653c3a87` |
| `terminal-emulator/src/test/java/com/termux/terminal/HistoryTest.java` | TEXT | 3659 | 85 | `8b1ad204a9b4` |
| `terminal-emulator/src/test/java/com/termux/terminal/KeyHandlerTest.java` | TEXT | 13381 | 203 | `666a1930f99e` |
| `terminal-emulator/src/test/java/com/termux/terminal/MokeIgnoreAltScreenTest.java` | TEXT | 2219 | 51 | `869e95b44396` |
| `terminal-emulator/src/test/java/com/termux/terminal/NotificationTest.java` | TEXT | 1495 | 34 | `2340a96d03fb` |
| `terminal-emulator/src/test/java/com/termux/terminal/OperatingSystemControlTest.java` | TEXT | 25085 | 593 | `e2358eb09a3a` |
| `terminal-emulator/src/test/java/com/termux/terminal/RectangularAreasTest.java` | TEXT | 7657 | 117 | `ddc73cd7fe9f` |
| `terminal-emulator/src/test/java/com/termux/terminal/ResizeTest.java` | TEXT | 15893 | 365 | `b0a9ca66ba30` |
| `terminal-emulator/src/test/java/com/termux/terminal/ScreenBufferTest.java` | TEXT | 2916 | 65 | `07bd3e39c7f9` |
| `terminal-emulator/src/test/java/com/termux/terminal/ScrollRegionTest.java` | TEXT | 7815 | 167 | `baa55a3896e8` |
| `terminal-emulator/src/test/java/com/termux/terminal/TerminalRowTest.java` | TEXT | 16836 | 432 | `8ed2ad25ae86` |
| `terminal-emulator/src/test/java/com/termux/terminal/TerminalScrollTokenTest.java` | TEXT | 2876 | 70 | `008d7ea7f61a` |
| `terminal-emulator/src/test/java/com/termux/terminal/TerminalTest.java` | TEXT | 14706 | 350 | `5ca659957f3d` |
| `terminal-emulator/src/test/java/com/termux/terminal/TerminalTestCase.java` | TEXT | 11590 | 334 | `5e7ea26480e3` |
| `terminal-emulator/src/test/java/com/termux/terminal/TextStyleTest.java` | TEXT | 2539 | 65 | `fc015cea35d7` |
| `terminal-emulator/src/test/java/com/termux/terminal/UnicodeInputTest.java` | TEXT | 6428 | 136 | `1db281112511` |
| `terminal-emulator/src/test/java/com/termux/terminal/WcWidthTest.java` | TEXT | 2216 | 81 | `d3be57d984a8` |
| `terminal-view/README.md` | TEXT | 1394 | 19 | `fda1e3a9d4f6` |
| `terminal-view/build.gradle.kts` | TEXT | 594 | 25 | `b2c4b7656177` |
| `terminal-view/proguard-rules.pro` | TEXT | 933 | 25 | `3f0600b371d4` |
| `terminal-view/src/main/AndroidManifest.xml` | TEXT | 23 | 2 | `21058c692532` |
| `terminal-view/src/main/java/com/termux/view/CellGlyph.java` | TEXT | 15784 | 321 | `cfd663f3f922` |
| `terminal-view/src/main/java/com/termux/view/GestureAndScaleRecognizer.java` | TEXT | 6669 | 149 | `1d1630d38ca2` |
| `terminal-view/src/main/java/com/termux/view/GlyphLayout.java` | TEXT | 1658 | 36 | `1eee9419c01d` |
| `terminal-view/src/main/java/com/termux/view/MokeScroll.java` | TEXT | 1072 | 27 | `26f5f72d3f2c` |
| `terminal-view/src/main/java/com/termux/view/ScrollMotion.java` | TEXT | 3180 | 70 | `6cd3319c88fb` |
| `terminal-view/src/main/java/com/termux/view/ScrollViewport.java` | TEXT | 3120 | 77 | `fa4f8590794d` |
| `terminal-view/src/main/java/com/termux/view/TerminalGraphics.java` | TEXT | 3562 | 88 | `3bf8a27fbe9e` |
| `terminal-view/src/main/java/com/termux/view/TerminalInputConnection.java` | TEXT | 4876 | 118 | `06db15d8761d` |
| `terminal-view/src/main/java/com/termux/view/TerminalRenderer.java` | TEXT | 19470 | 377 | `15865106dceb` |
| `terminal-view/src/main/java/com/termux/view/TerminalView.java` | TEXT | 85473 | 1848 | `e92666091c17` |
| `terminal-view/src/main/java/com/termux/view/TerminalViewClient.java` | TEXT | 2150 | 86 | `04bd10a54166` |
| `terminal-view/src/main/java/com/termux/view/support/PopupWindowCompatGingerbread.java` | TEXT | 2870 | 75 | `00c60e0045aa` |
| `terminal-view/src/main/java/com/termux/view/textselection/CursorController.java` | TEXT | 1471 | 55 | `6a7331f37588` |
| `terminal-view/src/main/java/com/termux/view/textselection/TextSelectionCursorController.java` | TEXT | 14130 | 411 | `9f3462a9fa9c` |
| `terminal-view/src/main/java/com/termux/view/textselection/TextSelectionHandleView.java` | TEXT | 11688 | 353 | `9c020d136f18` |
| `terminal-view/src/main/res/drawable/text_select_handle_left_material.xml` | TEXT | 524 | 10 | `3ce66a831646` |
| `terminal-view/src/main/res/drawable/text_select_handle_right_material.xml` | TEXT | 498 | 10 | `977579c11e9c` |
| `terminal-view/src/main/res/values/strings.xml` | TEXT | 163 | 5 | `c2880e9b490e` |
| `terminal-view/src/test/java/com/termux/view/CellGlyphTest.java` | TEXT | 32760 | 641 | `d1db6e70eee0` |
| `terminal-view/src/test/java/com/termux/view/GlyphLayoutTest.java` | TEXT | 3506 | 77 | `315ec06e9ced` |
| `terminal-view/src/test/java/com/termux/view/MokeScrollPolicyTest.java` | TEXT | 1594 | 34 | `95263cc27603` |
| `terminal-view/src/test/java/com/termux/view/ScrollMotionTest.java` | TEXT | 3074 | 74 | `e12f2b929839` |
| `terminal-view/src/test/java/com/termux/view/ScrollViewportTest.java` | TEXT | 3643 | 92 | `c7f6c7fbbe40` |

## 复算命令

```sh
git clone --depth 1 https://github.com/briqt/moke /tmp/opencode/external/moke
git -C /tmp/opencode/external/moke ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/moke ls-files | wc -l
```
