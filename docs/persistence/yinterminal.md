# YinTerminal 深度审查

- 来源：https://github.com/ditzabcd/YinTerminal
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：轻量 Android shell 客户端，持久化 root/user shell、ANSI 着色、内嵌 Python、脚本与快捷工具；不是完整的 VT 终端。

### 分层与机制

- Kotlin 层：`TerminalEmulator` 用正则分段处理 ANSI 颜色（16/256/RGB）并把结果写成 `AnnotatedString`；输出按 `stdout`/`stderr` 分流，系统 emoji 由 Android 绘制。
- `CommandInterpreter` 解析内置命令（哈希、Base64/Hex/URL、JSON、正则、系统信息，甚至 Morse 输出）；命令与 shell 输出混在同一文本流中。
- `LocalPty`/`NativePty` 维持单个 `su` 或 `/system/bin/sh`，使 `cd` 与环境变量跨命令保留；崩溃时把状态写到隐藏文件供下次恢复。
- Chaquopy 提供离线 Python 3.13；脚本、历史与快命令存为本地 JSON 文件；GPL-3.0 许可。

### 验证与风险

- 只有 2 个测试文件，没有网格状态机、光标寻址、交替缓冲区、滚动区或鼠标协议的可靠实现；正则解析 ANSI 容易在转义序列分片、嵌套 SGR 与 OSC 上出错。
- 隐藏崩溃状态文件和持久化 root shell 都是隐私/安全敏感点；自动请求 `su` 与本地明文 JSON 存储不符合本仓最小暴露原则。
- GPL-3.0 与本地依赖策略不相容，代码不能直接复制。

### 本地取舍

仅作为“不要这样做”的反例：ANSI 正则不能替代 VT 解析器；内置命令解释器与持久 shell 不应进入终端核心。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/output_processor.rs` | 对照真实 VT 分派与正则 ANSI 的根本差异 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | 对照输入编码不依赖内置命令解释器 |
| `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt` | 对照 UTF-8 提交与不可信长度钳位 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.github/workflows/release.yml` | TEXT | 5220 | 162 | `79788b17c4f9` |
| `.gitignore` | TEXT | 475 | 41 | `953d3d63b245` |
| `LICENSE` | DECLARED_EXCLUSION | 32249 | 616 | `2498488dc85d` |
| `README.md` | TEXT | 1457 | 29 | `b8b356355210` |
| `app/.gitignore` | TEXT | 6 | 1 | `5c3174b74edc` |
| `app/build.gradle.kts` | TEXT | 2662 | 94 | `00a1d49441f0` |
| `app/proguard-rules.pro` | TEXT | 380 | 12 | `82c0255c1cef` |
| `app/src/androidTest/java/yin/term/ExampleInstrumentedTest.kt` | TEXT | 476 | 16 | `0375a3f343ca` |
| `app/src/main/AndroidManifest.xml` | TEXT | 1334 | 35 | `208b284a82d3` |
| `app/src/main/CMakeLists.txt` | TEXT | 259 | 21 | `55c34fa43578` |
| `app/src/main/cpp/CMakeLists.txt` | TEXT | 260 | 21 | `608733603f2f` |
| `app/src/main/cpp/yinterm_pty.cpp` | TEXT | 8635 | 322 | `1161e177d67f` |
| `app/src/main/ic_launcher-playstore.png` | BINARY | 17658 | 97 | `4afda3e6030e` |
| `app/src/main/java/yin/term/MainActivity.kt` | TEXT | 631 | 22 | `29822ad1aa1c` |
| `app/src/main/java/yin/term/YinTermApp.kt` | TEXT | 602 | 26 | `e587eae4218f` |
| `app/src/main/java/yin/term/core/AnsiParser.kt` | TEXT | 11208 | 305 | `ef5634ba89f0` |
| `app/src/main/java/yin/term/core/AppDispatchers.kt` | TEXT | 298 | 10 | `759ba7cc82ae` |
| `app/src/main/java/yin/term/core/BaseTopBar.kt` | TEXT | 4442 | 121 | `f94b25af28c4` |
| `app/src/main/java/yin/term/core/YinIcons.kt` | TEXT | 11477 | 330 | `bc54cbac3fbd` |
| `app/src/main/java/yin/term/data/datasource/NativePty.kt` | TEXT | 1609 | 60 | `275710764c54` |
| `app/src/main/java/yin/term/data/datasource/PythonManager.kt` | TEXT | 76 | 3 | `b2f5cb48ed0f` |
| `app/src/main/java/yin/term/data/datasource/ShellManager.kt` | TEXT | 26287 | 563 | `7bad3962b003` |
| `app/src/main/java/yin/term/data/datasource/TerminalSession.kt` | TEXT | 26534 | 760 | `195f718a7f8a` |
| `app/src/main/java/yin/term/data/local/LocalEntities.kt` | TEXT | 606 | 30 | `9cfb107d2fd8` |
| `app/src/main/java/yin/term/data/local/YinLocalStorage.kt` | TEXT | 9933 | 292 | `957c74ae6232` |
| `app/src/main/java/yin/term/data/mapper/DbMapper.kt` | TEXT | 2446 | 93 | `153a80100979` |
| `app/src/main/java/yin/term/data/repository/CommandExecutorImpl.kt` | TEXT | 657 | 24 | `efb94a92a317` |
| `app/src/main/java/yin/term/data/repository/HistoryRepositoryImpl.kt` | TEXT | 1237 | 36 | `1212c7d8f975` |
| `app/src/main/java/yin/term/data/repository/ScriptRepositoryImpl.kt` | TEXT | 947 | 28 | `7799764b24c3` |
| `app/src/main/java/yin/term/data/repository/SettingsRepositoryImpl.kt` | TEXT | 2121 | 53 | `72251ff94697` |
| `app/src/main/java/yin/term/data/repository/TerminalLogRepositoryImpl.kt` | TEXT | 747 | 25 | `d09f2fc3e5ec` |
| `app/src/main/java/yin/term/di/AppContainer.kt` | TEXT | 1817 | 54 | `ecf6e9cc9856` |
| `app/src/main/java/yin/term/domain/model/History.kt` | TEXT | 101 | 6 | `2234dd39eac6` |
| `app/src/main/java/yin/term/domain/model/Script.kt` | TEXT | 122 | 7 | `afb9c870650a` |
| `app/src/main/java/yin/term/domain/model/Settings.kt` | TEXT | 257 | 10 | `f33959ba5646` |
| `app/src/main/java/yin/term/domain/model/TerminalLog.kt` | TEXT | 162 | 8 | `fceeef02feb5` |
| `app/src/main/java/yin/term/domain/model/TerminalState.kt` | TEXT | 119 | 9 | `576d52000d91` |
| `app/src/main/java/yin/term/domain/repository/CommandExecutor.kt` | TEXT | 152 | 6 | `09fc8073962c` |
| `app/src/main/java/yin/term/domain/repository/HistoryRepository.kt` | TEXT | 373 | 17 | `2c1c931894ec` |
| `app/src/main/java/yin/term/domain/repository/ScriptRepository.kt` | TEXT | 278 | 13 | `a83bec426561` |
| `app/src/main/java/yin/term/domain/repository/SettingsRepository.kt` | TEXT | 467 | 19 | `17087930dcec` |
| `app/src/main/java/yin/term/domain/repository/TerminalLogRepository.kt` | TEXT | 272 | 13 | `5327f7c4fd79` |
| `app/src/main/java/yin/term/ui/history/HistoryUiEffect.kt` | TEXT | 169 | 5 | `84d89de2a300` |
| `app/src/main/java/yin/term/ui/history/HistoryUiEvent.kt` | TEXT | 284 | 9 | `bb2a1222966c` |
| `app/src/main/java/yin/term/ui/history/HistoryUiState.kt` | TEXT | 364 | 13 | `f3dc9913d0a1` |
| `app/src/main/java/yin/term/ui/history/HistoryView.kt` | TEXT | 12060 | 308 | `b83cd2a9802d` |
| `app/src/main/java/yin/term/ui/history/HistoryViewModel.kt` | TEXT | 3280 | 90 | `eb2da8629bcd` |
| `app/src/main/java/yin/term/ui/main/MainView.kt` | TEXT | 3075 | 83 | `bb70a2f1155c` |
| `app/src/main/java/yin/term/ui/navigation/AppDestination.kt` | TEXT | 333 | 13 | `50e5ec99e6f2` |
| `app/src/main/java/yin/term/ui/navigation/AppNavigation.kt` | TEXT | 1768 | 48 | `41e131f7922c` |
| `app/src/main/java/yin/term/ui/navigation/AppViewModel.kt` | TEXT | 567 | 18 | `f8047c299b5f` |
| `app/src/main/java/yin/term/ui/script/ScriptUiEffect.kt` | TEXT | 166 | 5 | `ff59c972d627` |
| `app/src/main/java/yin/term/ui/script/ScriptUiEvent.kt` | TEXT | 579 | 14 | `3c0c90a5eaa7` |
| `app/src/main/java/yin/term/ui/script/ScriptUiState.kt` | TEXT | 785 | 21 | `a6858c85ae53` |
| `app/src/main/java/yin/term/ui/script/ScriptView.kt` | TEXT | 17457 | 435 | `a6d52e6157ed` |
| `app/src/main/java/yin/term/ui/script/ScriptViewModel.kt` | TEXT | 5556 | 159 | `69fbe38d391e` |
| `app/src/main/java/yin/term/ui/terminal/TerminalThemeStore.kt` | TEXT | 5452 | 154 | `214124185c8d` |
| `app/src/main/java/yin/term/ui/terminal/TerminalUiEvent.kt` | TEXT | 388 | 10 | `ba4673ff9dca` |
| `app/src/main/java/yin/term/ui/terminal/TerminalUiState.kt` | TEXT | 615 | 18 | `299f3a166bb1` |
| `app/src/main/java/yin/term/ui/terminal/TerminalView.kt` | TEXT | 48764 | 1276 | `c4452cfeb150` |
| `app/src/main/java/yin/term/ui/terminal/TerminalViewModel.kt` | TEXT | 8617 | 232 | `8c21380e4e62` |
| `app/src/main/java/yin/term/ui/theme/Color.kt` | TEXT | 567 | 19 | `0d333f6ab1c4` |
| `app/src/main/java/yin/term/ui/theme/Theme.kt` | TEXT | 1028 | 37 | `e3bef42998a0` |
| `app/src/main/java/yin/term/ui/theme/Type.kt` | TEXT | 1230 | 39 | `e4d2e32b0b95` |
| `app/src/main/res/drawable/ic_github.xml` | TEXT | 1199 | 5 | `5a6235aeeb5f` |
| `app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 1365 | 31 | `ba2f1f6fd635` |
| `app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml` | TEXT | 267 | 5 | `daf8d08484dc` |
| `app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml` | TEXT | 267 | 5 | `daf8d08484dc` |
| `app/src/main/res/mipmap-hdpi/ic_launcher.webp` | BINARY | 2842 | 7 | `0b9d33aab1fc` |
| `app/src/main/res/mipmap-hdpi/ic_launcher_round.webp` | BINARY | 2842 | 7 | `0b9d33aab1fc` |
| `app/src/main/res/mipmap-mdpi/ic_launcher.webp` | BINARY | 1842 | 10 | `e1a8da7e8573` |
| `app/src/main/res/mipmap-mdpi/ic_launcher_round.webp` | BINARY | 1842 | 10 | `e1a8da7e8573` |
| `app/src/main/res/mipmap-xhdpi/ic_launcher.webp` | BINARY | 3870 | 9 | `411cc21f7da0` |
| `app/src/main/res/mipmap-xhdpi/ic_launcher_round.webp` | BINARY | 3870 | 9 | `411cc21f7da0` |
| `app/src/main/res/mipmap-xxhdpi/ic_launcher.webp` | BINARY | 5976 | 30 | `4f1f58ec3722` |
| `app/src/main/res/mipmap-xxhdpi/ic_launcher_round.webp` | BINARY | 5976 | 30 | `4f1f58ec3722` |
| `app/src/main/res/mipmap-xxxhdpi/ic_launcher.webp` | BINARY | 8462 | 40 | `5a183ff84f7f` |
| `app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.webp` | BINARY | 8462 | 40 | `5a183ff84f7f` |
| `app/src/main/res/values/colors.xml` | TEXT | 378 | 10 | `b5626527d503` |
| `app/src/main/res/values/ic_launcher_background.xml` | TEXT | 120 | 4 | `cd40bafe618f` |
| `app/src/main/res/values/strings.xml` | TEXT | 69 | 3 | `cc1f4947de50` |
| `app/src/main/res/values/themes.xml` | TEXT | 457 | 11 | `7732a7956fed` |
| `app/src/test/java/yin/term/ExampleUnitTest.kt` | TEXT | 185 | 11 | `9ae0da403c31` |
| `build.gradle.kts` | TEXT | 217 | 5 | `9c608c78c340` |
| `gradle.properties` | TEXT | 1383 | 24 | `33dc6eb1b6c5` |
| `gradle/gradle-daemon-jvm.properties` | TEXT | 1164 | 13 | `0e23cebe2536` |
| `gradle/libs.versions.toml` | TEXT | 1743 | 29 | `e276dd259a27` |
| `gradle/wrapper/gradle-wrapper.jar` | BINARY | 45457 | 160 | `76805e32c009` |
| `gradle/wrapper/gradle-wrapper.properties` | TEXT | 250 | 7 | `3ab1c9f05b7c` |
| `gradlew` | TEXT | 8728 | 251 | `3238afb2aed5` |
| `gradlew.bat` | TEXT | 2843 | 94 | `94102713eb8f` |
| `settings.gradle.kts` | TEXT | 571 | 25 | `29afcd5c0b4c` |

## 复算命令

```sh
git clone --depth 1 https://github.com/ditzabcd/YinTerminal /tmp/opencode/external/YinTerminal
git -C /tmp/opencode/external/YinTerminal ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/YinTerminal ls-files | wc -l
```
