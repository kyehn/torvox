## ADDED Requirements

### Requirement: 已复查结论台账

本规范 MUST 自带逐项结论台账：每项上游测试资产 MUST 写明采纳门槛的判定结果，并给出
**可复核依据**（上游源码位置、语料中实际存在的字节、或按上游自己的驱动代码确认的无消费者
文件），MUST NOT 只写「其断言是自身期望值」这类未经验证的概括，MUST NOT 把结论只留在归档
变更里——归档是历史台账，不承担规范义务。

判定结果 MUST 在「违反某条门槛」「无门槛违反（并说明为何不违反或阻塞于何处）」之间显式
二选一，MUST NOT 留空：留空既可能被读成「尚未复查」，也会让「每项都给出违反哪一条的理由」
这条 Scenario 形同虚设。

许可证 MUST NOT 作为判定依据：本仓不引入许可证文件也不检查依赖许可证，故「许可证排除」
不是可用理由；因许可被排除的资产 MUST 改列其真正违反的门槛。

#### Scenario: 实测数字与差异归因

- **WHEN** 采纳上游资产或登记其差异
- **THEN** MUST 附今日实测的通过与差异数字、把差异归因到具体层，且临时探针 MUST 在判定后删除

MUST 采纳的资产 MUST 附实测数字（今日直接通过的用例数与差异聚类），实测探针 MUST 在判定
完成后删除。差异 MUST 归因到具体层（本仓 / 绑定层 / 终端上游），MUST NOT 以「上游行为不同」
笼统带过；拟归因于同一类边界时 MUST 穷举全部涉及的码位而非抽样——数量不止少数几个时说明
存在需要单独裁定的边界，不能用一句概括收尾。

#### Scenario: 结论可脱离归档独立复核

- **WHEN** 复查某项上游资产的采纳与否
- **THEN** 该项结论与依据 MUST 能在本规范内读到，MUST NOT 需要先翻归档变更

#### Scenario: 依据是可重放的事实而非概括

- **WHEN** 某项被判定为「期望值是该上游自身实现的输出」
- **THEN** MUST 给出该期望值的产出位置或产出方式，使读者打开上游即可核对；
      MUST NOT 以「其断言是自身期望值」收尾

#### Scenario: 失效语料被识别而非整包采纳

- **WHEN** 某语料目录内存在无同名输入的期望值文件
- **THEN** MUST 按上游驱动代码确认该文件是否被读取；无人读取者 MUST 记为失效语料，
      MUST NOT 计入可采纳规模

已复查结论台账如下。

| 资产 | 门槛 | 可复核依据 |
| --- | --- | --- |
| alacritty `ref` 语料（45 例） | 1 | 期望值由 `write_ref_test_results` 克隆**运行中**的网格后序列化落盘，回放测试再读该文件逐格比对——期望值即其自身实现的输出 |
| wezterm `k9::snapshot!` 快照 | 1 | 快照宏由 `k9` 依赖提供，置位环境变量即自动改写期望值，期望值可被实现自身覆盖 |
| kitty `kitty_tests` | 1 | 断言打在 C 扩展暴露的白盒钩子（如 `test_render_line`）上，断言对象是内部结构而非屏幕行为 |
| termux `terminal-emulator` JUnit（19 个文件） | 按断言形态分流，不整包判 | 实测该目录内 `assertLinesAre`/`assertLineIs` 手写屏幕文本断言 **271 处**，而 `MockTerminalOutput` 仅出现 **3 处**；6 个文件带外部规范出处（`vt100.net/docs/vt510-rm/` 引用 16 处，另有 Unicode 6.2、chromium issue 212704、Mozilla bug 746900 等）。故**屏幕文本类断言门槛 1 不成立**（锚定 VT510 等外部文档），**回写类断言（`TerminalTest`、`OperatingSystemControlTest` 打在 `mOutput` 与包级私有字段上）门槛 1 成立**。此前把整包判为门槛 1 属未经验证的概括，已按此更正 |
| contour `test/*.sh`（21 脚本） | 1 | 脚本只发序列并等待，无期望输出文件；其中 7 个以 `read` 等待人工按键，判定者是人眼 |
| xterm.js vttest 派生语料（76 例） | 门槛 1 不适用（期望值是外部真相），按输入分流不整包采纳 | 期望值是从 xterm 手抄的屏幕文本（外部真相），但同时固化了 xterm 当年的畸形行为：语料中确有多参数的 `CSI 2;3D`（CUB 只取首参数）与快照当年未实现的 `CSI 3 b`（ECMA-48 标准序列，本仓已实现并更合规）。按「按输入是否格式合规分流采纳」，只可采纳格式合规且语义唯一的子集 |
| rio `rio-vt/tests/sixel`（3 组） | 3 | `.rgba` 期望值确由三种外部编码器产出（外部真相），但采纳它要求本仓先实现 sixel——DESIGN 未要求该功能，属净增功能而非净增覆盖 |
| foot `tests/` | 无门槛违反：仓库内无资产可评估 | 仅 `meson.build` 与 `test-config.c`，无 VT 或网格语料；「无资产」是结论而非遗漏，复查时确认这一点即可 |
| esctest2（559 个 `test_*` 用例方法） | 无门槛违反：阻塞于绑定层缺能力 | 期望值是手写的外部真相（门槛 1 不成立），阻塞在别处：屏幕断言经 DECRQCRA 取矩形校验和——`AssertScreenCharsInRectEqual` 对每个待验字符调 `GetChecksumOfRect`，后者发 `DECRQCRA` 并读 DCS 应答。实测：在绑定层构建时取到的终端源码树（即 `libghostty-vt-sys` 的 `build.rs` 中 `GHOSTTY_COMMIT` 常量所指提交）内检索 `DECRQCRA` **零命中**，而终端上游 master 已具备分发与校验和计算 → 缺口在**绑定层**，前提未具备则 MUST NOT 引入 |

实测记录如下。

| 语料 | 实测 | 结论 |
| --- | --- | --- |
| UAX #29 `GraphemeBreakTest`（Unicode 18.0.0） | 共 1547 簇，剔除 241 簇含控制码位（换行/制表/擦除副作用，无法写入网格）后实测 1306 簇。本仓两条自有字素通道——渲染路径 `CellData`（`codepoint` + `grapheme_extra`）与快照路径 `CellSnapshot.graphemes`——**0 例互相不一致**；其中 922 簇与 UAX 期望逐码位相等；384 簇差异全部落在「首码位为无基字符」一类，首码位穷举恰为 7 个（U+0300、U+0308、U+094D、U+1160、U+11A8、U+200C、U+200D，均为组合标记或格式字符，终端单元无基字符可附着），属终端上游的单元模型。最长簇 6 个码位，单格上限从未触发，无截断 | 本仓搬运层对上游字素簇零缺陷，该语料对本仓无净增覆盖，不留存 |
| xterm.js vttest 派生语料（76 例） | 本次实测的是失效语料一项：`t0031-HPB.text` 无同名输入文件，且上游驱动代码只按 `*.in` 取用例，该期望值无消费者 → 1 例不计入可采纳规模。其余 75 例的通过数是归档复查的历史数字，**本次未重测**，不作为当前判定依据 | 失效语料已剔除；历史数字仅作背景，重测前 MUST NOT 引用它下结论 |

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

**性能对比资产登记：**

| 项目 | 终端内容渲染路径 | 基准资产 | 可核实的官方性能数字 |
| --- | --- | --- | --- |
| `termux/termux-app` | HWUI `Canvas`（`TerminalView` 继承 `View`，`TerminalRenderer` 以 `canvas.drawTextRun` 绘制） | 仓内无 benchmark 模块 | 无 |
| `sylirre/ghostty-android-terminal` | HWUI `Canvas`（`TerminalView` 继承 `View`，`canvas.drawText` 逐 run 绘制） | 仓内无 benchmark 模块 | 无 |
| 本项目 | wgpu 直绘 SurfaceView（终端内容不经 HWUI） | `android/benchmark` 宏基准模块（含系统侧逐图层采集 `SurfaceLayerFrames`）与 `native/benches` 微基准 | 无（模拟器数据只作数量级） |

两者的终端内容帧都经 HWUI 管线，本项目的终端内容帧由原生线程直绘 SurfaceView，故端到端帧率
不具跨应用可比性；跨应用帧面是系统侧帧面且分三层——逐图层 `dumpsys SurfaceFlinger --latency`
给逐帧三列（精确名匹配任何承载内容帧的图层，含本项目终端 SurfaceView 层），显示级
`dumpsys SurfaceFlinger --frametimeline` 给 display frame 级时序并按 `Owner Pid` 归属到内容
图层（两通道各有保留窗口，长突发场景的帧数须取应用侧计数），见 `performance-measurement` 的「跨应用性能对比的对照面与统一帧面」「跨应用性能对比
的帧面分层与覆盖面」「跨应用性能对比的图层名与图层覆盖判据」「跨应用性能对比的图层名在两类命令通道下的转义判据」「终端内容直绘 SurfaceView 时的帧面取数」「SurfaceView 内容帧的 jank 判定与 N/A 兜底」「BLAST 内容帧的期望呈现时刻不作迟到判据」各条。两者既无基准资产，也未
发布可核实的性能数字，故对比必须自建 harness，MUST NOT 引用其自述性能数字。

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

### Requirement: 复查的否定结论不得用未经验证的概括

台账中「其断言是自身期望值」这类判断 MUST 以该项目的实际断言形态统计为依据，
MUST NOT 仅凭测试基类的存在即整包判定。

#### Scenario: 整包判定被实测推翻时必须更正

- **WHEN** 实测显示某项目的断言主体与其被判定的形态不符
- **THEN** MUST 更正既有台账行并写明更正前后的统计数字，
      MUST NOT 在保留旧结论的同时只补充新依据

#### Scenario: 台账正文 MUST 写在 Requirement 块内

- **WHEN** 向 openspec 提交规范 delta 并归档
- **THEN** 台账等正文 MUST 位于某个 `### Requirement` 块之内，MUST NOT 放在与
      `## ADDED Requirements` 同级的标题下——归档只同步 Requirement 块内正文，
      同级标题下的内容会被静默丢弃且校验仍然通过
- **AND** 归档后 MUST 核对目标规范文件确实含有所写入的条目数，MUST NOT 仅凭
      `openspec validate` 与 markdownlint 通过即认定内容已落地

---

### Requirement: 六大上游对标结论按「能力 → 本仓现状 → 差距 → 阻塞层」登记

对标对象固定为六个：`sylirre/ghostty-android-terminal`、`termux/termux-app`、
`mrndstvndv/termux-ghostty`、`connectbot/termlib`、`libtermux/libtermux-android`、
`Uzaaft/libghostty-rs`（后者以 `Cargo.toml` 所钉 rev 为准）。每项 MUST 记录上游的
**实测做法**（文件与符号级），本仓现状，差距，以及差距可修的层；MUST NOT 以
「上游更好」这类概括代替定位。

#### Scenario: 复查结论可逐项复核

- **WHEN** 读到某项对标结论
- **THEN** 该项的上游做法、本仓现状、差距与阻塞层都能在表内读到，不需重新克隆上游

2026-10-12 复查台账（上游均取当日 `main`/master 浅克隆）：

| 能力 | 上游实测做法 | 本仓现状 | 差距 | 阻塞层 |
| --- | --- | --- | --- | --- |
| 全文搜索实现 | `ghostty-android-terminal` 在 `app/src/main/cpp/terminal_jni.c` 自研：`run_search` 全屏扫描、`search_cs` 保留区分大小写、`case_fold.h` 生成 BMP 折叠表、`search_dirty` 触发重扫、`MAX_MATCHES` 封顶；其注释写明「libghostty-vt has no built-in search」（所钉版本早于上游引入 search） | 走上游 `search::Search`（本规范「采用（2026 改判）」行） | 区分大小写：上游仅 ASCII 折叠、无开关 | 上游（`SearchOption` 只有 NEEDLE/SELECT_*，需上游新增才可能跟进） |
| 搜索命中高亮 | 同上：当前命中装为终端选区（`search reuses the selection slot`），高亮走选区通道 | 独立 `render_state.search_highlights` 通道，由原生按上游命中直接生成 | 多一条高亮通道 | 本仓（可选：复用选区槽可再删通道，代价是命中与选区互斥） |
| 搜索导航上下文 | 同上：导航从 Java 侧重跑扫描，陈旧屏幕坐标不需跟踪引用 | 导航走上游 `select_*`，视图侧把命中行居中 | 无 | 无 |
| 格子数据跨语言传输 | `termux-ghostty` 把脏行序列化为直连 JNI `ByteBuffer`（UTF-16 长度/折行标志/连续 cell 起点长度宽度/u64 样式/UTF-16 字符，8 字节对齐） | 网格数据全程在 Rust 内（最新帧邮箱 + 脏跟踪），不经 JNI | 不适用 | 无（我们的路径本就不跨 JNI，该优化无对象） |
| 修饰键栏 | `termux-app` 的 `termux-shared/.../extrakeys/`（`ExtraKeysInfo`/`ExtraKeysView`/`ExtraKeysConstants`/`SpecialButton`）含布局编辑器与按键配置 | 仅采纳默认布局与粘滞语义 | 布局编辑器 | PROHIBITED（明禁） |
| 选区拖动手柄 | `termlib` 的 `applyHandleDrag` 锚点语义 + 交叉翻转 | 已采纳（同源实现与单测） | 无 | 无 |
| Linux 用户态 SDK | `libtermux-android` 提供 `core`/`os`/`terminal-view`/`bootstrap-*`/`shizuku` 模块（README 自述实验阶段） | 自持 bootstrap 安装与终端视图 | Shizuku 集成 | PROHIBITED（明禁）；其余能力已由本仓自实现覆盖 |
| 其余 libghostty-rs 能力 | `gesture`/`select_output`/`format_selection_*`/`snapshot decoder`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report`/`device_attributes`/`xtversion`/`color_scheme` 回调/`set_terminfo_name`/`track_grid_ref`/`width_px`/`height_px` | 均未采纳 | 无消费者 | 本仓（按需取用，见「上游 API 取用判定登记」表；`report::ColorScheme::encode` 已采用见同名行） |
| 桌面通知回调 | `terminal.rs:3475` 的 `on_desktop_notification` 经 C ABI 的 `GhosttyTerminalDesktopNotification` 回调暴露；上游 `osc9.zig` 与 `rxvt_extension.zig` 分别把 OSC 9 与 OSC 777 的 `notify` 归到同一条 `show_desktop_notification` 命令 | 已采纳（2026-11-05 改判）：注册上游回调，通知按会话归属投递，前台应用内提示、后台系统通知、点击切换会话 | 无 | 无 |
| Kitty 图像 PNG 解码 | `kitty/graphics.rs:997` 的 `RustPngDecoder`（`pub const fn new`）经 `set_png_decoder` 注册；解码前先按 libghostty 分配器取定长缓冲，未受该上限约束的尺寸不会被 PNG 头放大 | 已采纳（本次改判）：删除本仓 60 行等价实现与 `png` 直接依赖 | 无 | 无 |
| 颜色语法与深浅判定 | `style.rs:390` `RgbColor::parse`（3/6/9/12 位 hex、X11 名、`rgb:`/`rgbi:`）与 `style.rs:415` `perceived_luminance`（文档写明「背景亮度超过 0.5 即视为浅色」） | 已采纳：主题色值与深浅分组都走上游，本仓不再手写 `#RGB` 展开与 Rec.601 公式 | 无 | 无 |
| 终端模式查询 | `terminal.rs:1380` `Mode::BRACKETED_PASTE`、`terminal.rs:1367` `Mode::FOCUS_EVENT` 具名常量 | 已采纳：`Query::ModeGet` 携带上游 `Mode`，删掉 `(u16, u8)` 裸号与各调用点的数字 | 无 | 无 |
| `fmt::Formatter` 全量纯文本 | `fmt.rs` `Formatter::new(terminal)` 文档写明只覆盖「a terminal's **active screen**」，只覆盖活动屏 | 不采纳：本仓的 `ReadAllText` 覆盖回滚 + 视口（`scrollback_len + grid_rows`），格式化器取不到已滚出的行 | 逐格 FFI 扫描（每格 `grid_ref`→`cell`→`codepoint`）在回滚填满时单趟可越过查询超时，靠 `all_text` 缓存兜住 | 上游（格式化器需补回滚范围参数） |
| `on_clipboard_read` 同步回调 | `terminal.rs:2754` 回调文档：应答须在回调返回前完成，回调期间 VT 流暂停；不装回调则 OSC 52 读取被忽略 | 不采纳：应答链是「VT 线程 → 会话队列 → `pollEvent` → Kotlin 读系统剪贴板 → JNI 回写 PTY」的异步链，改用回调须让 VT 线程阻塞等待 Android 侧剪贴板应答 | 读取请求的识别自研逐字节状态机（`output_processor.rs`），应答由本仓拼 OSC 52 帧 + base64 | 本仓（架构取舍：宁可不走上游回调，也不冻结 VT 线程） |

### Requirement: 语料判定补齐行

下表补齐既有「已复查结论台账」未覆盖的语料资产。既有表中的行继续有效。
本表每行 MUST 写清资产、判定与可复核依据；复查中发现未登记的语料资产 MUST 补行，
MUST NOT 以概括表述代替逐项判定。

#### Scenario: 语料资产逐项判定

- **WHEN** 判定某上游语料资产是否采纳
- **THEN** MUST 按下表给出判定与可复核依据，采纳规模 MUST 以仍可读取的输入为准

| 资产 | 判定 | 可复核依据 |
| --- | --- | --- |
| ghostty `corpus/stream-initial` 01–24 | **采纳后撤回**（不测上游） | 曾落地为 `native/tests/external_benchmarks/upstream_corpus.rs`（按发布标签取数、清单按名排序并断条数）；后随外部基准整体删除——期望由上游语义决定，通过只说明上游未变。日后复查 MUST NOT 重建，见「外部基准与 BDD 套件的退场」行 |
| migueldeicaza/SwiftTerm 175 个 Swift 测试文件 | 不采纳（门槛 1） | 期望值是其自研 VT 实现的输出；与 Ghostty 的 fuzz 语料对照思路曾由上条落地、后随外部基准撤回 |
| techsenger/jeditermfx `src/test/resources/testData` before/after 对与 vttest drift | 不采纳（门槛 1） | JediTerm 为第三方 VT，期望文件即其自身输出；与 `JetBrains/jediterm` 的 vttest 语料同口径 |
| pg83-shitty `tst/`（自述 >5000 例）及其收获清单（esctest/xterm vttest/libvterm 等） | 不采纳（门槛 1 + 穷举扩张禁止） | lib/vterm 为第三方自研 VT，黑盒快照属自验证；esctest/vttest 全量维持「不扩张成穷举 VT 一致」（terminal-state-regression） |
| not-elm/orzma_vt `tests/` | 不采纳（门槛 1 + 3） | 第三方 VT 差分，非上游资产 |
| paulrobello/par-term 与 par-term-emu-core 的 `tests`/`fuzz` | 不采纳（门槛 1 + 3） | 同上，第三方 VT 差分；其 fuzz 语料非上游资产 |
| midasdf/zt `/tests` sequence 输入 | 不采纳（门槛 1 + 3） | 第三方 Zig VT 差分 |
| lmorg/ttyphoon Go VT 测试 | 不采纳（测依赖行为） | 本仓 VT 语义不由自研测试覆盖（上游 VT 语义不由本仓测试覆盖 Requirement） |
| nihiL7331/sfte fuzz 宏 | 不采纳（无可评估资产） | 仓内仅 fuzz 宏定义，无语料 |
| veops/go-ansiterm integration tests | 不采纳（门槛 3 + 未编目） | 报告自述断言未编目；Go 库无 Android 落点，无补目必要 |
| nowledge-co/con `con-test` conformance harness | 不采纳（门槛 1） | 期望值为其自身实现输出；报告自述未复核 |
| z-hunter/Scanline-term vitest | 不采纳（无 VT 资产） | 文档自陈无 VT conformance suite、无测试矩阵 |
| lihop/godot-xterm `test/scenes`、asciicast 样例、`colortest.sh` | 不采纳（PROHIBITED + 人工目检） | asciicast 录制回放撞会话持久化禁止；`colortest.sh` 属人工目检；报告自述可执行命令未实际运行 |
| sanohiro/bcon 的 vte crate「for test/shimmer」 | 不采纳（门槛 1 + 3） | 与第三方 VT 差分同口径（qwertty 语料先例） |
| benyblack/ntilde gap matrix / conformance report CI 比对 | 仅参照（不引入） | 与 Ghostty 的差异矩阵思路可对照；语料本身是 .NET VT 自身输出 |
| alacritty/termbenchbot 指标维度（cursor_motion/dense_cells/scrolling/unicode） | 仅参照（不引入） | 性能方法论对照；本仓基准在 `native/benches`，设备侧宏基准见本节能力行 |
| KDE/konsole `tests/`、`autotests/`（emulation/screen） | 不采纳（门槛 1） | 期望值取自其自研 Vt102Emulation，报告自陈「需外部真相审计」；与 contour 行同口径 |
| Kuddev/pebrel `nebula_terminal/tests` 录制回放（`.recording` + expected grid JSON） | 不采纳（门槛 1 + 3） | 期望网格为其自身实现产物；其 release profile 借鉴已落地 `native/Cargo.toml` |
| kmscon/libtsm `test/` 单元与验收套件 | 不采纳（门槛 1 + 3） | libtsm 为可替换 VT 核（违单一来源），期望即其自身行为；「与 Ghostty 快照对照 harness」建议不引入（上游 fuzz 已覆盖） |
| ketraterm/KetraTerm 402 个测试文件与 JMH 基准 | 不采纳（门槛 1 + 3） | JVM/Swing 无 Android 落点；`tools/xterm-oracle` 为差异对照工具、非本仓语料，改写为 JNI 调用方可用 |
| mintty `testbidi.tc` 与生成脚本 | 不采纳（无消费者） | RTL/BiDi 未声明不立项（见能力裁定补齐行），宽度用例非本仓语料 |
| sudo-tee/hollow `src/lua/tests`、`src/mux/tests.zig`、bench 脚本 | 不采纳（门槛 3） | Emacs 插件与 Zig mux 侧断言，非 VT 语义语料；无 ConPTY 集成测试 |
| tuzig/terminal7 Vitest 与 Playwright e2e | 不采纳（门槛 3） | Web/xterm.js 集成测试，非 VT conformance |
| vercel-labs/wterm Vitest 矩阵与 Playwright e2e | 不采纳（门槛 3） | 覆盖 WASM bridge／DOM 渲染／WS transport，均 Web 形态，无 Android 落点 |
| wilsonglasser/oryxis crate tests（含 alacritty VT 套件） | 不采纳（门槛 1 + 未编目） | 断言范围未展开；第三方 VT 差分，口径同 go-ansiterm |
| kshivang/BossTerm「additional oracle」测试 | 不采纳（门槛 1） | jEditerm 派生 VT 的自身输出；另有 LGPL-3.0 许可审查前置 |
| felixse/FluentTerminal 的 `AdventureTime.itermcolors` 配色样例 | 不采纳（无落点） | iTerm 主题导入撞 `PROHIBITED.md` 设置节「自定义终端主题」；主题列表固定且构建期读取（`theme-configuration`） |
| dakra/ghostel 的 `test/hypothesis` 渲染 JSON corpora 与 `src/tests.zig` | 不采纳（门槛 1 + 3） | 期望值出自其自身渲染投影，断言对象为 Emacs 模块组件 |
| TragicWarrior/libvtty 的 `test/*.suite` 与 `.reference` | 不采纳（门槛 1 + 3） | libvterm fork 的黑盒参考输出即第三方 VT 自身行为（与 kmscon/libtsm 行同口径） |
| ghreprimand/odytty 的 native GPU tests、`fuzz/`、`benches` | 不采纳（门槛 1 + 3） | 自研 VT 的自身期望；GPL-3.0-only 仅作许可前置事实，不作判定依据 |
| fyne-io/terminal 的 `test_data` 与 `*_test.go` | 不采纳（门槛 1 + 3） | Go 自研解析器的期望；Go 模块无 Android Rust 落点 |
| godot-pty/gpty 的 crates tests 与 Godot 集成测试说明 | 不采纳（门槛 1 + 3） | alacritty 第三方 VT 差分；GPL-3.0-only 前置 |
| elementary/terminal 的 `src/tests`（Application、UnsafePasteTest 两例） | 不采纳（无可评估资产） | 仅应用启动与不安全粘贴对话框用例，后者撞 `PROHIBITED.md`「粘贴确认对话框」 |
| borisfaure/terminology 的 `tests/`（color、cursor shape、link detection、selection、OSC、DSR、tbc、lf margins） | 不采纳（门槛 1 + 未编目） | 报告自陈回归脚本「reference xterm behavior, can be adapted with expected diff」——期望改写属待做而非现成资产，判定言人眼读 diff；与 contour 行同口径 |
| PaNDa2code/zerotty 的 `tests/` shell tests | 不采纳（门槛 3 + 未编目） | 报告只记「shell tests」未展开断言；自研 VT 的 Zig 实现差异，无 Android 落点 |
| TermoraDev/termora 的 `src/test`（25 个测试）与 `issue-564`／`issue-1055` 回归数据 | 不采纳（门槛 1 + 3） | 断言对象为其自研 KTerm；回归数据属其自身 VT 语料；ssh/transfer 类用例依赖 SSHD mock |
| Yazelix/mars 的 `specs/` 计分卡、benchmarks 与 alacritty cell grid／Rio 渲染测试 | 不采纳（门槛 1 + 3） | Rio/alacritty 血统的第三方 VT 期望；benchmark 与 spec 用例须在目标设备重算，且其性能门禁为桌面口径（`29` 篇已裁定 spike 不做） |
| isene/glass 的 `test` 目录与内嵌 emoji 缓存 | 不采纳（无可评估资产） | 报告只记目录存在，未列断言；x86_64 汇编形态无 Android 落点 |
| cutefishos/terminal 的测试 | 不采纳（无可评估资产） | 报告自陈「None」（无测试数据） |
| hugopl/batata 的 `spec/` Crystal specs | 不采纳（门槛 3 + 未确认） | 报告自陈「whether they execute in a current runtime was not confirmed here」；GTK/VTE 形态无 Android 落点 |
| rricharz/Tek4010 的 `tektests` 与动画样例 | 不采纳（无落点） | Tek 协议未声明（Ghostty 无该协议），其语料属该协议专属 |

#### Scenario: 未登记的语料资产补行

- **WHEN** 复查中发现某仓的测试语料未在本表与既有「已复查结论台账」内
- **THEN** MUST 补行写明判定（含门槛号）与可复核依据，MUST NOT 以概括表述代替

### Requirement: 能力裁定补齐行

研究报告中出现、此前未逐条裁定的功能面，判定如下；「未声明」指 `DESIGN.md`
无对应功能声明，按 `STYLE.md` 不实现。判「PROHIBITED」时 MUST 引 `PROHIBITED.md`
条目，MUST NOT 以「未声明」笼统代替；错裁 MUST 按事实更正并保留更正前后结论。

#### Scenario: 功能面逐条裁定

- **WHEN** 生态扫描命中某个功能面
- **THEN** MUST 按下表给出出处与判定；`DESIGN.md` 未声明者 MUST 记为不立项

| 功能面 | 出处 | 判定 |
| --- | --- | --- |
| Model Context Protocol | godot-pty-gpty、lumina、ntilde、par-term、par-term-emu-core-rust、ratty、Scanline-term | **PROHIBITED 明禁**（`PROHIBITED.md` 终端节「Model Context Protocol」）；口径更正：先前记「未声明」属错裁，按 BiDi 更正同例改判；出处更正：先前引 copad、Lume 属误引（两报告无 MCP），改引实测命中者，同时补入 gpty |
| 插件系统、事件总线、mux daemon | ratty、par-term、copad、hollow | 未声明，不立项；出处更正：先前引 Scanline-term 属误引（该报告无插件/mux 内容），已按实测命中移除 |
| 无障碍/文本捕获（`readText` API） | vercel-labs/wterm | `DESIGN.md` 无无障碍面，**未声明，不立项**；朗读形态另撞 `PROHIBITED.md` 终端节「无障碍朗读」；口径更正：先前整行记「PROHIBITED 明禁」系以朗读条目覆盖整个无障碍面，按报告实测（仅可访问性/文本捕获）更正 |
| WebSocket/网络终端形态（服务端拆分、DOM 单元布局、Electron/Qt 集成、WebView 远程终端） | DomTerm、terminal7、wterm、copad（`copad-mobile`）、ThothTerm（LAN 模式） | 未声明，不立项（本仓无网络终端；wterm 的重连与背压设计仅在日后开网络终端时参考） |
| 分屏／平铺面板 | batata、godot-pty-gpty、marc2332-term、par-term、pebrel | 未声明，不立项；出处更正：先前引 BossTerm、maui-station 属误引（两报告的 `split` 分别指字形拆分宽度与 Service/UI 分层，非分屏），已替换为实测命中者 |
| git worktree 侧栏／集成 | marc2332-term、copad | 未声明，不立项（`DESIGN.md` 无 git 集成面） |
| ZMODEM/tysend 类文件传输 | terminology、konsole、termora、oryxis | 未声明，不立项 |
| 命令面板／shell 补全 overlay | Lume、lumina、par-term | 未声明，不立项 |
| 远程会话（设备配对、隧道、移动端控制） | Lume | 未声明，不立项 |
| AI 助手集成与 ACP/crash 分诊 | Lume、par-term、ttyphoon、zashterminal | 未声明，不立项（MCP 通道另见上表明禁行）；出处更正：lumina 报告只有 MCP server、无 AI 助手面，已移出 |
| vi-like 选择模式／可配置按键绑定 | Bobcat、pebrel | 未声明，不立项（「实体键盘快捷键设置」另有 PROHIBITED 明禁） |
| 文件管理器侧栏、快捷提示、语法高亮 | zashterminal | 未声明，不立项 |
| 结构化输出块／可点击树部件 | ttyphoon | 未声明，不立项 |
| 第三方渲染实现（cell 绘制、damage 驱动刷新、字形回退与 glyph cache、渲染帧租约、cell 批量上传） | iced_term、shitty、KetraTerm、godot-xterm | 不采纳：本仓字形图集 + 行级脏跟踪 + 渲染线程帧所有权已有等价实现（`native/src/render/`），代码不移植 |
| Terminal API 作 Compose contract／selection 映射 ViewModel | godot-xterm | 不采纳：本仓 `host-linkage` 的 JNI 边界与 `text-selection` 已有等价模型 |
| 会话恢复/持久化、配置保险库、daemon 常驻会话 | Scanline-term、marc2332-term、wilsonglasser、contour（独立 daemon 持久会话） | PROHIBITED 明禁（`PROHIBITED.md` 终端节「会话数据持久化 / 恢复」）；出处更正：copad 报告实为 AI 会话 detach/resume（运行时编排，非数据持久化），已移出本行 |
| CRT/scanline 视觉特效 | Scanline-term | 未声明，不立项 |
| 输出分块与命令状态着色（OSC 133 语义） | hugomyb/Lume | 上游 OSC 133 解析已登记；本仓不消费，DESIGN 未声明 prompt 标记语义；出处更正：先前引 ttyphoon、terminator 属误引（两报告无 OSC 133），ttyphoon 的输出块/退出状态着色归下方「结构化输出块」行 |
| DBus 远程控制 | qterminal | 未声明（Android 无对应形态），不立项 |
| 界面多语言/i18n 组织 | qterminal | 本仓界面仅简体中文（`STYLE.md`），不做 i18n；其余语言支持未声明，不立项 |
| 用户自定义主题覆盖与优先级链（用户文件 > 自动取色 > 默认） | termux-monet | **PROHIBITED 明禁**（`PROHIBITED.md` 设置节「自定义终端主题」）；本仓主题列表固定、构建期读取（`theme-configuration`） |
| RTL/BiDi 文本整形 | RtlTerminal、ThothTerm | **未声明，不立项**；口径更正：`PROHIBITED.md` 无 BiDi 条目，「已禁止」的表述无依据，正确口径是未声明。2026-11-02 复查：ThothTerm 已在 `PaintRenderer` 内实现 `BidiLayout`／`BidiRun` 与整段 RTL 整形（逻辑列↔视觉列映射见 `TranscriptScreen`），其做法为 Canvas 按方向段绘制并依赖平台整形；本仓渲染为 wgpu 单元格实例化（`native/src/render/cell_builder.rs`），`DESIGN.md` 明令「不做跨单元连字」，阿拉伯文字连接天然跨单元格，采纳即改渲染架构并违背既有声明，上游 Ghostty 亦无 BiDi。**不得反向采纳**其整段整形改造 |
| 界面 layoutDirection RTL | RtlTerminal | 未声明，不立项 |
| copy-on-selection（松手即复制） | godot-xterm | 不采纳：选择交互以菜单点击复制为唯一入口（`text-selection`） |
| bell 冷却/节流 | godot-xterm | 不另立一面：响铃已在 `runtime/BellPlayer.kt` 内按上游 `BellHandler.MIN_PAUSE` 同理由限频（`termux-configuration`「bell-character 的三态执行」），上游的独立冷却面无第二消费者，不重复实现 |
| Tektronix 4010/4014 | rricharz-Tek4010 | 未声明，不立项（Ghostty 无该协议） |
| 5250 主帧协议 | tn5250j | 不适用（非 ANSI/xterm 域） |
| O(1) row-map 滚动、内嵌 59k Nerd Font | midasdf/zt | 不需要/未声明：滚动归上游；内嵌字体与「按需加载宿主字体」的字体路线相悖 |
| 单帧处理 deadline/quantum、edge-triggered 唤醒 | missdeer-mostty | 不立项：本仓读链已有界通道（`OUTPUT_CHANNEL_BOUND` × 8KB）封顶，宿主轮询 `new_output`（`AtomicBool`，经 `take_new_output` 读取并清除）已实现对等语义；帧预算属过度设计嫌疑 |
| 「同引擎对标仓」的 PTY→UI 交接模式 | missdeer-mostty | 仅审计参照（本仓已有等价物）；该报告被「自研 VT 全部禁止」群体口径错裁，按事实更正——它与本仓同为 libghostty 唯一 VT 核 |
| Zig libghostty wrapper 的 callback 生命周期模式 | sudo-tee-hollow | 仅审计参照：本仓 wrapper 已有等价脏跟踪与状态转换实现；代码移植被许可未确认阻断，仅模式对照 |
| type-ahead 输入 | techsenger-jeditermfx | 未声明，不立项（`InputBatchBuffer` 已保序，与 JetBrains/jediterm 同口径） |
| IME 组字内外删除分流、组字光标模式 | anoterm | 仅参照（本仓已覆盖）：删除与改写面归 `ime-editor-contract`「替换类编辑必须到达终端」「已提交词的改写经重锚送达终端」，光标锚点归「组字光标位置回报给输入法」，无新增引入 |
| Unicode 17 graphemes/跨单元连字 | pg83-shitty | DESIGN 明示不做跨单元连字（单码位整形与复杂文种叠加除外） |
| alacritty_terminal 作替代 VT 核 | pop-os-cosmic-term | 不采纳（终端状态单一来源） |
| 设备性能自动化门禁 | nihiL7331-sfte | 登记现状：设备侧宏基准已存在并接入 CI（`scripts/test-emulator.nu` 调 `:benchmark:connectedBenchmarkReleaseAndroidTest`，`build.yml` 的模拟器步骤触发），缺的是阈值判定——基准只输出指标、阈值需真机标定（CI 为 swiftshader 软件渲染，帧率不作回归判据）；阈值自动化留待维护者裁定 |
| CI 产物哈希/二进制一致性比对 | vercel-labs-wterm | 登记待维护者裁定：`.github/` 属禁改区，本轮不改 |

#### Scenario: 功能面判定可溯源

- **WHEN** 读到某个功能面的判定
- **THEN** MUST 能在本表读到出处报告与依据（`DESIGN.md` 未声明、`PROHIBITED.md` 条目，或本仓既有实现）

### Requirement: 参照面与反面做法收敛补齐行

编号文档集（`00`–`30`）已从工作区删除，其参照面与反面做法 MUST 在本规范各表有可读去向；
已退役的 `docs/research/ecosystem.md` 与逐仓报告的参照面／反面做法同此要求。
反面做法 MUST 点名出处仓库与做法并标注「不得反向采纳」或「不采纳」，
MUST NOT 要求读者恢复已删除的研究报告才能复核。

#### Scenario: 反面做法点名与归属

- **WHEN** 评审参考实现的具体做法
- **THEN** MUST 按表点名出处并标明可参照或不得反向采纳

| 项 | 出处 | 判定与依据 |
| --- | --- | --- |
| PTY 输出合并投递与 UTF-8 粘包保留解码 | puppy、anoterm | 仅参照：本仓会话与输出管道在 Rust，字节直传上游 VT stream，无自研粘包解码；跨块半扫由 `output_processor.rs` 用例覆盖 |
| 迁移文档的问题清单组织 | termux-kotlin-app | 仅参照（文档思路，无代码落点）：本仓文档面为 `openspec/` 与 `docs/specification/`；其「保留上游测试套件」不是正例——单测仅 2 文件约 18 例，可采纳断言仅键位编码与鼠标两类（主台账 `reapercanuk39/termux-kotlin-app` 行） |
| 自造 `svc #0` syscall 做 JNI 桥 | on3-terminal | **不得反向采纳**：绑定面走标准 JNI／libc 调用，自造 syscall 无跨设备可移植性 |
| `curl \| sh` 式引导安装 | shafinthedev/Ply（`src/main.rs:102` 的 `curl -s {} \| sh`，包管理安装流程） | **不得反向采纳**：引导安装按 `bootstrap-install` 固定流程执行，不把远端脚本接进 shell 管道 |
| 渲染热路径内的 `measureText` | puppy | **不得反向采纳**：与既有台账行「在渲染热路径内改写终端状态」同口径，文本度量不得进每帧路径；其构建产物入库一并计入既有台账「仓库内提交 CMake 生成物与构建产物」行 |
| 治理文档膨胀（「文档海」） | termux-app-rafacodephi、termux-kotlin-app | **不得反向采纳**：文档面收敛为 `openspec/` 与 `docs/specification/`，研究过程记录不常驻 |
| 整包完整性校验（BLAKE3 与 sha256 双实现） | termux-app-rafacodephi（`BootstrapIntegrityVerifier` 走 BLAKE3；loader `BootstrapChecksumValidator` 走 sha256） | 不采纳：`DESIGN.md` 要求 bootstrap 不做 SHA-256 或其他完整性校验，`PROHIBITED.md` 另禁 sha256 独立校验文件；整包校验两种形态均不落地 |
| 删减式回退（缺源文件、删 Sixel 分支、去 API 版本守卫） | Autonomous-Agentic-Workflows/termux-app | **不得反向采纳**：跟 master 并保留上游能力；既有实测台账已记「官方的弱化子集（缺六个源文件）」 |
| JNI 字符串释放错对象（`ReleaseStringUTFChars`） | Autonomous-Agentic-Workflows/termux-app、Rprop/AnsolePlus、reapercanuk39/termux-kotlin-app（三仓 `termux.c:164-165` 同为 `(cmd, cmd_cwd)`；上游为 `(cwd, cmd_cwd)`） | **不得反向采纳**：JNI 字符串与数组释放 MUST 对象成对，错位即漏释放加错释放 |
| 选择空间以负行号寻址回滚区的心智模型 | CocaKova/charon（`TextSelection.kt` 行号 `coerceIn(-scrollbackSize, rows-1)` 经 `ScreenBuffer.relativeLine` 解析） | 仅参照：本仓选择空间与回滚寻址归上游，`text-selection` 只声明可观测契约，不下沉自研；其 IME「全文本框镜像」不采纳——本仓口径是组字基线为输入法可见文本的唯一镜像（`ime-editor-contract`） |
| 常驻通知（显示会话数）与退出按钮 | termux/termux-app | 已落地为 `DESIGN.md` 软件节要求（`TerminalForegroundService.kt`），研究行仅为出处对照 |
| 备份「完成后替换 + 孤儿清理」 | GlobalTechInfo/RedTerm | 未声明，不立项：本仓无备份／恢复面（`DESIGN.md` 无声明）；其搜索高亮两层透明度归 `text-search` |
| 行尾孤立高代理项越界读（宽表缺 `isLowSurrogate`／长度守卫） | Rprop/AnsolePlus（与上游同位置未修）、Adhi-hub07/on3-terminal（宽表缺 `isLowSurrogate` 校验） | **不得反向采纳**：`cjk-rendering`「行尾孤立代理项不越界」要求长度守卫，宽度判定 MUST NOT 越界读取码元；rafacodephi 的真实修复为主台账正例行 |
| 以空 `catch` 吞掉全部异常 | irealashu/ApexShell、Sunbr99/TerminalOS 等无终端实现组 | **不得反向采纳**：与「JNI 边界不得静默截断或静默吞错」同口径，错误 MUST 可观测，不得静默降级 |
| 以 su／Shizuku 提权换取 shell 能力 | eggie-admin/hydra-shell-android 等提权组 | **不得反向采纳**：撞 `PROHIBITED.md` 设置节「Shizuku 集成开关」明禁；本仓不引入提权通道 |

#### Scenario: 反面做法可独立复核

- **WHEN** 复查编号集某仓的反面做法
- **THEN** MUST 在本表读到出处仓库、做法与「不得反向采纳／不采纳」判定，
      MUST NOT 需要恢复已删除的编号报告

### Requirement: 研究阶段残留项与覆盖缺口登记

退役研究报告（编号集 `28`／`29`／`30` 三篇）中**未闭合**的条目 MUST 在本表留有可读去向，
与已落地项分开登记：已落地者 MUST 给出代码或能力落点，未查清者 MUST 写明「未查清」并说明
为何不凭猜测处理，覆盖缺口 MUST 写明缺的是哪半边与替代判据。MUST NOT 把未查清或不可测的
条目写成已解决，也 MUST NOT 因「做不到」而删除条目。

#### Scenario: 未闭合条目的去向

- **WHEN** 审计遗留未闭合条目
- **THEN** MUST 按下表登记维持不处理的理由；覆盖缺口 MUST 显式记为缺口，MUST NOT 冒充已覆盖

| 项 | 判定与依据 |
| --- | --- |
| `get_capabilities` 在模拟器上失败的真因 | **未查清，维持不处理**：无模拟器反馈闭环，加重试属凭猜测（编号集 `28`）。代码侧已落地的是同根因的可见缺陷修复——`attach_surface` 只在重建**确实成功**后才清失效位与连续失败计数（否则失败路径替成功路径还愿，自愈阈值永远攒不满，渲染停摆，`native/src/render/context/mod.rs`）；该修复带 `#[cfg(target_os = "android")]`、宿主无 seam，判别力用例会被迫把被测逻辑抄进测试体（第一版恒通过），故**据实登记为覆盖缺口**，不冒充已覆盖 |
| 焦点上报写出的那半边 | **覆盖缺口，维持不冒充**：`focus_event_bytes` 写出需在宿主单测里等一次 VT 线程往返，并发跑全量测试时该耗时无上界，写成断言即不稳定用例（编号集 `28`）。可确定覆盖的半边已钉在 `key-input-encoding`「查询窗口宽于 VT 空闲 tick」；写出半边只保留结构性保证（`Session::focus_event_bytes` 直写子 PTY） |
| 37 项过度复杂特性嫌疑 | **仅记录，不立即简化**（编号集 `28`）：含高风险六项（上游对标语料裁定流程、备用屏输入位移状态机、表面失效自愈限流、亚毫秒计时、渲染帧调度锁存群、字号上下限换算体系）。简化须单独立项并复现 `DESIGN.md` 保证，本轮不立项 |
| L2–L6 减码建议 | **有界重构建议（P1/P2），不扩大为变更**（编号集 `29`）：L1 批量合并已执行；红线不变——不减测试、不降覆盖、不弱化断言、不删 `DESIGN.md` 功能；想大幅减码只能改 `DESIGN.md` 收窄规格，属产品决策 |
| `sugaloaf`／`libsugaloaf` 承接 render／font 的 spike | **维持不做**（编号集 `29`）：须先有 lavapipe 基准且能复现 `DESIGN.md` 字体保证才动；`librio` 的 render-state pull API 与 staticlib 打包仅作设计参照 |
| esctest／vttest 真机精选子集 | **维持「不扩张成穷举 VT 一致」**（编号集 `30`）：全量一致套件不搬入本仓（违「不测试依赖行为」）；若引入只放真机 `connectedDebugAndroidTest` 且取精选子集。`esctest2` 另阻塞于绑定层缺 `DECRQCRA`（「已复查结论台账」esctest2 行） |
| 真实字体／bootstrap 数据扩面（原优先级 4） | **已由既有基准承担**：字体侧真度量由 `native/src/render/font/rasterization.rs` 的字体表基准（`cell_metrics_match_the_selected_face_own_tables` 按所选面自身表换算并核对格宽行高）覆盖，不再另立 hmtx 扩面；bootstrap 侧真实 zip 与装后可用性由设备侧用例覆盖；金图 OCR 属禁改区待维护者裁定（见「禁改区待维护者裁定项台账」） |
| debug 档 `cargo test` 链的是未优化 Zig 解析器 | **口径登记**（编号集 `30`）：上游 `libghostty-vt-sys` 构建脚本按 cargo 的 `DEBUG` 选 Zig `-Doptimize`，故 debug 档耗时结论 MUST NOT 外推 release；以大量组合符回放为前提的用例在 debug 档会顶穿 `flush` 上限（承载该上限的组合符回放用例已随外部基准删除，口径继续有效） |
| kitty 键盘协议 16 处上游与规范不一致 | **已逐条钉在用例内**（编号集 `30`）：方向键／Home／End／F1/F2/F4 在 kitty 模式下仍发 legacy 形式，媒体与音量键不发字节，只开消除歧义时 CapsLock/NumLock 不发字节；按「不把上游小瑕疵转为本仓测试失败」钉实测值并注明分歧，MUST NOT 按规范断言（承载用例已随外部基准删除，16 处分歧事实以本行为准） |
| CI connected 用例失败的归因口径 | **已更正为「一次一个确定根因」**（编号集 `28`）：初稿判「整体性随机不稳定」是错的——「每次用例都不同」不等于随机，逐次归因后每次都有一个确定根因，修好一个露出下一个。教训登记：判定「是否已修」MUST 核对修复提交时间与 CI 运行时间，MUST NOT 拿过期失败继续追查 |
| Kotlin 侧格式门禁的漏跑 | **教训登记**（编号集 `28`）：`scripts/check.nu` 含 `detekt spotlessCheck` 等 Kotlin 门禁，只跑 Rust 侧门禁会漏掉 Kotlin 侧格式违规；跨语言改动 MUST 跑整套门禁 |
| 报道指出但本仓已自行解决的项 | **已核销，不留待办**（编号集 `28`）：PTY 互斥量与无界写入争锁（现无互斥量）、`clipboard_text` 队列无上界（已封顶丢最旧）、VT 线程 resize 失败永久分歧（已有跨线程失败标记）、每次安装新建 `OkHttpClient`（已进程内缓存）、注释「500ms 防抖」漂移（现为具名常量 300ms）、`closeNotifyThread` 从不 `quitSafely`（复核为设计而非缺陷）；`InputBatchBuffer.close()` 已同批摘回调并复位排程标记 |
| 无可解析码位的负缓存推理 | **口径已更正**（编号集 `28`）：报告原话成立——本仓 `Pipeline::load_font_file`（`native/src/render/font/pipeline.rs`）的全部调用点都在测试模块内，生产无调用；易混的是 `font_db.rs` 中的 `font_database.load_font_file(...)`，那是 fontdb crate 的 `Database` 同名方法，非同一物。故负缓存放进 `clear_identity_caches` 后，其失效面与既有的 `cjk_glyph_cache` 完全相同，不存在「漏一处即字形消失」。用例钉两半（失败后入缓存、换字号后清空），各经一次变异验证 |
| kitty 图形上游测试载荷的退场 | **已退场，非遗漏**：20×15 与 128×96 上游载荷随「删除未发现本项目可解问题的上游测试资产」清理；现行 kitty 用例只保留 1×1 与内联 PNG。日后复查 MUST NOT 因「载荷不见了」重建它们 |
| 外部基准与 BDD 套件的退场 | **已退场，非遗漏**（维护者口径「不测上游，不写重复代码」）：以 UCD 宽度表／kitty 规范字节／ghostty fuzz 语料为期望的 `native/tests/external_benchmarks/`（9 例）与逐条复述既有单测的 `native/tests/bdd/`＋`native/tests/features/`（7 特性 12 场景）整体删除，`ureq`／`cucumber`／`tokio` 退出 dev-dependencies。前者期望由上游决定（判红只说明上游 rev 变动，实测分歧全落上游与终端惯例），后者零独占覆盖（唯一断言在 `native/src` 单测里已有且更强）。日后复查 MUST NOT 因「用例数变少」「Gherkin 叙述更可读」重建它们 |
| 外部基准与原计划的口径差异 | **登记在案**（编号集 `30`）：优先级 1 原写「进 BDD」，改为 Rust 集成测试目标（接缝相同，逐条代码化期望塞进 Gherkin 只多一层无收益的间接）；优先级 2 的「采样」二字作废（可打印码位全量比对）；优先级 3 改为「规范表当真值 + 分歧登记」（字节断言基本落在上游，本仓判据是增强请求是否到达编码器）；优先级 4／5 见上两行。三项落地后整体撤回，见「外部基准与 BDD 套件的退场」行 |

#### Scenario: 未查清项不被写成已解决

- **WHEN** 复查上表任一条目
- **THEN** MUST 读到它是「未查清」「覆盖缺口」还是「已核销」，MUST NOT 出现
      「无反馈闭环却声称已修」的表述

#### Scenario: 口径更正的旧结论被废弃

- **WHEN** 读到 CI 稳定性或 debug 档耗时的旧口径
- **THEN** MUST 以更正后的归因为准（一次一个确定根因、debug 结论不外推 release）
