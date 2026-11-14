## MODIFIED Requirements

### Requirement: Android 侧终端项目的复查台账

本规范 MUST 收录对 Android 侧终端项目的逐项复查台账，每项 MUST 写明：实质（它到底是终端
模拟器、UI 壳、环境 SDK，还是无源码的空仓库）、门槛判定结果、以及可复核依据。

判定 MUST 按断言的期望值来源分流，不得按「测试数量多」判断：手写屏幕文本并注明规范出处者
门槛 1 不成立；断言对象是自身回写字节流、或期望文件可被实现自身覆写者门槛 1 成立；
断言打在本仓不实现的组件上者门槛 3 不满足。

台账中 MUST 同时登记与 `DESIGN.md` 直接冲突、后续复查不得反向采纳的做法，即使其来自
同类项目且实现完整。

#### Scenario: fork 与重写项目的价值按内容差异判定

- **WHEN** 某项目自称是某上游的 fork 或重写版
- **THEN** MUST 以文件内容比对判定其相对上游的新增与改动，MUST NOT 以项目自述替代；
      若其测试目录与上游逐字节相同且零新增测试，则其价值等同于上游，MUST NOT 因「来自某个
      fork」而高估

#### Scenario: CI 显示绿灯但仓内无测试

- **WHEN** 某项目的持续集成声称运行单元测试
- **THEN** MUST 核实仓内实际测试文件数量；为零者 MUST 记为 CI 虚假绿灯，不得计为有测试资产

#### Scenario: 与 DESIGN 冲突的做法被登记为不可反向采纳

- **WHEN** 复查中发现某同类项目实现了 `DESIGN.md` 明确禁止或要求相悖的行为
- **THEN** MUST 在台账中点名文件与行号并标注「不得反向采纳」，
      MUST NOT 因该实现「更完备」而记为可参考点

实测台账如下。本次复查的 25 项 Android 侧终端项目逐条列在下面，门槛列显式二选一：
「违反 N」指门槛不成立；「无门槛违反」指门槛均成立，后接阻塞或不采纳理由。

**需改造后采纳（4 项）：**

| 项目 | 门槛 | 可复核依据 |
| --- | --- | --- |
| `sylirre/ghostty-android-terminal` | 门槛 1/2/3 均不违反，但须逐条验判别力 | 17 个文件 / 160 个 `@Test`，全在 `androidTest`。期望值锚定外部协议：`EmulatorVtTest.java:423` 断 DSR 应答字面量 `\u001b[1;1R`、`:647` 断 XTWINOPS `\u001b[8;5;20t`；大小写折叠表由脚本从 Python `unicodedata` 生成。与本仓同架构（libghostty-vt + JNI），是最正当的对照面；但本仓已有超链接、搜索、xtwinops、下划线覆盖，净增有限 |
| `reapercanuk39/termux-kotlin-app` | 门槛 1 不违反（仅限键位编码与鼠标两类断言） | `KeyHandlerTest.java:31-45` 断言 termcap 能力名到字节序列（`kb`→`\u007f`、`kB`→`\033[Z`、`kUP`→`\033[1;2A` 等），出处为 terminfo 规范；`TerminalTest.java:59-65` 断言越界点击按边缘钳位为 `\033[<0;1;1M` 并注明 termux-app issue 501。二者均属本仓自有功能（键位编码、鼠标），非上游 VT 语义 |
| `CocaKova/charon` | 门槛 1 不违反（仅限外部标准期望值子集）；其 corpus 门槛 1 与「可被实现覆写」均违反 | 外部标准类：`ModernContractTest`（DECRQM/DECRQSS/OSC52 按 ECMA-48）、`KittyKeyboard*`、`Utf8DecoderTest`（WHATWG 算法）、`TextSelection`；另有 `tools/conformance_vttest.py` 把外部套件跑在自己的 pty 上。反例：`CorpusTest.kt:26` 支持 `-Dcharon.regenGoldens` 用自身 `render()` 覆写期望，5 个 `.golden` 均属此类，MUST NOT 采纳；取其 IME 契约经验时，`updateSelection` 经 `javap` 实测为框架内部 @hide（API 33/36/37 公开 `InputConnection` 均无此符号，仅 IME 侧类含之），应用不可覆写，记为不采纳 |
| `Lord1Egypt/AndroidThothTerm` | 门槛 1 不违反，但仅作判据来源登记 | 2026-11-02 复查：`emulatorview/src/test` 5 个文件 / 34 个 `@Test`，期望值为人工手写外部真相（如控制字符串不得绘制、选区按逻辑序还原）。其引擎为 `jackpal/Android-Terminal-Emulator` 血统，OSC 只有 0/1/2、DA1 自述 `ESC[?1;2c`、DECSET 仅插入模式 4、鼠标只有 1000–1003 与 1006，无 sixel/kitty，能力低于本仓上游，断言只能当边界清单。逐项判定见本表下方「能力裁定补齐行」与「参照面与反面做法收敛补齐行」 |

**不可采纳（21 项）：**

| 项目 | 门槛 | 可复核依据 |
| --- | --- | --- |
| `termux/termux-app` | 屏幕文本类门槛 1 不违反、门槛 3 不满足；回写类门槛 1 成立 | 实测手写屏幕文本断言 271 处、`MockTerminalOutput` 仅 3 处、外部规范出处 16 处。门槛 3 不满足的原因是断言打在本仓不实现的上游 VT 引擎上 |
| `sohan-f/termux-monet` | 门槛 3 不满足 | 与官方 `terminal-emulator` 的差异经逐文件核验为纯格式化，`WcWidth` 的零宽区间表逐字节相同；新增仅 Monet 取色 14 例，断言的是刚写入的调色板值 |
| `ownerroc/termux-monet` | 门槛 1 成立、门槛 3 不满足 | 测试是官方 termux-app 的**旧快照**（少 `ApcTest.java`），fork 零新增测试；主源码缺 `ITermImage`/`TerminalSixel`/`TerminalBitmap`，以其为参照等于对齐更弱的上游 |
| `Autonomous-Agentic-Workflows/termux-app` | 门槛 3 不满足 | `terminal-emulator/src/test` 与官方逐字节相同、零原创测试；且它是官方的**弱化子集**（缺六个源文件，OSC 重构关键标识命中 18 处 vs 官方 37 处） |
| `rafaelmeloreisnovo/termux-app-rafacodephi` | 门槛 1 成立 | `terminal-emulator/src/test` 与官方逐字节相同；其 `rafaelia` 测试整条包在环境条件里，native 可用时**空跑**，且 `cycleAlwaysWithinRange` 只断自身取值的范围，零判别力 |
| `Rprop/AnsolePlus` | 门槛 3 不满足 | 停在 termux-app 旧 rev：`TerminalEmulator.java` 相对上游 595 删 / 65 增，`doDcs` 出现 0 次（上游 9）、`sixel` 0 次（上游 49），缺 `TerminalSixel`/`ITermImage`/`TerminalBitmap`；零测试 |
| `Adhi-hub07/on3-terminal` | 无门槛违反：零测试资产 | 全仓无任何 `src/test`；其 `TerminalEmulator.kt` 是 termux-app 的 Kotlin 改写（字段与算法逐行同构，**仓内无出处声明，此判断为结构推断**），不构成净增内容 |
| `frostre1997/kosh` | 无门槛违反：无本仓测试资产 | VT 全在 Maven 依赖 `terminal-emulator:v0.118.3`，仓内无实现；仅 2 个 Android Studio 模板测试；其 proot 树与 `ReTerminal` 经 `diff -rq` 完全相同，二者同源 |
| `RohitKushvaha01/ReTerminal` | 无门槛违反：无本仓测试资产 | 同上，VT 由 `core/main/build.gradle.kts:90-91` 的 Maven 依赖提供，仓内仅 2 个模板测试 |
| `GlobalTechInfo/RedTerm` | 门槛 3 不满足 | VT 走上游 jar，仓内无 `TerminalEmulator`；329 个用例覆盖 SSH/SFTP/distro/proot，本仓无此功能；`AnswerLatchTest.kt:46-56` 以真实时钟与固定 sleep 断时序，违反本仓「没有不稳定的测试」 |
| `mirute02/anoterm` | 门槛 1 不违反、门槛 3 不满足 | 124 个 `@Test`，期望值确为外部真相（`CharWidth.kt:1-36` 注明取自 Unicode `EastAsianWidth.txt`，UTF-8 用标量规则，VT 语义手写规格）；但断言对象是它自研的 VT 组件，本仓不存在该组件 |
| `8dmusichannels-star/terminator` | 无门槛违反：零测试资产，且 CI 虚假绿灯 | 全仓 `@Test` / `org.junit` 零命中，三个 Gradle 模块无 `testImplementation`，而 `.github/workflows/build.yml:24-25` 仍有「Run unit tests / gradle test」；另携带 12 个 ncurses 二进制 terminfo 条目 |
| `pavelc4/Rin` | 门槛 1 成立、门槛 3 不满足 | 全仓 34 个 `.rs` 中 `#[test]` 仅 4 个，全在包管理器模块且断自身默认值；终端侧零测试；`src/renderer/screen.rs:90-99` 的 `render_cell` 是空壳，实际绘制在 Kotlin 的 `Canvas` |
| `imlunixdev/puppy` | 门槛 3 不满足 | `TerminalBufferTest.kt` 10 例中 truecolor、OSC133 用例确为手抄标准，但覆盖面极窄；核心解析含已确认的标准偏差（`TerminalBuffer.kt:104` 的 `CSI J` 漏 `Ps=1` 且额外重置光标，`:105` 的 EL 遇空参会抛） |
| `shafinthedev/Ply` | 门槛 1 不违反但零判别力 | 无 VT 解析，`src/main.rs` 只把 `/system/bin/sh` 的 stdout 原样转发；两个测试均坏：`test_help_command` 断言从未打印的字符串，`test_echo_command` 测的是宿主 shell |
| `irealashu/ApexShell` | 无门槛违反：无 VT 测试资产 | 以正则 `\u001B\[[0-9;]*[a-zA-Z]` 冒充 VT 解析（`engine/AnsiParser.kt:12`），无光标、无擦除、无真彩色；仅 3 个 Android Studio 模板测试 |
| `deathlegionteamlk/jolz` | 无门槛违反：无源码可评估 | `git ls-files` 仅 `README.md`，README 自述「源码私有，仅公开 APK 与本 README」；技术栈声明无源码可核实 |
| `Sunbr99/TerminalOS` | 无门槛违反：仓库内无资产 | 全仓仅 `README.md`、`LICENSE` 与 3 行 `examples/monitor.sh`；README 描述的 Go 项目结构与 `tests/shell_test.go` 在仓库中一个都不存在 |
| `TOYTTQE/PufiX` | 无门槛违反：仓库内无资产 | 13 个 Python 文件的桌面 REPL 玩具，只**产生** ANSI 而从不解析，无网格、无 PTY；零测试 |
| `Malik1stmkw/termux-clone` | 无门槛违反：仓库内无资产 | 全仓仅 1 个 59 字节的 `README.md`，无任何源码或二进制；「终端模拟器」这一主张在仓内无支撑 |
| `eggie-admin/hydra-shell-android` | 无门槛违反：仓库内无终端资产 | 204 个文件中 Android 与原生源码为 0（62 md / 40 json / 39 py / 26 yml / 18 sh），APK 钉在外部仓库；现有测试全为 HTTP 与 adb 相关 |

**新增失效语料记录：**

| 项目 | 失效语料 | 依据 |
| --- | --- | --- |
| `Lord1Egypt/AndroidThothTerm` | `tests/` 共 **131 个文件 / 36 个子目录 / 924K**（2026-11-02 复查） | 全仓（含 `.github`、`build.gradle`、构建脚本）对 `tests/scripts`、`tests/wideChars` 等目录的引用数仍为 **0**。其中 `tests/scripts` 的期望写在脚本末尾 `#expected screen:` 注释里，判定者是人眼；`tests/wideChars` 是散文加手打屏幕 |

**不得反向采纳的做法：**

以下做法即使来自实现完整的同类项目，也与 `DESIGN.md` 相悖，后续复查 MUST NOT 记为可参考点：

| 做法 | 出处 | 与 DESIGN 的冲突 |
| --- | --- | --- |
| 双击选词、三击选行 | `ghostty-android-terminal` 的 `TerminalView.java:1076-1096`、`terminator` 的 `TerminalView.kt:1275-1279` | DESIGN 明令不支持多击选择 |
| OSC 8 链接做 scheme 白名单 | `terminator` 的 `TerminalView.kt:1482-1487` | DESIGN 要求不检查链接可用性，点击后经系统 API 跳转 |
| 屏幕更新回调里无条件清零滚动位置并全量重绘 | `on3-terminal` 的 `TerminalView.kt:249-260` | 与 DESIGN 的脏跟踪、跳过干净快照相悖 |
| 在渲染热路径内改写终端状态 | `puppy` 的 `TerminalView.kt:33` 每帧 `onDraw` 调 `buffer.resize` | DESIGN 禁止渲染路径副作用 |
| 把字符串长度当字节数传给写入接口 | `on3-terminal` 的 `TerminalEmulator.kt:204-206` 传 `data.length`（UTF-16 码元数）给 `write(ByteArray, offset, count)` | 任何非 ASCII 粘贴即越界或截断；属字符串到字节数组的长度换算缺测试锚点 |
| ED 顺手重置光标 | `puppy` 的 `TerminalBuffer.kt:104` 的 `Ps=2/3` 额外置 `cursorX=cursorY=0` | 规范 ED 不移动光标 |
| 仓库内提交 CMake 生成物与构建产物 | `terminator` 的 `terminal-emulator/src/main/cpp/Makefile`（内含作者本机绝对路径）、`on3-terminal` 的 `app/build/outputs/apk` | 构建卫生，且违反不得引入无程序消费者的二进制资产 |
| 把自研 VT 核留作「Ghostty 集成失败时的回退核」 | `kshivang/BossTerm`（`bossterm-core-mpp`，报告原话「if Ghostty integration fails, BossTerm is fallback」） | DESIGN 锁定终端状态单一来源，不设第二 VT 核；引入回退核即违单一来源 |
| WebView 远程终端作为移动端形态 | `marshallku/copad` 的 `copad-mobile` | DESIGN 无网络终端面（未声明）；终端状态与渲染须在本机（本仓 kotlin + Rust 本地栈） |
| 安全契约的正面不变量（外部 Intent 数据不得键入 PTY、只走工作目录／argv／环境变量等结构化通道；外部路径逐段校验且不跟随既有链接；检查失败即拒绝不落回宽松路径；每次漏洞修复留永久回归测试） | `Lord1Egypt/AndroidThothTerm`（`docs/security/THORNS.md`） | 已采纳为正面不变量：`app-lifecycle` 的「外部 Intent 的数据只走结构化通道」、`documents-provider` 的「外部寻址一律按根内包含性判定」与 `bootstrap-install` 的「引导归档的来源路径必须限定在应用私有目录内」三条 Requirement 承接；本仓对外入口为导出的启动 Activity 与文档提供者，两者各自的处置见上述条目；归档入口原先放开任意路径，已按私有目录包含性收紧并留回归用例 |
| 导出面策略测试（从构建产物读合并清单并与策略文件精确比对，多一项即失败） | `Lord1Egypt/AndroidThothTerm`（`tests/security/exported_components.py` 与 `tests/security/policy/`） | 仅参照：本仓导出面为启动 Activity 与文档提供器（`android/app/src/main/AndroidManifest.xml`），可用同样思路钉住不得新增导出组件；若采纳，检查体系须只用 Nushell（`STYLE.md`），不得新增 Python 体系 |
| 上传到会话当前目录（用 `/proc` 前台进程组解析会话真实工作目录，暂存加一步改名，只增不改） | `Lord1Egypt/AndroidThothTerm`（`term/src/main/java/com/thothterm/upload/` 的 `SessionDirectory`、`ProcFiles`、`StagingJournal`） | 未声明，不立项：本仓 `DESIGN.md` 无「上传到会话工作目录」条目；前台进程组解析为可靠技术判据，该判据的登记处即本行（原研究笔记所在的 `docs/persistence/android-thoth-term.md` 已删除） |
