# 规范深度审查报告

审查日期：2026-09-26。审查对象：`docs/specification/` 全部 6 份文档（`BUILD.md` `DESIGN.md` `PROHIBITED.md` `REFERENCE.md` `STYLE.md` `TESTING.md`）与 `AGENTS.md`，基准为 `main` @ `50386a8`。

> 后续阶段：把 `REFERENCE.md` 引用的 10 个参考项目全部克隆、把依赖源码与上游 ghostty 全部落地后重做的外部事实核验，见 [REFERENCE-REVIEW.md](REFERENCE-REVIEW.md)（31 项）。两份报告合并共 72 项。

审查方式：逐条提取文档中可验证的断言，回到 `native/src/`（Rust，30359 行）与 `android/app/src/`（Kotlin，38367 行）取证；辅以 `grep`/`glob` 全量扫描与 `.semgrep/` 规则覆盖度分析。文档中不可验证的描述（参考项目行为、外部链接）标注为「不可验证」，不计入问题数。

问题总计 **41 项**：A 类（代码违反规范）**22 项**、B 类（规范文档自身缺陷）**19 项**。另有 C 类门禁既有失败 2 项（审查前已存在，非本次引入）。

严重度定义：P0 违反明令禁止项；P1 违反明文要求且影响功能正确性或门禁可信度；P2 违反风格/一致性要求；P3 文档表述缺陷。

## 结论摘要

| 维度 | P0 | P1 | P2 | P3 | 小计 |
| --- | --- | --- | --- | --- | --- |
| A 代码违反规范 | 4 | 13 | 5 | — | 22 |
| B 规范文档缺陷 | 0 | 4 | 8 | 7 | 19 |

最严重的三项：

1. `PROHIBITED.md` 禁止的「无障碍朗读」被完整实现并上线，且四份规范文档零授权。
2. `DESIGN.md` 声明「不写入文件，不保存日志」，实际有四类日志落盘。
3. `TESTING.md` 要求「使用 release apk 进行测试」，实际全部 androidTest 跑在 debug 包上。

## A 类：代码违反规范

### A-P0

**A1. 无障碍朗读被完整实现，违反 `PROHIBITED.md:21`**

证据链：

- `android/app/src/main/java/terminal/emulator/ui/TerminalAccessibility.kt`（6.1K）：`AccessibilityLineProvider`、`AccessibilityLineNavigator`（上一行/下一行）、`DebouncedTextUpdater`、`contentDescription(...)`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt:2177-2178`（`contentDescription` + `IMPORTANT_FOR_ACCESSIBILITY_YES`）、`:2274-2333`（每帧经 JNI 拉取回滚区并改写 `contentDescription`）、`:2356-2375`（注册 3 个 `AccessibilityAction`）
- `android/app/src/main/res/values/strings.xml:109-111`：`accessibility_next_line`（下一行）、`accessibility_previous_line`（上一行）、`accessibility_read_screen`（**朗读屏幕**）
- 测试：`android/app/src/test/java/terminal/emulator/ui/TerminalAccessibilityTest.kt`
- 引入提交：`f6d2fab feat(ux): shortcuts, OSC 9;4, theme editor, audit, font synthesis`
- 授权检索：`grep -rl 'accessib\|talkback\|无障碍\|朗读' docs/specification/` 仅命中 `PROHIBITED.md` 自身；`openspec/` 零命中

`PROHIBITED.md:3` 规定「如果存在相关代码或文档需要彻底清理，不得存在任何相关代码」，因此这是必须整体删除的项，不是可裁剪项。

**A2. 四类日志落盘，违反 `DESIGN.md:82`「不写入文件，不保存日志」**

| 文件 | 位置 | 产物 |
| --- | --- | --- |
| `android/app/src/main/java/terminal/emulator/TerminalApp.kt` | `:143` | `crash_<ts>.log` |
| `android/app/src/main/java/terminal/emulator/monitor/BootGuard.kt` | `:95` | `fatal_<ts>.log` |
| `android/app/src/main/java/terminal/emulator/monitor/AnrWatchDog.kt` | `:143` | `anr_<ts>.log` |
| `android/app/src/main/java/terminal/emulator/monitor/ThermalMonitor.kt` | `:88` | `thermal_<ts>.log` |

均写入 `getDir("logs")`，且被「清除应用数据」一并删除（`TerminalViewModel.kt:972`）。`DESIGN.md:82` 同时要求「日志必须在 logcat 中可见」，即 logcat 即可，落盘无授权。

**A3. 全部 androidTest 跑在 debug 包，违反 `TESTING.md:22`「使用 release apk 进行测试，不得使用 debug apk」**

- `scripts/test-emulator.nu:8`：`^./gradlew ":app:connectedDebugAndroidTest"`
- `scripts/test-emulator.nu:11`：`^./gradlew ":app:installRelease"` 只为 `:benchmark:connectedReleaseAndroidTest`（`:13-15`）服务
- `.github/workflows/build.yml:45-52` 先构建 debug，`:69` 再跑 `test-emulator.nu`，故 app 侧始终是 debug
- 全仓无 `:app:connectedReleaseAndroidTest`

后果：`android/app/build.gradle.kts:63-64` 的 `isMinifyEnabled = true` / `isShrinkResources = true` 从未在设备测试中生效，`TESTING.md:26`、`:29-39` 共 11 条设备级验收项的可信度全部受损。

**A4. 测试代码探测 Rust 产物存在性，违反 `TESTING.md:12`；同时在测试源码内出现构建命令，违反 `BUILD.md:24`**

`android/app/src/test/java/terminal/emulator/bridge/NativeBridgeSmokeTest.kt`：

- `:40-46` 构造候选路径列表（含 `System.getenv("TERMINAL_NATIVE_LIB")`）
- `:57` `soCandidates.firstOrNull { it.isFile }` — 显式探测产物是否存在
- `:14` `:20` `:36` `:61` 出现 `cargo build --package native` / `cargo test`

`.semgrep/kotlin-deny-patterns.yml:109` 的 `no-unmanaged-build-exec` 只匹配 `ProcessBuilder`/`Runtime.exec`/`commandLine`，注释与 KDoc 不在覆盖范围内，故该违规不会被 `scripts/check-gradle.nu:4` 拦下。

### A-P1

**A5. Bootstrap 预设与规范不符，违反 `DESIGN.md:129`**

- 规范要求两项：`apt-android-7`（较大值）+ `2026.02.12-r1`（最新值），且 `DESIGN.md:14` 规定「有且只有」
- 实际仅一项：`android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt:1023` → `bootstrap-2026.06.21-r1%2Bapt.android-7`
- `2026.02.12-r1` 在全仓不存在
- `.semgrep/rust-deny-patterns.yml:95-99` 的 `no-deprecated-bootstrap-values` 只禁 `apt-android-5|2022.04.28`，是下限而非规范值，规则与 `DESIGN.md:129` 不同步

**A6. 线程模型与规范不符，违反 `DESIGN.md:62`**

规范：「每个终端会话独占一个线程，产出扁平化的单元格数组；共享渲染线程消费这些数组并驱动 wgpu」。

实际：每会话 **2 个** Rust 线程（`native/src/terminal/session.rs:307` reader、`:376` waiter）**加 1 个** Kotlin 渲染线程（`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt:1269`，`startRenderThread` 在 `:1207`），切换会话时停旧起新（`:1185`、`:2890-2931`）。不存在共享渲染线程。

**A7. 七项用户可见功能与组件未在任何规范中声明，违反 `DESIGN.md:14`「不可有未声明行为」与 `DESIGN.md:16`「不允许实现任何未在 docs/specification/ 声明的功能/逻辑」**

逐项对 `docs/specification/` 全量检索，均为零命中：

| 功能 | 代码位置 | 连带问题 |
| --- | --- | --- |
| Failsafe 会话 | `MainActivity.kt:54-58`（`EXTRA_FAILSAFE_SESSION`）、`res/xml/shortcuts.xml:13-24`、`TerminalRuntime.kt:421`、`:970-981` | `prefix = ""` 使 `TMPDIR` 变为 `/data/local/tmp`（`native/src/terminal/pty.rs:23`、`:886`），违反 `DESIGN.md:136` |
| 导出 DocumentsProvider | `AndroidManifest.xml:61-71`（`TerminalDocumentsProvider` + `MANAGE_DOCUMENTS`） | — |
| 导出 FileProvider | `AndroidManifest.xml:51-59`（`TerminalFileProvider`，authorities `com.termux.fileprovider`） | — |
| 独立进程 BootstrapInstallService | `AndroidManifest.xml:46-49`（`:install` 进程） | — |
| 前台服务 + 常驻通知 | `AndroidManifest.xml:37-40`、`service/TerminalForegroundService.kt:68-71`（`IMPORTANCE_LOW` 通知渠道）、`TerminalRuntime.kt:685`、`:2480`、`:2750-2767` | — |
| Nerd Font 字形标签 | `settings/SettingsRepository.kt:28`、`ui/NerdKeyLabels.kt`、`ModifierBar.kt:220-231` | 见 A17 |
| KeyboardMode（secure/raw） | `input/KeyboardMode.kt:7-13`、`TerminalSurface.kt:622-629` | 见 A17 |

**A8. 启动时应用数据兼容性检查完全缺失，违反 `DESIGN.md:236`**

`android/app/src/main/java/terminal/emulator/MainActivity.kt:175-183` 的 `onCreate` 只做 `installSplashScreen()` 与 edge-to-edge；`TerminalApp.kt` 无相关逻辑；全仓检索 `compat` 仅命中 `failsafe`/`WindowCompat`/`TextWidth`。规范要求「应用启动时检查应用数据兼容性，若存在问题可清除应用数据以确保正常启动」。

**A9. 工作区未使用依赖，违反 `BUILD.md:23`「不得保留未使用依赖」**

`Cargo.toml:19` 在 `[workspace.dependencies]` 声明 `libghostty-vt-sys`，但 `native/Cargo.toml:9-35` 无任何 crate 通过 `workspace = true` 引用它，仅由 `libghostty-vt` 传递带入（`Cargo.lock`）。`scripts/check-rust.nu:6` 的 `cargo machete` 只扫成员清单，结构上无法发现工作区级声明。同类重复：`Cargo.toml:9` 与 `native/Cargo.toml:10` 各自声明一次 `base64 = "0.23"`。

**A10. `BUILD.md:15`、`:16`、`:17` 三项构建校验均未实现**

- `scripts/build-android-libs.nu:37-54` 无 `NEEDED` 检查、无 `libghostty-vt.so` 复制分支；全仓 `readelf`/`NEEDED`/`libghostty-vt.so` 零命中
- `scripts/build-apk.nu:23-27`、`:32-36` 只 `glob *.apk` 并判空，从不检查包内是否含 `.so`，也不校验 `jniLibs/` 已填充
- 无 `.so` 体积门禁。实测 dev 产物 27.5 MiB 且 `file` 报 **not stripped**，release 9.95 MiB，差 2.8 倍，叠加 `android/app/build.gradle.kts:97` 的 `keepDebugSymbols += "**/libnative.so"`

**A11. 注释语言大规模违反 `STYLE.md:58`「使用简体中文编写注释和文档」**

| 范围 | 注释行 | 含中日韩字符 | 占比 |
| --- | --- | --- | --- |
| `native/src/**.rs` | 4757 | 682 | 14.3% |
| `android/app/src/main/**.kt` | 4093 | 281 | 6.9% |

样例：`native/src/terminal/pty.rs:940-943`、`TerminalRuntime.kt:1107-1115`、`bridge/Bridge.kt:244-270`。同时违反 `STYLE.md:66`「注释和文档保持极简」（`TerminalRuntime.kt:1107-1115` 用 9 行注释解释一个竞态）。

**A12. 生产代码 `#[allow]` 12 处，违反 `AGENTS.md`「生产代码中禁止 `#[allow]`」**

| lint | 数量 | 位置 |
| --- | --- | --- |
| `clippy::too_many_arguments` | 5 | `render/pass.rs:612`、`render/kitty.rs:136`、`render/cell_builder.rs:585`、`android/ffi.rs:404`、`android/ffi.rs:1120` |
| `clippy::not_unsafe_ptr_arg_deref` | 7 | `android/ffi.rs:887`、`:904`、`:978`、`:2808`、`:2895`、`:2936`、`:3040` |

`STYLE.md:37` 允许抑制的参数数量类 lint 可在最小范围抑制，故前 5 处尚可辩解；`not_unsafe_ptr_arg_deref` 不在该条允许清单内，且 7 处分散在 JNI 导出层应收敛为模块级一次性处理。

**A13. 仪器测试辅助函数静默吞错，违反 `TESTING.md:9`「禁止无声跳过测试」**

`android/app/src/androidTest/java/terminal/emulator/TestUtils.kt:84-143` 的 `openDrawer`：三次点击策略全部包在 `runCatchingCancellable` 中并以 `.getOrDefault(false)` 收敛，全失败后 `Thread.sleep(500)` 直接返回（`:141-142`），**不产生任何断言**。同型问题在 `:148`、`:151`、`:177`、`:182`、`:187`、`:204`、`:211`、`:216`、`:232`、`:240`、`:247`。`android/app/src/androidTest/java/terminal/emulator/SelectionEspressoTest.kt:89` 用 `.getOrDefault(false)` 吞掉 `feedTerminal` 写入失败。

同型吞错还见于 `cucumber/steps/NavigationSteps.kt:69`、`:90`、`:112`。

`.semgrep/kotlin-deny-patterns.yml:17-26` 的 `no-test-early-return` 只匹配 `@Test` 函数体内的裸 `return`，helper 函数与 lambda 内的 `getOrDefault` 均不在覆盖范围，故 `scripts/check-gradle.nu:4` 拦不下。

**A14. 字体列表去重方式违反 `DESIGN.md:103`「不得做手动判断，而是要求外部库 API 提供正确的字体列表」**

`TerminalViewModel.kt:728-731`：`(fileSystemFonts + rustFontFamilies).distinct()`，注释自述「只做精确去重」；而 `settings/SystemFonts.kt:6` 明确声明自身不去重。精确字符串比较无法折叠 `DroidSans` / `Droid Sans` / `Droid Sans Regular` 三种写法，规范点名的场景无法满足。

**A15. 工作区大面积钉死小版本，违反 `DESIGN.md:5`「依赖尽量使用最新版本，尽量不固定小版本」**

`Cargo.toml:9-33` 与 `native/Cargo.toml:10-43` 共 16 处 0.x 小版本钉死：`base64 "0.23"`、`cosmic-text "0.19"`、`fontdb "0.23"`、`jni "0.22"`、`lru "0.18"`、`nix "0.31"`、`png "0.18"`、`roxmltree "0.21"`、`linkify "0.11"`、`parking_lot "0.12"`、`foldhash "0.2"`、`swash "0.2"`、`proptest "1.11"`、`criterion "0.8"`、`cucumber "0.23"`、`shuttle "0.9"`。仅 `wgpu "30"`、`log`、`libc`、`regex`、`serde*`、`tokio` 遵循仅钉主版本。

例外说明：`fontdb` 客观上无法脱离 0.23（`cosmic-text` 0.19 内部重导出 fontdb 0.23，升级会产生 41 处类型错误），该处属规范与现实的真实张力，应在规范中显式记录而非默默钉死。

**A16. 两项持久化设置无写入方，属不可达状态，违反 `DESIGN.md:18`「最小体积，不做任何多余或不必要功能」**

- `USE_NERD_FONT_GLYPHS`：`SettingsRepository.kt:28` 持久化，读取方 `ModifierBar.kt:220`、`:231`；写入方仅 `TerminalViewModel.kt:1212-1215`，`ui/` 目录零调用
- `KEYBOARD_MODE`：`SettingsRepository.kt:29` 持久化，读取方 `TerminalSurface.kt:629`；写入方仅 `TerminalViewModel.kt:1218-1221`，`ui/` 目录零调用

**A17. 6 条设备级覆盖项无对应测试或断言过弱，违反 `TESTING.md:26-42` 覆盖清单**

| 规范行 | 覆盖项 | 现状 |
| --- | --- | --- |
| `TESTING.md:29` | 旧输出按顺序进入回滚区 | 仅断言深度：`ghostty_terminal/tests.rs:2616`、`:2634`；无顺序断言 |
| `TESTING.md:31` | 复制 `font.ttf` 后字体被正确设置并检查字形 | `cucumber/steps/FontSteps.kt:53-61` 只断言 `.termux` 目录存在；全测试树零处 `MapleMono` |
| `TESTING.md:36` | 搜索「上一个/下一个」滚动到对应位置 | `ui/TextSearchEndToEndTest.kt:244-268` 点击了按钮但只断言 `terminalText.contains(marker)`，从不断言视口移动 |
| `TESTING.md:37` | 不同字重（如 `SemiBoldItalic`）可正常显示 | 颜色/斜体有像素验收（`diag/SgrColorPixelAcceptanceTest.kt:115`、`diag/SgrItalicPixelAcceptanceTest.kt:113`），字重零像素测试 |
| `TESTING.md:41` | mksh 短提示符 / 长命令不横滚裁切 / `clear` 后首行首列 | 无任何测试 |
| `TESTING.md:42` | `bash` 会话忽略 `ENV` | 无任何测试；`native/src/terminal/pty.rs:896` 仅有注释 |

`TESTING.md:8` 要求「每个测试必须断言具体行为」，A17 中 `:36` 一项属明确违反。

### A-P2

**A18. 陈旧注释描述已禁止的壁纸功能，违反 `PROHIBITED.md:3`「不得存在任何相关代码」**

- `native/src/android/ffi.rs:2984-2992`：「the wallpaper is hidden behind opaque cell backgrounds」「checkerboard probe proved the background pass and cell transparency both work」
- `native/shaders/cell.wgsl:78`：「show only the glyph over the wallpaper」
- 实际背景是纯色 `LoadOp::Clear`（`render/pass.rs:82`、`:495`、`:869`），相关管线已不存在
- `ui/TerminalSurface.kt:2172` `holder.setFormat(PixelFormat.RGBA_8888)` 同属残留

**A19. `CucumberOptionsClass.kt:13` 保留 `tags = "not @wip"` 跳过开关，违反 `TESTING.md:9`**

当前无 `@wip` 场景，规则惰性；但这是一条常备的静默跳过通道，与 `TESTING.md:11`「不得跳过」相抵触。

**A20. `android/app/src/main/java/terminal/emulator/FontUtils.kt:43`、`:45` 仍映射 `armeabi-v7a` / `x86`，违反 `BUILD.md:25`「只支持 arm64-v8a、x86_64」**

`android/app/build.gradle.kts:52-53` 与 `rust-toolchain.toml:5` 均只声明两架构，此处为死映射。

**A21. `android/app/src/androidTest/java/terminal/emulator/TestUtils.kt` 依赖 `testTagsAsResourceId` 与语义描述符双路径，违反 `STYLE.md:63`「不得保留死代码」的直接体现**

`MainActivity.kt` 的 `testTagsAsResourceId` 同时服务于可访问性与测试定位，`ui/TerminalSurface.kt:2177` 的 `contentDescription` 被 A1 与测试共用，二者耦合使 A1 的删除牵连面扩大。

**A22. `.github/workflows/check.yml:42` 与 `fmt.yml:44` 存在不受管理构建，违反 `BUILD.md:24`「禁止不受管理的构建」**

- `check.yml:42`：`nix develop --command cargo build --package native`（绕开 `scripts/`）
- `fmt.yml:44`：`nix develop --command bash -c "pushd android && ./gradlew spotlessApply && ./gradlew detekt --auto-correct"`
- 对比 `build.yml:38-40`、`check.yml:41-44` 直接调 `./scripts/*.nu` 的规范写法

## B 类：规范文档自身缺陷

**B1（P1）`PROHIBITED.md` 章节结构自相矛盾**

全文只有一个一级标题「# 禁止实现」（`:1`），「## 横向/平板」（`:22`）与「## 设置」（`:26`）是二级标题，字面读会禁止以下条目，而它们全部是**需求**而非禁令（条目均为需求句式）：

`固定/取消固定按钮`（`:24`）、`光标闪烁开关`（`:28`）、`光标闪烁速度`（`:30`）、`光标样式`（`:32`）、`Shell 启动入口状态`（`:34`）、`自定义终端启动目录`（`:36`）、`终端回滚行数`（`:38`）、`修饰键栏布局编辑器`（`:40-46`）、`自定义终端主题`（`:48`）、`Shizuku 集成开关`（`:50`）。

需拆分为独立文件或重构标题层级，否则规范语义不可判定。

**B2（P1）`PROHIBITED.md:36` 与 `DESIGN.md:126` 直接冲突**

前者把「自定义终端启动目录」列为条目，后者明写「**不提供启动目录设置**」。代码遵循 `DESIGN.md`（`TerminalRuntime.kt:934` 注释 + `workingDirectory = effectiveHome`）。

**B3（P1）`PROHIBITED.md:28-30` 与 `DESIGN.md:160` 冲突**

前者要求光标闪烁开关与速度调节条，后者明写「输入光标为方块样式…**不闪烁**」。

**B4（P1）`PROHIBITED.md:38` 与 `DESIGN.md:196` 冲突**

前者要求回滚行数「提供调节条」，后者要求「回滚行数和 Termux 保持一致，如 `2K`」。代码遵循 `DESIGN.md`：`SettingsRepository.kt:36` `FIXED_SCROLLBACK_LINES = 2_000`，无 UI 入口。

**B5（P2）`PROHIBITED.md:7` 与 `DESIGN.md:198` 冲突**

前者禁止「不通过环境变量接收用户设置或在内部传递数据」，后者要求「环境变量 `ENV` 可以设置为 `/data/data/com.termux/.mkshrc`」。代码同时实现并断言（`native/src/terminal/pty.rs:900-902`、`native/src/terminal/pty.rs:1135`、`:1146`；`native/tests/features/shell环境变量.feature:32-33`；`openspec/specs/shell-env/spec.md`）。需在 `PROHIBITED.md:7` 明确「自定义」仅指用户自定义变量。

**B6（P2）`PROHIBITED.md:48` 与 `DESIGN.md:110` 内容重复且措辞不一致**

同一「自定义终端主题 / 默认 Dracula Plus」需求出现在两个文件，措辞不同，易产生双源漂移。

**B7（P2）`STYLE.md:58-61` 规则文本自身违反同文件 `:60`、`:61`**

`:60` 禁止出现 `nix/store`，`:61` 禁止出现 `fish`/`dash`/`zsh`，而规则正文即包含这三个词。

**B8（P2）`STYLE.md:61` 表述过宽，误伤修饰键常量**

`android/app/src/main/java/terminal/emulator/ui/ToolbarPreferences.kt:55` 的 `DASH("-", "-")` 是 Termux 修饰键栏的连字符键。规则应限定为「不得引用这三款 shell」，而非「不得出现这些字符串」。`.semgrep/rust-deny-patterns.yml:105` 用大小写敏感 `\b(fish|dash|zsh)\b`，故实际未拦截，规则与实现已不一致。

**B9（P2）`STYLE.md:29` 与 `scripts/setup-emulator.nu` 冲突**

`:29` 禁止「无助于提升清晰度的中间变量（如 `let start = …`）」，但 `scripts/setup-emulator.nu:29` 的 `$start` 是超时判定必要状态。规则应区分「纯别名」与「跨步状态」。

**B10（P2）`STYLE.md:31` 与 `flake.nix:143-144` 写法不一致**

规则要求 Nushell 脚本内使用 `./scripts/xxx.nu`，`flake.nix` 的 `shellHook` 使用 `nu scripts/xxx.nu`。

**B11（P2）`BUILD.md:7` 与 `flake.nix` 不符**

规范称「`ANDROID_NDK_HOME` 已预设，无需回退查找」，但 `flake.nix:112-139` 的 `env` 只设置 `LD_LIBRARY_PATH`、`VK_ICD_FILENAMES`、`FONTCONFIG_FILE`，全仓无 `ANDROID_NDK_HOME` 定义。

**B12（P2）`STYLE.md:37` 的「全局设置规则」在 Rust 侧无载体，是 C1 门禁失败的根因**

`android/detekt.yml:7-22` 全局关闭了 `CognitiveComplexMethod`、`CyclomaticComplexMethod`、`LongMethod`、`LongParameterList`、`NestedBlockDepth`、`TooManyFunctions`，与 `STYLE.md:37` 吻合。Rust 侧**没有任何等价物**：无 `clippy.toml`、无 `.clippy.toml`、`Cargo.toml` 无 `[lints.clippy]`。配合 `scripts/check-rust.nu:4` 的 `--deny warnings`，风格类 lint 直接升级为错误。

**B13（P3）`DESIGN.md:46-56` 的 crate 树与实际不符**

文档树只列 `native/src/android/`、`render/`、`terminal/`，实际还有 `native/src/render/font/`（7 个文件）；顶层实际为 `lib.rs`、`event.rs`、`log_chunk.rs`、`prop_tests.rs`，文档未列。

**B14（P3）`DESIGN.md:229` 点名的 `ghostty_terminal_reset` 在代码中不存在**

实际 JNI 导出名为 `resetTerminal`（`native/src/android/ffi.rs:653-674` → `session.rs:526-528` → `internal.rs:797`）。功能等价，但规范点名的符号无法检索。

**B15（P3）`DESIGN.md:72`「上游 `libghostty-vt` / `libghostty-vt-sys` 固定跟踪 git master」与实际不符**

`Cargo.toml:18-19` 指向第三方 fork `Uzaaft/libghostty-rs` 的固定 rev `5988a0b7`，不是上游 ghostty 的 master。该 rev 恰为该 fork 的 master HEAD，故「无本地补丁」成立、「跟踪上游 master」不成立。

**B16（P3）`AGENTS.md` 要求的 openspec 归档未完成**

`openspec/changes/2026-09-23-font-selection-spec/tasks.md` 的 2.11（验证后 `openspec archive`）未勾选，且 `openspec/specs/font-selection/` 已存在，属「已同步未归档」状态，违反「完成后进行归档和删除」。

**B17（P3）`README.md:3` 技术描述失准**

称 `libghostty-vt-sys` 为 VT 解析器；实际 VT 解析走 `libghostty-vt`（`native/src/terminal/ghostty_terminal/internal.rs:4`），而 `libghostty-vt-sys` 是未被引用的工作区声明（见 A9）。

**B18（P3）`PROHIBITED.md:44` 自相矛盾**

「编辑器支持预览和长按拖动位置，拖动位置为按键起始坐标」——位置本身即起始坐标，需求不可判定。

**B19（P3）`PROHIBITED.md:50` 含过期的硬编码绝对路径**

`/data/app/~~Sa3_liMwmjUIoWwNMF_x7w==/moe.shizuku.privileged.api-No2vLGXjkKhlYU6TcXtuHg==/lib/arm64/libshizuku.so` 在 Android 10+ 分包机制下已无效，且与 `AGENTS.md`「禁止硬编码 `/data/.*/files` 形式的应用数据路径」的精神冲突。

## C 类：审查前已存在的门禁失败

这两项在本次审查开始前即失败（已用 `git stash` 对比原始 `Cargo.lock` 确认与依赖更新无关），是 C 类独立缺陷：

**C1. `cargo clippy --workspace --all-targets -- --deny warnings` 4 处失败**

- `native/src/terminal/ghostty_terminal/commands.rs:112`、`:119`、`:126`：`clippy::type_complexity`，`tx: Sender<Option<((u32, u32), (u32, u32))>>`
- `native/src/terminal/ghostty_terminal/internal.rs:99`：`clippy::too_many_arguments`（8/7），`process_query`

根因是 B12：`STYLE.md:37` 允许在全局设置风格规则，但 Rust 侧缺少 `clippy.toml` / `[lints.clippy]` 载体，而门禁用 `-D warnings`。

**C2. `markdownlint-cli2` 5 处失败**

- `openspec/changes/archive/2026-09-23-session-list-redesign/specs/session-list/spec.md:22`（MD038）
- `openspec/changes/archive/2026-09-24-fix-modifier-bar-gesture-cancel/design.md:15`、`:19`（MD032）
- `openspec/specs/session-list/spec.md:3`（MD022）
- `openspec/specs/text-selection/spec.md:3`（MD022）

这 4 个文件与规范文档同属 openspec 体系，规范与规格文档的排版门禁目前是红的。

## 已核验通过的高风险断言

以下为规范中容易被怀疑但实测合规的项，一并记录以免后续重复排查：

| 断言 | 结论 | 证据 |
| --- | --- | --- |
| `minSdk=33` / `compileSdk=37` / `targetSdk=28` / `versionCode=2000` / `versionName=0.1.0` / `VERSION_17` | 通过 | `android/app/build.gradle.kts:42-45`、`:78-79` |
| `ndkVersion` 未在代码中固定 | 通过 | 全仓零 `ndkVersion` / `ndkPath`（仅 `BUILD.md:20` 提及） |
| Rust 1.98 / edition 2024 | 通过 | `Cargo.toml:5-6`；`rust-toolchain.toml:2` 用 `stable` 渠道 |
| 仅 `arm64-v8a` / `x86_64` | 通过 | `android/app/build.gradle.kts:52-53`；`rust-toolchain.toml:5` |
| 环境变量白名单（10 项 + `ENV`） | 通过 | `native/src/terminal/pty.rs:17-20`、`:842-846`、`:882-887`、`:897-916` |
| 未设置 `LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD` | 通过 | `native/src/terminal/pty.rs:1200-1228` 断言并拒绝额外变量 |
| 宿主透传 14 个变量 | 通过 | `native/src/terminal/pty.rs:861-876`，顺序与 `DESIGN.md:142` 一致 |
| `ENV` 未传路径时不注入 | 通过 | `native/src/terminal/pty.rs:900-902`；`android/ffi.rs:509-513` |
| Shell 入口查找顺序 `bash` → `login` → `/system/bin/sh` | 通过 | `TerminalRuntime.kt:1946-1948`、`bridge/Bridge.kt:124` |
| Bootstrap 原子替换 + 旧目录改名不自动删 | 通过 | `installer/BootstrapInstaller.kt:321-345` |
| 不写 bootstrap 状态标记 | 通过 | `MainActivity.kt:276-277` |
| AOSP testkey 签名（含 debug 变体） | 通过 | `android/app/build.gradle.kts:31-38`、`:46`、`:60`、`:65` |
| 字体：`fonts.xml` 解析失败即崩 / `font.ttf` 与 `ttc` `otf` 默认 / `.termux/font` 目录 | 通过 | `settings/SystemFonts.kt:34-49`、`FontUtils.kt:10-15`、`TerminalViewModel.kt:724-726` |
| 无启动目录设置 / 仅一项主字体 | 通过 | `TerminalRuntime.kt:934`、`SettingsRepository.kt:18-30` |
| 只渲染当前会话，后台/设置暂停 | 通过 | `TerminalScreen.kt:237-280`、`MainActivity.kt:335-353` |
| 清除应用数据不触碰 `filesDir` | 通过 | `TerminalViewModel.kt:967-985` |
| 搜索防抖 150ms / 大小写 / 上下条 / 关闭 | 通过 | `TerminalScreen.kt:171`、`TextSearchBar.kt:55`、`:117`、`:139`、`:160`、`:167`、`:252` |
| 修饰键栏固定 2 行 7 列 | 通过 | `ModifierBar.kt:65`、`ToolbarPreferences.kt:211-229` |
| Shell 崩溃保留会话 / 启动失败不回退 | 通过 | `TerminalRuntime.kt:487-517`、`:2644` |
| 无 `#[ignore]` / `@Ignore` / `assumeTrue` | 通过 | 全仓零命中 |
| 无 `anyhow`（须 `thiserror 2`） | 通过 | `native/Cargo.toml:33`；`Cargo.lock` 中 anyhow 仅由 `cucumber` 传递引入 |
| 核心终端数据路径无 `unsafe` | 通过 | `ghostty_terminal/**`、`render/cell_builder.rs`、`render/pass.rs` 零命中 |
| 无 Java 文件 / 无 JNA / 无逐单元格 `Canvas.drawText` / 无 `/proc/self/exe` / 无裸指针 FFI | 通过 | 全仓零命中或仅注释 |
| 无 crate features | 通过 | `Cargo.toml`、`native/Cargo.toml` 均无 `[features]` |
| Nushell 脚本无 `\|\|` / `which` / `ignore-errors` / `err>` / `step-label` | 通过 | `scripts/*.nu` 零命中 |
| CI Action 版本用默认分支，无步骤 `name` | 通过 | 三个 workflow 一致，例外项符合 `STYLE.md:48` |
| 提交消息单行、作者统一 | 通过 | 560 个提交全部 `jane <jane@computer.local>`，0 个多行消息，0 个 merge |

## `.semgrep/` 门禁覆盖度缺口

现有 42 条规则（`rust-deny-patterns.yml` 26 条、`kotlin-deny-patterns.yml` 13 条、`android-deny-patterns.yml` 3 条）对构建、测试、环境变量、脚本风格覆盖良好，但 `PROHIBITED.md` 的 16 项禁止内容中只有 4 项有规则：

| `PROHIBITED.md` 条目 | 规则 | 状态 |
| --- | --- | --- |
| 内嵌 proot（`:16`） | `no-embedded-proot` | 已覆盖 |
| termux-api（`:12`） | `no-termux-api-refs` | 已覆盖 |
| `termux.env`（`:15`） | `no-termux-api-refs` | 已覆盖 |
| Bootstrap sha256 sidecar（`:6`） | `no-bootstrap-sidecar` | 已覆盖 |
| 无障碍朗读（`:21`） | — | **无规则，A1 因此零阻力上线** |
| 粘贴确认对话框（`:9`） | — | 无规则 |
| 会话数据持久化（`:8`） | — | 无规则 |
| 实体键盘快捷键设置（`:10`） | — | 无规则 |
| 背景图片/透明背景/模糊（`:14`） | — | 无规则 |
| 内嵌 bootstrap 资产（`:17`） | — | 无规则 |
| 选中菜单 ◀/▶（`:5`） | — | 无规则 |
| 自定义环境变量（`:7`） | `no-forbidden-env-var-set` | 仅覆盖 `LD_*`/`PWD` 类，不覆盖「用户自定义变量」 |
| MCP（`:11`） / 桌面环境 X11（`:18`） / 自绘放大镜（`:19`） | — | 无规则 |

## 建议处置顺序

1. **A1**：删除无障碍朗读全套代码、字符串、测试；`ui/TerminalSurface.kt` 需同步清理 `:2177-2178`、`:2274-2400`。这是唯一违反「不得存在任何相关代码」的项。
2. **B1-B5**：先修规范，消除 `PROHIBITED.md` 的结构与冲突问题，否则 A2-A8 的「合规方向」无法判定（例如 A5 该改代码还是改规范，取决于 `DESIGN.md:129` 是否仍然有效）。
3. **A2**：删除四类日志落盘，或在 `DESIGN.md:82` 显式授权崩溃诊断文件（推荐后者并限定保留期，但需先定规范）。
4. **A3**：`scripts/test-emulator.nu:8` 改 `:app:connectedReleaseAndroidTest`，需先验证 release 下 `androidTest` 源码集与 cucumber runner 在 minify 后可用。
5. **A4**：删除 `NativeBridgeSmokeTest.kt` 的候选路径探测与构建命令文案，改为固定 `../../target/release/libnative.so` 直接 `System.load`，并给 `.semgrep/kotlin-deny-patterns.yml` 增加注释/KDoc 匹配。
6. **A13 + A19**：把 `TestUtils.kt` 的 helper 改为显式失败，删除 `not @wip` 通道。
7. **B12 + C1**：新增 `clippy.toml` 或 `Cargo.toml` 的 `[lints.clippy]`，与 `android/detekt.yml` 对齐后 C1 自然转绿。
8. **A9 + A18 + A20**：删除 `libghostty-vt-sys` 工作区声明与重复的 `base64` 声明，清理壁纸残留注释，删除 `armeabi-v7a`/`x86` 死映射。
9. **A11**：注释语言改写量最大（Rust 约 3400 行、Kotlin 约 3800 行），建议分批按模块推进。
10. **A17**：补齐 4 条缺失覆盖项，修正 2 条弱断言。

## 复核方法

```text
# A1 无障碍朗读
grep -rn 'accessib\|朗读\|TalkBack' android/app/src/main/ | head

# A2 日志落盘
grep -rn 'getDir("logs")' android/app/src/main/

# A3 测试变体
grep -n 'connectedDebugAndroidTest\|connectedReleaseAndroidTest' scripts/test-emulator.nu

# A4 环境探测
grep -n 'isFile\|System.getenv' android/app/src/test/java/terminal/emulator/bridge/NativeBridgeSmokeTest.kt

# A5 Bootstrap 预设
grep -n 'bootstrap-' android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt

# A9 未使用依赖
grep -n 'libghostty-vt-sys' Cargo.toml native/Cargo.toml

# A11 注释语言占比
grep -rhoE '^\s*(//|/\*|\*)[^\n]*' native/src --include='*.rs' | wc -l
grep -rhoE '^\s*(//|/\*|\*)[^\n]*' native/src --include='*.rs' | grep -cP '[\x{4e00}-\x{9fff}]'

# A12 生产 #[allow]
grep -rn '#\[allow(' native/src --include='*.rs'

# A13 静默吞错
sed -n '84,143p' android/app/src/androidTest/java/terminal/emulator/TestUtils.kt

# A16 不可达设置
grep -rn 'setUseNerdFontGlyphs\|setKeyboardMode' android/app/src/main/java/terminal/emulator/ui/

# B12 Rust 侧无全局 lint 配置
find . -name 'clippy.toml' -not -path './target/*'
grep -n '\[lints' Cargo.toml native/Cargo.toml

# C1 门禁
nix develop --command cargo clippy --workspace --all-targets -- --deny warnings
nix develop --command nu scripts/check-rust.nu
nix develop --command nu scripts/check-gradle.nu
```
