## MODIFIED Requirements

### Requirement: 研究报告逐份结论登记在本规范内

`docs/research` 的全部研究报告（编号文档集 31 份 + 逐仓报告 73 份 + `ecosystem.md`，
另加 6 份综合研究报告，合计 111 份，已全部退役）的可执行项 MUST 在本规范内有逐份
可复核的结论：
每项 MUST 写明判定结果（采纳 / 不采纳及违反哪条采纳门槛 / 未声明不立项 /
PROHIBITED 明禁）与依据，MUST NOT 只以「其余若干份已核对」这类群体概括收尾，
MUST NOT 把逐份结论只留在归档变更里。

判定口径：外部语料的期望值若为其自身实现的输出（可被实现覆写、或固化了该实现
当年的偏差），门槛 1 成立，不采纳；断言打在本仓不实现的第三方 VT 组件上，门槛 3
不满足，不采纳；功能若 `DESIGN.md` 未声明，按 `STYLE.md` 不实现，登记为不立项；
撞 `PROHIBITED.md` 者登记为明禁。许可证 MUST NOT 作为判定依据。

#### Scenario: 任何一份研究报告都有可复核行

- **WHEN** 复查任一份已退役 `docs/research` 报告的结论
- **THEN** 在本规范的本表或相邻台账表内读到该报告的判定与依据，
      MUST NOT 需要先翻归档变更

#### Scenario: 漏登文档被发现时补行而非改口径

- **WHEN** 复查发现某份报告在既有台账中无逐份结论（如成文早于台账归档而漏登）
- **THEN** MUST 补该报告的判定行并说明此前漏登的事实，
      MUST NOT 把它并入「其余若干份」的群体概括

#### Scenario: 错裁按事实更正

- **WHEN** 某报告被群体口径错裁（如把同引擎对标仓按「自研 VT」裁掉）
- **THEN** MUST 按该报告真实内容重新判定并写明更正前后结论

### Requirement: 研究报告逐份索引与文档可移除性

`docs/research` 的每份研究报告 MUST 在本规范内有一条**以报告文件名为键**的索引行，
写明该报告的实质、判定结果与依据位置（本规范某张表、其他 spec 的能力名、或已落地代码）。
索引行 MUST 能独立回答「该仓有无可用资产、结论是什么、为什么」，MUST NOT 以
「其余若干份已核对」这类群体概括收尾。

索引完整是本仓删除研究文档的**前置条件**：任取一份报告，其全部可执行项的去向
MUST 已在索引行内可读到；满足该条件后报告本体 MUST 可被删除而不丢失任何仍需遵守的结论。
全部 111 份报告（编号文档集 31 份 + 逐仓报告 73 份 + `ecosystem.md` + 6 份综合研究报告）
已整体删除，索引表是这批研究的**唯一常驻载体**；6 份综合研究报告的索引行 MUST 承接
其全部仍需遵守的结论——文档在库时索引行是这些报告结论的登记处，文档删除后由同键索引行
承接，MUST NOT 因文档已删而让结论失去落点。

#### Scenario: 索引外的补充事实

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
- **THEN** 索引表内有以该文件名为键的行，且该行的判定与依据可在本规范内读到

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

索引表如下。

| 报告 | 实质 | 判定 | 依据 |
| --- | --- | --- | --- |
| `ghostty-android-terminal.md` | 同引擎（libghostty-vt）Android 终端的集成模式深研：Java Canvas UI → JNI → 预编译 libghostty-vt.a → arm64chroot/arm64emu 四层架构 | 已核结，无可移植资产：同引擎集成印证四项（回调经 `feed()` 后拉取而非上推、winsize 带 `ws_xpixel`／`ws_ypixel`、选区经 `GHOSTTY_TERMINAL_OPT_SELECTION` 交引擎跟随、bidi 与歧义宽不支持）本仓已是同一形状；可参考点四项已落地（mksh SIGWINCH 不变量归 `startup-grid-resize`，grapheme 行级标志归 `CellData` 的 `grapheme_extra` 故无带外溢出缓冲层，编码前同步 setopt 归 `set_options_from_terminal`，写清为什么归 openspec 变更的 design.md）；不适用：arm64chroot/arm64emu 与 rootfs 资产（`DESIGN.md` 未声明用户态模拟器，`PROHIBITED.md` 明禁内嵌 proot 与预装发行版）、自建 native 搜索（`text-search` 已定上游 `search::Search` 单一来源）、OSC 52/9;4 side-scanner 与富输入 Editable 镜像（`DESIGN.md` 未要求）、Canvas 渲染全套（本仓 wgpu/Vulkan 直绘） | 本表 `01-sylirre-ghostty-android-terminal.md` 行；`text-search`；`startup-grid-resize`；`key-input-encoding`；`host-linkage` |
| `redterm.md` | proot 发行版终端 + SSH 客户端（复用 Termux 上游库、不自研引擎）的外围工程实践与防错模式 | 已核结，可参考资产只在工程实践层：四项参考点已落地或由既有规范承接（wake lock 自检重取归 `TerminalForegroundService`，启动路径单一事实源归 `ShellEnv` → `pty::base_env`，假对端测试归 `native/src/terminal/mock_pty.rs`，注释纪律归 `STYLE.md`「注释极简」）；不适用：`script` 会话录制（本仓 Rust 侧天然持有 PTY 读端且规格未要求）、AppLog 持久化日志（`DESIGN.md` 明禁不写日志文件）、259 项调色板纯函数（主题自带 16 色）、`SessionDescriptor` 身份与显示名分离（会话归 Rust 侧，`session-list` 已定「序号 + 标题」）、Choreographer 合帧 invalidate 与链接正则检测（渲染与链接识别在 Rust 侧）、发行版管理／proot／内嵌 OpenSSH／SFTP／文件管理器／备份还原（范围外或 `PROHIBITED.md` 明禁）、CPU% 通知与桌面小部件（通知栏只报会话数量与退出按钮）；已核实无缺口：16 KB 页对齐（`libnative.so` 的 LOAD 段对齐 0x4000）、通知诚实性、bootstrap 权限保真（`bootstrap-install` 走 zip 清单承担权限） | 本表 `GlobalTechInfo-RedTerm.md` 与 `06-GlobalTechInfo-RedTerm.md` 行；`bootstrap-install`；`shell-env`；`logcat-hygiene`；`session-list` |
| `reference-verification.md` | 参考项目核验文档：上游 A（ghostty-android-terminal）160 条仪器化测试与上游 B（termux-app）145+2 条 JVM 测试逐条对照本仓测试 | 已核结，结论为「本仓测试面已闭合」：上游 A 160 条中 80 条有等效用例、80 条对应能力被禁止或未声明、0 条部分对应，每条等效均给出双方精确测试名；上游 B 的 19 个测试文件按「直接锚点／引擎域／未声明」三分逐区核对，无「上游已测且规范允许而本仓未测」条目；按像素滚动的手势层／运行期层／FFI 与原生渲染层三层证据链闭合；文中五条前版误记（上游测试计数、`TerminalUiTest` 条数、`ExtraKeysConfigTest` 与 `ThemeModelTest` 归类、测试名归属）已按源码实读更正 | 本表 `01-sylirre-ghostty-android-terminal.md` 与 `02-termux-termux-app.md` 行；`scroll-gesture-settle`；`terminal-state-regression` |
| `terminal-emulators.md` | 终端模拟器横向研究报告：ghostty／alacritty／rio／kitty／wezterm／xterm／foot 七项目的渲染、解析、线程模型与输入路径 | 已核结，结论为「与本仓同族者仅 ghostty 与 rio，且本仓在这两点上同构」：同构印证五项（独立渲染线程 + 合帧唤醒、实例化渲染、部分写退回并保留写进度、引擎与宿主解耦、表驱动解析）本仓已是同一形状；可参考点五项已核结（锁让渡自查落在 `native/src/terminal` 零引用渲染状态、damage 语义清单落在当前与上一光标行标脏且滚动变化强制全量重绘、输入三档压测落在 `bulk_output_is_processed_without_loss` 与 `cell_frame_drop_warning_is_throttled`、quirks 集中层落在既有 workaround 就地注释、xterm 仲裁基准落在 `terminal-state-regression`）；不采用：alacritty 列级 damage（本仓脏带以行为最小单位）、rio 多后端回退（`DESIGN.md` 明禁 CPU/OpenGL 回退）、kitty 时间性拒收（本仓已有有界排空 + 计数退回双背压）、foot box-drawing 几何绘制（规格未要求）、kitty SIMD 多宽度扫描（解析归 libghostty-vt）、rio／foot 主线程渲染（正是本仓独立渲染线程要规避的形状） | 本表 `29-减码杠杆与libghostty家族结论收敛.md` 行；`terminal-state-regression`；`performance-measurement`；`render-loop-scroll-cadence` |
| `termlib.md` | ConnectBot termlib（Compose 终端组件库，vendored libvterm）深研，含四层性能测量体系与 macrobenchmark 评估 | 已核结，结论为「macrobenchmark 有意义但边界明确」：`AppStartupBenchmark` 刚需、`InteractionAnimationBenchmark` 有效、终端内容渲染架构上测不到（Rust 线程 wgpu 直绘不产生主线程 doFrame）、CI 只编译不运行是对的（swiftshader 帧率不可作回归判据）；可参考点六项已核结（真机测量规程与合成负载取舍归 `performance-measurement`，增量一致性归 `cached_build_matches_full_build_byte_for_byte` 与 `resize_cycles_do_not_accumulate_stale_rows`，criterion 基准不进测试套件且 `check.nu` 走 `--quick`，本仓不建 JNI 全局引用故无所有权环）；不适用：`PackedCells`／`TerminalSnapshot` 等 Kotlin 网格结构（本仓设计上无 Kotlin 网格）、OSC 133 无障碍导航与 Review Mode（规格未要求且 `PROHIBITED.md` 明禁无障碍朗读）、Unicode 宽度表生成脚本（归 libghostty-vt）、Canvas 几何绘制与 bidi level-1 细节（归 wgpu 侧与 libghostty-vt）、Magnifier 选区（文本选择须遵循 Termux 设计） | 本表 `30-VT测试边界与外部基准接入收敛.md` 行；`performance-measurement`；`ime-animation-smoothness`；`text-selection` |
| `termux-app.md` | termux-app（v0.118.3）深研：四模块结构、终端会话三线程与双向队列、bootstrap 安装流程、修饰键栏三态、前台服务与测试基座 | 已核结，结论为「DESIGN 依 Termux 定规的条目本仓已对齐」：已对齐项逐条在位（修饰键栏布局与粘滞语义、2 行 7 列、回滚 2K 行、块状光标不闪烁、`TERM=xterm-256color` 与 `COLORTERM=truecolor`、bootstrap zip + `SYMLINKS.txt` + staging + 原子 rename、通知常驻「会话数量 + 退出按钮」、applicationId `com.termux`、AOSP testkey 签名）；可参考点五项已核结（`KEYCODE_BREAK` → `key::Key::Pause` 补齐归 `key-input-encoding`，auto-repeat 阈值改取 `ViewConfiguration` 系统值归 `modifier-bar-auto-repeat`，双 client 生命周期归 Rust 侧会话归属与 LeakCanary 用例，主线程消息泵由渲染线程 latch 等价承担，`TerminalTestCase` 式直测由 `ghostty_terminal/tests/` 承担）；不适用：插件体系（`PROHIBITED.md` 明禁 Termux:API、termux-exec、Termux:Tasker、Termux:Boot，其余插件属 `DESIGN.md` 未声明不立项）、上游 `termux.properties` 的设置项体系（本仓无该设置面；`termux.properties` 文件本身按 `termux-configuration` 只解析 `config.md` 列出的允许键，`PROHIBITED.md` 只禁未列出的键）、F-Droid／GitHub 双分发签名管理（本仓单 testkey）、sharedUserId 与多 APK 签名兼容约束 | 本表 `02-termux-termux-app.md` 行；`key-input-encoding`；`modifier-bar-auto-repeat`；`bootstrap-install`；`session-list` |
| `8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 子集 + C PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；其 OSC 注释清单作生态参照面，OSC 52 安全姿态归 OSC 52 条款；其 `NativePty.c` 一类 C PTY 桥无落点——本仓 PTY 在 Rust 侧（`nix::pty`），仓内无 C 源；多击选词／OSC 8 白名单／提交构建产物三做法属不得反向采纳 | 「Android 侧终端项目的复查台账」terminator 行；「OSC 覆盖逐项核对」；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `DanteAlighierin-foot.md` | C/Wayland 自研 VT 极简终端 | 不采纳：仓内无 VT／网格语料（「无资产」而非门槛违反）；自研 C 解析器违单一来源，box-drawing／组合宽度只作结构对照 | 「已复查结论台账」foot 行 |
| `GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 发行版运行器 | 不采纳（门槛 3）：仓内无本仓组件，329 例属 SSH/SFTP/proot 面；16KB 页对齐登记禁改区 | 台账不可采纳行；「禁改区待维护者裁定项台账」；参照面「搜索高亮两层透明度」归 `text-search`，备份「完成后替换 + 孤儿清理」归「参照面与反面做法收敛补齐行」 |
| `JetBrains-jediterm.md` | JVM/Swing 可嵌入终端库（自研 VT + Pty4J） | 不采纳（门槛 1）：before/after 期望即其自身输出；vttest 派生子集按输入分流逐例判定 | 「已复查结论台账」vttest 行；「按输入是否格式合规分流采纳」 |
| `KDE-konsole.md` | C++/Qt 桌面终端（Vt102Emulation） | 不采纳：桌面 C++ 自研 VT；profile/session 编辑撞 PROHIBITED，DBus／i18n／tysend 未声明 | 「能力裁定补齐行」DBus 远程控制行、界面多语言/i18n 组织行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」konsole 行；「不得反向采纳的做法」 |
| `KDE-maui-station.md` | Qt6/Kirigami 移动终端（MauiKit） | 不采纳：QML 集成在 Android 无对应形态；仓内无 VT 资产（其 `split` 指 Service/UI 分层进程模型，非分屏） | 生态参照面组行；「能力裁定补齐行」分屏／平铺面板行（该行已记其误引更正） |
| `KDE-yakuake.md` | Konsole KPart 下拉式壳 | 不采纳：无新仿真（emulation 全在 Konsole，本行与 `KDE-konsole.md` 同口径）；下拉 UX 无 Android 对应；其 profile/session 复用撞 `PROHIBITED.md`「会话持久化」 | 生态参照面组行；`PROHIBITED.md` 会话持久化条；`KDE-konsole.md` 索引行 |
| `Kuddev-pebrel.md` | Rust+GPUI 桌面 GPU 终端（vte 核） | 部分采纳：release profile（LTO／codegen-units／strip）已落地，其「hot path O3」**不采纳**——移动端 release 以 `opt-level = "z"` + LTO 为体积目标，逐点 O3 与 DESIGN 的体积取向相悖；录制回放语料不采纳（门槛 1）；PTY 事件循环与 redraw anchor 已被本仓 VT 线程 + 行级脏跟踪越过；GPL-3.0 不引入代码 | `native/Cargo.toml`；「语料判定补齐行」pebrel 行；「能力裁定补齐行」分屏／平铺面板行与 vi-like 选择模式／可配置按键绑定行 |
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
| `vercel-labs-wterm.md` | Zig/WASM + DOM 渲染（封装 libghostty） | 不采纳：WASM/DOM 无法作为 Android 依赖；无障碍/文本捕获未声明（朗读形态撞「无障碍朗读」明禁）；WS 重连／背压未声明；CI 产物哈希登记禁改区；其 DOM 脏行跟踪／部分重绘思路已由本仓行级脏跟踪与部分帧带越过，Ghostty 封装层的页面零初始化修复属上游内部实现 | 「语料判定补齐行」wterm 行；「能力裁定补齐行」WebSocket 行与无障碍行；「禁改区待维护者裁定项」 |
| `wilsonglasser-oryxis.md` | Rust/iced + alacritty SSH 客户端 | 不采纳：alacritty 核冲突；config vault／会话恢复撞 PROHIBITED；crate tests 未编目 | 「能力裁定补齐行」会话恢复/持久化行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」oryxis 行 |
| `z-hunter-Scanline-term.md` | Tauri/React/xterm.js 复古终端 | 不采纳：无 VT conformance 套件；MCP 明禁、CRT 特效未声明、会话恢复明禁 | 「语料判定补齐行」Scanline 行；「能力裁定补齐行」Model Context Protocol 行 |
| `ecosystem.md` | 生态参考与测试边界（参照面评级、禁止项、测试边界） | 内容已并入：参照面评级 → 本表「实质」列（评级语义与「生态参照面组行」定义见本条正文）；禁止项 → 本条「禁止项三条保留」段；测试边界 → `terminal-state-regression` 与本能力「上游 VT 语义不由本仓测试覆盖」；参照面表的可落地项按名落点（`pty-lifecycle` 收 `IUTF8`／像素 winsize／前台组收尾，`text-search` 收搜索上限与两层高亮，`settings-propagation` 收 live／spawn-time，`render-stability` 收键盘动画期不暂停，通知常驻归 `DESIGN.md` 软件节），其余 → 「参照面与反面做法收敛补齐行」 | 本表；`pty-lifecycle`；`text-search`；`settings-propagation`；`render-stability`；「参照面与反面做法收敛补齐行」；`terminal-state-regression` |
| `00-汇总与可参考点收敛.md` | 编号集总纲：25 仓汇总、P0/P1/P2 分级与 openspec 落点 | 已处置：P0 由各能力承接、`bootstrap-install` 单列立项；P1/P2 为实现指导不新立项 | `bootstrap-install`；「参照面与反面做法收敛补齐行」；本表 `01`–`25` 各行 |
| `01-sylirre-ghostty-android-terminal.md` | Java+C 壳，VT 全委托预编译 `libghostty-vt.a`，Canvas 拉快照过 JNI | 需改造后采纳（同引擎同架构最正当对照面）：可采纳类为锚定外部协议的 VT 断言；Java 持会话、CPU/GL 回退、SAF 复制字体、自定义主题编辑器不采纳 | 「Android 侧终端项目的复查台账」`sylirre/ghostty-android-terminal` 行；「不得反向采纳的做法」 |
| `02-termux-termux-app.md` | 上游基准：自研 VT + Canvas，约 4.8 万行 | 不采纳（门槛 3）：断言打在本仓不实现的上游 VT 引擎；行为参考归各能力（PTY 双线程、退出保留现场、键矩阵、回滚 2000） | 复查台账 `termux/termux-app` 行；`pty-lifecycle`；`shell-entry`；`terminal-state-regression` |
| `03-sohan-f-termux-monet.md` | termux fork + Monet 运行时取色引擎 | 不采纳（门槛 3）：新增仅 14 例断言刚写入的调色板值；Monet 动态取色与自定义主题撞 `PROHIBITED.md` 设置节 | 复查台账 `sohan-f/termux-monet` 行；「能力裁定补齐行」用户自定义主题行；`theme-configuration` |
| `04-8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 全矩阵 + C 真 PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；OSC 52 门控归 OSC 52 条款；多击选词／OSC 8 白名单／提交产物不得反向采纳 | 同仓现行索引行 `8dmusichannels-star-terminator.md`；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `05-deathlegionteamlk-jolz.md` | 仅 README 与 Release APK，源码私有 | 无门槛违反：无源码可评估，其声称细节不得引用；两处声称与本仓方向一致且已由本仓规范覆盖——宿主环境继承归 `shell-env`（本仓取固定透传名单而非全量继承）、logcat 可见崩溃报告归 `DESIGN.md` 日志要求 | 复查台账 `deathlegionteamlk/jolz` 行；`shell-env`；`DESIGN.md` |
| `06-GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 多发行版运行器 | 不采纳（门槛 3）：329 例属 SSH/SFTP/proot 面；单源启动脚本、通知退出、备份替换归本规范各表 | 同仓现行索引行 `GlobalTechInfo-RedTerm.md`；复查台账 `GlobalTechInfo/RedTerm` 行；`bootstrap-install` |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | termux 重型 fork：会话加固 + 自研 bootstrap 治理 | 不采纳（门槛 1）：`terminal-emulator/src/test` 与官方逐字节相同；正例为会话加固（先进程组收尾） | 复查台账 `rafaelmeloreisnovo/termux-app-rafacodephi` 行；「参照面与反面做法收敛补齐行」整包完整性校验（BLAKE3 与 sha256 双实现）行 |
| `08-pavelc4-Rin.md` | Rust 引擎 + Compose 壳，手写 545 行 ANSI 解析 | 不采纳（门槛 1 + 3）：手写解析必然不完备（反衬 Ghostty 单源）；Canvas 指针进 Rust 绘制违渲染分层 | 复查台账 `pavelc4/Rin` 行；「禁止项三条保留」段（不换 VT 核） |
| `09-Lord1Egypt-AndroidThothTerm.md` | JackPal 血统 Java 仿真器 + BiDi 渲染 | 门槛 1 不违反但仅作判据来源；`tests/` 122 文件对全仓零引用属失效语料；BiDi 未声明；交互侧可参考项本仓已有等价或更强实现——长按取词与拖拽手柄归 `text-selection`、虚拟键布局归 `modifier-bar-height-reservation`、会话列表标题监听归 `session-list`、常驻通知归 `DESIGN.md` 软件节、staging 与失败清理归 `bootstrap-install` | 复查台账 `Lord1Egypt/AndroidThothTerm` 行；「新增失效语料记录」行；「能力裁定补齐行」RTL/BiDi 行；`text-selection`；`modifier-bar-height-reservation`；`session-list`；`bootstrap-install` |
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
| `28-复审循环与残留问题收敛.md` | 二十余轮复审循环记录：已修／误报／不改／仍未修逐条 | 已处置：可执行项逐条落代码与判别力用例；两处原判「仍未修」已由后续裁定收口——IME 组字锚点经**公开** `InputMethodManager.updateCursorAnchorInfo` 实现（`InputConnection` 上的同名符号才是 `@hide`），rev 哈希张力按「hash 只在依赖钉里」裁定；其余未查清与覆盖缺口归「研究阶段残留项与覆盖缺口登记」；`UnsupportedQueueFamily` 的定性（surface 丢失的后果而非独立缺陷，`get_capabilities` 失败即报此错）已落 `native/src/render/context/mod.rs` 重建慢路径的注释与自愈阈值 | `ime-editor-contract`「组字光标位置回报给输入法」；本能力「rev 本身不进正文」；「研究阶段残留项与覆盖缺口登记」；`native/src/render/context/mod.rs` |
| `29-减码杠杆与libghostty家族结论收敛.md` | 减码 L1–L6 分级与红线；libghostty-vt 对比 librio | 采纳并已执行（L1）：JNI 批量合并按 `DESIGN.md` 背书落地（标题／单元格宽高／主题目录／会话计数收敛为批量查询），`getMode` 导出删除；L2–L6 维持「有界重构建议，不扩大为变更」；红线（不减测试、不删 DESIGN 功能）为约束；不换核、不等完整 libghostty 上 Android 归禁止项三条保留 | 「禁止项三条保留」段；「研究阶段残留项与覆盖缺口登记」L2–L6 行；`openspec/changes/archive/` 台账 |
| `30-VT测试边界与外部基准接入收敛.md` | VT 测试边界与外部基准接入；优先级 1–3 曾落地后又撤回 | 已撤回：原落地 `native/tests/external_benchmarks/`（UAX #11 全量宽度、ghostty fuzz 语料回放、kitty 键盘协议）按维护者口径「不测上游」整体删除，见「外部基准与 BDD 套件的退场」行；边界结论「不扩张成穷举 VT 一致」不变 | `terminal-state-regression`；本能力「上游 VT 语义不由本仓测试覆盖」；「语料判定补齐行」`corpus/stream-initial` 行 |
| `终端模拟器生态深度研究报告.md` | 生态深度报告：S/A/B/C 评级、五条 VT 路线、跨仓扫描 | 已处置：评级语义归本条「生态参照面组行」定义；路线与扫描结论归本条「生态内其余仍需对照的结论」段 | 本条「生态扫描中仍作为判断依据保留的事实」段与「原 `ecosystem.md` 的 S/A/B/C 参照面评级」段；本表各行 |

上表首 6 行键为曾现役、后随 `docs/research/` 整体删除的 6 份综合研究报告，
其结论由同键索引行承接；其余各行为更早退役报告的索引。

上表末 31 行键为另一份退役编号集的文件名（`00`–`30` 去掉 `26` 的 30 份编号文档
加 `终端模拟器生态深度研究报告.md`；`26` 为 API 清单，成文时即移出研究目录，不在
本次退役范围）；该编号集曾用于立项 `bootstrap-install`、PTY 收尾清单、搜索上限与
文档提供器形态，其逐仓结论已由「Android 侧终端项目的复查台账」承接、其余结论按
上表各行指向表内可读位置。
