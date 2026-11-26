# research-ledger Specification

## Purpose

研究文档的逐份处置结论登记在本能力内：每份研究报告的索引行（实质、判定与依据
位置）与逐份处置中补齐的可执行句去向都逐条留档，使「读全一份报告 → 判入去向 →
补规范或代码 → 删除该份」的过程可脱离总体叙述独立复核。落点可以指向其它能力

## Requirements

### Requirement: 研究报告逐份索引与文档可移除性

`docs/research` 的每份研究报告 MUST 在本规范内有一条**以报告文件名为键**的索引行，
写明该报告的实质、判定结果与依据位置（上游对标能力各表、其他 spec 的能力名、或已落地代码）。
索引行 MUST 能独立回答「该仓有无可用资产、结论是什么、为什么」，MUST NOT 以
「其余若干份已核对」这类群体概括收尾。

索引完整是本仓删除研究文档的**前置条件**：任取一份报告，其全部可执行项的去向
MUST 已在索引行内可读到；满足该条件后报告本体 MUST 可被删除而不丢失任何仍需遵守的结论。
全部 111 份报告（编号文档集 31 份 + 逐仓报告 73 份 + `ecosystem.md` + 6 份综合研究报告）
已整体删除，本仓自产的 `extensibility-research.md` 亦按同一协议登记后删除（第 112 份）；
索引表是这批研究的**唯一常驻载体**；6 份综合研究报告的索引行 MUST 承接
其全部仍需遵守的结论——文档在库时索引行是这些报告结论的登记处，文档删除后由同键索引行
承接，MUST NOT 因文档已删而让结论失去落点。

#### Scenario: 索引外的补充事实

- **WHEN** 引用生态扫描得出的判断
- **THEN** MUST 使用下文保留的事实，MUST NOT 引用已退役的研究报告原文

生态扫描中仍作为判断依据保留的事实：跨 25 仓扫描 `wgpu` 零命中、完整 GPU 终端渲染
路径仅本仓；Canvas 逐格体系在 CJK 性能上先天不足，Termux 与 ghostty 均已批注
「等宽 ASCII 的 advance 恰为 cellWidth」前提，本仓字形图集 + 整形正是该前提的职责边界；
与本仓同用 libghostty-vt 的对照组为 `mostty`／`hollow`／`con`／`ghostel`
四家壳（`ghostel` 为 Emacs 动态模块且已支持 Android/Termux）；
本仓无死代码可白捡（`allow(dead_code)`／`todo!`／`unimplemented!` 计 0），减码只能靠重构。
禁止项三条保留：不 vendor 第三方 VT 代码片段、不更换 VT 核、不等待完整 libghostty 支持 Android。
换核对照结论一并保留：`alacritty_terminal`／`wezterm-terminal` 体量可 rival 但违单一来源；
完整 `libghostty` 的 `ghostty_platform_e` 仅 `INVALID/MACOS/IOS`、对 Android 无收益；
`rio-vt` 未上 crates.io、须自 rio 仓库自建——在本仓确切约束（Android + Rust 宿主 + wgpu +
无头 + C ABI + kitty 图形 + 单一来源）下无「更完整又能直接换」者。
测试边界由 `terminal-state-regression` 与该能力「上游 VT 语义不由本仓测试覆盖」承担：
VT 解析归上游，上游权威 fuzz 为 `libghostty` 的 `test/fuzz-libghostty`（parser／stream／osc），
本仓只测接缝与契约，外部数据即基准、不入库。

原 `ecosystem.md` 的 S/A/B/C 参照面评级是研究期分流工具，其作用已由索引表的「判定」列
与各台账取代；表中依据列写作「生态参照面组行」的行共享同一事实依据（上段生态扫描
与 `DESIGN.md` 的未声明口径）：这些报告的形态或技术栈与本仓无交集，其可用内容止于
语言级或算法级对照，且已由本仓既有实现越过或撞明禁/未声明——各行「判定」列 MUST
写明该行自身不可采纳的具体理由，不得只以组名代替；不再单设评级表。

生态内其余仍需对照的结论：终端状态单一来源无第二家（「复用 Ghostty」三条路——Java + 预编译库、
自绑 Rust、复用 Termux 库——本仓取自绑 Rust 且唯一在 wgpu 上落地；「自研 VT」四条路全部是子集，
Termux 自身也被 fork 删过 `doDcs`／`sixel`）；会话不随 Activity 走是跨生态共识
（ghostty 进程单例、`ThothTerm` 前台 Service、本仓会话归 Rust），ghostty `attachSession`
的跨会话清理五项已由 `session-list` 登记；Web/WASM 系（即便直接封装 libghostty）与
桌面壳系（复用 xterm.js／QML）都把 VT 解析与渲染放在别处，参照面只剩产品面，而产品面
多属本仓明禁项或未声明项。

两处研究期悬问已实地闭合：`pg83/shitty` 许可为双轨——2023-07-11 导入的基线（含
`lib/vterm`）保持 GPL-3.0-or-later，此后一手贡献 MIT/GPL3 双选，本仓按自研 VT 与
不 vendor 双规则不引入代码；`orhun/ratty` 的 font/atlas 悬问——`src/` 全部 20 个条目
内无 font/atlas 模块，字体与图集随 Bevy 默认栈，原「本仓已越过该层」的判断维持。
字体表的真值对照保留在 `native/src/render/font/rasterization.rs` 与 `font_db.rs`
的宿主字体库用例（数据取自开发环境固定的 fontconfig 字体集），外部数据不入库。

#### Scenario: 索引覆盖到文件

- **WHEN** 遍历 `docs/research/` 曾有（现已整体退役）的任一份报告文件
- **THEN** 索引表内有以该文件名为键的行，且该行的判定与依据可在已点名的能力或代码位置读到

#### Scenario: 删除报告不丢结论

- **WHEN** 某份报告被删除（已退役的 105 份现已整体删除）
- **THEN** 其提出的可执行项去向（已落地代码位置、被否的门槛理由、未声明/PROHIBITED 判定）
      仍可在索引行与所指向的表内读到，MUST NOT 需要恢复该报告才能复核

#### Scenario: 重新引入报告须同批补索引

- **WHEN** 日后重新引入任何研究文档
- **THEN** 同一批改动 MUST 同时补上其索引行；缺少索引行的报告 MUST NOT 入库

#### Scenario: 索引不重开已裁定事项

- **WHEN** 为某报告补索引行
- **THEN** 该行 MUST 引用既有判定，MUST NOT 借索引重开已裁定的能力面或改写门槛

#### Scenario: 在库报告的登记与计数

- **WHEN** 研究报告以文件形式入库
- **THEN** 该报告 MUST 在同批改动内补上以文件名为键的索引行与补齐行，索引完整 MUST 仍是其日后删除的前置条件
- **AND** 计数 MUST 分列已退役与在库两份，已退役 112 份的索引表仍是那批研究的唯一常驻载体；在库 5 份为 `extensibility-and-remote-backends-2026.md`、`rex-protocol-support-2026.md`、`rust-dependency-survey-2026.md`、`terminal-performance-comparison-2026.md` 与 `warp-terminal-research-2026.md`

索引表如下。

| 报告 | 实质 | 判定 | 依据 |
| --- | --- | --- | --- |
| `ghostty-android-terminal.md` | 同引擎（libghostty-vt）Android 终端的集成模式深研：Java Canvas UI → JNI → 预编译 libghostty-vt.a → arm64chroot/arm64emu 四层架构 | 已核结，无可移植资产：同引擎集成印证四项（回调经 `feed()` 后拉取而非上推、winsize 带 `ws_xpixel`／`ws_ypixel`、选区经 `GHOSTTY_TERMINAL_OPT_SELECTION` 交引擎跟随、bidi 与歧义宽不支持）本仓已是同一形状；可参考点四项已落地（mksh SIGWINCH 不变量归 `startup-grid-resize`，grapheme 行级标志归 `CellData` 的 `grapheme_extra` 故无带外溢出缓冲层，编码前同步 setopt 归 `set_options_from_terminal`，写清为什么归 openspec 变更的 design.md）；不适用：arm64chroot/arm64emu 与 rootfs 资产（`DESIGN.md` 未声明用户态模拟器，`PROHIBITED.md` 明禁内嵌 proot 与预装发行版）、自建 native 搜索（`text-search` 已定上游 `search::Search` 单一来源）、OSC 52/9;4 side-scanner 与富输入 Editable 镜像（`DESIGN.md` 未要求）、Canvas 渲染全套（本仓 wgpu/Vulkan 直绘） | 本表 `01-sylirre-ghostty-android-terminal.md` 行；`text-search`；`startup-grid-resize`；`key-input-encoding`；`host-linkage` |
| `redterm.md` | proot 发行版终端 + SSH 客户端（复用 Termux 上游库、不自研引擎）的外围工程实践与防错模式 | 已核结，可参考资产只在工程实践层：四项参考点已落地或由既有规范承接（wake lock 自检重取归 `TerminalForegroundService`，启动路径单一事实源归 `ShellEnv` → `pty::base_env`，假对端测试归 `native/src/terminal/mock_pty.rs`，注释纪律归 `STYLE.md`「注释极简」）；不适用：`script` 会话录制（本仓 Rust 侧天然持有 PTY 读端且规格未要求）、AppLog 持久化日志（`DESIGN.md` 明禁不写日志文件）、259 项调色板纯函数（主题自带 16 色）、`SessionDescriptor` 身份与显示名分离（会话归 Rust 侧，`session-list` 已定「序号 + 标题」）、Choreographer 合帧 invalidate 与链接正则检测（渲染与链接识别在 Rust 侧）、发行版管理／proot／内嵌 OpenSSH／SFTP／文件管理器／备份还原（范围外或 `PROHIBITED.md` 明禁）、CPU% 通知与桌面小部件（通知栏只报会话数量与退出按钮）；已核实无缺口：16 KB 页对齐（`libnative.so` 的 LOAD 段对齐 0x4000）、通知诚实性、bootstrap 权限保真（`bootstrap-install` 走 zip 清单承担权限） | 本表 `GlobalTechInfo-RedTerm.md` 与 `06-GlobalTechInfo-RedTerm.md` 行；`bootstrap-install`；`shell-env`；`logcat-hygiene`；`session-list` |
| `reference-verification.md` | 参考项目核验文档：上游 A（ghostty-android-terminal）160 条仪器化测试与上游 B（termux-app）145+2 条 JVM 测试逐条对照本仓测试 | 已核结，结论为「本仓测试面已闭合」：上游 A 160 条中 80 条有等效用例、80 条对应能力被禁止或未声明、0 条部分对应，每条等效均给出双方精确测试名；上游 B 的 19 个测试文件按「直接锚点／引擎域／未声明」三分逐区核对，无「上游已测且规范允许而本仓未测」条目；按像素滚动的手势层／运行期层／FFI 与原生渲染层三层证据链闭合；文中五条前版误记（上游测试计数、`TerminalUiTest` 条数、`ExtraKeysConfigTest` 与 `ThemeModelTest` 归类、测试名归属）已按源码实读更正 | 本表 `01-sylirre-ghostty-android-terminal.md` 与 `02-termux-termux-app.md` 行；`scroll-gesture-settle`；`terminal-state-regression` |
| `terminal-emulators.md` | 终端模拟器横向研究报告：ghostty／alacritty／rio／kitty／wezterm／xterm／foot 七项目的渲染、解析、线程模型与输入路径 | 已核结，结论为「与本仓同族者仅 ghostty 与 rio，且本仓在这两点上同构」：同构印证五项（独立渲染线程 + 合帧唤醒、实例化渲染、部分写退回并保留写进度、引擎与宿主解耦、表驱动解析）本仓已是同一形状；可参考点五项已核结（锁让渡自查落在 `native/src/terminal` 零引用渲染状态、damage 语义清单落在当前与上一光标行标脏且滚动变化强制全量重绘、输入三档压测落在 `bulk_output_is_processed_without_loss` 与 `newest_frame_reaches_consumer_after_a_burst_of_pushes`、quirks 集中层落在既有 workaround 就地注释、xterm 仲裁基准落在 `terminal-state-regression`）；不采用：alacritty 列级 damage（本仓脏带以行为最小单位）、rio 多后端回退（`DESIGN.md` 明禁 CPU/OpenGL 回退）、kitty 时间性拒收（本仓已有有界排空 + 计数退回双背压）、foot box-drawing 几何绘制（规格未要求）、kitty SIMD 多宽度扫描（解析归 libghostty-vt）、rio／foot 主线程渲染（正是本仓独立渲染线程要规避的形状） | 本表 `29-减码杠杆与libghostty家族结论收敛.md` 行；`terminal-state-regression`；`performance-measurement`；`render-loop-scroll-cadence` |
| `termlib.md` | ConnectBot termlib（Compose 终端组件库，vendored libvterm）深研，含四层性能测量体系与 macrobenchmark 评估 | 已核结，结论为「macrobenchmark 有意义但边界明确」：`AppStartupBenchmark` 刚需、`InteractionAnimationBenchmark` 有效、终端内容渲染架构上测不到（Rust 线程 wgpu 直绘不产生主线程 doFrame）、CI 只编译不运行是对的（swiftshader 帧率不可作回归判据）；可参考点六项已核结（真机测量规程与合成负载取舍归 `performance-measurement`，增量一致性归 `cached_build_matches_full_build_byte_for_byte` 与 `resize_cycles_do_not_accumulate_stale_rows`，criterion 基准不进测试套件且 `check.nu` 走 `--quick`，本仓不建 JNI 全局引用故无所有权环）；不适用：`PackedCells`／`TerminalSnapshot` 等 Kotlin 网格结构（本仓设计上无 Kotlin 网格）、OSC 133 无障碍导航与 Review Mode（规格未要求且 `PROHIBITED.md` 明禁无障碍朗读）、Unicode 宽度表生成脚本（归 libghostty-vt）、Canvas 几何绘制与 bidi level-1 细节（归 wgpu 侧与 libghostty-vt）、Magnifier 选区（文本选择须遵循 Termux 设计）；**资料齐备性缺口（2026-11-21 实测）**：该报告已按索引协议删除，`docs/persistence/` 现存参考留档只有 `ghostty-android-terminal.md` 与 `termux-app.md` 两份，缺 `termlib.md`——按 `code-health`「设计文档明列的参考项目必须有研究留档」记为缺口；补齐落点属受保护目录且未获明确同意，登记为待办，补齐范围为分层方式、输入顺序、生成物复现校验与基准体系四类事实 | 本表 `30-VT测试边界与外部基准接入收敛.md` 行；`performance-measurement`；`ime-animation-smoothness`；`text-selection` |
| `termux-app.md` | termux-app（v0.119.0-beta.3）深研：四模块结构、终端会话三线程与双向队列、bootstrap 安装流程、修饰键栏三态、前台服务与测试基座 | 已核结，结论为「DESIGN 依 Termux 定规的条目本仓已对齐」：已对齐项逐条在位（修饰键栏布局与粘滞语义、2 行 7 列、回滚 2K 行、默认块状光标与可选样式、`TERM=xterm-256color` 与 `COLORTERM=truecolor`、bootstrap zip + `SYMLINKS.txt` + staging + 原子 rename、通知常驻「会话数量 + 申请唤醒锁 + 退出按钮」、applicationId `com.termux`、AOSP testkey 签名）；可参考点五项已核结（`KEYCODE_BREAK` → `key::Key::Pause` 补齐归 `key-input-encoding`，auto-repeat 阈值改取 `ViewConfiguration` 系统值归 `modifier-bar-auto-repeat`，双 client 生命周期归 Rust 侧会话归属与 LeakCanary 用例，主线程消息泵由渲染线程 latch 等价承担，`TerminalTestCase` 式直测由 `ghostty_terminal/tests/` 承担）；不适用：插件体系（`PROHIBITED.md` 明禁 Termux:API、termux-exec、Termux:Tasker、Termux:Boot，其余插件属 `DESIGN.md` 未声明不立项）、上游 `termux.properties` 的完整设置项体系（本仓只按 `termux-configuration` 与 `docs/specification/CONFIG.md` 解析声明的允许键；应用启动时读取一次，不解析 `colors.properties` 与 `termux.float.properties`）、F-Droid／GitHub 双分发签名管理（本仓单 testkey）、sharedUserId 与多 APK 签名兼容约束 | 本表 `02-termux-termux-app.md` 行；`key-input-encoding`；`modifier-bar-auto-repeat`；`bootstrap-install`；`session-list` |
| `8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 子集 + C PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；其 OSC 注释清单作生态参照面，OSC 52 安全姿态归 OSC 52 条款；其 `NativePty.c` 一类 C PTY 桥无落点——本仓 PTY 在 Rust 侧（`nix::pty`），仓内无 C 源；多击选词／OSC 8 白名单／提交构建产物三做法属不得反向采纳 | 「Android 侧终端项目的复查台账」terminator 行；「OSC 覆盖逐项核对」；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `DanteAlighierin-foot.md` | C/Wayland 自研 VT 极简终端 | 不采纳：仓内无 VT／网格语料（「无资产」而非门槛违反）；自研 C 解析器违单一来源，box-drawing／组合宽度只作结构对照 | 「已复查结论台账」foot 行 |
| `extensibility-research.md` | 本仓自产的可扩展性研究：终端引擎库化交付、会话传输接口扩展点、远程后端（Herdr／Rex）选型，附五个候选项目形态与上游扩展机制对比 | 已核结，全部不采纳：应用内插件运行时／脚本加载、Model Context Protocol、Termux:API 兼容层撞 `PROHIBITED.md` 明禁；库化拆分（拆 crate 与 Gradle library 模块）须改受保护 `Cargo.toml`／`settings.gradle.kts`／`build.gradle.kts`，未获用户明确同意不动；远程后端属产品决策且 `DESIGN.md` 未声明该功能面。报告本体按本 Requirement 的可移除性条款退役（索引行为唯一常驻载体） | `PROHIBITED.md`（插件体系、Model Context Protocol、Termux:API、内建命令）；`DESIGN.md`「不做不必要功能」「不实现未在 docs/specification/ 声明的功能/逻辑」；`STYLE.md`「不得定义任何 crate features」；AGENTS.md 受保护文件约束 |
| `GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 发行版运行器 | 不采纳（门槛 3）：仓内无本仓组件，329 例属 SSH/SFTP/proot 面；16KB 页对齐登记禁改区 | 台账不可采纳行；「禁改区待维护者裁定项台账」；参照面「搜索高亮两层透明度」归 `text-search`，备份「完成后替换 + 孤儿清理」归「参照面与反面做法收敛补齐行」 |
| `JetBrains-jediterm.md` | JVM/Swing 可嵌入终端库（自研 VT + Pty4J） | 不采纳（门槛 1）：before/after 期望即其自身输出；vttest 派生子集按输入分流逐例判定 | 「已复查结论台账」vttest 行；「按输入是否格式合规分流采纳」 |
| `KDE-konsole.md` | C++/Qt 桌面终端（Vt102Emulation） | 不采纳：桌面 C++ 自研 VT；profile/session 编辑撞 PROHIBITED，DBus／i18n／tysend 未声明 | 「能力裁定补齐行」DBus 远程控制行、界面多语言/i18n 组织行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」konsole 行；「不得反向采纳的做法」 |
| `KDE-maui-station.md` | Qt6/Kirigami 移动终端（MauiKit） | 不采纳：QML 集成在 Android 无对应形态；仓内无 VT 资产（其 `split` 指 Service/UI 分层进程模型，非分屏） | 生态参照面组行；「能力裁定补齐行」分屏／平铺面板行（该行已记其误引更正） |
| `KDE-yakuake.md` | Konsole KPart 下拉式壳 | 不采纳：无新仿真（emulation 全在 Konsole，本行与 `KDE-konsole.md` 同口径）；下拉 UX 无 Android 对应；其 profile/session 复用撞 `PROHIBITED.md`「会话持久化」 | 生态参照面组行；`PROHIBITED.md` 会话持久化条；`KDE-konsole.md` 索引行 |
| `Kuddev-pebrel.md` | Rust+GPUI 桌面 GPU 终端（vte 核） | 部分采纳：release profile（LTO／codegen-units／strip／`opt-level = 3`）已落地，其「hot path O3」**采纳**——实测体积取向的 `opt-level = "z"` 把三条 CPU 热路径基准拖慢一倍以上（键入 92µs → 357µs、单元格实例构建 129µs → 267µs、CJK 解析 232ns → 510ns），代价只有原生库增大 18%；录制回放语料不采纳（门槛 1）；PTY 事件循环与 redraw anchor 已被本仓 VT 线程 + 行级脏跟踪越过；GPL-3.0 不引入代码 | `native/Cargo.toml`；「语料判定补齐行」pebrel 行；「能力裁定补齐行」分屏／平铺面板行与 vi-like 选择模式／可配置按键绑定行 |
| `PaNDa2code-zerotty.md` | Zig+Vulkan 早期终端 | 不采纳：Zig 转换对 Android 无用途，自研 VT 违单一来源；CI 与环境搭建方式无可移植结论 | 生态参照面组行；「语料判定补齐行」zerotty 行 |
| `PerBothner-DomTerm.md` | C/Java/JS WebSocket+DOM 终端 | 不采纳：Web UI 栈形态，无 Rust 依赖可嵌入 | 「能力裁定补齐行」WebSocket 行 |
| `RohitKushvaha01-ReTerminal.md` | Compose 壳 + Termux View | 不采纳（门槛 3）：VT 走上游 jar；proot 禁入 | Android 台账（kosh／ReTerminal 同源行）；`documents-provider`（其文档提供器形态已落地） |
| `TermoraDev-termora.md` | Kotlin/JVM Swing 终端 + SSH（自研 KTerm） | 不采纳：自研 VT 违单一来源；KeyEncoder／Selection／Find 模型本仓已有等价实现；`issue-564`／`issue-1055` 回归数据属其自身 VT 语料；ssh/transfer 测试依赖 SSHD mock，与本仓无落点 | 生态参照面组行；「语料判定补齐行」termora 行 |
| `TragicWarrior-libvtty.md` | C VT 库（libvterm fork） | 不采纳：VT 核不可换；C 库经 NDK 引入净增 FFI 且违单一来源；按键映射表与颜色缓存算法只作结构对照，不引入 C 代码 | 生态参照面组行；「语料判定补齐行」libvtty 行 |
| `Yazelix-mars.md` | Rust/wgpu（Rio 派生，alacritty 核） | 不采纳：VT 核冲突；字形基线与图集本仓已有等价实现；`sugarloaf`/`libsugaloaf` 承接 render/font 的 spike 维持不做（`29` 篇 P2 有界 spike，须复现 DESIGN 字体保证才动）；`teletypewriter` 的 PTY 在 Android 仍须 JNI 桥，无直接落点；其 config spec 清单只作 UX 元数据对照，设置面以 `settings-propagation` 为准 | 生态参照面组行；「语料判定补齐行」mars 行 |
| `ademmenh-zebra.md` | C+GTK3+VTE 极简示例 | 不采纳：VTE on GTK3 的极简 demo（`main.c` + Makefile），报告自陈「no new insights」且「几乎无可借鉴」；无测试数据 | 生态参照面组行 |
| `alacritty-termbenchbot.md` | 终端基准自动化工具 | 不采纳：`bench.sh` 依赖桌面环境（报告自陈 Android 须另建 NDK harness，不可直接用）；输入生成脚本与阈值建议可作方法论，本仓基准在 `native/benches` | 「语料判定补齐行」termbenchbot 行；「能力裁定补齐行」设备性能自动化门禁行 |
| `aligrudi-fbpad.md` | C framebuffer 极简终端 | 不采纳：framebuffer 单头、无 Android 落点（许可未声明不作判据） | 生态参照面组行 |
| `benyblack-ntilde.md` | .NET/Avalonia+Skia 终端 + conformance 矩阵 | 仅参照：差异矩阵与回放调试思路；语料为该实现自身输出，不采纳 | 「语料判定补齐行」ntilde 行 |
| `borisfaure-terminology.md` | C/EFL 桌面终端 | 不采纳：自研 terminal state 违单一来源，EFL 不在 Android；`tests/` 的 shell 回归脚本期望写在脚本内、以人眼读 diff 为主（报告自陈「可改写为 expected diff」属待做而非现成资产）；link／cursor／OSC 等用例的期望值非现成外部真相；tysend 文件传输未声明；主题格式本仓以构建期 alacritty-theme 为唯一入口 | 「语料判定补齐行」terminology 行；`theme-configuration`；「能力裁定补齐行」ZMODEM/tysend 行 |
| `contour-terminal-contour.md` | C++23+OpenGL 桌面终端（自带 vtconformance） | 不采纳（门槛 1）：脚本只发序列等待、7 个靠人工按键；图集与下划线几何本仓已有等价实现；其 Vi 式选区与图像协议（sixel/regis）未声明；其列举的 synchronized output／reflow／OSC 8 三项本仓均不需自建——前两者归上游 `libghostty-vt`（见本条「上游 VT 语义不由本仓测试覆盖」），OSC 8 亦为上游解析、本仓只经 `hyperlink_at` 查询 | 「已复查结论台账」contour 行；同步刷新走「不采用的上游回调不得留有通道描述」（`on_render_hold` 不接线）；「能力裁定补齐行」vi-like 行 |
| `cutefishos-terminal.md` | C++/Qt/QML（内嵌 `qmltermwidget` 控件） | 不采纳：QML/C++ 形态在 Android 无对应；所嵌控件的 VTE/Konsole 血统未逐 crate 编译确认（报告自陈不能声明为 Android 可移植）；无测试数据 | 生态参照面组行；「语料判定补齐行」cutefishos 行 |
| `dakra-ghostel.md` | Zig + libghostty-vt 的 Emacs 动态模块（同引擎第四家，已支持 Android/Termux） | 仅参照：同引擎 FFI 模式（分配器／Stream／screen pages／pin tracking）与脏跟踪已由本仓 wrapper 覆盖，代码不移植（GPL-3.0-or-later + Emacs 模块形态）；其 PTY resize 的像素字段语义已归 `pty-lifecycle`；预编译模块的分发流程对 APK 原生库无对应形态 | 上段生态扫描的同引擎对照组事实；「语料判定补齐行」ghostel 行；`pty-lifecycle`；`native/src/terminal/ghostty_terminal/` |
| `elementary-terminal.md` | Vala/GTK4 + VTE 桌面终端 | 不采纳：VTE 与单一来源冲突；不安全粘贴对话框撞 PROHIBITED；VTE shell 设置集成（GApplication settings schema）无 Android 落点 | 「能力裁定补齐行」；PROHIBITED 粘贴确认对话框；「语料判定补齐行」elementary 行 |
| `felixse-FluentTerminal.md` | UWP+C#/WebView（xterm.js），已停更 | 不采纳：VT 与渲染都在别处；主题导入撞「构建期读取、主题列表固定」；Shell Profile 模型与自定义会话撞 `PROHIBITED.md`（启动入口由 `shell-entry` 固定，不设 profile 编辑器）；托盘 IPC／更新器属桌面形态；`TerminalBridge` 事件桥属 WebView host 模式、本仓无对应形态；其测试组织（xunit／Moq／AutoFixture／FluentAssertions）属 .NET 栈，本仓测试走 `cargo test` 与 Gradle 目标，无落点 | `theme-configuration`；`shell-entry`；「语料判定补齐行」felixse 行（`.itermcolors` 样例）；生态参照面组行 |
| `floooh-chipz-kitty.md` | Zig 游戏 demo，用 Kitty 图形协议 | 不采纳：Kitty 图形由上游承担；无终端核心内容 | 「上游 API 取用判定登记」Kitty 行 |
| `fyne-io-terminal.md` | Go/Fyne 终端 widget | 不采纳：Go 不可用于 Android Rust 层；`test_data` 属其自研解析器期望 | 「语料判定补齐行」fyne 行 |
| `ghreprimand-odytty.md` | Rust+wgpu 自研 VT | 不采纳：自研 VT 违单一来源；wgpu 渲染组织本仓已越过；box-drawing 算法与无障碍控制未声明（朗读形态撞 `PROHIBITED.md` 无障碍朗读明禁）；native GPU 测试套件属其自身期望，GPL-3.0-only 前置 | 生态参照面组行；「语料判定补齐行」odytty 行；「能力裁定补齐行」无障碍行 |
| `godot-pty-gpty.md` | Rust+Godot 扩展（alacritty PTY，AI 向） | 不采纳：MCP 明禁、事件总线与平铺多路复用未声明；portable-pty 无 Android 后端；GPL-3.0-only 前置 | 「能力裁定补齐行」Model Context Protocol 行与分屏／平铺面板行；「语料判定补齐行」gpty 行 |
| `harmoninux-HiSH.md` | HarmonyOS + QEMU 跑完整 Linux VM | 不采纳：QEMU 全虚拟机编排（ArkTS/C++ 壳 + `libqemu-system-aarch64`），仓内无 VT 解析器与渲染，仅由 guest 终端体现；QEMU 作 Android 宿主属「内嵌用户态/虚拟机」明禁方向且过重；无终端测试 | 生态参照面组行 |
| `hugomyb-Lume.md` | Tauri/SolidJS+xterm.js（远程／AI） | 不采纳：Web 栈；replay ring 与会话持久化撞 PROHIBITED；命令面板、远程会话、AI 集成均未声明；OSC 133 命令块仅作上游能力对照 | 「能力裁定补齐行」会话恢复/持久化行、命令面板／shell 补全 overlay 行、远程会话行、AI 助手集成行、输出分块与命令状态着色行 |
| `hugopl-batata.md` | Crystal/GTK4+VTE 桌面平铺终端 | 不采纳：Crystal/VTE 与 Android 无交集；分屏未声明；其 VTE hyperlink match-setup（`match_add_regex`／hover 模式）无落点——本仓链接经上游查询（`text-selection` 口径）；`spec/` 的 Crystal specs 能否在当前运行时执行未确认，不计可采纳资产 | 「能力裁定补齐行」分屏行；「语料判定补齐行」batata 行 |
| `iewnfod-lumina-terminal.md` | Tauri/Rust+React+xterm.js（含 MCP server） | 不采纳：WebView 形态；MCP 明禁、命令面板未声明；portable-pty 不适用 Android | 「能力裁定补齐行」Model Context Protocol 行、命令面板／shell 补全 overlay 行 |
| `ii8-havoc.md` | C/Wayland 极简终端（libtsm） | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行 |
| `isene-glass.md` | x86_64 汇编 + X11 极简终端 | 不采纳：仅 x86_64、成熟度低、无 Android 落点；X11 wire 协议与内嵌 emoji 缓存的极简二进制思路无可移植结论（本仓图集与字体栈已越过该层） | 生态参照面组行；「语料判定补齐行」glass 行 |
| `ismail-yilmaz-Bobcat.md` | C++17/U++ 桌面终端（外部 TerminalCtrl） | 不采纳：VT 为外部依赖且非 Ghostty；U++ 非 Android；vi-like 选择器与按键绑定未声明 | 生态参照面组行；「能力裁定补齐行」vi-like 行；「OSC 覆盖逐项核对」1337 行 |
| `javaterminal-TerminalFX.md` | Java/JavaFX 终端 widget（core／local 分离） | 不采纳：JavaFX 无 Android 目标；core/PTY 解耦模型本仓 JNI 边界已有等价 | 生态参照面组行 |
| `joshka-qwertty-term.md` | Rust 重写 Ghostty 语义 + Metal 渲染 | 不采纳（门槛 1）：差分期望即其自身实现输出；本仓 VT 语义整体交上游；其 `examples/frame-capture` 的 bytes→PNG 无程序消费者，撞门槛 2（不引入无消费者的二进制资产） | 「上游 VT 语义不由本仓测试覆盖」 |
| `kemokempo-iced_term.md` | Rust/ICED(wgpu) + alacritty 核 | 不采纳：alacritty 核违单一来源；iced 非 Android UI 目标；cell 绘制与本仓渲染已有等价 | 生态参照面组行；「能力裁定补齐行」第三方渲染实现行 |
| `ketraterm-KetraTerm.md` | Kotlin/JVM Swing 终端库 + xterm oracle | 不采纳：JVM/Swing 无 Android 目标；oracle 为自身实现输出，帧租约本仓渲染线程已持有 | 生态参照面组行；「语料判定补齐行」KetraTerm 行；「能力裁定补齐行」第三方渲染实现行 |
| `kmscon-libtsm.md` | C 可复用 VT 状态机库 | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行；「语料判定补齐行」libtsm 行 |
| `kshivang-BossTerm.md` | KMP 终端库（jEditerm 派生）+ Compose | 不采纳：VT 核冲突；Compose 接线本仓已有同栈实现（其 `split` 指字形拆分宽度，非分屏）；LGPL-3.0 许可审查前置；其「Ghostty 集成失败时的回退核」定位**不得反向采纳**（DESIGN 单一来源不设回退核），JNI-free 的 Kotlin 直调 PTY 与本仓会话归 Rust 相悖 | 生态参照面组行；「语料判定补齐行」BossTerm 行；「不得反向采纳的做法」回退核一条 |
| `leoberbert-zashterminal.md` | Python/GTK4+VTE 桌面终端 | 不采纳：VTE 冲突；会话管理与广播属桌面形态；文件管理器侧栏、快捷提示、语法高亮与 AI 集成未声明 | 生态参照面组行；「能力裁定补齐行」文件管理器行与 AI 集成行 |
| `lihop-godot-xterm.md` | Godot 终端控件（libtsm/node-pty） | 不采纳（PROHIBITED + 人工目检）：asciicast 回放撞会话持久化；colortest 靠人眼 | 「语料判定补齐行」godot-xterm 行；「能力裁定补齐行」Terminal API 行与第三方渲染实现行 |
| `lmorg-ttyphoon.md` | Go/Wails 多媒体终端（自研 VT） | 不采纳：自研 VT + WebView；输出块分组与命令退出状态着色、结构化输出块、AI 集成未声明 | 生态参照面组行；「语料判定补齐行」ttyphoon 行；「能力裁定补齐行」结构化输出行与 AI 集成行 |
| `lxqt-qterminal.md` | C++/Qt6（qtermwidget） | 不采纳：桌面 C++；DBus／i18n 未声明 | 「能力裁定补齐行」DBus 远程控制行与界面多语言/i18n 组织行 |
| `marc2332-term.md` | Rust+Freya Flatpak 桌面终端 | 不采纳：桌面 GUI；JSON 会话持久化撞 PROHIBITED，分屏未声明 | 「能力裁定补齐行」会话恢复/持久化行与分屏／平铺面板行 |
| `marshallku-copad.md` | Rust+原生 UI 跨平台 AI 终端（alacritty） | 不采纳：alacritty 核冲突；插件／事件总线、comux 工作树集成未声明；报告无 MCP（此前误记已更正）；其 `copad-mobile` 走 WebView 远程终端，在 Android 本地终端约束下无对应形态 | 「能力裁定补齐行」插件系统行与 git worktree 行；「能力裁定补齐行」WebSocket/网络终端形态行 |
| `midasdf-zt.md` | Zig 极简多后端终端 | 不采纳：row-map 归上游；内嵌字体与「按需加载宿主字体」路线相悖 | 「能力裁定补齐行」row-map 行 |
| `migueldeicaza-SwiftTerm.md` | Swift 自研 VT（含 Ghostty fuzz 对照） | 不采纳（门槛 1）：期望即其自研实现输出；fuzz 语料对照曾由本仓 `external_benchmarks` 落地，后按「不测上游」撤回（见「外部基准与 BDD 套件的退场」行） | 「语料判定补齐行」SwiftTerm 行 |
| `mintty-mintty.md` | C/Win32+Cygwin 终端 | 不采纳：宽度表归上游；Sixel 已登记、BiDi 未声明 | 「能力裁定补齐行」BiDi 行；「语料判定补齐行」mintty 行 |
| `mirbehnam-RtlTerminal.md` | C#/WPF Windows RTL 终端 | 不采纳：RTL 整形与 layoutDirection 均未声明 | 「能力裁定补齐行」RTL/BiDi 文本整形行与界面 layoutDirection RTL 行 |
| `missdeer-mostty.md` | Zig + libghostty 桌面原生终端 | 不立项：单帧 deadline／quantum 属过度设计，本仓读链已有界；PTY→UI 模式已有等价物 | 「能力裁定补齐行」单帧处理 deadline/quantum 行与「同引擎对标仓」的 PTY→UI 交接模式行（含错裁更正） |
| `mrlima4095-OpenTTY-J2ME.md` | J2ME MIDlet + RISC-V 模拟器 | 不采纳：**非终端仿真器**（RV32IM ELF 模拟器 + 内嵌 Lua + 虚拟 VFS 的 J2ME 平台）；其 `tests/check_*` 无终端仿真用例，LCDUI 测试对 Compose 无意义；平台与运行环境整体不匹配 | 生态参照面组行 |
| `nihiL7331-sfte.md` | C 单头文件极小终端（st 衍生） | 不采纳：仓内仅 fuzz 宏无语料；设备性能门禁登记在案 | 「语料判定补齐行」sfte 行；「能力裁定补齐行」设备性能自动化门禁行 |
| `not-elm-orzma.md` | Rust 自研 VT + WebView 混合终端 | 不采纳（门槛 1+3）：第三方 VT 差分；WebView 超范围 | 「语料判定补齐行」orzma 行 |
| `nowledge-co-con-terminal.md` | Rust+GPUI 全量 Ghostty 的 AI 终端 | 不采纳（门槛 1）：conformance 期望为自身输出；完整 libghostty 无 Android 平台；其 config string patch 与「sanitized font family」设计无落点——本仓主题固定且构建期读取（`theme-configuration`），字体族由 `font-selection` 管辖 | 「语料判定补齐行」con 行；`theme-configuration`；`font-selection`；本条「禁止项三条保留」段（不等待完整 libghostty 支持 Android） |
| `omnipty-terminal-omnipty.md` | Rust+GPUI+alacritty 桌面终端 | 不采纳：GPUI（Zed 引擎）非 Android UI 目标；VT 源头 alacritty 属核冲突；其自定义字节流事件循环报告自陈「仍属设计素材、未经复核验证」，而本仓会话与输出管道已在 Rust 落地；`tests/` 报告未展开框架与覆盖 | 生态参照面组行 |
| `orhun-ratty.md` | Rust/Bevy/wgpu 3D GPU 终端 | 不采纳：present 管线早于本仓字形图集 + cell 实例化；Bevy 过重 | 生态参照面组行（含 font/atlas 悬问闭合） |
| `paulrobello-par-term-emu-core-rust.md` | Rust VT 引擎核 + C/Python 绑定 | 不采纳（门槛 1+3）：第三方 VT 差分；Ghostty 覆盖更广 | 「语料判定补齐行」par-term 行 |
| `paulrobello-par-term.md` | Rust/wgpu/egui GPU 终端 | 采纳（已落地）：GPU surface 尺寸钳制到设备上限与 no-op configure 跳过；egui／agent API、命令面板、ACP 分诊不采纳；Kitty graphics geometry（`c`／`r`／偏移）由上游承担，无本仓落点 | `render-stability`「Surface 尺寸钳制到设备上限」；「上游 API 取用判定登记」Kitty 行；「能力裁定补齐行」命令面板行与 AI 集成行 |
| `pg83-shitty.md` | C++/Vulkan 桌面终端（自研 vterm） | 不采纳（门槛 1 + 穷举扩张禁止）；许可双轨悬问已闭合；damage 驱动渲染与字形回退本仓已有等价 | 「语料判定补齐行」shitty 行；「能力裁定补齐行」第三方渲染实现行 |
| `pop-os-cosmic-term.md` | Rust/iced + alacritty 终端 | 不采纳：alacritty_terminal 作替代核违单一来源 | 「能力裁定补齐行」 |
| `rricharz-Tek4010.md` | C/SDL Tek 图形仿真 | 不采纳：Tek 序列未声明（Ghostty 无该协议）；`tektests`／动画样例属其自身协议的语料，无落点 | 「能力裁定补齐行」Tektronix 4010/4014 行；「语料判定补齐行」Tek4010 行 |
| `sanohiro-bcon.md` | Rust/OpenGL ES bare-TTY 终端 | 不采纳（门槛 1+3）：vte crate 作 test shim 属第三方 VT 差分 | 「语料判定补齐行」bcon 行 |
| `sudo-tee-hollow.md` | Zig/LuaJIT + Ghostty 核桌面终端 | 仅参照：同引擎 wrapper 回调生命周期已有等价实现，代码不移植；插件/mux 测试非 VT 语料 | 「能力裁定补齐行」Zig libghostty wrapper 的 callback 生命周期模式行与插件系统行；「语料判定补齐行」hollow 行 |
| `techsenger-jeditermfx.md` | JavaFX/JediTerm xterm 仿真器 | 不采纳（门槛 1）：before/after 期望与其实现同源；type-ahead 未声明 | 「语料判定补齐行」jeditermfx 行；「能力裁定补齐行」 |
| `tenox7-aclock.md` | C 终端文字时钟玩具 | 不采纳：玩具级 VT 绘制；其 escape 绘制样例可作渲染器兼容性人工对照目标，非语料 | 生态参照面组行 |
| `tn5250j-tn5250j.md` | Java IBM 5250 主机终端 | 不采纳：非 ANSI/xterm 域 | 「能力裁定补齐行」5250 行 |
| `tuzig-terminal7.md` | TS/xterm.js/Capacitor 远程终端 | 不采纳：Web-first 无 Android 落点、网络终端未声明；webexec 超 local-only 范围 | 「语料判定补齐行」terminal7 行；「能力裁定补齐行」WebSocket 行 |
| `veops-go-ansiterm.md` | Go 屏幕模拟／命令提取 | 不采纳（门槛 3）：Go 库无 Android 落点，断言未编目 | 「语料判定补齐行」go-ansiterm 行 |
| `vercel-labs-wterm.md` | Zig/WASM + DOM 渲染（封装 libghostty） | 不采纳：WASM/DOM 无法作为 Android 依赖；无障碍/文本捕获未声明（朗读形态撞「无障碍朗读」明禁）；WS 重连／背压未声明；CI 产物哈希登记禁改区；其 DOM 脏行跟踪／部分重绘思路已由本仓行级脏跟踪与部分帧带越过，Ghostty 封装层的页面零初始化修复属上游内部实现 | 「语料判定补齐行」wterm 行；「能力裁定补齐行」WebSocket 行与无障碍行；「禁改区待维护者裁定项台账」 |
| `wilsonglasser-oryxis.md` | Rust/iced + alacritty SSH 客户端 | 不采纳：alacritty 核冲突；config vault／会话恢复撞 PROHIBITED；crate tests 未编目 | 「能力裁定补齐行」会话恢复/持久化行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」oryxis 行 |
| `z-hunter-Scanline-term.md` | Tauri/React/xterm.js 复古终端 | 不采纳：无 VT conformance 套件；MCP 明禁、CRT 特效未声明、会话恢复明禁 | 「语料判定补齐行」Scanline 行；「能力裁定补齐行」Model Context Protocol 行 |
| `00-汇总与可参考点收敛.md` | 编号集总纲：25 仓汇总、P0/P1/P2 分级与 openspec 落点 | 已处置：P0 由各能力承接、`bootstrap-install` 单列立项；P1/P2 为实现指导不新立项 | `bootstrap-install`；「参照面与反面做法收敛补齐行」；本表 `01`–`25` 各行 |
| `01-sylirre-ghostty-android-terminal.md` | Java+C 壳，VT 全委托预编译 `libghostty-vt.a`，Canvas 拉快照过 JNI | 需改造后采纳（同引擎同架构最正当对照面）：可采纳类为锚定外部协议的 VT 断言；Java 持会话、CPU/GL 回退、SAF 复制字体、自定义主题编辑器不采纳 | 「Android 侧终端项目的复查台账」`sylirre/ghostty-android-terminal` 行；「不得反向采纳的做法」 |
| `02-termux-termux-app.md` | 上游基准：自研 VT + Canvas，约 4.8 万行 | 不采纳（门槛 3）：断言打在本仓不实现的上游 VT 引擎；行为参考归各能力（PTY 双线程、退出保留现场、键矩阵、回滚 2000） | 复查台账 `termux/termux-app` 行；`pty-lifecycle`；`shell-entry`；`terminal-state-regression` |
| `03-sohan-f-termux-monet.md` | termux fork + Monet 运行时取色引擎 | 不采纳（门槛 3）：新增仅 14 例断言刚写入的调色板值；Monet 动态取色与自定义主题撞 `PROHIBITED.md` 设置节 | 复查台账 `sohan-f/termux-monet` 行；「能力裁定补齐行」用户自定义主题行；`theme-configuration` |
| `04-8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 全矩阵 + C 真 PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；OSC 52 门控归 OSC 52 条款；多击选词／OSC 8 白名单／提交产物不得反向采纳 | 同仓现行索引行 `8dmusichannels-star-terminator.md`；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `05-deathlegionteamlk-jolz.md` | 仅 README 与 Release APK，源码私有 | 无门槛违反：无源码可评估，其声称细节不得引用；两处声称与本仓方向一致且已由本仓规范覆盖——宿主环境继承归 `shell-env`（本仓取固定透传名单而非全量继承）、logcat 可见崩溃报告归 `DESIGN.md` 日志要求 | 复查台账 `deathlegionteamlk/jolz` 行；`shell-env`；`DESIGN.md` |
| `06-GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 多发行版运行器 | 不采纳（门槛 3）：329 例属 SSH/SFTP/proot 面；单源启动脚本、通知退出、备份替换归上游对标能力各表 | 同仓现行索引行 `GlobalTechInfo-RedTerm.md`；复查台账 `GlobalTechInfo/RedTerm` 行；`bootstrap-install` |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | termux 重型 fork：会话加固 + 自研 bootstrap 治理 | 不采纳（门槛 1）：`terminal-emulator/src/test` 与官方逐字节相同；正例为会话加固（先进程组收尾） | 复查台账 `rafaelmeloreisnovo/termux-app-rafacodephi` 行；「参照面与反面做法收敛补齐行」整包完整性校验（BLAKE3 与 sha256 双实现）行 |
| `08-pavelc4-Rin.md` | Rust 引擎 + Compose 壳，手写 545 行 ANSI 解析 | 不采纳（门槛 1 + 3）：手写解析必然不完备（反衬 Ghostty 单源）；Canvas 指针进 Rust 绘制违渲染分层 | 复查台账 `pavelc4/Rin` 行；「禁止项三条保留」段（不换 VT 核） |
| `09-Lord1Egypt-AndroidThothTerm.md` | JackPal 血统 Java 仿真器 + BiDi 渲染 | 门槛 1 不违反但仅作判据来源；`tests/` 语料对全仓零引用属失效语料（2026-11-02 复查为 131 文件、仍零引用）；BiDi 已在其 `PaintRenderer` 内有实现但本仓不立项（理由见「能力裁定补齐行」RTL/BiDi 行）；交互侧可参考项本仓已有等价或更强实现——长按取词与拖拽手柄归 `text-selection`、虚拟键布局归 `modifier-bar-height-reservation`、会话列表标题监听归 `session-list`、常驻通知归 `DESIGN.md` 软件节、staging 与失败清理归 `bootstrap-install`；新增的安全契约不变量与导出面策略测试见「参照面与反面做法收敛补齐行」对应两行 | 复查台账 `Lord1Egypt/AndroidThothTerm` 行；「新增失效语料记录」行；「能力裁定补齐行」RTL/BiDi 行；「参照面与反面做法收敛补齐行」安全契约与导出面策略测试两行；`text-selection`；`modifier-bar-height-reservation`；`session-list`；`bootstrap-install` |
| `10-ownerroc-termux-monet.md` | 同源换皮的 termux fork | 不采纳（门槛 1 成立、门槛 3 不满足）：测试为官方旧快照、零新增；缺 `ITermImage` 等属更弱上游 | 复查台账 `ownerroc/termux-monet` 行 |
| `11-irealashu-ApexShell.md` | Compose 壳：每命令 `ProcessBuilder`，正则染色冒充 VT | 无门槛违反：无 VT 测试资产；逐命令执行／提权／历史持久化撞明禁 | 复查台账 `irealashu/ApexShell` 行；「参照面与反面做法收敛补齐行」空 catch 行 |
| `12-Sunbr99-TerminalOS.md` | 仓内仅 README、LICENSE 与 3 行脚本；README 声称的 Go 结构不存在 | 无门槛违反：无资产可评估 | 复查台账 `Sunbr99/TerminalOS` 行 |
| `13-imlunixdev-puppy.md` | Kotlin + C++ PTY，自研 VT 子集 + proot | 不采纳（门槛 3）：核心解析含已确认标准偏差；合并投递与粘包解码归本仓 Rust 侧 | 复查台账 `imlunixdev/puppy` 行；「参照面与反面做法收敛补齐行」渲染热路径内的 measureText 行 |
| `14-CocaKova-charon.md` | 纯 Kotlin 自研 VT + SSH，自研界质量标杆 | 门槛 1 部分不违反（外部标准期望值子集）；corpus golden 可被实现覆写 MUST NOT 采纳；`updateSelection` 经 `javap` 实测为框架 `@hide` | 复查台账 `CocaKova/charon` 行；`ime-editor-contract`「组字光标位置回报给输入法」 |
| `15-RohitKushvaha01-ReTerminal.md` | Termux 库复用 + proot，Material 3 壳（已停更） | 无门槛违反：VT 在 Maven 依赖、仓内仅模板测试；文档提供器与透传名单形态归本仓能力；可配登录 shell 的形态收敛到本仓启动入口框（绝对路径二进制、不做存在性检查、不设脚本路径） | 复查台账 `RohitKushvaha01/ReTerminal` 行；`documents-provider`；`shell-env`；`shell-entry` |
| `16-eggie-admin-hydra-shell-android.md` | Godot 壳 + 云集成，无终端代码 | 无门槛违反：Android 与原生源码为 0；提权撞 `PROHIBITED.md` 设置节「Shizuku 集成开关」明禁，云集成与 AI 集成在 `PROHIBITED.md`／`DESIGN.md` 均无条目，属未声明、不立项 | 复查台账 `eggie-admin/hydra-shell-android` 行；「参照面与反面做法收敛补齐行」提权行 |
| `17-Autonomous-Agentic-Workflows-termux-app.md` | 旧基线 fork，含真实回退（缺源文件、删 Sixel、去版本守卫） | 不采纳（门槛 3）：弱化子集；跟 master 并保留上游能力 | 复查台账 `Autonomous-Agentic-Workflows/termux-app` 行；「参照面与反面做法收敛补齐行」删减式回退行 |
| `18-mirute02-anoterm.md` | Kotlin/Compose 自研 VT 子集 + SSH | 不采纳（门槛 3）：断言对象为其自研组件；宽度与解码口径与本仓一致处归各能力 | 复查台账 `mirute02/anoterm` 行；`cjk-rendering`；`text-search`；「能力裁定补齐行」IME 组字内外删除分流、组字光标模式行 |
| `19-TOYTTQE-PufiX.md` | 桌面 Python 文字交互壳 | 无门槛违反：只产生 ANSI 从不解析，无网格、无 PTY | 复查台账 `TOYTTQE/PufiX` 行 |
| `20-Malik1stmkw-termux-clone.md` | 空仓库（一句 README，无源码） | 无门槛违反：无资产 | 复查台账 `Malik1stmkw/termux-clone` 行 |
| `21-Adhi-hub07-on3-terminal.md` | termite 范式：C PTY + Kotlin 自研 VT | 无门槛违反：零测试资产；PTY 子进程核对清单归 `pty-lifecycle`；自造 `svc #0` syscall 不得反向采纳 | 复查台账 `Adhi-hub07/on3-terminal` 行；`pty-lifecycle`；「参照面与反面做法收敛补齐行」自造 `svc #0` syscall 做 JNI 桥行 |
| `22-frostre1997-kosh.md` | ReTerminal 同系 + 预装 Alpine rootfs | 无门槛违反：VT 在 Maven 依赖；预装 rootfs 与共享存储启动撞明禁 | 复查台账 `frostre1997/kosh` 行 |
| `23-shafinthedev-Ply.md` | Rust 行循环 REPL + TextView 壳 | 门槛 1 不违反但零判别力：两个测试均坏；管道式引导安装不得反向采纳 | 复查台账 `shafinthedev/Ply` 行；「参照面与反面做法收敛补齐行」`curl \| sh` 式引导安装行 |
| `24-Rprop-AnsolePlus.md` | 旧版 Termux 打包（剥离图像协议） | 不采纳（门槛 3）：停在旧 rev、图像族全缺；JNI 字符串释放错位不得反向采纳 | 复查台账 `Rprop/AnsolePlus` 行；「参照面与反面做法收敛补齐行」JNI 字符串释放错对象行 |
| `25-reapercanuk39-termux-kotlin-app.md` | termux 完整 Kotlin 化 + AI 代理运行时 | 门槛 1 不违反（仅键位编码与鼠标两类断言）；MCP 撞明禁、AI 代理运行时未声明不立项；宽表体量佐证不自研宽字符表 | 复查台账 `reapercanuk39/termux-kotlin-app` 行；「能力裁定补齐行」Model Context Protocol 行 |
| `27-改进点收敛与openspec落点.md` | 改进点收敛四类表（已覆盖/在途/允许/明确拒绝）与落点 | 已处置：初判「新增能力 0」经复核自证伪并改正，落 `grid-extraction-incremental`；其余 P1/P2 为实现指导 | `grid-extraction-incremental`；本表 `01`–`25` 各行 |
| `29-减码杠杆与libghostty家族结论收敛.md` | 减码 L1–L6 分级与红线；libghostty-vt 对比 librio | 采纳并已执行（L1）：JNI 批量合并按 `DESIGN.md` 背书落地（标题／单元格宽高／主题目录／会话计数收敛为批量查询），`getMode` 导出删除；L2–L6 维持「有界重构建议，不扩大为变更」；红线（不减测试、不删 DESIGN 功能）为约束；不换核、不等完整 libghostty 上 Android 归禁止项三条保留 | 「禁止项三条保留」段；「研究阶段残留项与覆盖缺口登记」L2–L6 行；`openspec/changes/archive/` 台账 |
| `终端模拟器生态深度研究报告.md` | 生态深度报告：S/A/B/C 评级、五条 VT 路线、跨仓扫描 | 已处置：评级语义归本条「生态参照面组行」定义；路线与扫描结论归本条「生态内其余仍需对照的结论」段 | 本条「生态扫描中仍作为判断依据保留的事实」段与「原 `ecosystem.md` 的 S/A/B/C 参照面评级」段；本表各行 |

上表首 6 行键为曾现役、后随 `docs/research/` 整体删除的 6 份综合研究报告，
其结论由同键索引行承接；其余各行为更早退役报告的索引。

上表末 31 行键为另一份退役编号集的文件名（`00`–`30` 去掉 `26` 的 30 份编号文档
加 `终端模拟器生态深度研究报告.md`；`26` 为 API 清单，成文时即移出研究目录，不在
本次退役范围）；该编号集曾用于立项 `bootstrap-install`、PTY 收尾清单、搜索上限与
文档提供器形态，其逐仓结论已由「Android 侧终端项目的复查台账」承接、其余结论按
上表各行指向表内可读位置。

### Requirement: 逐份处置补齐行台账

逐份处置中发现的、此前无规范去向的可执行句 MUST 在下表补一行，键为该句所属报告的
文件名，写明该句与去向（已实现并给代码位置／已由某能力承接／未声明不立项／
`PROHIBITED` 明禁／仅参照）；同一份的补齐行 MUST 与该份的删除同批提交，
MUST NOT 攒批后一次补写。整批研究收敛为 105 个键、187 行，与 `tasks.md` 逐份声明的
「补齐行 N 条」之和相等。

#### Scenario: 补齐行与删除同批

- **WHEN** 复核任一份报告的删除提交
- **THEN** 该提交内 MUST 含该份的全部补齐行，MUST NOT 只在删除后再补

#### Scenario: 补齐行给得出可复核的位置

- **WHEN** 读到表中任一行
- **THEN** 其「去向」MUST 是可检索的规范条目名、代码路径或明确的不立项判定，
      MUST NOT 只写「已核对」「无问题」这类无法复核的表述

补齐行台账如下。

| 报告 | 此前无去向的句 | 去向 |
| --- | --- | --- |
| `00-汇总与可参考点收敛.md` | P1「DNS 写 resolv.conf」 | 未声明，不立项：本仓无 proot 发行版运行器与容器 DNS 面（`DESIGN.md` 无声明）；引导安装只覆盖 `bootstrap-install` 的四条，不含 DNS 写入 |
| `00-汇总与可参考点收敛.md` | P1「读写循环可中断」「资源清理幂等」 | 已实现：读线程在输出通道关闭（会话收尾）时退出循环（`native/src/terminal/session/mod.rs`）；资源回收走 `pty-lifecycle`「收尾先终止前台进程组」的有界挂断→升级路径，`destroySession`（`native/src/android/ffi.rs` → `destroy_session_inner`）单次生效 |
| `00-汇总与可参考点收敛.md` | P1「JNI 几何数组校验」 | 已实现且已有规范：`initSession` 对 `rows`／`cols` 做 `u32::try_from`，负数即抛 `IllegalArgumentException`（`native/src/android/ffi.rs`），口径归「JNI 边界不得静默截断或静默吞错」 |
| `00-汇总与可参考点收敛.md` | P1「历史追加单流常开」「读线程 16 ms 合并投递」 | 不适用：本仓会话为单一 PTY 流、字节直传上游 VT stream，无按命令分段的「历史流」，也无合并投递窗口（见「参照面与反面做法收敛补齐行」PTY 输出合并投递行） |
| `00-汇总与可参考点收敛.md` | P1「shell 引用单引号转义」 | 仅参照：本仓不生成 shell 命令行——无 `curl \| sh` 式引导、也无 proot 启动脚本生成，引导安装按 `bootstrap-install` 固定流程执行 |
| `00-汇总与可参考点收敛.md` | P1「主机密钥首见/变更双态问询」「裁弱算法」「传输断点续传」 | 未声明，不立项：本仓无 SSH／SFTP／文件传输面（`DESIGN.md` 无声明，与 termora 行同口径）；日后若开该面再评估 |
| `00-汇总与可参考点收敛.md` | P2「吞吐基线」 | 仅参照：本仓基准面是设备侧宏基准（`android/benchmark`，现状登记见「能力裁定补齐行」设备性能自动化门禁行），不引入第三方的基线做法 |
| `00-汇总与可参考点收敛.md` | P2「点按时才扫链接 + 跨换行拼接」 | 已实现（语义等价）：打开链接只取选区起点的 OSC 8 属性、跨换行区段归上游解析（`text-selection`「打开链接」），本仓不自建屏面扫描 |
| `00-汇总与可参考点收敛.md` | P2「色相保持只动明度」 | 归「能力裁定补齐行」用户自定义主题覆盖行：Monet 同族取色手法整面撞 `PROHIBITED.md` 设置节「自定义终端主题」，不因单条合成手法另立条目 |
| `00-汇总与可参考点收敛.md` | P2「引号优先屏扫、折行估计、尾标点修剪、扩展名白名单」 | 不采纳：本仓不做屏面文本启发式提取；打开文件/链接的判据是选区文本与 OSC 8 属性（`text-selection`），启发式扫描与「MUST NOT 检查可达性」口径相悖 |
| `00-汇总与可参考点收敛.md` | P2「后台停收集」 | 已实现：切后台暂停渲染但维持会话推进、事件留队（`android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt`），即本仓口径的「停渲染、继续推进、不丢事件」 |
| `00-汇总与可参考点收敛.md` | P2「宽表体量不自研佐证」 | 已登记：主台账 `reapercanuk39/termux-kotlin-app` 行（宽度事实取自上游网格单元，不自研宽字符表） |
| `01-sylirre-ghostty-android-terminal.md` | P0「回调缓冲同次返回（`terminal_jni.c:100-119`）」 | 已实现（形态不同、语义等价）：本仓 `OutputProcessor::process` 在**一次调用内**返回 `OutputSnapshot`（含本块命中的 OSC 52 读取请求），VT 应答不经上行回调（`native/src/terminal/output_processor.rs`），与「不采用的上游回调不得留有通道描述」同口径 |
| `01-sylirre-ghostty-android-terminal.md` | P1「OSC8 链接二次确认」 | 不采纳：本仓打开链接经选择菜单**一次点击即生效**且只检查格式（`text-selection` 条款「条目 MUST 一次点击即生效。」与「打开链接 MUST 仅检查格式」，即其 Scenario「一次点击即生效」），不做打开前二次确认 |
| `01-sylirre-ghostty-android-terminal.md` | P2「防 tofu 字形替换思路（须用系统或图标包实现）」 | 仅参照：本仓以回退族供字形并按 `font-selection`「上报实际供出字形的字体」上报（设置页文案「其他被使用字体」），不做字形替换；内嵌图标包属未声明的字体路线 |
| `01-sylirre-ghostty-android-terminal.md` | P2「网格尺寸 HUD」 | 未声明，不立项（`DESIGN.md` 无调试叠加层） |
| `01-sylirre-ghostty-android-terminal.md` | P2「真机测试矩阵」 | 仅参照：本仓设备侧面为 `android/app/src/androidTest` 与模拟器基准（`scripts/test-emulator.nu`），不建跨设备矩阵 |
| `01-sylirre-ghostty-android-terminal.md` | 禁止项「CPU/OpenGL 回退」 | 已明禁：DESIGN.md 架构节的「GPU Vulkan 渲染（无 CPU/OpenGL 回退）」「GPU Vulkan 渲染（无 CPU/OpenGL 回退）」 |
| `02-termux-termux-app.md` | P0「PTY 双线程加队列加主线程泵模型」 | 仅参照：本仓形态为读线程 → 有界输出通道 → 渲染线程轮询（`native/src/terminal/session/mod.rs`、`NativeBridge.pollEvent`），事件靠拉取而非主线程泵，语义已等价 |
| `02-termux-termux-app.md` | P0「回滚默认 2000」 | 已实现：`DEFAULT_SCROLLBACK_LINES = 2000`（`native/src/terminal/session/mod.rs`），并解除上游默认字节上限使行数成为唯一约束（`native/src/terminal/ghostty_terminal/internal/mod.rs`） |
| `02-termux-termux-app.md` | P0「会话事件最小集」 | 已实现（形态不同）：本仓事件为 `pollEvent` 拉取的最小事件集（退出、剪贴板、标题、响铃等），无客户端回调注册面；响铃按 DESIGN 的 `bell-character` 三态执行，事件集内含 `Bell` |
| `02-termux-termux-app.md` | P1「输入连接掩码配置」 | 已实现：`android/app/src/main/java/terminal/emulator/input/TerminalEditorInfo.kt` 按 termux `onCreateInputConnection` 同口径设 `inputType` 与 `IME_FLAG_NO_FULLSCREEN` |
| `02-termux-termux-app.md` | P1「前台通知与 WakeLock 策略」 | 已实现：常驻通知行已登记（`TerminalForegroundService.kt`），同文件的唤醒锁设单次持有上限并在半程续期 |
| `02-termux-termux-app.md` | P1「topRow 滚动与缩放字体手势划分」 | 已实现：`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt` 以 `ScaleGestureDetector` 划分捏合缩放与滚动手势（含实时缩放预览） |
| `02-termux-termux-app.md` | P2「配置文件落盘后移动避免半读」 | 不适用：本仓设置走 DataStore（`SettingsDataStoreProvider.kt`），无自写配置文件；安装侧落盘原子性归 `bootstrap-install`「原子换入与回滚」 |
| `03-sohan-f-termux-monet.md` | P0「对比度与可辨识度门限思想及纯函数可测拆分」 | 不适用：本仓主题为构建期固定的上游配置，无运行时合成撞色面；颜色算法面只保留 `RgbColor::parse` 与 `perceived_luminance`（`native/src/terminal/theme.rs`），不另造颜色算法 |
| `03-sohan-f-termux-monet.md` | P0「固定主题校验与预览可复用其测试套路」 | 已实现：`android/app/src/test/java/terminal/emulator/ui/theme/ThemeRepositoryTest.kt` 与 `TerminalThemePaletteTest.kt` 覆盖目录解析与无法解析主题名的清除；预览归 `theme-configuration`「预览名在框下方」 |
| `03-sohan-f-termux-monet.md` | P1「壁纸变更缓存」 | 不适用：本仓无壁纸取色面（Monet 整链明禁） |
| `03-sohan-f-termux-monet.md` | P1「状态栏导航栏随终端底色联动并按亮度切图标」 | 已实现：`android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` 的 `WindowInsetsControllerCompat` 段以终端底色亮度设 `isAppearanceLightStatusBars`／`isAppearanceLightNavigationBars`；该面未单立能力规范 |
| `03-sohan-f-termux-monet.md` | P2「色相保持只动明度」 | 同 `00-汇总与可参考点收敛.md` 行：Monet 同族取色手法整面撞明禁，不因单条合成手法另立条目 |
| `04-8dmusichannels-star-terminator.md` | P0「seccomp fork 变通思路（裸 clone 避 bionic `fork` 的 clone3 标志）」 | 仅参照：本仓 PTY 在 Rust 侧建（`native/src/terminal/pty.rs`），不经过 bionic `fork`，不存在 clone3 被 seccomp 拒绝的面；日后若遇同类拒绝按该方向排查 |
| `04-8dmusichannels-star-terminator.md` | P0「历史追加单流常开避每块开关」 | 同 `00-汇总与可参考点收敛.md` 行：不适用（本仓为单一 PTY 流，无按命令分段的「历史流」与开关） |
| `04-8dmusichannels-star-terminator.md` | P1「OSC52 读写门控加**设置开关**」 | 门控已实现且有规范（「OSC 52 读取请求的收割与应答须对每个会话有界」，写入只经上游回调通道落系统剪贴板）；用户侧的开关项 `DESIGN.md` 未声明，不加设置项 |
| `04-8dmusichannels-star-terminator.md` | P2「捆绑 terminfo 的离线思路」 | 不采纳：本仓 `TERM` 固定（`shell-env`），不捆绑 terminfo 资产；该仓的 12 条 ncurses 条目已登记在语料判定 `8dmusichannels-star/terminator` 行 |
| `04-8dmusichannels-star-terminator.md` | P2「会话导出 txt 的落盘方式」 | 未声明，不立项（`DESIGN.md` 无会话导出面） |
| `04-8dmusichannels-star-terminator.md` | 禁止项「local echo 开关」「purge 语义开关」 | 未声明，不立项（本仓无这两类设置项）；「keymapper 自定义映射」撞 `PROHIBITED.md` 终端节「实体键盘快捷键设置」 |
| `05-deathlegionteamlk-jolz.md` | P2「继承宿主环境而非清空」「logcat 崩溃报告」 | 已有去向：索引行 `05-deathlegionteamlk-jolz.md` 已写明归 `shell-env`（固定透传名单而非全量继承）与 `DESIGN.md` 日志要求 |
| `05-deathlegionteamlk-jolz.md` | 声称中的「桌面小部件、内建包管理器与 24 内建命令、11 配色、8 Activity 形态」与禁止项「开机脚本、API 桥」 | 未声明，不立项（`DESIGN.md` 无对应面；README-only 仓的实现细节不得引用）；插件系统已在「能力裁定补齐行」判不立项，配色面撞 `PROHIBITED.md` 设置节「自定义终端主题」 |
| `06-GlobalTechInfo-RedTerm.md` | P0「单源启动脚本加挂载清单同源加引用转义」＋P1「启动脚本纯函数可单测」 | 不适用：本仓 bundle 只有单一安装入口（`bootstrap-install`），无发行版启动脚本与 proot 挂载清单；引用转义面只存在于 shell 启动入口（`shell-entry`） |
| `06-GlobalTechInfo-RedTerm.md` | P0「键栏 token 与双排与自动隐藏」 | 部分不采纳：本仓修饰键栏布局固定，token 可编辑撞 `PROHIBITED.md` 设置节「修饰键栏布局编辑器」；「双排」与「自动隐藏」为本仓未声明形态，`DESIGN.md` 未声明键栏自动隐藏 |
| `06-GlobalTechInfo-RedTerm.md` | P0「DNS 写 resolv.conf」 | 同 `00-汇总与可参考点收敛.md` 行：不适用（本仓无发行版 rootfs，DNS 由 Android 解析器负责） |
| `06-GlobalTechInfo-RedTerm.md` | P1「崩溃与 ANR 与诊断与磁盘占用的排障矩阵」 | 未声明，不立项（`DESIGN.md` 只要求日志在 logcat 可见、不写文件） |
| `06-GlobalTechInfo-RedTerm.md` | P1「快捷设置磁贴与快捷方式形态」 | 未声明，不立项（`DESIGN.md` 无磁贴／快捷方式面） |
| `06-GlobalTechInfo-RedTerm.md` | P2「字体导入管理」＋「bashrc 模板」 | 未声明，不立项：`font-selection` 无字体导入面（字体族取自系统与设置固定项）；bashrc 模板撞 `PROHIBITED.md`「环境变量编辑功能」与自定义环境面 |
| `06-GlobalTechInfo-RedTerm.md` | P2「传输错误可解释化」「密钥与已知主机交互」与禁止项「自带 ssh/sftp/xz 与兼容库」 | 不适用：本仓无传输面（`DESIGN.md` 无 SSH／SFTP／隧道／录制） |
| `06-GlobalTechInfo-RedTerm.md` | 禁止项「全部文件权限换文档提供器」 | 反向已实现：本仓用 `documents-provider`（SAF `DocumentsProvider`）暴露 home，不申请全文件权限 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | P0「会话创建失败即清理并结束、防资源重复清理、读写循环可中断」 | 已实现并已立规范：`pty-lifecycle`「拆卸可重入、读循环可中断且组杀不误伤本进程」（本次随该文档补入）；实现见 `native/src/terminal/session/mod.rs` 的 `request_exit`／`read_error_action`／`join_with_timeout` |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | P0「JNI 几何与数组校验、失败路径内存与引用清理」 | 已实现且有规范：入参收窄与越界口径归「JNI 边界不得静默截断或静默吞错」（`native/src/android/ffi.rs` 的 `u32::try_from` 分支），失败路径的释放归 Rust 所有权与 `Drop`，无手工引用计数面 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | P1「先进程组后单进程的关闭顺序（须先补规范）」 | 已立规范并实现：`pty-lifecycle`「收尾先终止前台进程组」要求 `kill(-pgid, SIGHUP)` 补 `SIGCONT` 后强杀；「须先补规范」一项因此闭合 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | P1「退出文案 StringBuilder 化等小优化」 | 不采纳：本仓无对应拼接热点，改动只为对齐上游写法而不带可测收益 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | P2「启动路径纯函数化测试（`PathTreatmentUtilsTests`）的思路」 | 已实现同向：本仓启动路径判定为纯函数并各有单测（`native/src/terminal/pty.rs` 的 `is_pie`／`read_shebang_interpreter` 与其用例） |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | 禁止项「kP/kN 重映射」 | 不采纳：翻页键编码 MUST 走上游 `key::Encoder`（`key-input-encoding`），该重映射属无 Ghostty 依据的上游行为变更 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | 禁止项「图像协议剥离」 | 反向已实现：本仓保留 Kitty 图形协议（含存储上限与 PNG 解码），剥离方向与 `DESIGN.md` 相反 |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | 禁止项「传感器桥与审计膨胀」「自研 bootstrap 与回滚与收据」「bootstrap sha256 校验文件」「治理工作流膨胀」 | 未声明，不立项；其中整包校验与治理膨胀已分别在「能力裁定补齐行」与「参照面与反面做法收敛补齐行」判死（撞 `PROHIBITED.md` 终端节「Bootstrap zip 的 sha256 旁挂校验文件」条） |
| `08-pavelc4-Rin.md` | P1「读写线程与应答回写分离」「resize 同步引擎与 pty」「ENV 指向 home 下 mkshrc」 | 已有去向：应答回写分离与 resize 归 `pty-lifecycle`（「VT 应答回写逐帧有界」「生成播种行列尺寸且 resize 携带像素尺寸」）；`ENV` 不采纳：`DESIGN.md` 禁止设置 `ENV` 与 `.mkshrc`，`shell-env` 明确不注入 |
| `08-pavelc4-Rin.md` | P2「解析器按命令枚举组织的代码结构（仅结构思路）」 | 不采纳：解析唯一真源为 Ghostty，本仓只在 `output_processor.rs` 自研 OSC 52 读取请求的逐字节收割，命令枚举式解析器结构无落点 |
| `08-pavelc4-Rin.md` | 反面价值 P0「手写 545 行仍是不完备子集，须在汇总中引用」 | 已实现：`00-汇总与可参考点收敛.md` 行与该仓复查台账行进已写明（门槛 1 成立、门槛 3 不满足，终端侧零测试），反衬 Ghostty 单源决策 |
| `08-pavelc4-Rin.md` | 禁止项「自带包管理器 rpkg」「su/root 切换」 | 未声明，不立项（`DESIGN.md` 无包管理与提权面）；提权另有 `PROHIBITED.md` 设置节「Shizuku 集成开关」与「参照面与反面做法收敛补齐行」提权行明禁 |
| `08-pavelc4-Rin.md` | 禁止项「红色染色退出文案」 | 不采纳：退出文案形态由 `shell-entry` 固定为 `[Process completed (code X) - press Enter]`，不带颜色 |
| `08-pavelc4-Rin.md` | 「portable-pty 属外部库封装，本项目不引入」 | 已实现：本仓 PTY 自建（`native/src/terminal/pty.rs`，`nix` 直接管 `fork`／`exec`／`openpty`），未引入外部 PTY crate |
| `09-Lord1Egypt-AndroidThothTerm.md` | P0「长按取词加拖拽手柄加文本提取的交互状态机」「虚拟键布局」「会话数组回调加标题监听」「常驻通知做法」「staging 加失败清理语义」 | 已有去向：选择交互归 `text-selection`；键栏归 `modifier-bar-*` 四条；会话列表与标题归 `session-list`；常驻通知归「常驻通知（显示会话数）与退出按钮」行（`TerminalForegroundService.kt`）；staging 与失败清理归 `bootstrap-install`「原子换入与回滚」 |
| `09-Lord1Egypt-AndroidThothTerm.md` | P1「读写线程划分与结束释放顺序」 | 已有去向：`pty-lifecycle`「收尾先终止前台进程组」与本次补入的「拆卸可重入、读循环可中断且组杀不误伤本进程」 |
| `09-Lord1Egypt-AndroidThothTerm.md` | P1「resize 失败保护」 | 已实现并已立规范：`pty-lifecycle` 增量「网格 resize 被丢弃时不得把旧尺寸当权威」（本次随该文档补入），实现见 `native/src/terminal/session/mod.rs` 的 `ResizeOutcome::Dropped` 与置脏重试 |
| `09-Lord1Egypt-AndroidThothTerm.md` | P1「双指缩放步长」 | 已有去向：`font-selection`（步长取整与捏合上下界同源，调节条末端、捏合末端与落盘值三者一致） |
| `09-Lord1Egypt-AndroidThothTerm.md` | P2「双向列映射测试思路」 | 未声明，不立项：BiDi 逻辑视觉列映射在 `DESIGN.md` 无声明；本仓列宽语义归 `cjk-rendering` |
| `09-Lord1Egypt-AndroidThothTerm.md` | P2「回滚与选择用例清单」 | 已实现同向：回滚与选择用例在 `native/src/terminal/ghostty_terminal/tests/`（回滚保留、viewport delta、reset 清选区）与 Kotlin 侧选择归一化用例内 |
| `09-Lord1Egypt-AndroidThothTerm.md` | 禁止项「桌面与远端服务」 | 未声明，不立项（另撞 `PROHIBITED.md` 终端节「桌面环境，X11」） |
| `10-ownerroc-termux-monet.md` | 全篇结论「无新增可参考点，沿用 02 上游行为参考与 03 门限测试思路」＋P2「同源 fork 换皮差异作为反面组织方式」 | 已登记为不采纳：索引行写明其测试为官方旧快照、fork 零新增、缺 `ITermImage`／`TerminalSixel`／`TerminalBitmap` 属更弱上游，参照面不因换皮 fork 扩张；禁止项（背景图整链、模糊布局、自定义样式开关）撞 `PROHIBITED.md` 背景与自定义主题两条明禁 |
| `11-irealashu-ApexShell.md` | P2「附件栏常用键组织（仅键位思路，须按 Termux 2x7 落）」 | 已有去向：本仓修饰键栏为两行固定布局（`modifier-bar-height-reservation` 的两行总高、`modifier-bar-*` 的键位编码），附件栏式「常用键快捷行」不新增形态 |
| `11-irealashu-ApexShell.md` | P2「提权状态徽标的展示思路（不引入提权本身）」 | 未声明，不立项（本仓无提权面，`PROHIBITED.md` 明禁 Shizuku 集成与提权） |
| `11-irealashu-ApexShell.md` | 反面价值 P0「无 PTY 方案不可交互，证明真 PTY 必要，须在汇总中引用」＋禁止项「逐命令执行模式、正则染色输出、历史持久化、脚本库执行」 | 已实现：汇总行（`00-汇总与可参考点收敛.md`）与参照面行「以空 `catch` 吞掉全部异常」组已写明该组为反面做法；逐命令执行与正则染色撞 `DESIGN.md` 的真 PTY 与 Ghostty 唯一真源，历史持久化撞 `PROHIBITED.md`「会话数据持久化 / 恢复」，脚本库执行未声明 |
| `12-Sunbr99-TerminalOS.md` | 全篇「无 Android 终端代码、无对照价值；自研 shell 与启动真实 shell 方向相反」＋禁止项「自研 shell 解释器、内建命令集、插件注册表」 | 已登记为不采纳：索引行与复查台账行写明仓内无资产（README 声称的 Go 结构与测试文件均不存在）；本仓启动真实 shell（`shell-entry` 的 bash／login／system sh 顺序），自研 shell 与插件注册表未声明，且源码缺失不得引用 |
| `13-imlunixdev-puppy.md` | P1「OSC133 显式标记才消费、普通输入不动」＋禁止项「shell 模式扩展语义（未立规范前）」 | 未声明，不立项：OSC 覆盖逐项核对表 133 行已写明「上游解析（若支持）；本仓不消费」，`DESIGN.md` 未声明 prompt 标记语义；该文档自设的前提（引入前须先立规范）当前不成立 |
| `13-imlunixdev-puppy.md` | P2「有界历史环形槽」「缓冲单测形态」 | 部分不适用：本仓回滚槽由上游 Ghostty 持有（行数固定 2000、字节上限解除，见 `pty-lifecycle` 与 `native/src/terminal/session/mod.rs`），无自研环形槽；缓冲与跨分块解析用例方向已同向（`native/src/terminal/ghostty_terminal/tests/`） |
| `14-CocaKova-charon.md` | P0「fuzz 全性加种子固定、语料回放金、吞吐基线做法」 | 部分已有去向：语料回放归「ghostty `corpus/stream-initial` 01–24」行（曾采纳落地于 `native/tests/external_benchmarks/upstream_corpus.rs`，后整体撤回，见「外部基准与 BDD 套件的退场」行）；吞吐基线同 `00-汇总与可参考点收敛.md` 行（设备宏基准，不进仓库资产）；他仓 fuzz 语料已按门槛逐项判死不引入 |
| `14-CocaKova-charon.md` | P0「shell 引用转义惯用法」「点按时才扫链接加跨换行拼接」「搜索限量加批量查询」 | 已有去向：引用转义与点按时扫链接同 `00-汇总与可参考点收敛.md` 行（前者仅参照，后者归上游超链接与 OSC 8 查询）；搜索限量归 `text-search`「查询长度与命中规模有上限」 |
| `14-CocaKova-charon.md` | P0「预测线状态机思想（以 Ghostty 与本地行状态重实现）」＋「粘滞语义与预测逃生」 | 粘滞归 `modifier-bar-sticky-encoding`；预测线未声明，不立项（`DESIGN.md` 无预测输入面，故「预测逃生」亦无落点） |
| `14-CocaKova-charon.md` | P1「首次与变更双态问询文案」「传输断点续传状态机」「补全三源排序」与 P2「导出合并预览、舰队导入、环回转发映射、主题预览布局」 | 不适用：本仓无 SSH／SFTP／vault／补全／传输面（`DESIGN.md` 未声明），主题预览布局亦不因他仓形态扩面 |
| `14-CocaKova-charon.md` | 禁止项「片段持久化」「功能键页」「自研链接拦截」 | 逐项判死：片段持久化撞不持久化精神（`PROHIBITED.md`「会话数据持久化 / 恢复」同口径）且未声明；功能键页不引入（本仓键栏固定两行）；自研链接拦截方向相反——本仓链接识别只走 Ghostty 查询、不做额外检查 |
| `15-RohitKushvaha01-ReTerminal.md` | P1「文档提供器根、建删、搜索标志的形态」「宿主变量透传名单（取交集，禁 `LD_LIBRARY_PATH`）」「初始化脚本首次落盘」 | 已有去向：提供器形态归 `documents-provider`（`queryRoots`／`querySearchDocuments` 与「变更操作按树 URI 客户端契约覆盖」）；透传名单归 `shell-env`（白名单，明禁 `LD_LIBRARY_PATH`／`LD_PRELOAD`／`PWD`）；「初始化脚本首次落盘」不适用——本仓 bundle 由 `bootstrap-install` 经 URL 安装并原子换入，无 assets 初始化脚本 |
| `15-RohitKushvaha01-ReTerminal.md` | P1「文档提供器缩略的形态」 | 未声明，不立项：本仓不声明 `FLAG_SUPPORTS_THUMBNAIL` 与 `openDocumentThumbnail`，图标取自默认投影的列；引入缩略图面须先补规范 |
| `15-RohitKushvaha01-ReTerminal.md` | P2「Compose 偏好组件库组织」「登录 shell 设置项形态」 | 代码组织仅参照不引入；设置项形态归 `shell-entry`（启动入口设置框只收绝对路径二进制，不支持脚本路径、不检查存在） |
| `15-RohitKushvaha01-ReTerminal.md` | 禁止项「外部命令服务」「proot 与 chroot 双模式」 | 未声明，不立项；其余禁止项（复用 Termux 会话与视图、预装 Alpine、自定义环境变量扩展）已在复查台账行与 `PROHIBITED.md` 明禁 |
| `16-eggie-admin-hydra-shell-android.md` | 全篇「无终端模拟器代码、无 VT／PTY／渲染；云与 AI 集成与本仓禁止项冲突；无可参考点」 | 已登记为不采纳：复查台账行写明 204 个文件中 Android 与原生源码为 0（APK 钉在外部仓库）；提权撞 `PROHIBITED.md` 设置节「Shizuku 集成开关」明禁；云集成与 AI 集成在 `PROHIBITED.md`／`DESIGN.md` 中无条目，按上方「WebSocket/网络终端形态」与「AI 助手集成与 ACP/crash 分诊」两行口径作未声明、不立项（口径更正：本行原写「云与 AI 集成撞相关明禁」，该条目并不存在，以本行为准）；提权同组已在「参照面与反面做法收敛补齐行」判死 |
| `17-Autonomous-Agentic-Workflows-termux-app.md` | 全篇「旧基线加删减、无新增可参考点」＋两处真实回退（删 Sixel 分支、去 API 版本守卫）与禁止项「删减后的模拟器与渲染器、任何回退改动」 | 已登记为不采纳：复查台账行写明测试与官方逐字节相同且为弱化子集；回退与守卫两条归「参照面与反面做法收敛补齐行」的「删减式回退」行（跟 master 并保留上游能力），同组仓的 `ReleaseStringUTFChars` 错对象亦已在该表判死 |
| `18-mirute02-anoterm.md` | P1「异种拒加首见指纹展示」「裁弱算法」「馈送分片加同监绘制」「备屏不污染回滚」「尺寸变更回滚追随」 | 前三项不适用：本仓无 SSH／算法协商面，输出管道为 Rust 字节直传（同粘包解码行）；后两项仅参照——备屏与回滚语义归上游 Ghostty，本仓不重实现 |
| `18-mirute02-anoterm.md` | P2「引号优先加折行估计加白名单」「后台停收集加 bell 节流」 | 链接屏扫白名单不采纳：本仓链接识别只走 Ghostty 查询、不做自研屏扫（与 charon 行同口径）；bell 节流已由 `runtime/BellPlayer.kt` 的限频窗口覆盖（`termux-configuration`「bell-character 的三态执行」），上游的后台停收集形态不另立 |
| `18-mirute02-anoterm.md` | P2「回滚上限防爆内存」 | 已实现：回滚行数固定 2000（`native/src/terminal/session/mod.rs` 的 `DEFAULT_SCROLLBACK_LINES`），`set_scrollback_max_bytes(Some(usize::MAX))` 解除字节上限（上游语义：`None` 是恢复默认的 10 000 字节预算）、无用户可调项（`PROHIBITED.md` 明禁回滚行数设置） |
| `19-TOYTTQE-PufiX.md` | 全篇「无 PTY／VT／渲染；动态执行外部文件与落盘画像与本仓方向冲突；无可参考点」 | 已登记为不采纳：复查台账行写明 13 个 Python 文件的桌面 REPL 只产生 ANSI 从不解析、无网格无 PTY、零测试；动态执行外部文件与落盘画像未声明，不立项 |
| `20-Malik1stmkw-termux-clone.md` | 全篇「仓库仅一句 README，无代码，无可参考点」 | 已登记为不采纳：复查台账行写明全仓仅 1 个 59 字节的 `README.md`、无源码或二进制，「终端模拟器」主张在仓内无支撑 |
| `21-Adhi-hub07-on3-terminal.md` | P0「子进程开从端、setsid 后开从端建控终端、关多余 fd、清空环境后重装的核对清单」＋P1「termios 与窗口像素尺寸设置；exec 失败写从端报错后退出的诊断姿势」 | 已实现并已立规范：`pty-lifecycle` 增量「子进程侧的 fork/exec 不变量」（本次随该文档补入）四场景分别钉住控制终端、`FD_CLOEXEC`、`100 + errno` 退出码与从端错误行、`IUTF8` 与流控关闭；实现见 `native/src/terminal/pty.rs` 的 `spawn` 与 `child_exit_with_reason`。该文档自设的前提（「规范未细化 fork 步骤，引入前须先立规范」）因此闭合 |
| `21-Adhi-hub07-on3-terminal.md` | 「`clearenv` 后重装与本项目固定白名单加宿主透传方向一致，但须以本项目名单为准」 | 已有去向：本仓不经 `clearenv`，而以显式 `envp` 传入白名单（`native/src/terminal/pty.rs` 的 `build_env`，白名单输入结构 `ShellEnv` 在 `native/src/terminal/shell_env.rs`），宿主仅透传固定名单 |
| `22-frostre1997-kosh.md` | 全篇「结论与 15 相同并更差、无新增可参考点」＋「可写 HOME」「起于共享存储」＋禁止项「预装 rootfs 与工具链、proot 全套、复用 Termux 会话」 | 已登记为不采纳：复查台账行写明 VT 在 Maven 依赖、仓内零实现、proot 树与 `ReTerminal` 逐字节相同；「起于共享存储」的冲突归 `shell-entry`「无启动目录设置」（家目录固定为 `/data/data/com.termux/files/home`）；预装 rootfs／busybox／python 撞 `PROHIBITED.md` 预装发行版与内嵌 bootstrap 明禁；「可写 HOME」未声明，不立项 |
| `23-shafinthedev-Ply.md` | 全篇「无 PTY／VT／会话；`proot.rs` 仅桩；包管理为管道远程脚本属严重反面；无可参考点」 | 已登记为不采纳：复查台账行写明无 VT 解析、只转发 `/system/bin/sh` 的 stdout、两个测试均坏（断言从未打印的字符串、测的是宿主 sh）；`curl \| sh` 式引导安装已在「参照面与反面做法收敛补齐行」判死（引导安装按 `bootstrap-install` 固定流程，不把远端脚本接进 shell 管道） |
| `24-Rprop-AnsolePlus.md` | 全篇「旧基线删减打包、无新增能力」＋禁止项「删减后的模拟器、错位的 JNI 释放、删减的应用层」 | 已登记为不采纳：复查台账行写明停在旧 rev、图像族（`doDcs`／`sixel`／`TerminalSixel` 等）全缺、零测试；JNI 字符串释放错对象与行尾孤立高代理项越界读两条已在「参照面与反面做法收敛补齐行」判死 |
| `25-reapercanuk39-termux-kotlin-app.md` | P2「Kotlin 迁移文档的问题清单组织（仅文档思路）」「宽字符表体量作为不自研的佐证」 | 已有去向：文档组织归「迁移文档的问题清单组织」行（仅参照，本仓文档面为 `openspec/` 与 `docs/specification/`）；宽表佐证并入「宽度表归上游」口径（本仓不自研宽字符表，见 mintty 行与 `cjk-rendering`） |
| `25-reapercanuk39-termux-kotlin-app.md` | 禁止项「整套代理运行时、MCP、LLM 调用、JNI 错位、自研宽表」 | 逐项判死：MCP 撞 `PROHIBITED.md` 终端节「Model Context Protocol」明禁（`docs/specification/PROHIBITED.md` 唯一相关条目）；代理运行时与 LLM 调用在 `PROHIBITED.md`／`DESIGN.md` 中均无条目，按上方「AI 助手集成与 ACP/crash 分诊」行口径作**未声明，不立项**——口径更正：本行原写「外部服务与 AI 集成明禁」，该条目并不存在，以本行为准；JNI 错位归「JNI 字符串释放错对象」反面行；自研宽表同「宽度表归上游」；Kotlin 化本身无参考价值（本仓 Kotlin 仅 Compose 薄层，会话归 Rust） |
| `27-改进点收敛与openspec落点.md` | §3「允许但无新能力」6 条（PTY fork 核对、UTF-8 粘包跨块、读线程合并投递与按帧上限、长按重复时序、纯函数可单测与文档清单结构、OSC133／预测线／补全／通知音） | 前两条已有去向：PTY fork 核对清单本次随 `21-Adhi-hub07-on3-terminal.md` 立入 `pty-lifecycle` 增量「子进程侧的 fork/exec 不变量」，粘包跨块归「PTY 输出合并投递与 UTF-8 粘包保留解码」行；读线程合并投递与按帧上限、长按重复时序归既有实现（`session/output.rs` 的 `MAX_PTY_WRITES_PER_FRAME` 按帧上限、`ModifierBar` 的 400/80ms）；纯函数可单测与文档清单结构为方法类，按 `TESTING.md` 既有体系执行；OSC133／预测线／补全／通知音未声明，不立项（同 OSC 表 133 行与预测线行） |
| `27-改进点收敛与openspec落点.md` | §1「新增能力 1 项 `grid-extraction-incremental`」与三处残留（振铃注释失真、手势不采用依据不足、回归口径失实引用）、两处声明冲突 | 已闭合：`grid-extraction-incremental` 能力现存；三处残留与两处冲突的逐条处置见 `28-复审循环与残留问题收敛.md` 行，不在本行预判 |
| `28-复审循环与残留问题收敛.md` | §1 循环记录、§2 过度复杂特性、§3 与 §3.1 长按十二项、§4 基础功能五项 | 已落地并已登记：§1 各轮新增项与 §3.1 十二项的现状在代码中可验（`resolveWordSelectionBounds`、`SELECTION_RECTANGLE`、`cursor_in_viewport`、`SelectionState.pasteOnly` 等），不新增简化立项；§2 的过度清单按原文「仅记录，不立即简化」维持；§4 五项裁定（振铃保持静默并清理注释、连字收窄、滚动条措辞、裸链接不实现、权限字面差距）与 `terminal-state-regression`／`text-selection` 现状一致 |
| `28-复审循环与残留问题收敛.md` | §15.4 与 §18.6「仍未修」三项（IME 组字光标锚点、批量即时性无单测与 `focus_event` 半边覆盖缺口、依赖哈希与禁止哈希的字面张力） | 逐项现状：光标锚点已闭合——`ime-editor-contract` 改走公开 `InputMethodManager.updateCursorAnchorInfo` 并明禁反射 `@hide` 同名方法，实现见 `ImeCursorContract.kt` 与 `TerminalSurface.kt`，宿主单测 `ImeCursorContractTest`；批量即时性与 `focus_event` 另一半据实登记为覆盖缺口（结构保证，不以重试或放宽断言冒充）；哈希张力以措辞化解——规范只写「以 `Cargo.toml` 所钉 rev 为准」，规范与非归档文档中 hash 字面量为 0 处（实测：`docs/`、`openspec/specs/`、`openspec/changes/` 非归档部分均无 40 位或 7–8 位 hash 命中），仅 `openspec/changes/archive/**` 存有历史追加信息里的提交号——那是「当时做了这件事」的证据，semgrep 哈希规则按设计排除归档目录，改写等于伪造记录；`STYLE.md` 禁改区不动 |
| `28-复审循环与残留问题收敛.md` | §16.4「符号族不进入 `FontInfo`」、§17.3 登记备查、§17.4 设计已拟未实施、§19.3 评估后不改、以及本机无模拟器导致的 connected 用例缺口 | 已落地或据实登记：符号族上报已按 §17.4 拟案落地（`other_families` ＋设置页「其他字体」＋`font-selection` 规约）；`ModifierBar` 重建登记备查不再动；§19.3 两项（`load_font_file` 身份缓存、设置页关闭时无条件恢复渲染）评估后不改，依据在原文；模拟器 `get_capabilities` 失败与仪器化断言缺口属环境受限，据实登记，不凭猜测加重试 |
| `29-减码杠杆与libghostty家族结论收敛.md` | §一 血缘与性能结论、§二 减码 L1–L6 分级与红线、§三 libghostty-vt 与完整 libghostty、§四 与 librio／sugaloaf 对比 | 已登记：结论落在既有行——L2–L6 为「有界重构建议（P1/P2），不扩大为变更」且 L1 批量合并已执行，`sugaloaf`／`libsugaloaf` spike「维持不做」，红线（不减测试与覆盖、不弱化断言、不删 `DESIGN.md` 功能）与禁止项三条（不 vendor 第三方 VT 代码、不换 VT 核、不等完整 libghostty 支持 Android）原样保留 |
| `30-VT测试边界与外部基准接入收敛.md` | §一 不测／保留／为何必须保留、§二 外部基准三类、§三 务实优先级、§四 禁止项、§五 核验记录 | 已登记：结论落在既有行——「维持不扩张成穷举 VT 一致」（若引入只放真机 `connectedDebugAndroidTest` 且取精选子集）、「真实字体／bootstrap 数据扩面已由既有基准承担」；禁止项（不搬全量外部一致套件、不把上游小瑕疵转为本仓失败、不配 TERM／terminfo、不新增独立测试体系、外部数据只作 fixture 真值不 vendor）与 `TESTING.md`／`PROHIBITED.md` 一致 |
| `30-VT测试边界与外部基准接入收敛.md` | §六 执行结果（三项落地、6.5 口径差异、优先级 4／5 未动） | 曾实现后撤回：新测试目标 `native/tests/external_benchmarks/`（`main.rs` ＋ `support.rs` ＋ `unicode_width`／`upstream_corpus`／`keyboard_protocol`）与运行期取数（`ureq` 仅在 dev-dependencies）已整体删除，按维护者口径「不测上游」——三项实测分歧全落上游与终端惯例，本仓产品修复为零；6.5 的三处口径差异作为历史登记保留；优先级 4／5 未动已由真机工具链受限登记 |
| `终端模拟器生态深度研究报告.md` | §0–§18 主体（25 仓评级表、五条 VT 路线、渲染／PTY／字体／选择／修饰键／搜索／会话／主题／bootstrap／IME／高级特性、工程质量反面、P0／P1 清单、一句话结论） | 已处置：评级语义归「生态参照面组行」，路线与扫描结论归「生态内其余仍需对照的结论」段，逐仓结论落索引表与逐仓行；P0 第 1–5、7、8 项各有规约落点（搜索上限与折叠 → `text-search`；设置下发 → `settings-propagation`；选择把手翻转与自计数 → `text-selection`；PTY 组杀／像素 winsize／`IUTF8` → `pty-lifecycle`；IME 填坑与 `ACTION_MULTIPLE` → `ime-editor-contract`；孤立代理项守卫 → `cjk-rendering`；OSC 逐项核对与切会话清空 → 本条「OSC 覆盖逐项核对」与 `session-list` 对照口径）；§16「明确不采纳」清单与 `PROHIBITED.md`／`DESIGN.md` 一致 |
| `终端模拟器生态深度研究报告.md` | §10 Android 12+ 幻影进程限制、§11 ChromePalette 主题联动、§16 P0 第 6 项录制会话回放测试 | 仅记录／不采纳：幻影进程属平台 cgroup 约束、本仓无对应设置面，如实记录不立项；ChromePalette「主题联动」不采纳（主题走 alacritty-theme 固定列表与跟随系统分组，见 `theme-configuration`）；录制回放不采纳——charon corpus 可被实现覆写、`Kuddev/pebrel` 的 `.recording` 门槛 1 与 3 均违反，CJK／回滚／重排回归由既有 pin 与契约用例承担 |
| `终端模拟器生态深度研究报告.md` | §19 复核附录七处口径收敛、§20 五组复验补充、§21 跨仓扫描 | 已处置：收敛后的口径并入逐仓行与反面行（Rin 为 `vte` 包装适配层而非手写解析器、kotlin-app 单测仅 2 文件约 18 例、terminator 的 OSC 系分散注释、AW `ReleaseStringUTFChars` 误写对象见反面行、代理项守卫见 `cjk-rendering`）；`wgpu` 零命中与 `libghostty` 仅 sylirre 命中，把「GPU 渲染唯一、仅两家复用 Ghostty」由定性升级为量化确认，结论不变 |
| `8dmusichannels-star-terminator.md` | 全篇（仓库概况、关键代码、对比结论、可借鉴项、测试数据、设计模式、「四次审阅记录」） | 已处置：整体不采纳（零测试资产且 CI 虚假绿灯，见索引表与逐仓行）；其中性项各有去向——`NativePty.c` 与 JNI PTY 桥作 spawn／收尾参考（seccomp fork 变通见「逐份处置补齐行台账」的 `04-` 行，组杀与像素 winsize → `pty-lifecycle`），OSC 注释清单作「OSC 覆盖逐项核对」的生态参照面，超链接 scheme 白名单与双击／三击选择入反面行（`DESIGN.md` 明禁），Sixel／Kitty 图像路由不与本仓 kitty 实现混同；许可维度按「自研 VT ＋不 vendor」双规则只读参考，不引入代码或许可证文件；§「测试数据」自陈断言范围未逐条转录、非全面 VT，登记为无可用测试资产；§「四次审阅记录」为研究期自述，无独立断言 |
| `DanteAlighierin-foot.md` | 全篇（概况、结构、对比、可借鉴项、测试数据、设计模式、许可、「四次审阅记录」与 branding 自陈） | 已处置：整体不采纳（自研 C 解析器违单一来源，见索引行）；可借鉴项逐个裁定——CSI/DCS 解析器结构不采纳（同上），box-drawing 合成与组合 emoji 宽度在本仓无对应实现面（字形由字体／回退层提供，不合成），故不立项；Wayland fractional scaling 属平台不符（本仓 Android）；§「测试数据」自陈未清点可跑 harness，登记为无可用测试资产；MIT 许可无引入动作；报告自陈未核验与上游 foot 的 fork／mirror 关系，不作断言 |
| `GlobalTechInfo-RedTerm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：整体不采纳（门槛 3，见索引表与逐仓行）；中性项逐个去向——以 Termux jar 作回退不采纳（`DESIGN.md` VT 单一来源）、`libproot` 集成违 `PROHIBITED.md` 内嵌 proot／预装发行版禁令、备份「完成后替换＋孤儿清理」已入行、常驻通知（会话数＋退出按钮）已落地、Android 15+ 16KB 页 `.so` 对齐登记在禁改区行、SSH／SFTP／keyring 与多发行版注册表属本仓无此功能面；§「测试数据」329 例覆盖面为 SSH/SFTP/proot，门槛 3 不满足；GPL-3 许可无引入动作 |
| `JetBrains-jediterm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、审阅记录） | 已处置：整体不采纳（门槛 1——before/after 期望即其自身输出，见索引行与语料行）；中性项逐个裁定——vttest 派生语料按「输入分流」逐例判定、不与本仓期望混同；`jediterm-pty`／Pty4J 属 JVM 抽象且平台不符（本仓 PTY 归 Rust），不作后端；终端模型 resize／超链接／鼠标与键盘编码各有对应能力规约或反面行；type-ahead 与 `InputBatchBuffer` 保序同口径、未声明不立项；DECSET 2026 同步输出在原生侧无消费者，登记为未声明不立项；「组件可作 Kotlin 侧期望参考」自陈须先审计期望真值，与门槛 1 同判；Apache-2.0／LGPLv3 双许可无引入动作 |
| `KDE-konsole.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：整体不采纳（桌面 C++ 自研 VT，见索引行）；中性项逐个去向——`tests/`／`autotests/` 期望值取自其自研 Vt102Emulation、须外部真相审计，按门槛 1 不采纳；kitty 键盘协议已有专门能力规约；profile／session 编辑撞 `PROHIBITED.md`，DBus／i18n／tysend（ZMODEM）未声明不立项；copy／paste、按键绑定、历史、搜索与屏幕缓冲／窗口跟踪分别由既有能力规约与上游 `libghostty-vt` 承担（不重实现）；插件系统按「能力裁定补齐行」插件系统行作未声明、不立项；Qt for Android 与 GPL v2 均无引入动作 |
| `KDE-maui-station.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：整体不采纳（QML 集成在 Android 无对应形态，仓内无 VT 资产，见索引行）；中性项逐个去向——搜索／动作视图与移动 UI portal 属平台形态差异、本仓已有对应能力规约，不新立项；嵌入式终端服务与「Service 与 UI 经守护进程分离」可作会话归属 Rust 的对照（不改代码）；`split` 一词已更正（指 Service／UI 分层进程模型，非分屏，见分屏行出处更正）；§「测试数据 unknown」登记为无资产；MauiKit 的 C++／LGPL-GPL 无引入动作 |
| `KDE-yakuake.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：整体不采纳（无新仿真，全在 Konsole，见索引行）；中性项逐个去向——下拉式 Quake UX 无 Android 对应形态、profile／session 复用模式撞 `PROHIBITED.md` 且与 Konsole 行同口径；KPart 集成与 C++ 均不可移植；§「测试数据 None notable」登记为无资产；GPL 无引入动作 |
| `Kuddev-pebrel.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：部分采纳，其余各有去向——release profile（LTO／codegen-units／strip／`opt-level = 3`）已落地 `native/Cargo.toml`，「hot path O3」采纳（实测 `opt-level = "z"` 把键入、单元格实例构建、CJK 解析三条热路径拖慢一倍以上，代价只有原生库增大 18%）；录制回放语料与 `.recording`＋expected grid 不采纳（门槛 1＋3，`ref` 目录同 alacritty `ref` 口径）；PTY 事件循环／线程／redraw anchor 已被本仓 VT 线程与行级脏跟踪越过；damage／cell-grid／resize／search／selection 与字形图集渲染均已有实现，仅作对照；vi mode、分屏、OSC cwd 分别归「能力裁定补齐行」与 OSC 逐项核对表（OSC 7 上游解析、本仓不消费）；§「测试数据」其余用例断言对象为其自研 VT 模型、本仓无该组件；GPL-3.0 无引入动作 |
| `PaNDa2code-zerotty.md` | 全篇（概况、结构、对比、可借鉴项、测试数据、设计模式、许可、审阅记录） | 已处置：整体不采纳（Zig 转换对 Android 无用途、自研 VT 违单一来源、CI／环境无可移植结论，见索引行）；中性项逐个去向——`tests/` shell tests 未编目且为自研 VT 差异，门槛 3 不采纳；Vulkan 性能目标与本仓 wgpu／Vulkan 同向，仅作对照不新立项；配置 UX 已有 `settings-propagation`；「自研 VT 血统不可核验」按报告自陈记录，不作断言；MIT 许可无引入动作 |
| `PerBothner-DomTerm.md` | 全篇（概况、结构、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可） | 已处置：整体不采纳（Web UI 栈形态，无 Rust 依赖可嵌入，见索引行）；中性项逐个去向——WebSocket／服务端拆分／DOM 单元布局／Electron-Qt 集成归「能力裁定补齐行」WebSocket 行（未声明、不立项）；REPL 式「客户端执行与结果展示」属其产品形态、本仓无对应面，不立项；§「可改用的设计」自陈 JS／web 资产不得嵌入（本仓渲染归 Rust），与 `DESIGN.md` 一致；§「测试数据」为 shell 集成、平台不符，登记为无资产；许可自陈不确定，无引入动作 |
| `RohitKushvaha01-ReTerminal.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、停更） | 已处置：整体不采纳（门槛 3，VT 走上游 jar、proot 禁入，见索引行与同源行）；中性项逐个去向——Material 3 Compose 壳与文档提供器／透传名单形已在 `documents-provider` 等规约与既有行；多模块 `core:*` 布局与 fastlane 元数据结构本仓无对应面，不立项；以 Termux View 交互代码作 UX 基线但不作依赖（不 vendor ＋单一来源）；§「测试数据」仅 androidTest／test 资源、断言范围未枚举，登记为无本仓测试资产；仓已停更如实记录 |
| `TermoraDev-termora.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 VT 违单一来源，见索引行）；中性项逐个去向——KeyEncoder／Selection／Find（含 `SubstrFinder`）／鼠标模式与追踪／滚动模型与回滚／终端读取器在本仓已有等价实现，仅作对照；控制序列矩阵、字符集 designate、OSC 与 TextStyle 归上游 `libghostty-vt`（单一来源，本仓不逐项重实现），逐项覆盖以 OSC 核对表为准；插件市场按「能力裁定补齐行」插件系统行作未声明、不立项；ZMODEM 未声明不立项；§「测试数据」25 个用例与 `issue-564`／`issue-1055` 断言对象为其自研 KTerm、部分依赖 SSHD mock 与网络，门槛 1＋3 不采纳；AGPL-3.0／商业双许可无引入动作；其自评与桌面系同属 C 级参照面的结论维持 |
| `TragicWarrior-libvtty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VT 核不可换，C 库经 NDK 引入净增 FFI 且违单一来源，见索引行）；中性项逐个去向——按键映射表（vt100／rxvt／xterm）与颜色缓存／RGB 近似算法只作结构对照，不引入 C 代码；鼠标驱动与模块化 CSI 命令结构本仓已有对应实现面；`test/*.suite`＋`.reference` 为第三方 VT 自身行为、门槛 1＋3 不采纳；VT 覆盖矩阵未给、KMScon 血统未确认，报告自陈按源码确认为准，不作断言；LGPL 2.1 core 直接链接触发分发／链接义务，无引入动作 |
| `Yazelix-mars.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（alacritty 核冲突，见索引行）；中性项逐个去向——`sugarloaf`／`libsugaloaf` 承接 render/font 的 spike 维持不做；`teletypewriter` 属第三方 PTY，本仓 PTY 归 Rust 自实现；字形基线与 `cell baseline` 放置、字体回退与等宽 sizing 本仓已有等价实现（`face_covers`／`other_families`）；kitty 图形与源矩形已在 `render/kitty.rs`＋`kitty_graphics.wgsl` 落地，仅作对照；链接命中段与边缘标点裁剪归 DESIGN「链接经 libghostty-vt 查询、无额外检查」，自研扫描不采纳；配置规格清册与 Nix 包元数据契约本仓无对应面（`flake.nix` 属禁改区），不立项；§「测试数据」计分卡／benchmark／alacritty cell grid／Rio 渲染测试门槛 1＋3 不采纳，性能方法学由既有基准承担；多 crate 许可声明无引入动作 |
| `ademmenh-zebra.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VTE on GTK3 极简 demo，无新洞见，见索引行）；§「可借鉴」「可改用的设计」自陈「almost none」「None」，无待办项；§「测试数据 none」登记为无资产；报告自评「T grade」不在台账 S/A/B/C 参照面语义内，按「仅记录在案」处理；GPL-3.0 无引入动作 |
| `alacritty-termbenchbot.md` | 全篇（概况、代码要点、可直接用、可间接用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：仅参照不引入（见指标维度行与索引行）；逐项去向——`bench.sh`／`xinit`／systemd 依赖桌面环境，Android 须另建 NDK harness，不直接复用；vtebench 输入生成脚本与 15 家终端调用配置作加速边界对照，本仓基准已由 `native/benches` 与设备侧宏基准承担；指标维度（`cursor_motion`／`dense_cells`／`scrolling`／`unicode`）按「仅参照」处置；数据库／时间戳／绘图存储设计仅在新增内部基准套件时才考虑，不立项；历史测量数据与阈值须在目标设备重算，不得直接沿用；§「测试数据」自陈只有 benchmark 场景、无正确性 fixture 与单测，登记为无测试资产；MIT 无引入动作 |
| `aligrudi-fbpad.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（framebuffer 单头、无 Android 落点，见索引行）；§「可借鉴」的极简 cell 渲染与终端状态实现属自研 VT 面，违单一来源，不引入（且不属于本仓 `libghostty-vt` 的职责边界）；§「可改用的设计 None」与 §「测试数据 None」登记为无待办、无资产；许可：根目录未发现 LICENSE／COPYING 且无显式 SPDX，报告自陈未找到，不作臆断也不作判据；C 级结论维持 |
| `benyblack-ntilde.md` | 全篇（概况、代码要点、可直接用、可间接用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：仅参照不引入（见索引行与差异矩阵行）；逐项去向——与 Ghostty 的差异／覆盖矩阵思路仅作对照，语料为该 .NET VT 自身输出、门槛 1 不采纳；MCP 服务器与 AI agent 控制撞 `PROHIBITED.md` 终端节明禁（口径更正行）；确定性渲染与回放调试（快照＋golden＋像素／行比较）在本仓已有等价手段——`native/src/render/cell_builder.rs` 的 `CachedInstances` 行级增量缓存与 `cached_incremental_rebuild_matches_full_build`（与完整重建逐实例比对）加行级脏跟踪帧带断言，不新增图像 golden 基线（易被字体与 GPU 差异腐蚀，且违门槛 2）；字形缓存／RowCache 与字体回退布局、Rust PTY FFI 结构与本仓图集／`face_covers`／`ffi.rs` 对照；Skia+SkiaSharp 渲染模式与 .NET/Avalonia 运行时平台不符；命令辅助 provider 栈、mux 守护进程与会话序列化（持久化撞明禁）、`RenderPerfWriter` 度量导出均不立项；§「测试数据」VT／MCP／mux 用例断言细节未展开，登记为无可用资产；MIT 无引入动作 |
| `borisfaure-terminology.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 terminal state 违单一来源、EFL 不在 Android，见索引行）；逐项去向——`tests/` 的 shell 回归脚本期望写在脚本内并以人眼读 diff 为主，报告自陈须改写为 expected-output 审计，按门槛 1＋未编目不采纳；链接探测用例归链接判定行（本仓经 `libghostty-vt` 查询、无自研扫描器）；主题／配色格式撞 `PROHIBITED.md` 自定义主题／调色板编辑明禁，不采纳；inline 图像与媒体播放属未声明面（本仓图像走 kitty 图形协议，视频播放不立项）；tysend 类文件传输未声明不立项；BSD-2-Clause 无引入动作 |
| `contour-terminal-contour.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 VT backend 违单一来源、C++/OpenGL 与 Android Vulkan 不兼容，见索引行）；逐项去向——`vtconformance` 与 377 例按门槛 1 不采纳（脚本只发序列等待、部分依赖人工按键，期望须审计）；解析器事件设计与 bulk scan 仅在自研 VT 前提下才适用，不立项；字形图集、光栅化、box drawing、下划线几何、文本簇分组与图像纹理本仓已有等价实现或归上游引擎；synchronized output 与 OSC 8 归既有 OSC 核对表口径、reflow 属上游职责；sixel／regis 图像协议按未声明处理（本仓图像走 kitty 图形协议）；daemon／持久会话协议撞 `PROHIBITED.md` 禁会话持久化；vcpkg 依赖栈无 Android 落点；许可表述不确定，无引入动作 |
| `cutefishos-terminal.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（QML／C++ 形态在 Android 无对应，见索引行）；逐项去向——`qmltermwidget` 的 QML surface 集成只在其「内嵌 web view」设想下成立，本仓禁 Web UI 栈且渲染归 Rust，不采纳；桌面版式与配置特性无 Android 对应面；§「测试数据 None」登记为无可评估资产；所嵌控件血统报告自陈未逐 crate 编译确认，不作 Android 可移植的断言；Model／View 划分与本仓 Rust／Kotlin 分层同构，仅作对照；许可表述不确定，无引入动作 |
| `dakra-ghostel.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：仅参照（同引擎第四家；FFI 模式、脏跟踪与 pin 语义见索引行，`RenderState::update` 加行／cell 迭代即本仓的对应实现）；逐项去向——PTY resize 像素字段与 native PID 语义归 `pty-lifecycle`；kitty graphics 发射（清屏命令与 placement）本仓 `kitty.rs`／`kitty_graphics.wgsl` 已有，仅作对照；输入与键编码、超链接、鼠标、样式面映射各有能力规约；预编译模块安装流程的信任 UX 在 Android 由 APK 原生库承担（分发流程归禁改区 CI，不引入）；`src/tests.zig` 与 `test/hypothesis` 渲染 JSON 语料／property tests 门槛 1＋3 不采纳（断言对象为 Emacs 环境投影），可视回归设计仅作参考；上游 rev 追踪口径维持「以 `Cargo.toml` 所钉 rev 为准」；GPL-3.0-or-later 无引入动作 |
| `elementary-terminal.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VTE 与单一来源冲突、GTK/Vala 无 Android 落点，见索引行）；逐项去向——VTE shell 设置集成（GApplication settings schema）无 Android 落点；不安全粘贴对话框撞 `PROHIBITED.md`（补齐行已记，含其唯一相关用例 `UnsafePasteTest`）；搜索工具栏设计对照本仓全文搜索规约（已有实现，见「全文搜索」能力），仅作产品面对照；Elementary／GNOME 主题约定撞主题明禁；缩放浮层无待办（字号缩放由设置面承担）；§「测试数据」两例登记为无可评估资产；GPL 无引入动作；C 级结论维持 |
| `felixse-FluentTerminal.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VT 与渲染都在 xterm.js／WebView，已停更，见索引行）；逐项去向——Shell Profile 模型、设置按页面拆分、更新服务、托盘 IPC 与通知服务组织分别撞 `PROHIBITED.md`（自定义会话／设置节）或无 Android 对应面（托盘 IPC、UWP 更新服务），不立项；iTerm／Fluent 主题解析与其 `AdventureTime.itermcolors` 样例撞「自定义终端主题」明禁，且主题列表固定、构建期读取（补齐行已记）；Web UI 事件桥（`TerminalBridge`）属浏览器／WASM host 模式，本仓禁 Web UI 栈；MVVM／XAML 分层与配置序列化字段设计仅作对照，须按本仓设置契约重写；§「测试数据」为 .NET 服务层单测（xunit＋Moq＋AutoFixture），既无 VT／渲染语料也无 Android instrumentation，登记为无可评估资产；GPL-3.0 无引入动作 |
| `floooh-chipz-kitty.md` | 全篇（概况、对比、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Kitty 图形由上游承担、无终端核心内容，见索引行与 Kitty 取用判定行）；逐项去向——Kitty 图形协议的支持要求与终端图像渲染流程对照本仓 `native/src/render/kitty.rs` 与 `kitty_graphics.wgsl`（图形由上游引擎解析、本仓只负责纹理与放置），无新增需求；Zig／libvaxis 渲染风格与游戏 ROM／trace 无关本仓；§「测试数据 none」登记为无资产；许可自陈「MIT unknowns」，未确认，不作判据、无引入动作 |
| `fyne-io-terminal.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Go 模块无 Android Rust 落点，见索引行）；逐项去向——CSI／OSC／DCS 解析器结构设计课仅在自研 VT 前提下有效，本仓归 `libghostty-vt`，不立项；`test_data` 与 `*_test.go` 属其自研解析器期望、门槛 1＋3 不采纳；Go 跨平台 PTY 集成与本仓 Rust 侧 PTY 实现对照，不引入；「终端状态对象＋转义处理器＋渲染更新」模式本仓同构（引擎状态＋行级脏跟踪）；许可表述未确认，不作判据；C 级结论维持 |
| `ghreprimand-odytty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 VT 违单一来源，见索引行）；逐项去向——`src/paths` 的「仅用 std」架构纯度规则与本仓「Rust 核心隔离于宿主回调」同向，属既有实现约束，不新增条文；wgpu 文本渲染与 box drawing 组织本仓已越过（字形图集＋cell 实例化）；无障碍控制与朗读形态撞 `PROHIBITED.md` 明禁；主题与 config／keybinding 无待办（撞明禁或未声明）；§「测试数据」`fuzz/`、`benches`、`tests` 序列与 native GPU 测试套件属其自身 VT 期望，门槛 1＋3 不采纳；其 sequence coverage 报告自陈须按源码 matrix 测、未展开，不作断言；wgpu 版本差异仅作对照；GPL-3.0-only 无引入动作 |
| `godot-pty-gpty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（MCP 明禁、事件总线与平铺多路复用未声明、`portable-pty` 无 Android 后端、GPL-3.0-only 前置，见索引行）；逐项去向——JSON-RPC／MCP 终端控制撞 `PROHIBITED.md` 终端节明禁（口径更正行）；`Capture lines` 与命令捕获归「输出分块与命令状态（OSC 133 语义）」未声明不立项；分屏／平铺面板未声明不立项（含出处更正）；PTY 环境消毒规则与本仓 `pty-lifecycle` 的子进程侧 fork／exec 不变量（`IUTF8`、`IXON`／`IXOFF`、setsid＋TIOCSCTTY、dup2）同向，属既有实现；`portable-pty` 依赖 `/dev/ptmx` 假设与 Android sandbox 冲突，本仓 PTY 自实现，不引入；许可元数据（声称 MIT／Apache）与根 LICENSE 的 GPL-3.0-only 不一致，如实记录且不作引入；§「测试数据」crates tests 与 Godot 集成测试门槛 1＋3 不采纳；Core／bridge 分离与 commands-events 模式仅作对照 |
| `harmoninux-HiSH.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（QEMU 全虚拟机编排、仓内无 VT 解析器与渲染，见索引行）；逐项去向——VM 生命周期、镜像下载与存储、端口转发属未声明面，不立项；移动端特殊键做法对照本仓输入／额外按键规约（已有实现）；产品打包与分享形态无 Android 对应；qemu 跑 RISC-V 的路径报告自陈未实测、超出范围，不作断言；§「测试数据」自陈无 VT 测试，登记为无资产；许可要求署名（custom）如实记录，无引入动作 |
| `hugomyb-Lume.md` | 全篇（概况、代码要点、可直接用、可间接用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（Web/Tauri 栈、xterm.js 渲染、replay ring 与会话持久化撞明禁，见索引行）；逐项去向——命令面板／shell 补全 overlay、远程会话（配对／隧道／移动端控制）、AI 助手集成分别属未声明不立项；OSC 133 命令块与输出分块仅作上游能力对照，本仓不消费；`portable_pty` 在 Android 的 sandbox 限制沿用既有裁定（无官方后端，本仓 PTY 自实现）；`native_grid` 的跨语言重同步与 xterm dump 快照范式不适用——本仓网格数据全程在 Rust 内、走脏跟踪（跨语言传输行已记）；会话写锁／扇出与进程组收尾、组杀对照本仓既有 PTY 收尾与界通道实现；事件 JSON IPC 的时延／内存开销对照「跨 JNI 扁平数据 MUST 用直接量而非 JSON」条；配置与工作区 schema 撞明禁或未声明；§「测试数据」`#[cfg(test)]` 广度与前端测试均未转写，登记为无可用资产；MIT 无引入动作 |
| `hugopl-batata.md` | 全篇（概况、代码要点、可直接用、可间接用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（VTE-based GTK4 桌面应用，无 Android 原生核，见索引行）；逐项去向——平铺／stack／leaf 布局与 Compose 面板设想属分屏／多窗格未声明面，不立项；`match_add_regex` 与 hyperlink hover 触发模式撞自研正则／URL 扫描明禁（链接经 `libghostty-vt` 查询，无额外检查）；「以 VTE 为基线校验 Ghostty 独立语义」不成立——VTE 是第三方 VT，其行为不构成外部真值，期望值须来自标准文本或上游协议文档（采纳门槛 1）；Gio settings 持久化与主题 schema 无 Android 落点（主题另撞明禁）；其 `spec/` 报告自陈未确认可在当前运行时执行、属 Crystal 架构细节，登记为不可运行资产；MIT 无引入动作；低相关结论维持 |
| `iewnfod-lumina-terminal.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Tauri＋xterm.js WebView 形态，见索引行）；逐项去向——MCP 服务器撞 `PROHIBITED.md` 终端节明禁（口径更正行）；命令面板与 shell 补全 overlay 未声明不立项（其中 AI 助手面已按出处更正移出）；标签／profile 管理与工作区序列化撞会话持久化明禁；autoproxy 同步属未声明面，不立项；xterm.js 的 unicode／graphemes 插件能力对照本仓字素与 CJK 宽度实现（已有，归 `cjk-rendering`）；`portable-pty` 无 Android 后端，沿用既有裁定；§「测试数据」`tests/` 为前端／应用测试且断言未展开，登记为无可用资产；MPL-2.0 无引入动作 |
| `ii8-havoc.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（libtsm 为可替换 VT 核，违单一来源，见索引行）；逐项去向——其仓内 vendored libtsm 副本的所有权问题与 `BUILD.md`「可以从外部下载可以自动处理的文件不得 vendor」双规则同向，记录备查；Wayland shm 渲染形态（可能 CPU 绘制）与 Android 无交集；极简终端架构与键位绑定无新增需求（输入归 `key-input-encoding`、额外按键已有实现）；§「可改用的设计 None」与 §「测试数据 None」登记为无待办、无资产；MIT 无引入动作；C 级结论维持 |
| `isene-glass.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（仅 x86_64 汇编 + 直接 X11 wire，无 Android 落点，见索引行）；逐项去向——X11 wire 实现仅桌面可用、与本仓 Android 图形栈无交集；内嵌 emoji 缓存与「极小二进制」概念已被本仓字形图集＋字体栈越过，不引入；直系统调用设计属平台形态差异；其 VTE 血统报告自陈未确立，不作断言；§「测试数据」只记 `test` 目录与 emoji 缓存存在、未列断言，登记为无可评估资产；Unlicense 允许复制但无需要，如实记录；C 级新奇怪例结论维持 |
| `ismail-yilmaz-Bobcat.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VT 为外部 `TerminalCtrl` 且非 Ghostty、U++ 非 Android，见索引行）；逐项去向——vi-like 选择器与按键绑定未声明不立项（可配置快捷键另有明禁）；图像协议中的 kitty 已由本仓 `kitty.rs` 承担，sixel／iTerm2／Jexer 属未声明面，不立项；正则搜索撞明禁（本仓搜索规格无正则）；profiles／split 无 Android 对应且分屏未声明；Linkifier／Finder／QuickText 对照本仓链接查询与选区规约（已有）；Web／headless 后端形态不适用；§「测试数据 None specific」登记为无资产；其 `TerminalCtrl` 外部依赖报告自陈未在本报告分析，不作断言；§「可借鉴」中 Web/HTML5 Canvas 渲染一句在文档中表述损坏（含 XML 片段），不据以立论；GPL-3.0 无引入动作 |
| `javaterminal-TerminalFX.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（JavaFX 无 Android 目标，见索引行）；逐项去向——其自含的 Java 终端模拟属自研 VT 面、违单一来源（索引行未展开处在此补齐），RoboVM 式嵌入不推荐；core（screen／session 抽象）与 local（PTY）解耦模型与本仓 JNI 边界同构，属既有实现，仅作对照；PTY 后端依赖（pty4j／JNA）无 Android 落点；§「测试数据」core 测试自陈 VT 合规覆盖有限，登记为无可评估资产；模块化 UI／进程分离设计课仅参考；MIT 无引入动作 |
| `joshka-qwertty-term.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（门槛 1：差分期望即其自身实现输出；换核违单一来源，见索引行）；逐项去向——「以 `qwertty-term-vt` 替换 `libghostty-vt` 并赴 Android」撞「禁止项三条保留」的「不更换 VT 核」与换核对照结论（`ghostty_platform_e` 仅 Apple 等），且其页面模型集成成本自陈高，不立项；`vt-diff` 的 176 例差分语料与 fuzz 属第三方 VT 互差分、期望非外部真值，与 bcon 行同口径不采纳；`examples/frame-capture` 的 bytes→PNG 无程序消费者，撞门槛 2（不引入无消费者的二进制资产）；refcount 样式、页式回滚、Kitty 图形模式、原生 Rust PTY 与「渲染 run／脏证明」对照本仓行级脏跟踪与既有实现，仅作对照；`miri` 子集登记为无可用资产；Metal／CoreText 渲染与 Android 无交集；MIT 及其上游署名说明如实记录，无引入动作 |
| `kemokempo-iced_term.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（alacritty 核违单一来源、ICED 非 Android UI 目标，见索引行）；逐项去向——cell 绘制、字体度量与调色板主题、命令／事件通信模式、PTY shell 设置与键位映射分别归「第三方渲染实现」不采纳行（本仓图集＋行级脏跟踪＋渲染线程帧所有权已有等价）或输入／设置面（未声明或已有实现）；其 `alacritty_terminal` 的 `Cell`／ANSI 类型不引入；§「可改用的设计」建议 MIT 借码做渲染适配，因已有等价实现且换核面冲突而无收益，不采纳；§「测试数据」examples 与 `#[cfg(test)]` 未展开，登记为无可用资产；MIT 无引入动作 |
| `ketraterm-KetraTerm.md` | 全篇（概况、代码要点、可直接用、可间接用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（JVM/Swing 无 Android 目标、oracle 为自身实现输出，见索引行与语料行）；逐项去向——渲染帧租约与 conflated worker 已由本仓渲染线程帧所有权承担（「第三方渲染实现」不采纳行）；屏存储布局（并行原生数组、簇 arena、回滚行回收）与 reflow 归上游 `libghostty-vt`，本仓只读快照，不照搬；选择（含 wrap-aware）、异步搜索、Kitty 键盘、括号粘贴、像素鼠标、OSC 超链接、Unicode 簇／宽度／emoji 与 256 真彩各有既有能力规约；`sync output`、shell integration 元数据（OSC cwd／命令状态）、补全 ranking 与学习、host-owned 权限策略均未声明不立项；Pty4J 与 BOM 依赖模型无 Android 落点；输入编码的 mode-aware 序列对照本仓 IME 与键编码规约；§「测试数据」402 测试文件、JMH 约 30 热路径、testkit／connector 假件与 `tools/xterm-oracle` 门槛 1＋3 不采纳，上游仓库的 `docs/development/conformance-testing.md` 报告自陈未重跑，不作断言；Apache-2.0 无引入动作 |
| `kmscon-libtsm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（libtsm 为可替换 VT 核，违单一来源，见索引行）；逐项去向——「Ghostty 不可用时以 libtsm 作后端」撞「禁止项三条保留」的「不更换 VT 核」（且 `libghostty` 有 Android 外平台约束，换核对照结论已记）；经 NDK＋JNI 引入 C 库的适配成本与收益不成比例，不立项；屏行／属性／选区数据结构与 DEC 模式、状态机边界归上游引擎职责，仅作对照；「自建 harness 与 Ghostty 快照对照」不引入（上游 fuzz 已覆盖）；§「测试数据」`test/` 单元与验收套件按门槛 1＋3 不采纳（期望即其自身行为），其行为参考价值已被上游权威 fuzz 取代；报告自陈 libtsm 源文件名需另行打开确认，不作断言；MIT 无引入动作 |
| `kshivang-BossTerm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（jEditerm 派生 VT 核冲突、Compose 同栈已有实现、LGPL-3.0 审查前置，见索引行）；逐项去向——「Ghostty 集成失败时以 BossTerm 作回退核」不得反向采纳（单一来源）；其 Maven 模块 `com.risaboss:bossterm-core` 与「additional oracle」测试按门槛 1 不采纳；`compose-ui` 面向 Kotlin/JVM 桌面、迁 Android 的改动未验证，不据以立论；`TerminalStarter`／`ScreenHistoryStorage` 为目录占位、报告未穷尽转写，且屏历史／会话共享撞会话持久化明禁，不作断言；ICU4J 字素切分与 split-width 问题对照本仓字素与 CJK 宽度实现（已有），不引入 ICU4J；其「无 JNI 的 Kotlin 直调 PTY」架构与本仓 PTY 归 Rust 的分工相反，不采用；报告自陈 `minSdk=24`，与本仓 `android/app/build.gradle.kts` 的 `minSdk = 33` 不同，该条无落点；telnet／SSH 功能缺口属未声明面；§「测试数据」仅凭路径名声称覆盖、未列方法，登记为无可评估资产；报告 785 行已记 `split` 属误引更正 |
| `leoberbert-zashterminal.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（VTE/GTK 桌面形态，无 Android 落点，见索引行）；逐项去向——会话管理与 SecureCRT 导入撞会话持久化明禁（配置／会话序列化另属未声明面），不立项；文件管理器侧栏、快捷提示、语法高亮与 AI 助手集成（含「显式发送」隐私模型）均属未声明不立项（能力裁定行已记）；**输入广播（多会话同发）**此处补齐判定：`DESIGN.md` 无声明，不立项；GTK 设置面板无 Android 对应；§「测试数据」仅记仓库内存在测试、未列断言且自陈对 VT 无用，登记为无可评估资产；GPL-3.0 无引入动作；C 级结论维持 |
| `lihop-godot-xterm.md` | 全篇（概况、代码要点、可复用库、间接利用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（libtsm 核违单一来源、Godot 运行时非目标，见索引行与语料行）；逐项去向——`Terminal` 的 public API（cols／rows／cell_size／max_scrollback 取设）不需照作，本仓 Compose 侧契约已按自有界面定义；selection／copy API 中的 copy-on-selection 另有不采纳行（复制以菜单点击为唯一入口），`copy_all`／`select` 等对照既有选区规约；bell 冷却／节流不另立一面（限频已在 `runtime/BellPlayer.kt` 覆盖）；ConPTY 的 resize 重试常量（`CONPTY_RESIZE_STABLE_FRAMES`／`MAX_FRAMES`）属 Windows 专有、不适用，Android PTY 的 ECHO／resize 由 `pty-lifecycle` 承担；`libuv`／`node-pty` 与 `portable-pty` 均不引入（本仓 PTY 自实现）；Godot shader 渲染、`signal age` 与 cell 属性图模式归「第三方渲染实现」不采纳行（本仓行级脏跟踪＋图集已有等价）；xrdb 主题导入与主题打包撞自定义主题明禁；JetBrains Mono 字体资产撞 `BUILD.md`「可以从外部下载可以自动处理的文件不得 vendor」（字体取自宿主字体库）；§「测试数据」Godot 测试场景、xrdb 导入测试、asciicast 样例、`colortest.sh` 与 resize／conpty／bell 场景按门槛 1＋3、人工目检与通关记录不采纳，且报告自陈未运行其命令，不作断言；MIT 无引入动作 |
| `lmorg-ttyphoon.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 Go VT ＋ Wails WebView，见索引行）；逐项去向——输出块分组与命令退出状态着色归「输出分块与命令状态着色（OSC 133 语义）」未声明口径（该行已记先前引 ttyphoon 属误引、报告无 OSC 133）；结构化输出块与可点击树部件未声明不立项；tmux 控制面归「插件系统、事件总线、mux daemon」未声明不立项；AI tracing 归 AI 助手集成未声明行；Go VT 与其单元测试按「测依赖行为」不采纳（本仓 VT 语义不由自研测试覆盖）；Go／Wails 前后端分离与 Web 重前端形态无 Android 落点（Compose 原生部件已覆盖其富覆盖层设想）；报告自陈 `virtualterm` 无可移植边界，不作断言；§「测试数据」`test/`、`references_context_test.go` 与 escape-key 测试自陈断言粒度未展开，登记为无可评估资产；GPL-2.0-only 无引入动作 |
| `lxqt-qterminal.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Qt6 壳 + qtermwidget 旧血统 VT，无 Android 渲染路径，见索引行）；逐项去向——DBus 远程控制与界面多语言／i18n 各有未声明不立项行；tab／session 管理与 profile UX 撞会话持久化明禁或未声明面，书签／下拉模式报告自陈对单终端 Android UI 不适用；Model-View 式 widget holder 与桌面 C++ 均无落点；§「测试数据」只记存在 test 目录且以 GUI 为主、VT 期望数据有限，登记为无可评估资产；其 qtermwidget 与 Konsole 同源（GPL）如实记录，无引入动作；D 级结论维持 |
| `marc2332-term.md` | 全篇（概况、代码要点、可直接用、间接有用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（Freya 桌面 GUI，无 Android 目标，见索引行）；逐项去向——tabs／shells／projects 的 JSON 会话持久化撞 `PROHIBITED.md` 会话持久化明禁；分屏／面板拖拽与 tab 叶子拓扑未声明不立项（其 `SubcomposeLayout` 类比仅作 UI 洞见，报告自陈未排期）；git worktree 侧栏／`gix` 集成未声明不立项；`Terminal::new` 公开 API 与「宿主 GUI 持抽象 Terminal child＋回调」模型以「迁移到桌面」为前提，本仓 Android 定位下不立项（本仓 `host-linkage` 已有等价边界）；键盘配置与 `marcterm.toml` 默认值属未声明／明禁面；Flatpak 发布流程无 Android 对应；§「测试数据」自陈应用外无 VT 测试、仓内配置与 test 资产用途未逐一解包，登记为无可用资产；MIT 无引入动作 |
| `marshallku-copad.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（alacritty 核冲突、`copad-mobile` 走 WebView 远程终端，见索引行）；逐项去向——`copad-term` C ABI 桥与「Rust 状态驱动原生 UI」模式本仓 `host-linkage`／ghostty wrapper 已有等价，仅参照；`coctl`／JSON-RPC 与 comux 多路复用、事件总线／触发器、插件（含文档插件服务）归「插件系统、事件总线、mux daemon」未声明不立项；AI 会话 detach／resume 与输出事件流的判定已按出处更正落在会话恢复行；git worktree 集成未声明不立项；**web API／socket 与遥测**此处补齐判定：`DESIGN.md` 无对外服务与遥测面，不立项（本仓不出网、不采集）；其「FFI 是否可用于 Ghostty」自陈不确定，不作断言；§「测试数据」monorepo 各类测试与 E2E、alacritty cell 测试按门槛 1 不采纳；MIT 无引入动作 |
| `midasdf-zt.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Zig 自研 VT、无 Android 落点，见索引行）；逐项去向——批量解析快路径与 SIMD 单元格状态归上游引擎职责，损坏追踪对照本仓行级脏跟踪（已有等价）；O(1) row-map 滚动归上游、内嵌 59k Nerd Font 与「按需加载宿主字体」路线相悖，均不需要／未声明；comptime 配置设计与编译期后端抽象属 Zig 语言面，无落点；§「测试数据」`/tests` 序列输入与基准脚本按门槛 1＋3 不采纳（第三方 Zig VT 差分、基准无本仓落点），且报告自陈测试断言与 runner 未编目，不作断言；其 VT parser 与 crate 的关系报告自陈须从构建文件确认、未确认，不据以立论；MIT 无引入动作 |
| `migueldeicaza-SwiftTerm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 VT 违单一来源、Swift 生态与 JNI 不兼容，见索引行与语料行）；逐项去向——转义序列与 CSI 参数解析设计以自研 VT 为前提，本仓归上游；Kitty 图形、剪贴板、键盘编码、搜索／选区算法各有既有实现（`kitty.rs`／OSC 52／`key-input-encoding`／搜索与选区规约），不引入；175 个 Swift 测试文件与 `EmbeddedCore` 测试按门槛 1 不采纳（期望即其自研实现输出），且报告自陈需移植并做「期望真值审计」；其 `GhosttyFuzzCorpusTests`／`GhosttyFuzzRegressionTests` 的「与 Ghostty fuzz 语料对照」思路曾由上一条落地为 `native/tests/external_benchmarks/upstream_corpus.rs`（按发布标签取数、清单按名排序），该落地已随外部基准整体删除，故此处只作对照、不新增资产；SwiftPM traits 可移植性与锁／队列 I/O、service object 模式属 Swift 运行时面，无 Android 落点；MIT 无引入动作 |
| `mintty-mintty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Win32/Cygwin 自研终端，平台与 VT 均异构，见索引行与语料行）；逐项去向——字符宽度／EAW／ambiguous width／组合与全角表归上游 `libghostty-vt`（本仓渲染只按网格单元像素宽度排布，见 `native/src/render/cell_builder.rs`），其表生成器与「重新生成 Kotlin/Rust 数据」不采纳（低硬编码、不 vendor 可外部获取的数据）；Sixel／Regis 图像协议按既有口径未声明不立项（`DESIGN.md` 未要求该功能，采纳即净增功能而非覆盖），Tektronix 4010/4014 另有未声明行；`minibidi`／RTL 整形按 BiDi 未声明不立项（`PROHIBITED.md` 无该条目，口径已更正）；Win32 搜索／剪贴板／滚动／鼠标交互形态属 Windows 专有，无 Android 落点（本仓 `text-selection` 与剪贴板路径已有）；§「测试数据」`testbidi.tc` 与生成脚本按无消费者不采纳（宽度用例非本仓语料），且报告自陈无 Android／instrumentation 测试、内容以数据生成为主；GPL-2.0-only 与 `STYLE` 不允许直接复用，无引入动作 |
| `mirbehnam-RtlTerminal.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（C#/WPF Win 专有、自研 `TerminalBuffer`，见索引行）；逐项去向——RTL／双向文本整形与界面 `layoutDirection` RTL 各有未声明不立项行（`PROHIBITED.md` 无 BiDi 条目，口径已更正为未声明）；`ConPtySession` 与 ConPTY 会话管理属 Windows 专有，无 Android 落点；「Compose 终端按键设置」属实体键盘快捷键设置明禁面，不立项；自研 VT 违单一来源；§「测试数据」报告仅记存在 tests 目录、内容未知，登记为无可评估资产；MIT 无引入动作（其「Compose 本身支持 RTL」仅作对照，不改为立项） |
| `missdeer-mostty.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不立项（桌面 Win32/macOS、渲染后端异质，见索引行）；逐项去向——SPSC 字节环＋`PostMessage` 交接、ring-full parking／wake signaling、`reader_stop` 关停与 edge-triggered 唤醒已有等价物（本仓有界读链与「同引擎对标仓」PTY→UI 交接模式仅作审计参照，已有更正记录其与本仓同为 libghostty 唯一 VT 核）；单帧 8 ms deadline 与按 pane 2 ms quantum 不立项（属过度设计嫌疑）；tabs／session 关闭行为／子菜单启动器属桌面形态与未声明面；主题加载与配置热重载撞自定义主题明禁（本仓主题构建期读取、列表固定）；ring buffer 改 Rust/JNI 的数组＋atomics 方案不采纳（复用既有界通道）；其 Ghostty VT 交互（title／PWD／超链接查询）中，title 与 pwd 回调按零消费者不采用，OSC 8 超链接本仓已解析并经桥接暴露；§「测试数据」`tools/` PowerShell 脚本与探针、ConPTY 图像策略与渲染器验收测试按门槛 3 与平台专有判为无可用资产，报告亦自陈非正式 VT 语料；根目录未见 LICENSE，报告自陈不据此断言，无引入动作；「Zig＋Ghostty 亦可达高性能」不改变本仓已定 Rust 选型 |
| `mrlima4095-OpenTTY-J2ME.md` | 全篇（仓库类型、概况、可复用库、可间接利用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（非终端仿真器的 J2ME 平台，见索引行）；逐项去向——「内嵌 VM 模式启发插件架构」按「能力裁定补齐行」插件系统行作未声明、不立项；Lua 面向用户的脚本 API、虚拟进程模型与 FUSE 式 RMS 存储均属未声明面，不立项；`ELF.java` 的 RV32IM 加载／跳转槽重定位与 `tests/check_*`、`run_all.sh`、ELF 校验脚本无本仓落点且无终端仿真用例，判为无可用资产；J2ME LCDUI 测试对 Compose 无意义，Java ME mock stub 不必移植（JVM 侧已有测试替身）；报告自陈「能复用于自建模拟器环境」以先建该环境为前提，不立项；**表述更正**：报告第 18 行称「Android 已通过 `libghostty-vt` 内含 RISC-V Linux 环境」为事实错误（`libghostty-vt` 是 VT 解析库，与本仓无 RISC-V／Linux 用户态环境），第 29 行「未声明 WSL 执行路径」在本仓无对应物，均不据以立论；MIT 无引入动作 |
| `nihiL7331-sfte.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（st 衍生的自研 C 单头文件终端，违单一来源，见索引行）；逐项去向——ST 补丁式实现、极简 poll/read 循环与配置覆盖层无落点（本仓读链与配置面已定）；经 NDK 编译该 C 源码报告自陈「不推荐」，不采纳；「极简实现降开销、速度须在目标设备实测」已落在设备性能自动化门禁行名下（现状为宏基准已接入 CI、阈值判定待真机标定，本报告仅为其出处）；§「测试数据」仓内仅 fuzz 宏定义、无语料，判为无可评估资产；根目录无 LICENSE／COPYING 且报告未找到 SPDX 声明，如实登记、无引入动作 |
| `not-elm-orzma.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 `orzma_vt` 违单一来源、WebView 混合形态超本仓范围，见索引行与语料行）；逐项去向——`orzma_vt` 的 VT 解析／超链接／SGR 实现归上游，OSC 8 超链接本仓已解析并经桥接暴露（命名口径归上游），不新增；ratatui widget 与 JS 桥／客户端-服务端协议属桌面与 WebView 形态，无 Android 落点；「终端内嵌实时 WebView」保留为桌面设计洞见、不立项（Android View 嵌接不在报告范围）；其「VT crate 与渲染器分离」与本仓既有分层同构，无改动；生成式 AI 使用政策属仓库治理文档、与产品面无关，不立项；§「测试数据」`crates/orzma_vt/tests` 按门槛 1＋3 不采纳；报告自陈渲染细节来自包清单、后端栈未完全枚举，不作断言；MIT 无引入动作 |
| `nowledge-co-con-terminal.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（GPUI＋**全量** libghostty，Android 无平台支持，见索引行）；逐项去向——`con-ghostty` 的 FFI 安全包装（`once`／`tick`、应用回调、配置串合并与外观回读）在同引擎对照组口径下仅作审计参照，本仓 wrapper 已有等价实现；宿主策略合并中的剪贴板策略对照本仓 OSC 52 有界收割（已有），不透明度／模糊撞透明背景明禁；config string patch 与「sanitized font family」无落点（本仓主题固定、构建期读取）；按平台的原生进程／PTY 集成（Windows ConPTY、POSIX）无 Android 落点，本仓 PTY 自实现；GPUI 与多平台后端模块不适用；§「测试数据」报告自陈 tests 目录与冒烟／集成脚本未完全枚举、`con-test` 的 conformance harness 未重跑，判为无可评估资产（门槛 1）；「若本仓考虑桌面全量 Ghostty 可作比较」与禁止项「不等待完整 libghostty 支持 Android」并行登记，不改变现状；上游 Ghostty 许可报告自陈不明，不据以立论，MIT 无引入动作 |
| `omnipty-terminal-omnipty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（GPUI＋alacritty 桌面终端，见索引行）；逐项去向——自定义字节流事件循环报告自陈「设计素材、未经复核验证」，本仓会话与输出管道已在 Rust 落地，不引入；workspace／侧树与 copy mode 归「vi-like 选择模式」未声明口径及分屏未声明口径，不立项；**TOML 热重载**此处补齐判定：撞自定义主题／配置面明禁（本仓主题固定、构建期读取），不立项；Kitty 协议集成本仓已有（`native/src/render/kitty.rs`，解析归上游）；状态栏 git 检测归 git 集成未声明口径；GPUI 桌面 shell 库模式与 Rust/alacritty 依赖无 Android 落点；§「测试数据」报告自陈 `tests/` 框架与覆盖未展开、另有large image 测试，判为无可评估资产；MIT 无引入动作 |
| `orhun-ratty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Bevy 桌面栈过重、VT 由 Ratatui 承担属核冲突，见索引行）；逐项去向——present 管线／`terminal_present.wgsl` 与渲染系统早于本仓字形图集＋cell 实例化路线，仅作参照；其 font/atlas 悬问已闭合（`src/` 全部 20 个条目内无 font/atlas 模块，字体随 Bevy 默认栈），原「本仓已越过该层」判断维持；Kitty 图像协议实现本仓已有（`native/src/render/kitty.rs`，解析归上游）；MCP server 二进制撞 `PROHIBITED.md` MCP 明禁；`src/plugin.rs` 插件系统归插件／事件总线未声明口径；窗口 warp 与 3D mode（Mobius）／inline 3D 属视觉特效未声明面，不立项（与 CRT／scanline 特效同口径）；`portable-pty` 沿用既有裁定（无 Android 后端，本仓 PTY 自实现，`forkpty` 路径已在 `pty-lifecycle`）；Bevy ECS／camera-present 分离无 Android 落点；§「测试数据」报告自陈以 demo 为主、无显著单测，判为无可用资产；MIT 无引入动作 |
| `paulrobello-par-term-emu-core-rust.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（第三方 VT 核违单一来源，见索引行）；逐项去向——「以 MIT crate 嵌入 Rust Android 层作 VT 备选」与「fork 以扩功能面」均不立项（Ghostty 覆盖面更广，FFI 与测试维护成本另增风险）；cbindgen C API 相对本仓既有 JNI 边界无增益；Python 绑定用于 fuzz／参考对照按门槛 1 不采纳；VT 引擎 API 面、IPC／MCP schema（MCP 另撞明禁）、terminfo 与 `shell_integration` 均属未声明面，不立项；tab／mux 支持归 mux daemon 未声明口径；web terminal 前端归 WebSocket／网络终端形态未声明口径；Kitty 图形本仓已有；§「测试数据」`tests`、`fuzz` 与文档按门槛 1＋3 不采纳（期望即其自身输出、fuzz 语料非上游资产）；报告自陈 `crates/` 绑定清单未展开，不作断言；MIT 无引入动作 |
| `paulrobello-par-term.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：**本条有采纳项且已落地**——GPU surface 尺寸钳制到设备上限与 no-op configure 跳过，已写入 `render-stability` 规约（「Surface 尺寸钳制到设备上限」）；其余逐项去向——内联图像 payload／bounds 与 Kitty graphics geometry（`c`／`r`／偏移）由上游承担，无本仓落点；egui 覆盖层、agent／tool surface 与 ACP 分诊归 AI 集成未声明口径，命令面板归命令面板未声明口径，插件触发器与 mux 归插件／事件总线／mux daemon 未声明口径（MCP 另撞明禁）；分屏未声明不立项；外部 VT 核违单一来源；`par-term-config`／`par-term-keybindings` 子 crate 报告自陈未验证 Android 依赖兼容与维护成本，不引入（本仓配置面自持）；200+ 配置项与 feature flags 属未声明面；§「测试数据」`AUDIT.md`、tests、渲染测试、输入测试套件与配置校验按门槛 1＋3 不采纳；MIT 无额外引入动作 |
| `pg83-shitty.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（自研 `lib/vterm` 违单一来源，见索引行）；逐项去向——damage 驱动 GPU 渲染、resize 事务、字形回退与 glyph cache 归「第三方渲染实现」不采纳行（本仓字形图集＋行级脏跟踪＋渲染线程帧所有权已有等价）；Unicode 17 grapheme 与跨单元连字按既有裁定（`DESIGN` 明示不做跨单元连字）；VT52–VT5xx 语义与历史 reflow 归上游；Sixel 与主题打包分别按未声明与自定义主题明禁不立项，Kitty 键盘已有；内嵌字体撞 `BUILD.md`「可以从外部下载可以自动处理的文件不得 vendor」；PTY socket 黑盒协议、`tst/`（自述 >5000 例）及其收获清单（esctest／xterm vttest／libvterm 等）按门槛 1＋穷举扩张禁止不采纳；`lib/vterm` 许可双轨悬问已实地闭合（基线 GPL-3.0-or-later、其后一手贡献 MIT/GPL3 双选），按自研 VT 与不 vendor 双规则不引入代码；PTY tabs／sessions 属桌面形态与未声明面；§「测试数据」`tst/DIFF.md`、`tst/COVERAGE.md` 与 roundtrip 分区设施登记为无可评估资产（黑盒自验证）；C++/Python 构建体系无 Android 落点 |
| `pop-os-cosmic-term.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（libcosmic 桌面 GUI ＋ alacritty 核，见索引行）；逐项去向——`alacritty_terminal` 作替代 VT 核已按终端状态单一来源单列不采纳；libcosmic 与 `cosmic-text` 整形不引入（本仓按宿主字体库取字形，`font_db.rs`／`rasterization.rs`），GPU 与字体布局的取舍本仓已定；配置配色／Profiles 撞自定义主题明禁，tabs／窗口模型与快捷键／粘贴交互分属未声明面与明禁面（粘贴确认、实体键盘快捷键设置）；`terminal.rs` 的 PTY＋alacritty 用法仅作参照，本仓 PTY 自实现；「term event channel 状态同步」与本仓输出通道同构，无改动；§「测试数据」报告称无显著 VT 测试（alacritty 自带），判为无可用资产；许可表述含混（正文一作「MIT? GPL-3 allowed」），以根 `LICENSE` 的 GPL-3.0-only 为准，无引入动作 |
| `rricharz-Tek4010.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Tek 存储管图形终端仿真，与本仓通用 VT 面无交集，见索引行）；逐项去向——Tektronix 4010/4014 序列未声明不立项（Ghostty 无该协议）；存储管余辉／淡出的「真实速度模拟」属视觉特效未声明面（与 CRT／scanline 特效同口径），不立项；`main.c`／`tek4010.c`／`tube.c`／`ards.c` 与 SDL 绘制无 Android 落点；§「测试数据」`tektests`、动画 `plt` 与 `ardsfiles`／`demos` 按无落点不采纳（属该协议专属语料）；许可报告自陈未确认（「? LICENSE」），不作断言、无引入动作 |
| `sanohiro-bcon.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Linux bare-TTY 自研核，见索引行）；逐项去向——`vte` crate 作测试／垫片按门槛 1＋3 不采纳（第三方 VT 差分，与 qwertty 语料先例同口径）；`vte::Perform`＋Grid 状态架构与 DRM／KMS／EGL／GBM／evdev／xkbcommon 属桌面裸控制台面，无 Android 落点；Sixel 解码器按其来源未确立（报告自陈）不据以立论，且 Sixel 按未声明不立项；panes／splits 未声明不立项；搜索与鼠标拖拽本仓已有等价规约，通知面（终端通知回调）按零消费者不采用；IME 本仓已有规约；「若 Android 绕过 GPU」的设想以 wgpu 已足够为由不立项；§「测试数据」报告自陈 `tests/` 未分类，判为无可评估资产；MIT 无引入动作 |
| `sudo-tee-hollow.md` | 全篇（概况、代码要点、可直接用、间接有用、可改用的设计、测试数据、设计模式、对比、结论、审阅记录） | 已处置：整体仅参照、不采纳（Zig 桌面＋Windows 优先，见索引行）；逐项去向——`src/term/ghostty.zig` 的运行时回调注册、回调空槽资源管理与渲染脏标记生命周期，本仓 wrapper 已有等价实现（`native/src/terminal/ghostty_terminal/internal/mod.rs` 的 `RenderState` 与行／单元迭代），代码不移植；`render_dirty`／空闲 pane 优化与本仓行级脏跟踪同构，无改动；atlas／glyph cache／shaping／kerning 归「第三方渲染实现」不采纳行（本仓字形图集＋宿主字体库路线）；Kitty 图形与分段处理本仓已有（`native/src/render/kitty.rs`），解析归上游；ConPTY 与 WSL bypass helper 含移动沙箱不具的特权、且报告自陈 Android 不需要，不移植；Lua 插件／pane／overlay／widget 属未声明面（同「能力裁定补齐行」插件系统行），配置驱动 keymap／actions 撞 `PROHIBITED.md` 设置节「实体键盘快捷键设置」明禁；§「测试数据」`src/lua/tests`、`src/mux/tests.zig` 与 bench 脚本按门槛 3 不采纳（非 VT 语义语料），`nightwatch` 属 UI／浏览器测试工具、非 VT 断言，报告自陈无纯 VT 语料、无 ConPTY 集成测试；许可未确认（README／build 无明确 OSS 许可字段）且未复核，按不 vendor 双规则不引入代码；同引擎对照组口径已记其与本仓同为 libghostty 唯一 VT 核，报告自陈未逐文件读完 77 个源文件，不作断言 |
| `techsenger-jeditermfx.md` | 全篇（概况、代码要点、可直接用库、间接有用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（JavaFX/JediTerm 派生 xterm 仿真器，见索引行与同源 `JetBrains-jediterm` 行）；逐项去向——`src/test/resources/testData` 的 before/after 对与 vttest drift 按门槛 1 不采纳（期望文件与其实现在同源），「包成 Kotlin 资源再经 `libghostty-vt` 比对」的方案即以第三方自证期望为真值，仍不成立；`pty4j` 沿用既有裁定（无 Android 后端，本仓 PTY 自实现）；`Terminal`／`DataStreamIteratingEmulator` 的模拟器-缓冲接口仅作对照，本仓 `host-linkage` 的 JNI 边界已定；type-ahead 输入按未声明不立项（`InputBatchBuffer` 已保序）；鼠标超链接类与 OSC 8 本仓已有（解析归上游），**超链接高亮模式枚举**此处补齐判定：`DESIGN.md` 无该交互面，未声明不立项；`UserSettingsProvider` 覆写与深色主题默认值分属设置未声明与自定义主题明禁；tabs、JavaFX UI 与 JPMS 模块化无 Android 落点；§「测试数据」约 23 个 core 单测与语料同门槛、报告自陈无 Android instrumentation 测试，判为无可用资产；Apache-2.0／LGPL-3 双许可无引入动作 |
| `tenox7-aclock.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（玩具级 VT 绘制演示，见索引行）；逐项去向——「VT 绘制序列测试可验证渲染器（类 vettering）」维持为**人工对照目标而非门禁语料**（无期望真值、非上游资产，不纳入任何测试套件）；「纯 escape 绘制式 UI」不立项；§「测试数据」`wanted.txt` 与 `contrib`／`binaries`／`bootimages` 判为无可用资产（含二进制产物）；许可报告自陈未确认（「BSD? LICENSE」），不作断言、无引入动作 |
| `tn5250j-tn5250j.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（IBM 5250 主机接入客户端，非通用 Unix 终端，见索引行与「5250 主帧协议」不适用行）；逐项去向——「legacy 主机接入协议」与 Java 安全／协议模式无本仓落点（非 ANSI／xterm 域，且本仓无主机接入面）；Swing 桌面应用形态与 Ant 构建无 Android 落点；§「测试数据」`src/tests` 为 5250 解析／版面测试、无 xterm VT 用例，判为无可用资产；许可报告自陈未确认（「GPL? or LGPL?」），不作断言、无引入动作 |
| `tuzig-terminal7.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Web-first + xterm.js + Capacitor 混合壳，见索引行）；逐项去向——远程多路复用与会话 UX、WebRTC／`webexec` 网络代理均属网络终端未声明面且超出本仓 local-only 范围，不导入；**跨设备剪贴板集成**此处补齐判定：本仓剪贴板面已有（OSC 52 有界收割），跨设备同步属未声明面，不立项；Capacitor 原生壳与 TypeScript 客户端无 Android 原生落点（本仓 Compose＋Rust 自持）；§「测试数据」Vitest 与 Playwright e2e 按门槛 3 不采纳（Web／xterm.js 集成测试，非 VT conformance）；GPL 无引入动作 |
| `veops-go-ansiterm.md` | 全篇（概况、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Go 跳板机安全过滤库，无 Android／Rust 落点，见索引行）；逐项去向——屏幕回放与序列化纪律、从 VT 序列提取命令的「服务端可改写」设想均属服务端转录面，本仓是用户终端且无录制回放声明，未声明不立项；goroutine parser worker 与本仓读线程＋有界通道同构，无改动；`screen` 与 `stream` 解耦的构造本仓由上游 VT＋渲染分层承担；§「测试数据」integration tests 报告自陈断言未编目、无补目必要（门槛 3）；Apache-2.0 无引入动作（Go 源码不可移植） |
| `vercel-labs-wterm.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（Web/WASM＋DOM 形态，见索引行）；逐项去向——「Pluggable core：`TerminalCore` 接口＋Zig/WASM 与 Ghostty WASM 两套后端」正是单一来源的反面，不采纳（本仓已直接使用 Rust 绑定）；`@wterm/ghostty` 的 Ghostty 封装层内存管理，其中 WASM 线性内存「页面零初始化」修复属 WASM 形态专属、无 Android 落点，其分配器／Stream／screen pages／pin tracking 一类同引擎 FFI 模式已由本仓 wrapper 覆盖（同引擎第四家口径）；无障碍／文本捕获（`readText`）未声明不立项（朗读形态另撞明禁）；WebSocket 传输的背压与重连、框架绑定、`just-bash`／`markdown` 包均属未声明面，仅在日后开网络终端时参考；`check-wasm`／CI 二进制比对的借鉴登记在禁改区候选行（`.github/` 待维护者裁定，本轮不改）；Kitty 图像、OSC 序列、软换行元数据与跨 scrollback 搜索归上游或本仓已有；§「测试数据」Vitest 矩阵、Playwright e2e 与文档站场景按门槛 3 不采纳，报告自陈期望值皆 Web／DOM 行为；Ghostty 封装的 Zig 0.15.2 版本矩阵与本仓工具链无关；Apache-2.0 无引入动作，亦不 vendor 其 Zig 片段 |
| `wilsonglasser-oryxis.md` | 全篇（概况、结构、对比、可复用库、可借鉴项、可改用的设计、测试数据、设计模式、许可、结论） | 已处置：整体不采纳（iced 桌面 SSH 客户端＋alacritty 核，见索引行）；逐项去向——SSH／SFTP／serial／telnet／mosh 协议面与 FIDO2 无本仓落点（本仓为 local-only 用户终端）；「config vault 与跨设备同步」撞会话持久化明禁且含隐私面，不立项；crate tests（含 alacritty VT 套件）按门槛 1＋未编目不采纳；ZMODEM 类文件传输按未声明不立项；iced/wgpu 终端 widget 不移植（本仓 wgpu 渲染自持、UI 为 Compose）；「app＋core＋terminal」分层与本仓既有分层同构，无改动；报告自陈 `oryxis-terminal` 的渲染器耦合关系未判定，不作断言；AGPL-3.0 无引入动作 |
| `z-hunter-Scanline-term.md` | 全篇（概况、代码要点、可直接用、间接有用、可改用的设计、测试数据、设计模式、对比、结论） | 已处置：整体不采纳（Tauri＋React＋xterm.js 桌面 Web 终端，见索引行）；逐项去向——xterm.js／React／WebGL 渲染替换路线违第三方渲染与单一来源两条，且无 Android 落点；`conpty-oxide` 与随包 Windows ConPTY 动态库属平台专有且撞 `BUILD.md`「可以从外部下载可以自动处理的文件不得 vendor」，不移植；`mcp.rs` 的 agent 终端桥（`terminal-n` 句柄、生命周期错误面）撞 MCP 明禁，AI 助手／agent 请求面归 AI 集成未声明口径；会话恢复与多 tab 分属会话持久化明禁与未声明；集成浏览器标签页、preset／home 控制器、CRT 特效分别按未声明与视觉特效未声明不立项；`scanline-virtual-screen` 宿主与模拟器属 Web 渲染器形态，ConPTY 状态机读写仅在日后增平台时有参照意义；§「测试数据」vitest 前端测试按无 VT 资产不采纳（报告自陈断言未编目、无 conformance 套件、无测试矩阵、无性能资产）；`outputs/readme` 的「超轻量、即时读取」设计原则与「文档仍属可用设计空间」仅作参照；报告自陈仓内 `AGENTS.md` 非项目专用，不作依据；MIT 无引入动作 |
| `extensibility-and-remote-backends-2026.md` | 建议的「会话字节传输与生命周期契约」抽象面 | 未声明，不立项：`DESIGN.md` 无远程／网络终端面，`STYLE.md` 禁止实现未声明功能；须先补声明再实现，接口形态限于字节源（写、读、尺寸协商）与生命周期（请求退出、取退出码）两组契约，本地 pid／winsize ioctl／fd 能力留在本地实现侧 |
| `extensibility-and-remote-backends-2026.md` | 建议的 Rust crate 发布元数据与 Android library 交付 | 不采纳（待产品决策）：须改受保护 `Cargo.toml`／`settings.gradle.kts`／`build.gradle.kts`，未获明确同意；且无外部使用者前提下收益近零，与「代码量最小」相悖 |
| `extensibility-and-remote-backends-2026.md` | Herdr 的服务端渲染 surface 模式与本仓 Ghostty 模拟器重复 | 仅参照：服务端权威渲染属未声明面；本仓终端状态单一来源不设服务端渲染后端；若日后接 Herdr，须先在字节流模式与服务端渲染模式之间裁决 |
| `rex-protocol-support-2026.md` | 建议的会话传输面接缝（字节源写／读／尺寸协商 + 生命周期请求退出／取退出码） | 未声明，不立项，但为该形态的唯一前置：本仓 `Pty` trait 混合字节读写与本地 OS 能力（`child_pid`、`get_winsize`、`master_borrowed_fd`、`try_clone_reader_fd`、`set_nonblocking`），`Session::spawn_with_pty` 与 `PtyMaster::new` 为 `pub(super)`，`Session::request_exit` 走本地进程组信号路径；接口形态沿用 `extensibility-and-remote-backends-2026.md` 补齐行已登记的两组契约，本地 pid／winsize ioctl／fd 能力留在本地实现侧 |
| `rex-protocol-support-2026.md` | 建议新增 WebSocket／TLS 传输依赖 | 不采纳（受保护文件未获同意）：须改 `native/Cargo.toml`，属 AGENTS.md 受保护文件；未获明确同意前不得新增依赖 |
| `rex-protocol-support-2026.md` | 外部项目的可复用资产（控制面与数据面双通道、协议 crate 与传输实现分层、Rust 侧持有协议真值、快照记录按完整记录切分、字节计数精确续传） | 仅参照，不引入代码：能力面未声明；其中「协议真值归 Rust 侧」与本仓「终端状态、PTY、VT 线程归属 Rust」同向，「控制面与字节面分离」沿用 `extensibility-and-remote-backends-2026.md` 补齐行已登记的纪律；RexPocket 把协议状态放 Kotlin 侧的做法与本仓分层相反，不反向采纳 |
| `rex-protocol-support-2026.md` | 客户端保存 reconnect token 与远端会话标识以支持应用被杀后接回同一会话 | 撞 `PROHIBITED.md` 终端节「会话数据持久化 / 恢复」：须由维护者在声明阶段划清边界，在划清前不得实现；数据面 resume（服务端保留 60 秒 + 客户端按已收字节计数续传）本身不属该明禁项 |
| `rust-dependency-survey-2026.md` | 建议的依赖候选清单（`rustix`、`thread-priority`、`inotify-rs`、`encoding_rs`、`tabwriter`、`textwrap`、`crossbeam`、`moka`、`cargo-dinghy`、`insta`、`serial_test`、`test-log`） | 逐项不采纳：无落点者各自职责已由既有实现或平台能力覆盖（PTY 路径由 `nix` + `libc` 的 fork/execve 表达、目录监视用 `android.os.FileObserver`、终端字节恒为 UTF-8 且解码归上游、定宽单元格不做弹性列对齐、`flume` 已覆盖四条 MPMC 通道、缓存面已用 `lru`）；`insta` 的快照期望文件入库违反 `TESTING.md` 与「外部数据不入库」，`serial_test` 的全局串行化会掩盖竞争，`test-log` 的第二条日志通路无消费者；改动落在受保护 `native/Cargo.toml` 者（`insta`）未获明确同意前不得新增依赖 |
| `rust-dependency-survey-2026.md` | 建议把 `cosmic-text` 的 `fontdb` 约束抬到 0.24 | 不采纳：`cosmic-text 0.19.0` 的 vendored 清单声明 `[dependencies.fontdb] version = "0.23"`，0.24 越界；且本仓需要 `fontdb::ID` 与 `cosmic_text::fontdb::ID` 同版本，改 `native/Cargo.toml` 会破坏类型同一性。解锁判据是「cosmic-text 发布 > 0.19.0」，不是「fontdb 发布 0.24」；阻断原文见 `native/Cargo.toml` 注释 |
| `rust-dependency-survey-2026.md` | `warpdotdev/*` 的产品面（智能体、云执行、会话共享、AI 集成、协作 subagent） | 未声明，不立项：`DESIGN.md`／`PROHIBITED.md` 无对应条目，按 `STYLE.md` 不实现未声明功能；AI 集成与外部服务撞 `PROHIBITED.md` 终端节「Model Context Protocol」与会话数据持久化明禁；MUST NOT 以「Warp 这样做」作为实现依据 |
| `rust-dependency-survey-2026.md` | 建议的直接依赖补丁升级 `lru` 0.19.0 → 0.19.1 | 已实施：`native/Cargo.lock` 例行升级（两版源码差异实测仅清单 `edition` 改 2024 与 `compile_fail` doctest 位置调整，生产 API 无变化），受保护的 `native/Cargo.toml` 未改 |
| `terminal-performance-comparison-2026.md` | 建议的跨应用统一帧面：`dumpsys SurfaceFlinger --latency <layer>` 取逐帧呈现时序、`--latency-clear <layer>` 清空环形历史 | 采纳为逐图层通道：写入 `performance-measurement` 的「跨应用性能对比的帧面分层与覆盖面」条；实测推翻先前「只覆盖 legacy 图层、BLAST 不可寻址」的判断——名称精确即命中任何承载内容帧的图层，含本项目终端 SurfaceView（BLAST）层（实测 127 行中 14 行非零，清零后随终端输出重新累积）；依据为 AOSP 源码 `SurfaceFlinger::dumpStats`（首行 `getPacesetterVsyncPeriod().ns()`，`traverseLegacyLayers` 精确比较 `layer->getName()`）与本轮设备实测 |
| `terminal-performance-comparison-2026.md` | 图层名的取法与判据：名字须精确为 `<base>#<id>`，`--list` 才是权威来源；只返回首行即未匹配、全零行即无帧记录 | 采纳并补正：写入 `performance-measurement` 的「跨应用性能对比的帧面分层与覆盖面」条与「图层未匹配或无帧记录即判采集失败」场景；本轮补正两点——名称含 `[` `]` `(` `)` 时 MUST 加引号传给设备 shell，否则命令出错且无输出（判采集失败）；「全零行」只说明该图层没有内容帧记录，不是「不可寻址」。依据为 AOSP 源码 `Layer::getName()` 返回 `mName`（构造时 `printf("%s#%d", args.name, sequence)`）、`RequestedLayerState::name` 同式，以及帧统计固定输出 127 个槽位行（`NUM_FRAME_RECORDS = 128` 减当前槽）且 `getStats` 以 `desiredPresentTime == 0` 跳过空槽；全量 dump 里的 `Layer [id] <label>` 实测与 `--list` 的 `<name>` 同形 |
| `terminal-performance-comparison-2026.md` | 用 Perfetto `android.surfaceflinger.frametimeline` 作终端内容帧的跨应用判据 | 不采纳：官方文档 `docs/data-sources/frametimeline.md` 明确「SurfaceViews are currently not supported」，本项目终端内容在 SurfaceView 内不在其覆盖内，对照对象走 HWUI 管线在其覆盖内，两者不是同一数据面；理由写入 `performance-measurement` 的「终端内容直绘 SurfaceView 时的帧面取数」「SurfaceView 内容帧的 jank 判定与 N/A 兜底」条与不采纳条 |
| `terminal-performance-comparison-2026.md` | 用图层级 `--latency` 覆盖本项目终端内容帧 | 采纳（实测推翻原假设）：先前依据「BLAST 图层不在 `mLegacyLayers`」判为不采纳，本轮实测推翻——运行中的终端 SurfaceView（BLAST）图层用精确名即可取到逐帧三列，`--latency-clear` 后归零、终端再次输出后重新累积；先前「9 个图层只有 4 个可寻址」的观察实为把「行全零」误记为「不可寻址」。依据与去处同「跨应用性能对比的帧面分层与覆盖面」条 |
| `terminal-performance-comparison-2026.md` | `--frametimeline -all` 的 display frame 下挂着逐图层条目（`Owner Pid`、`Layer ID`、`Is Buffer?`、呈现状态与呈现时刻） | 采纳：写入 `performance-measurement` 的「跨应用性能对比的帧面分层与覆盖面」条；这是唯一同时具备「同口径」与「属主归属」的通道。依据为 AOSP 源码 `FrameTimeline::DisplayFrame::dump` 遍历 `mSurfaceFrames`、`Layer::createSurfaceFrameForBuffer` 传入 `mOwnerPid`/`getSequence()`/`mName`，以及本轮设备实测（终端 BLAST 图层条目带 `Owner Pid` 与 `Is Buffer? = 1`） |
| `terminal-performance-comparison-2026.md` | SurfaceView 内容帧的调度器预测与 jank 分类字段恒为空 | 采纳为判据约束：写入 `performance-measurement` 的「终端内容直绘 SurfaceView 时的帧面取数」「SurfaceView 内容帧的 jank 判定与 N/A 兜底」条；SurfaceView 帧不参与 Choreographer 调度预测（`Token` 为 `-1`、`Prediction State` 为 `None`、`Jank Type` 为 `None`、`Present Metadata` 为 `Unknown Present`），jank MUST 由呈现间隔与刷新周期比较得出；`Present time` 偶为 `N/A`，MUST 以 `Last latch time` 兜底且 MUST NOT 当作 0 |
| `terminal-performance-comparison-2026.md` | 系统侧两通道的保留窗口上限：`--latency` 每图层 127 行、`--frametimeline` 最近 64 个 display frame | 采纳为取数约束：写入 `performance-measurement` 的「跨应用性能对比的帧面保留窗口与帧数口径」条与「保留窗口饱和不作总帧数」场景；实测持续输出时两通道同时饱和（3 秒内应用侧呈现 551 帧，系统侧仍只有 127 行与 64 条），故系统侧通道只给时序，帧数 MUST 取各应用自己的计数口径 |
| `terminal-performance-comparison-2026.md` | 两条渲染管线在系统侧逐图层通道的同通道实测：HWUI 的系统 Settings 窗口图层 `--latency` 127 行中 41 行非零、`--frametimeline` 条目带 `Owner Pid`；本项目的 BLAST SurfaceView 层同样命中两通道 | 采纳为可对比性依据：写入 `performance-measurement` 的「跨应用性能对比的对照面与统一帧面」条；HWUI 与 SurfaceView 两条管线的终端内容都落在系统侧逐图层通道内，取数口径一致 |
| `terminal-performance-comparison-2026.md` | 对照对象的包名与本项目撞名：上游 Termux 的 `com.termux` 与本项目 `applicationId` 相同，同设备安装实测报 `INSTALL_FAILED_VERSION_DOWNGRADE` | 采纳为编排约束：写入 `performance-measurement` 的「跨应用性能对比的对照面与统一帧面」条与「对照应用与被测应用撞包名」场景；同设备 MUST 轮换安装或给对照构建改包名，并记录 APK 校验和与安装顺序 |
| `terminal-performance-comparison-2026.md` | 建议的七面采集清单（跨应用帧时序、应用内帧时序、进程与系统状态、能耗与热、崩溃与退出、日志与制品、深度 trace） | 采纳：写入 `performance-measurement`；采集侧全量落盘、不筛选、不限级，按 tag 或关键字的过滤只发生在分析侧；空结果判红（沿用既有「采集为空即判红」条） |
| `terminal-performance-comparison-2026.md` | 建议的 12 类场景矩阵与设备环境固定化清单（刷新率、固定性能模式、温度注入、动画缩放、电池、屏幕与网络、字体与网格） | 采纳：写入 `performance-measurement` 的「全场景矩阵与环境固定化」条；同一场景给各应用完全相同的字节流与网格尺寸，负载不等价则对比无效 |
| `terminal-performance-comparison-2026.md` | 建议的四类失败归因规则（应用归因／系统归因／人为与编排归因／采集失败）与 `REASON_OTHER` 口径、证据不足记「未定」 | 采纳：写入 `performance-measurement` 的「失败归因与采集失败」条；归因依据只来自 exit-info reason、FrameTimeline jank 类型、logcat 与 dropbox 四源交集 |
| `terminal-performance-comparison-2026.md` | 建议的极端场景资源上限（场景级 wall-clock 上限、文本与 trace 输出上限、`process_stats` 轮询下限、制品目录配额、单被测应用并发上限）与 JSONL 加制品目录的落盘格式 | 采纳：写入 `performance-measurement` 的「极端场景的资源上限」条；超限即终止并记录，不为失败场景继续消耗设备资源 |
| `terminal-performance-comparison-2026.md` | 建议用宏基准 `FrameTimingMetric` / `StartupTimingMetric` 作跨应用主判据 | 不采纳：官方明确不推荐在模拟器上跑；目标应用须非 debuggable；两者量的是各目标自身管线的帧，跨应用语义不同；既有规约已把它定位为「模拟器数量级回归」用途。（补充实测：宏基准库的 `NOT-PROFILEABLE` 错误只在 API 29–30 触发，本仓 minSdk 31 故 `android:profileable` 缺失不构成采集障碍，前稿据此写的「指标无采集前提」已更正） |
| `terminal-performance-comparison-2026.md` | 建议用 `dumpsys gfxinfo` 的帧数作终端内容帧判据 | 不采纳：该入口是 `ThreadedRenderer.handleDumpGfxInfo`（`dumpGlobalProfileInfo` 加 `WindowManagerGlobal.dumpGfxInfo`），只统计经 ViewRootImpl/RenderThread 绘制的帧，SurfaceView 直绘不在其中；与既有「帧级数据面按绘制主体选源」一致 |
| `terminal-performance-comparison-2026.md` | 建议用 Perfetto `android.log` 数据源、跨设备 `gpu.counters`、`screenrecord` 作判据 | 不采纳：`android.log` 仅 userdebug 构建可用（正式版真机改用 logcat 全量落盘）、`gpu.counters` 的 counter ID 由设备描述符给定因而跨设备不可比、`screenrecord` 的编码与分辨率引入偏差只作人工复核制品；理由写入 `performance-measurement` 的不采纳条 |
| `terminal-performance-comparison-2026.md` | 建议新增跨设备多应用编排脚本（设备侧循环安装、场景执行、采集与制品归档） | 只登记，不实施：落点在 `scripts/`，属 AGENTS.md 受保护目录，须维护者明确同意后实施；现有可复用落点为 `android/benchmark` 仪器测试模块与 `scripts/test-emulator.nu` |
| `terminal-performance-comparison-2026.md` | 图层名的取法：`--list` 在 Android 15 起每行是 `RequestedLayerState{<名字> <键>=<值> ...}`，名字要从包装里取；同一 SurfaceView 的父层帧行全为零，内容在 BLAST 子层 | 采纳并补正：写入 `performance-measurement` 的「跨应用性能对比的图层名与图层覆盖判据」条，并新增场景「父层不带内容帧时取子层」；内容层 MUST 以承载内容帧为据选取，MUST NOT 按图层类型认定。依据为 AOSP 源码 `SurfaceFlinger::listLayers` 调 `layer->getDebugString()` 与本轮设备实测（父层 `--latency` 帧行全零、BLAST 子层有 127 行） |
| `terminal-performance-comparison-2026.md` | 图层名的转义按命令通道分两类：经真实 shell 加引号，`UiAutomation` 采集时原样传入 | 采纳并更正先前结论：先前只记「名称 MUST 加引号」，对宏基准通道是错的——写入 `performance-measurement` 的新条「跨应用性能对比的图层名在两类命令通道下的转义判据」及两个场景；依据为 AOSP 源码 `android/app/UiAutomationConnection.java` 的 `executeShellCommandWithStderr` 执行 `Runtime.getRuntime().exec(command)`（不经 shell、按空白切词、引号原样传参）与 `android/app/UiAutomation.java` 的 `executeShellCommand(String)` 转发路径，另有设备 shell 实测 `syntax error: unexpected '('` 佐证 |
| `terminal-performance-comparison-2026.md` | BLAST 内容帧的「实际呈现减期望呈现」恒为一个刷新周期左右（实测 p50 16.2ms，刷新周期 16.67ms），逐帧比较会把每帧都判成迟到 | 采纳为判据约束：写入 `performance-measurement` 的新条「BLAST 内容帧的期望呈现时刻不作迟到判据」及场景；丢帧数 MUST 取呈现间隔跨过的刷新槽位数（`round(间隔 ÷ 刷新周期) - 1`），半周期以内的抖动（实测间隔 12.2–20.9ms）MUST NOT 记为丢帧。依据为 AOSP 源码 `Layer::dumpFrameStats` 给出的 `desiredPresentTimesNano` / `actualPresentTimesNano` / `frameReadyTimesNano` 与本轮设备实测分布 |
| `terminal-performance-comparison-2026.md` | 把图层级通道落成仓内可执行采集 | 已实施：新增 `android/benchmark` 的 `SurfaceLayerFrames`（按 `--list` 取图层名、逐候选查帧记录并取承载内容帧者、场景前 `--latency-clear`、场景后解析逐帧三列并按呈现间隔折算丢帧）与 `SurfaceLayerFramesTest`（实测样本解析与无内容帧判空），并接入滚动宏基准（`ScrollPerformanceBenchmark` 断言终端内容层确有帧记录，且槽位比例不低于 0.35）；实测 2026-11-21 本机 AVD（API 35 模拟器，软件渲染）三轮保留窗口内丢帧 10/13/14 帧、间隔 p50 16ms、p90 19–21ms、最大 97–102ms；先按绝对丢帧数设阈值（`MAX_SCROLL_MISSED_LAYER_FRAMES = 21`），后因该量随宿主负载同涨改为槽位比例阈值（见下一行）；规范落点为 `performance-measurement` 的场景「终端内容帧另有系统侧逐图层证据」 |
| `terminal-performance-comparison-2026.md` | 系统侧逐图层通道的丢帧数是保留窗口内的绝对量，宿主被抢占时与窗口内帧数同涨（实测轻到中载丢 6–32 帧、并发编译任务抢占丢 41 帧、8 个忙循环超额占用丢 89–113 帧），不能作阈值 | 采纳为阈值口径约束：写入 `performance-measurement` 的新条「帧面阈值用相对量而非绝对丢帧数」及场景；滚动宏基准的断言从「丢帧数不超上限」改为「槽位比例（呈现频率 ÷ 设备刷新率）不低于 0.35」，该比例无量纲且不随窗口内帧数塌缩；实测槽位比例轻到中载 0.80–0.96、超载 0.49–0.58，阈值取最差实测的约 1.5 倍余量 |
| `terminal-performance-comparison-2026.md` | 启动耗时随宿主负载整体变慢（实测同一构建在超额占用的宿主上冷启动十轮中位数从 2.0s 涨到 3.5s、逐轮峰值 4.5s，原预算 3.4s） | 采纳为判定口径约束：写入 `performance-measurement` 的新条「宏基准的启动耗时判定与宿主负载口径」及场景；启动宏基准由逐轮硬判改为「各轮中位数对预算、最差一轮对硬上限」，预算留出覆盖该情形的余量；改后同一构建在同一宿主上通过（冷启动中位数 3.5s、峰值 3.8s，热启动中位数 1.5s、峰值 2.4s，均在预算内） |
| `terminal-performance-comparison-2026.md` | 启动耗时可拆成首帧段（`startActivityAndWait` 走 `am start -W` 的系统侧口径）与终端界面可见段（UiAutomator 检测口径），两段受宿主负载的影响不同（宿主超载时首帧段从约 1.0s 涨到约 2.9s，检测段基本不变） | 采纳为可观测性要求：写入 `performance-measurement` 的同条与场景「启动耗时分两段记录」；基准逐轮记录两段并在汇总行给出两段中位数，分得清「应用慢」与「检测慢」 |
| `terminal-performance-comparison-2026.md` | 用测试进程内的定长 CPU 循环作宿主速度参照、据以归一化启动耗时 | 不采纳：实测宿主超额占用时该参照只慢 28%–70%（21ms → 28–37ms）且同一轮次内自身抖动近一倍，而冷启动中位数慢了 78%–84%（2011ms → 3468–3694ms）——启动是多线程且含 I/O 的延迟敏感过程，参照是单线程吞吐敏感的，二者不成比例，按参照折算会把宿主慢误判成应用慢；理由写入 `performance-measurement` 的「不采纳的测量手段须记录理由」条与报告第十一节 |
| `terminal-performance-comparison-2026.md` | 显示级帧时间线的时刻以保留窗口里最旧一帧为基准，窗口滑动时基准前移，故它不是稳定时钟（实测相隔 8 秒的两次 dump，最新一帧的相对时刻都是约 1.05 秒；AOSP `FrameTimeline::dumpAll` 取 `mDisplayFrames[0]->getBaseTime()`） | 采纳为取数约束：写入 `performance-measurement` 的新条「显示级帧时间线的时刻基准与跨通道一致性」及场景；按场景时间窗筛选前 MUST 先换算基准，MUST NOT 直接拿 dump 里的相对时刻与场景起止比较 |
| `terminal-performance-comparison-2026.md` | 逐图层 `--latency` 通道给的是绝对纳秒时刻（CLOCK_MONOTONIC，与 `elapsedRealtime` 同域），与显示级帧时间线记录同一批物理呈现，按呈现时刻序列对齐可换算基准（实测 63 个时刻里 62 个在 0.5ms 内对上） | 采纳为换算办法与一致性校验：写入 `performance-measurement` 的同条与场景「两条系统侧通道的呈现时刻必须一致」；换算后 MUST 用同一图层在两条通道里的呈现时刻逐个校验（容差 1ms），对不上判取数失败 |
| `terminal-performance-comparison-2026.md` | 显示级输出里图层条目行尾的 `[*]` 是 jank 标记（AOSP `SurfaceFrame::dump`），不属于图层名；dump 里最后一帧的呈现时刻常为 `N/A` | 采纳为解析约束：写入报告的显示级小节与实现（`FrameTimelineFrames` 解析时去掉标记、把 `N/A` 记为无值不参与计算）；图层名 MUST 与 `--list` 给的名字同形 |
| `terminal-performance-comparison-2026.md` | 把显示级帧时间线落成仓内可执行采集 | 已实施：新增 `android/benchmark` 的 `FrameTimelineFrames`（解析逐 display frame 与逐图层条目、按属主归属、按基准偏移筛场景时间窗、两条通道呈现时刻校验）与 `FrameTimelineFramesTest`（实测样本解析、时间窗筛选、基准换算与一致性校验），并接入滚动宏基准（2026-11-21 实测三轮各归属到 63 帧终端内容，基准偏移逐轮不同） |
| `terminal-performance-comparison-2026.md` | exit-info 归因通道的实测语义：dump 按 `ApplicationExitInfo #N` 分块（块内三行一组）、无退出记录时只有 dump 头、时刻是墙钟本地时间、死亡后立即可见、公开 reason 常量止于 `REASON_PACKAGE_UPDATED`（`REASON_MEMORY_LIMITER` 自 android-37.2 起才公开） | 已实施：新增 `android/benchmark` 的 `ExitInfoRecords`（分块解析、四类归因、场景时间窗筛选、窗口内退出即判红并附归因）与 `ExitInfoRecordsTest`（8 个单测，用实测 dump 字面量断言），并接入冷启动／温启动／滚动／三组交互动画宏基准；端到端验证用「APK 内缺 native 库 → 启动即崩」构造真实崩溃，冷启动宏基准判红并给出 `APP CRASH(EXCEPTION)` 归因；`performance-measurement` 的「进程退出归因的采集与判红」条与「失败归因与采集失败」条；已落地代码 `android/benchmark` 的 `ExitInfoRecords`、`ExitInfoRecordsTest`；`docs/research/terminal-performance-comparison-2026.md` |
| `terminal-performance-comparison-2026.md` | 每次测量须记录的环境项（设备型号、ROM 构建号、API、刷新率、电池电量与状态、电池温度、热状态与被测应用版本标识）与「环境漂移被记录」的口径 | 已实施：新增 `android/benchmark` 的 `DeviceEnvironment`（`getprop`、`dumpsys battery`、`dumpsys thermalservice`、`dumpsys display`、`dumpsys package` 五路采集并逐项严格解析，任何一项读不到即判红）与 `DeviceEnvironmentTest`（10 个单测，用实测输出字面量断言），每次测量前采集并随实测行落盘（`[bench-env]` 行）；环境固定化（刷新率锁定、性能档位、温度注入）只记录未固定，登记为待做；`performance-measurement` 的「测量环境快照的采集与判红」条与「全场景矩阵与环境固定化」条；已落地代码 `android/benchmark` 的 `DeviceEnvironment`、`DeviceEnvironmentTest`；`docs/research/terminal-performance-comparison-2026.md` |
| `terminal-performance-comparison-2026.md` | 数据落地格式（每轮一份 JSONL 与一个制品目录、行字段表、制品附校验和、制品配额轮转）与宏基准框架的准备轮次事实 | 已实施：新增 `android/benchmark` 的 `MeasurementArtifacts`（行记录序列化、制品写入与 SHA-256、配额轮转、超限丢弃）与 `MeasurementArtifactsTest`（6 个单测，校验和用公开标准向量断言），并接入各宏基准（每轮一行，目录落在测试输出目录里并随测试结果回到宿主）；框架的准备轮次在行里以 `measured=false` 标出（实测温启动 11 行对应 10 个 trace）；`performance-measurement` 的「测量结果的落盘格式与配额」条与「极端场景的资源上限」条；已落地代码 `android/benchmark` 的 `MeasurementArtifacts`、`MeasurementArtifactsTest`；`docs/research/terminal-performance-comparison-2026.md` |
| `terminal-performance-comparison-2026.md` | 跨应用启动对比面：系统侧 `am start -W` 的 TotalTime 是跨应用唯一同口径的启动耗时（各应用自报的启动指标跨应用不可比）、`cmd package resolve-activity --brief` 的输出形态与未安装时的失败判据、权限弹窗抢走启动（系统侧 `Activity` 变成权限控制器的 Activity）、冷启动准备动作（强停）自身留下的退出记录 | 已实施：新增 `android/benchmark` 的 `AppLaunch`（解析 `am start -W` 与 `resolve-activity` 输出、预授权限、强停与拉起）与 `CrossAppLaunchBenchmark`（仪器参数 `targetPackages` 驱动逐包冷启动、校验拉起的是被测应用且启动态为 `COLD`、逐轮落盘 TotalTime 与 WaitTime 并给 p50/p95）与 `AppLaunchTest`（9 个单测，用实测输出字面量断言）；写入 `performance-measurement` 的新条「跨应用启动对比的系统侧口径与准备动作」及五个场景；实测同轮内两个包各五轮全部通过（p50 873ms 与 689ms，模拟器上只作数量级参考） |
| `terminal-performance-comparison-2026.md` | 环境固定化清单（刷新率锁定、性能档位、动画缩放、温度注入）与各平台通道的实测语义：模拟器只有一个显示模式（刷新率天然固定）、`cmd display set-user-preferred-display-mode` 在本镜像上抛 `SecurityException` 但仍写入全局设置、固定性能模式无核对通道、动画缩放置 0 会让交互动画基准测不到动画 | 已实施：新增 `android/benchmark` 的 `EnvironmentPinning`（钉住刷新率、三类动画缩放、热状态与固定性能模式并核对实际生效值，钉不住即判红，无核对通道记「不可核对」，复原设备原值）与 `EnvironmentPinningTest`（8 个单测，用实测输出字面量断言），各基准用例以 `EnvironmentPinningRule` 在用例前固定、用例后复原；写入 `performance-measurement` 的「全场景矩阵与环境固定化」条并补三个场景（固定不到即判红、无核对通道只记不可核对、结束后复原原值）；固定后的实际生效值随环境快照落进测量行 |
| `terminal-performance-comparison-2026.md` | 宿主负载的记录口径：用 `/proc/stat` 的 steal 计数占比表达宿主抢占，并给出可标定的抢占阈值 | 已实施：新增 `android/benchmark` 的 `HostLoad`（解析 cpu 汇总行的 steal 列、取窗口内增量占比、阈值 `HOST_CONTENTION_STEAL_RATIO = 0.10`）与 `HostLoadTest`，并由 `measureScenario` 在场景窗口前后各取一次快照、随测量行落盘 `host_load`；阈值按四档实测标定（模拟器空闲 0.1%、宿主安静而模拟器活跃 1.9%–5.2%、3 个忙循环 44%–46%、8 个忙循环 38%–43%），写入 `performance-measurement` 的新条「宿主负载与进程 CPU 时间的记录口径」与场景「宿主被抢占时两个口径同时可见」 |
| `terminal-performance-comparison-2026.md` | 进程自身的 CPU 时间作为宿主无关的工作量口径（`/proc/<pid>/stat` 的 utime/stime 与 starttime，配合 `/proc/uptime` 得进程年龄） | 已实施：新增 `android/benchmark` 的 `ProcessCpuTime`（字段序号从最后一个右括号之后起算，滴答按 USER_HZ 100 折毫秒）与 `ProcessCpuTimeTest`，跨应用启动基准逐轮记 `process_cpu_millis`／`process_age_millis` 并给 cpuP50 汇总；实测抢占时墙钟中位数从 856ms 涨到 2191–2275ms 而进程 CPU 时间只从 730ms 涨到 970–1020ms，故两者必须同记；写入 `performance-measurement` 的同条 |
| `terminal-performance-comparison-2026.md` | 按宿主抢占占比（steal）折算启动耗时或帧率 | 不采纳：steal 记的是全部 vCPU 的被抢时间，空闲的 vCPU 被抢走并不拖慢被测进程，按占比缩放会把被测进程算得比实际更快；负载只作事后归因的证据，判定用进程 CPU 时间、槽位比例这类负载无关的相对量；理由写入 `performance-measurement` 的同条与场景「负载不折算进耗时」 |
| `terminal-performance-comparison-2026.md` | `am start -W` 的总耗时行只在系统测到启动时才打印（AOSP `ActivityManagerShellCommand` 按 `result.totalTime >= 0` 打印），启动即崩溃时启动态为 `UNKNOWN (0)` 且该行缺失 | 已实施：`android/benchmark` 的 `AppLaunch` 把总耗时改为可空并在缺失时显式判失败（读成 0 会把「系统没测到」记成「启动很快」），跨应用启动基准同时校验启动态为 `COLD`；写入 `performance-measurement` 的「跨应用启动对比的系统侧口径与准备动作」与两个场景「系统没测到启动时按缺失总耗时判红」「启动态未知即判红」；实测「APK 缺 native 库 → 启动即崩」形态为 `Status: ok` / `LaunchState: UNKNOWN (0)` / 无 `TotalTime` 行 |
| `terminal-performance-comparison-2026.md` | 同一窗口内既有准备动作留下的强停记录又有真正的崩溃记录时的归因取舍 | 已实施：`android/benchmark` 的 `ExitInfoRecords.withProcessExitAttribution` 按「应用故障 → 环境 → 其它」排序取归因记录，准备动作的强停记录不再盖掉崩溃原因；写入 `performance-measurement` 的「进程退出归因的采集与判红」与场景「退出归因按可 actionable 程度取」 |
| `terminal-performance-comparison-2026.md` | 12 类全场景矩阵（统一负载、指标、采集面、判据与硬上限）与「测量行能引用矩阵项」 | 已实施：新增 `android/benchmark` 的 `ScenarioMatrix`（12 项数据、每项带 wall-clock／输出字节／采样间隔下限，`scenarioById` 按编号取）与 `ScenarioMatrixTest`（契约与覆盖断言），冷启动／温启动／滚动与跨应用启动基准按 S1／S2／S7 接入，落盘行带 `scenario.matrix_id`；写入 `performance-measurement` 的「全场景矩阵与环境固定化」与场景「测量行引用矩阵项」 |
| `terminal-performance-comparison-2026.md` | 极端场景的资源上限（wall-clock 与输出字节）必须执行，超限即终止并记「超限终止」 | 已实施：新增 `android/benchmark` 的 `ScenarioBudget`（单调时钟算窗口、输出字节记账、超限抛 `ScenarioLimitExceeded`）与 `ScenarioBudgetTest`，`measureScenario` 在场景窗口结束后校验 wall-clock 上限、超限即强停被测应用并把判定记为「超限终止」；写入 `performance-measurement` 的「极端场景的资源上限」与场景「超限终止落进测量行」 |
| `terminal-performance-comparison-2026.md` | 热启动不新建 Activity，宏基准自带的启动等待在系统报「Activity not started」时退化成固定两秒（实测十一轮每次都睡满两秒，测得中位 2838ms，而同一次启动的系统侧总耗时只有 113ms） | 已实施：热启动场景 S3 改走系统侧 `am start -W`，取 TotalTime 作切前台耗时并校验启动态为 `HOT`（否则当场判红），依据为 `androidx.benchmark.macro.MacrobenchmarkScope.amStartAndWait` 源码的「Unable to safely detect Activity launch, waiting 2s」分支；写入 `performance-measurement` 的新条「热启动的系统侧口径与内容层取证」及三个场景；实测 2026-11-22 本机 AVD（API 35 模拟器，软件渲染）十一轮：切前台耗时中位 113ms、终端界面可见中位 858ms、总耗时 929–1568ms（中位 971ms） |
| `terminal-performance-comparison-2026.md` | 终端内容层的 Surface 随界面回到前台重建（实测按 home 后内容层从图层清单消失，切回前台后以新的图层号重建），故内容层帧必须取启动之后拿到的图层 | 已实施：热启动场景的内容层取名移到启动之后，并给出有上限的等待（出帧比图层出现晚），等不到即判红；写入 `performance-measurement` 的同条与场景「热启动的内容帧从重建后的图层取」；实测每轮内容层 3 帧、`gfxinfo` 3–5 帧 |
| `terminal-performance-comparison-2026.md` | 空载要判「不画东西」，而本项目终端内容直绘 SurfaceView：`gfxinfo` 记不到那些帧，只看它会把「终端内容还在刷」误判成空载 | 已实施：新增 `android/benchmark` 的 `DeviceMemory`（解析 `dumpsys meminfo` 的 App Summary 汇总行取 PSS 与 RSS）与 `DeviceMemoryTest`（2 个单测，用实测 dump 字面量断言），新增 `SteadyStateBenchmark` 以「终端内容层画出第一帧」为就绪信号、静置十秒后取系统侧逐图层的内容帧、应用进程的 `gfxinfo` 帧数与 `/proc/<pid>/stat` 的进程 CPU 时间，三个面任一出现持续绘制或持续占用即判红；写入 `performance-measurement` 的新条「空载场景的判据面」及两个场景；实测 2026-11-22 本机 AVD（API 35 模拟器，软件渲染）三轮：界面帧 0 帧、内容层 0 帧、进程 CPU 时间 150ms（三轮一致）、PSS 66–71 MB |
| `terminal-performance-comparison-2026.md` | 空载场景用宏基准框架执行 | 不采纳：宏基准的帧类指标在「一帧都没出」的窗口里没有样本，框架会因「Unable to read any metrics during benchmark」直接报错，而空载要的恰恰是「没有帧」这个事实；改为普通仪器测试执行，数据面在系统侧逐图层通道与进程计数上，不依赖框架指标；理由写入报告第五节「稳态空载的判据面与实测形态」 |
| `terminal-performance-comparison-2026.md` | 持续输出的负载须由设备侧 shell 自产（固定批数、每批固定行数、批间固定间隔），批次是否真的跑完以批末完成标记计数判定，呈现帧数不为零证明不了字节流完整 | 已实施：新增 `android/benchmark` 的 `SustainedOutputLoad`（命令生成、`input text` 的空格转义、PTY 的 CRLF 展开字节记账）与 `SustainedOutputLoadTest`（4 个单测），新增 `ContinuousOutputBenchmark` 以完成标记计数判定负载是否跑完、未跑完即强停被测应用；写入 `performance-measurement` 的新条「持续输出场景的负载完成与呈现判据」及「负载未跑完即判红」场景；实测 2026-11-22 本机 AVD（API 35 模拟器，软件渲染）负载窗口 7.0 秒（宿主轻载）到 10.4 秒（宿主被抢占）、吞吐 40.9–41.8KB/s 与 28.2KB/s |
| `terminal-performance-comparison-2026.md` | 持续输出期间的内存峰值只能在负载进行中采（负载结束后内存可能回落），呈现节奏须同时取呈现汇总与系统侧逐图层两个面 | 已实施：`ContinuousOutputBenchmark` 在等完成标记的轮询里按 1 秒间隔采 `dumpsys meminfo` 的 PSS/RSS 峰值，判据取呈现窗口数与最长停摆、内容层间隔 p95 与刷新槽位占用、进程 CPU 时间与 PSS 增长，显示级帧时间线按属主归属并与逐图层通道交叉校验；写入同条的三个场景；实测轻载 PSS 增长 16.5–19.7MB、内容层间隔 p95 118–120ms、宿主被抢占时 p95 150ms、进程 CPU 从 880–950ms 涨到 3710ms |
| `terminal-performance-comparison-2026.md` | 原生呈现汇总是 debug 级日志：系统侧 `am start` 拉起时若只给宏基准的 Intent extra，进程仍停在 release 缺省 WARN 等级，基准读不到帧、表现为 shell 就绪超时（实测 90 秒后报「没有收到 shell 的任何输出」） | 已实施：新增 `android/benchmark` 的 `startAndWaitAtDebugLogLevel`（`am start -W -n <activity> --es terminal.emulator.log_level DEBUG`），持续输出场景改由它拉起；写入 `performance-measurement` 的同条与场景「系统侧拉起的日志等级同样要传」；理由写入报告第五节「持续输出的判据面与实测形态」 |
| `terminal-performance-comparison-2026.md` | 通知权限弹窗会盖住被测应用：被盖住时输入法不弹出、终端拿不到输入焦点，场景会死在输入前置条件上；空载场景更隐蔽——弹窗盖住时界面停画，静置窗口里的零帧是遮挡造成的 | 已实施：持续输出与稳态空载两个场景在拉起前显式授予通知权限，空载场景另加 `assertAppInForeground`（`dumpsys window` 的 `mFocusedApp` 必须是被测包）；写入 `performance-measurement` 的「持续输出场景的负载完成与呈现判据」与「空载场景的判据面」两条及「权限弹窗盖住应用时判红」「应用被弹窗盖住时判红」场景；实测 2026-11-23 本机 AVD 修复前报「输入法未弹出：终端没有输入焦点」，修复后持续输出与空载两场景均通过 |
| `terminal-performance-comparison-2026.md` | 设备自身不健康时（实测模拟器 SystemUI 无响应）输入法服务不再出面，场景报的还是「输入法未弹出」，容易被读成应用回归 | 登记为设备健康前提，不改判据：场景在输入前置条件上判红并写明原因，编排须先确认设备健康（SystemUI 正常）再采数；理由写入报告第五节「持续输出的判据面与实测形态」 |
| `terminal-performance-comparison-2026.md` | 突发输出的无损判据须由两个哨兵证明：完成标记（logcat）证明生产者写完全部字节，流末尾的会话标题哨兵（OSC 2）证明终端把整段流解析到了末尾；呈现帧数证明不了无损 | 已实施：新增 `android/benchmark` 的 `BurstOutputLoad`（命令生成、`input text` 的空格转义、送达字节按 PTY 的 CRLF 展开记账）与 `BurstOutputLoadTest`，新增 `BurstOutputBenchmark`（标题哨兵在会话抽屉里读取、完成后读回滚行数、四面判据）；写入 `performance-measurement` 的「突发输出场景的无损与呈现判据」与场景「流被截断即判红」「生产者没写完即判红」 |
| `terminal-performance-comparison-2026.md` | 提示符默认每次上报会话标题（OSC 0），会覆盖流末尾的标题哨兵，测量前须换成不含转义的提示符 | 已实施：`BurstOutputBenchmark` 的 setup 在测量窗口外把提示符改为普通提示符；写入 `performance-measurement` 的同条与场景「流被截断即判红」 |
| `terminal-performance-comparison-2026.md` | 回滚区按配置封顶（默认 2000 行），突发时内存不随字节数线性增长；突发时终端基本满帧，与持续输出的形状不同，两场景阈值不共用 | 已实施：`BurstOutputBenchmark` 读回滚行数并按实测标定 PSS 增长、呈现窗口数与槽位占用下限；写入 `performance-measurement` 的同条与场景「回滚区封顶后突发不再涨内存」 |
| `terminal-performance-comparison-2026.md` | S8–S12 五个场景的负载生产、完成判定与帧面判据 | 已实施：新增 `WideCharacterLoad`／`WideCharacterBenchmark`（S8 宽字符行，负载按 PTY 的 CRLF 展开记账字节，标题哨兵证明排版正确）、`InteractionLatencyBenchmark`（S9 回显延迟与按键到首个呈现双口径）、`InputFloodBenchmark`（S10 连续按键与活性探针）、`BackgroundForegroundBenchmark`（S11 切回前台与旋转）、`ResourceStressBenchmark`（S12 低内存、强压、灭屏、Doze），输出类场景共用 `OutputScenarioFrames`（三通道取数与六项判据）与 `OutputLoadCompletion`；写入 `performance-measurement` 的「S8–S12 场景执行器」条；实测本机 AVD 上 S7–S12 逐个跑通 |
| `terminal-performance-comparison-2026.md` | 界面重建后旧图层的帧历史里留着重建之前的帧，算进首帧延迟会让恢复看起来是零延迟；旋转的计时起点须取系统确认转向的时刻而非发起旋转的时刻 | 采纳为取数口径：写入 `performance-measurement` 的新条「恢复与旋转的首帧取数口径」及三个场景；`SurfaceLayerFrames` 的 `firstPresentMillisAfter` 与 `awaitContentLayerPresentAfter` 取重建确认之后的首个呈现，`BackgroundForegroundBenchmark` 以 `dumpsys window` 的 `mCurrentRotation=ROTATION_90` 确认时刻为旋转计时起点，等待出帧的上限（8 秒）与首帧判据的上限（5 秒）分开 |
| `terminal-performance-comparison-2026.md` | 灭屏压力的复原须按当前屏幕状态判断：电源键是切换语义，按一次即翻转一次 | 采纳为复原口径：写入 `performance-measurement` 的新条「资源极端的屏幕状态复原」及场景；`ResourceStressBenchmark` 的 `needsScreenStateRestore` 与 `ResourceStressBenchmarkTest` 覆盖四种状态组合 |
| `terminal-performance-comparison-2026.md` | 缺 native 库的 APK 启动即崩（`native library not loaded: theme catalog unavailable`），退出归因给 `APP CRASH(EXCEPTION)` | 登记为构建前提并复用为端到端归因样本：写入报告第十二节的实测形态；构建顺序仍是先出 `.so` 再出 APK，该形态据此被认成应用故障而非环境问题 |
| `terminal-performance-comparison-2026.md` | 跨应用帧面端到端跑通（对照应用按本仓口径取帧）与跨设备多应用编排 | 未完成：前者缺对照应用装到设备上（上游 Termux 与本项目撞包名，须按应用轮换安装），后者落点在受保护的 `scripts/`，须维护者明确同意后实施；现有可复用落点为 `android/benchmark` 的仪器测试模块与 `scripts/test-emulator.nu` |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的秘密脱敏能力（`secrets.rs` + `secret_redaction` crate，`ObfuscateSecrets` 三态，复制时 respect 脱敏） | 未声明，不立项：`DESIGN.md` 无输出脱敏/秘密识别面；技术上可落地（上游 Ghostty 不提供，需自研或取第三方库），登记为候选能力，须先声明再实现；不撞 `PROHIBITED.md` |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的自研 VT 核（alacritty 血统 + `vte` 状态机，`crates/warp_terminal` 整体） | 不采纳：违 `DESIGN.md` 终端状态单一来源（Ghostty），本仓 VT 语义由 `libghostty-vt` 提供；Warp 的 grid/选区/回滚/搜索是第二套真相，不移植 |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的自研 UI 框架与 Metal 渲染器（`warpui`/`warpui_core`，macOS Metal 主路径，wgpu 实验性） | 不采纳：渲染栈不重合（本仓 Vulkan 唯一、无回退、目标 Android；Warp Metal 主路径、wgpu 实验性、目标桌面）；移植渲染代码没有对象 |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的 Vi 模式与内置编辑器（`vim` crate、`editor` crate 67k 行、`sum_tree`） | 未声明，不立项：`DESIGN.md` 无 Vi 模式/内置编辑器面；`sum_tree` 仅在日后声明内置编辑器时有参考价值 |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的命令补全/LSP/仓库元数据（`warp_completer`/`lsp`/`repo_metadata`/`fuzzy_match`/`natural_language_detection`） | 未声明，不立项：`DESIGN.md` 无命令补全/LSP/仓库元数据面 |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的 `ref_tests` 测试资产（alacritty harness 移植，40 组 golden：`.recording` 输入 + `grid.json` 期望） | 不采纳（门槛 1）：期望 grid 由实现自身产出，自验证；与本仓「上游 VT 语义不由本仓测试覆盖」口径一致；harness 形态仅作参照 |
| `warp-terminal-research-2026.md` | 建议移植 Warp 的多平台支持（macOS/Linux/Windows/FreeBSD/Web） | 不适用：本仓仅 Android（`BUILD.md` 仅 arm64-v8a/x86_64）；Web/WASM 形态与本仓无交集 |

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

#### Scenario: 不得反向采纳的做法

- **WHEN** 表内某行被标为「不得反向采纳」
- **THEN** 该行 MUST 点名出处仓库与具体做法，MUST NOT 只写「不采纳」这类结论

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
| 静态门禁的实测基线 | **已跑齐**：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`:app:detekt`、`:app:spotlessCheck` 全通过；`aislop` 0 error、2 条 `complexity/file-too-large` 风格警告（`render/cell_builder.rs` 与 `render/cell_builder/tests.rs`）——按 code-health 审计口径如实登记而不处置：`STYLE.md` 允许文件大小类风格违反，且不得为凑行数阈值拆分源码；`jscpd` 按 code-health 审计参数复算：生产口径 3 处克隆 / 30 行 / 0.05%、全量口径 26 处 / 257 行 / 0.25%，与基线逐项一致（改动未引入新重复）；MUST NOT 把工具链的 linker 弃用提示写成代码缺陷 |
| 默认入口探测与登录 shell 语义 | **已按 DESIGN「Shell」节落地**：探测顺序为 `{login, bash, zsh, fish, sh}`，文件存在即用（不检查权限、不检查 ELF），均不存在时用 `/system/bin/sh`；登录语义由 shell 自身的 `-l` 取得，唯 `bin/login` 不带参数——上游 termux-tools 脚本自身以 `exec "$SHELL" -l "$@"` 启动 shell，多带参数会吞掉 motd。DESIGN 写的「argv[0] = "-login"」在 Android 15+ 不可达：应用数据目录里的 ELF 只能经系统链接器启动（SELinux 拒绝 app_data_file 上的 `execute_no_trans`），而链接器被直接调用时把被加载程序的 argv[0] 改写成程序路径（AOSP bionic `linker/linker_main.cpp`：`g_argv = args.argv + 1`），argv[0] 无从送达；`-l` 是上游同义做法。改前实现只探测 `{bash, login}`，顺序与 DESIGN 相反 |
| `shell-env` 的 `PATH` 条款自相矛盾 | **已更正条款文字**：要求段原写「宿主 `PATH` 原值在前、`{prefix}/bin` 在后」，而同规范的场景段写「`PATH` 以 `{prefix}/bin` 开头」，实现（`native/src/terminal/pty/env.rs`）与上游 termux-app（`PATH` 取 `$PREFIX/bin`，Termux 工具优先）均为 prefix 在前，故矛盾出在文字。条款改为「`{prefix}/bin` 在前，宿主 `PATH` 原值在后」，实现不动。`DESIGN.md` 的「原 `PATH` 附加 …/usr/bin」未指明位置，解释以场景段与上游为准 |
| bootstrap 实终端用例的整类失败 | **已定位并修复**：整类连跑时 postinst 输出持续滚动，命令回显被冲出视口，固定睡 1.5s 后读到的全是安装噪声；改为写入唯一哨兵并等待**独立成行的哨兵**出现，单用例与整类均通过。MUST NOT 用延长睡眠或过滤输出掩盖 |
| `SettingsScreenTest` 在 en-US 模拟器上的抽屉按钮失败 | **已定位并修复**：测试工具硬编码中文 `contentDescription` 查找抽屉键，且通用 `desc/text "Settings"` 会点到输入法的设置入口并让 Gboard 的 `SettingsActivity` 盖住应用；改为只用 testTag 暴露的 `Key_DRAWER`／`SettingsButton` resource id，单方法 3/3 与整类 8/8 均通过。MUST NOT 用重试或过滤掩盖这一失败 |
| 复制超长选区的截断方向 | **已更正为 DESIGN 口径**：`DESIGN.md`「文本选择」节要求「复制内容超过 `CLIPBOARD_TEXT_MAX_LENGTH` 时仅取后段」，而实现取的是前段（`truncateTextAtCharBoundary` 的 `take` 语义）。选区尾端是最新输出，取前段留下的正是用户最不需要的最旧内容。改为 `util/TextTruncation.kt` 的 `truncateTextKeepingTail`（起点落在低代理项时前移一格保住整对）；粘贴仍取前段（用户复制的起点），安装诊断同样取前段。分享与复制同口径 |
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

### Requirement: 近六日提交逐条归因

近六日提交 MUST 按主题逐条归类，归类结果 MUST 对应到具体症状判据。

#### Scenario: 分类汇总

- **WHEN** 汇总 2026-11-19 起的 160 个提交
- **THEN** 分类条数 MUST 为：文档与规范 31、日志与诊断 27、测试与基准 24、其他 20、
      渲染与表面 17、会话与 Shell 14、构建与门禁 14、滚动与输入 12、设置 1
- **AND** 文档、测试与基准类 MUST NOT 当作产品回归来源；日志、渲染、滚动与会话类
      MUST 以同设备新日志和针对性测试复核后再下结论

#### Scenario: 逐条归因的复核

- **WHEN** 复核近六日任一提交是否导致新症状
- **THEN** MUST 先按上表定位类别，再核对该提交的改动文件与对应症状判据；
      MUST NOT 把文档提交或基准提交当作产品回归来源
- **AND** 新日志未复现某症状时 MUST 记为「未复现」，MUST NOT 写成「已彻底修复」

#### Scenario: 触及症状的提交逐条结论

- **WHEN** 复核近六日中触及日志、渲染、滚动与会话路径的提交
- **THEN** 因果结论 MUST 逐条写明：
  - `fix(render): 遮罩回读等待本设备提交并按设备丢失归一` 与其后的归一扩展：移除了
    回读阻塞等待这一应用侧触发条件；新日志丢失与 panic 均为 0
  - `fix(scroll): 新输出贴底同拍清掉亚行余量`：修复贴底后先冲过底边再弹回的余量；
    `ScrollRemainderTest` 钉住同拍清零
  - `feat(log): Android 日志接入 Timber 并用 governor 节流慢日志`（后续替换为 Log4K）：
    只新增诊断入口，不是慢帧来源；新日志的 3 组 `SLOW_*` 全在启动与首次输出
  - `refactor(shell): 按规范移除 ENV、mkshrc 与 PROMPT_COMMAND 注入`：按规范去掉注入，
    新日志的 shell 启动与提示符正常
  - `refactor(android): 按职责拆分渲染循环` 与其后的单次迭代拆分：治理 ART 编译 arena；
    新日志的 `Compiler allocated` 属于 Compose 的屏幕 lambda，不是渲染循环
  - `chore(native): 推进 libghostty-vt 钉住 rev 并适配破坏性 API`：上游破坏性变更适配，
    新日志无 panic 与设备丢失
- **AND** 其余类别 MUST 记为文档、测试或构建改动，MUST NOT 当作五类症状的来源
