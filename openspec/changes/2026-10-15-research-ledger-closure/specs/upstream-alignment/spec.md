## ADDED Requirements

### Requirement: 研究报告逐份结论登记在本规范内

`docs/research` 全部研究报告的可执行项 MUST 在本规范内有逐份可复核的结论：
每项 MUST 写明判定结果（采纳 / 不采纳及违反哪条采纳门槛 / 未声明不立项 /
PROHIBITED 明禁）与依据，MUST NOT 只以「其余若干份已核对」这类群体概括收尾，
MUST NOT 把逐份结论只留在归档变更里。

判定口径：外部语料的期望值若为其自身实现的输出（可被实现覆写、或固化了该实现
当年的偏差），门槛 1 成立，不采纳；断言打在本仓不实现的第三方 VT 组件上，门槛 3
不满足，不采纳；功能若 `DESIGN.md` 未声明，按 `STYLE.md` 不实现，登记为不立项；
撞 `PROHIBITED.md` 者登记为明禁。许可证 MUST NOT 作为判定依据。

#### Scenario: 任何一份研究报告都有可复核行

- **WHEN** 任取一份 `docs/research` 报告复查其结论
- **THEN** 在本规范的本表或相邻台账表内读到该报告的判定与依据，
      MUST NOT 需要先翻归档变更

#### Scenario: 漏登文档被发现时补行而非改口径

- **WHEN** 复查发现某份报告在既有台账中无逐份结论（如成文早于台账归档而漏登）
- **THEN** MUST 补该报告的判定行并说明此前漏登的事实，
      MUST NOT 把它并入「其余若干份」的群体概括

#### Scenario: 错裁按事实更正

- **WHEN** 某报告被群体口径错裁（如把同引擎对标仓按「自研 VT」裁掉）
- **THEN** MUST 按该报告真实内容重新判定并写明更正前后结论

### Requirement: 语料判定补齐行台账

下表补齐既有「已复查结论台账」未覆盖的语料资产。既有表中的行继续有效。
本表每行 MUST 写清资产、判定与可复核依据；复查中发现未登记的语料资产 MUST 补行，
MUST NOT 以概括表述代替逐项判定。

| 资产 | 判定 | 可复核依据 |
| --- | --- | --- |
| ghostty `corpus/stream-initial` 01–24 | **采纳**（已落地） | 上游自家规范种子，期望按种子名声明的意图外部可推；落地为 `native/tests/external_benchmarks/upstream_corpus.rs`，按发布标签取数、清单按名排序并断条数，断言本仓接线回读 |
| migueldeicaza/SwiftTerm 175 个 Swift 测试文件 | 不采纳（门槛 1） | 期望值是其自研 VT 实现的输出；与 Ghostty 的 fuzz 语料对照思路已由上条落地 |
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
| alacritty/termbenchbot 指标维度（cursor_motion/dense_cells/scrolling/unicode） | 仅参照（不引入） | 性能方法论对照；本仓基准在 `native/benches`，设备侧自动化门禁见本节能力行 |
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

#### Scenario: 未登记的语料资产补行

- **WHEN** 复查中发现某仓的测试语料未在本表与既有「已复查结论台账」内
- **THEN** MUST 补行写明判定（含门槛号）与可复核依据，MUST NOT 以概括表述代替

### Requirement: 能力裁定补齐行台账

研究报告中出现、此前未逐条裁定的功能面，判定如下；「未声明」指 `DESIGN.md`
无对应功能声明，按 `STYLE.md` 不实现。判「PROHIBITED」时 MUST 引 `PROHIBITED.md`
条目，MUST NOT 以「未声明」笼统代替；错裁 MUST 按事实更正并保留更正前后结论。

| 功能面 | 出处 | 判定 |
| --- | --- | --- |
| Model Context Protocol | godot-pty-gpty、lumina、ntilde、par-term、par-term-emu-core-rust、ratty、Scanline-term | **PROHIBITED 明禁**（`PROHIBITED.md` 终端节「Model Context Protocol」）；口径更正：先前记「未声明」属错裁，按 BiDi 更正同例改判；出处更正：先前引 copad、Lume 属误引（两报告无 MCP），改引实测命中者，同时补入 gpty |
| 插件系统、事件总线、mux daemon | ratty、par-term、copad、hollow | 未声明，不立项；出处更正：先前引 Scanline-term 属误引（该报告无插件/mux 内容），已按实测命中移除 |
| 无障碍/文本捕获（`readText` API） | vercel-labs/wterm | `DESIGN.md` 无无障碍面，**未声明，不立项**；朗读形态另撞 `PROHIBITED.md` 终端节「无障碍朗读」；口径更正：先前整行记「PROHIBITED 明禁」系以朗读条目覆盖整个无障碍面，按报告实测（仅可访问性/文本捕获）更正 |
| WebSocket/网络终端形态（服务端拆分、DOM 单元布局、Electron/Qt 集成） | DomTerm、terminal7、wterm | 未声明，不立项（本仓无网络终端；wterm 的重连与背压设计仅在日后开网络终端时参考） |
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
| 界面多语言/i18n 组织 | qterminal | 未声明，不立项 |
| 用户自定义主题覆盖与优先级链（用户文件 > 自动取色 > 默认） | termux-monet | **PROHIBITED 明禁**（`PROHIBITED.md` 设置节「自定义终端主题」）；本仓主题列表固定、构建期读取（`theme-configuration`） |
| RTL/BiDi 文本整形 | RtlTerminal、ThothTerm | **未声明，不立项**；口径更正：`PROHIBITED.md` 无 BiDi 条目，「已禁止」的表述无依据，正确口径是未声明 |
| 界面 layoutDirection RTL | RtlTerminal | 未声明，不立项 |
| copy-on-selection（松手即复制） | godot-xterm | 不采纳：选择交互以菜单点击复制为唯一入口（`text-selection`） |
| bell 冷却/节流 | godot-xterm | 不接线（`on_bell` 无消费者，既有台账行），连带节流不立项 |
| Tektronix 4010/4014 | rricharz-Tek4010 | 未声明，不立项（Ghostty 无该协议） |
| 5250 主帧协议 | tn5250j | 不适用（非 ANSI/xterm 域） |
| O(1) row-map 滚动、内嵌 59k Nerd Font | midasdf/zt | 不需要/未声明：滚动归上游；内嵌字体与「按需加载宿主字体」的字体路线相悖 |
| 单帧处理 deadline/quantum、edge-triggered 唤醒 | missdeer-mostty | 不立项：本仓读链已有界通道（`OUTPUT_CHANNEL_BOUND` × 8KB）封顶，level-poll + `AtomicBool` 已实现对等语义；帧预算属过度设计嫌疑 |
| 「同引擎对标仓」的 PTY→UI 交接模式 | missdeer-mostty | 仅审计参照（本仓已有等价物）；该报告被「自研 VT 全部禁止」群体口径错裁，按事实更正——它与本仓同为 libghostty 唯一 VT 核 |
| Zig libghostty wrapper 的 callback 生命周期模式 | sudo-tee-hollow | 仅审计参照：本仓 wrapper 已有等价脏跟踪与状态转换实现；代码移植被许可未确认阻断，仅模式对照 |
| type-ahead 输入 | techsenger-jeditermfx | 未声明，不立项（`InputBatchBuffer` 已保序，与 JetBrains/jediterm 同口径） |
| IME 组字内外删除分流、组字光标模式 | anoterm | 仅参照（本仓已覆盖）：删除与改写面归 `ime-editor-contract`「替换类编辑必须到达终端」「已提交词的改写经重锚送达终端」，光标锚点归「组字光标位置回报给输入法」，无新增引入 |
| Unicode 17 graphemes/跨单元连字 | pg83-shitty | DESIGN 明示不做跨单元连字（单码位整形与复杂文种叠加除外） |
| alacritty_terminal 作替代 VT 核 | pop-os-cosmic-term | 不采纳（终端状态单一来源） |
| 设备性能自动化门禁 | nihiL7331-sfte | 登记现状：无自动化门禁，性能证据为代码注释内 ad-hoc 实测；自动化留待维护者裁定 |
| CI 产物哈希/二进制一致性比对 | vercel-labs-wterm | 登记待维护者裁定：`.github/` 属禁改区，本轮不改 |

#### Scenario: 功能面判定可溯源

- **WHEN** 读到某个功能面的判定
- **THEN** MUST 能在本表读到出处报告与依据（`DESIGN.md` 未声明、`PROHIBITED.md` 条目，或本仓既有实现）

### Requirement: 参照面与反面做法收敛补齐行

编号文档集（`00`–`30`）已从工作区删除，其参照面与反面做法 MUST 在本规范各表有可读去向；
`docs/research/ecosystem.md` 与逐仓报告的参照面／反面做法同此要求。
反面做法 MUST 点名出处仓库与做法并标注「不得反向采纳」或「不采纳」，
MUST NOT 要求读者恢复已删除的研究报告才能复核。

| 项 | 出处 | 判定与依据 |
| --- | --- | --- |
| PTY 输出合并投递与 UTF-8 粘包保留解码 | puppy、anoterm | 仅参照：本仓会话与输出管道在 Rust，字节直传上游 VT stream，无自研粘包解码；跨块半扫由 `output_processor.rs` 用例覆盖 |
| 迁移文档的问题清单组织 | termux-kotlin-app | 仅参照（文档思路，无代码落点）：本仓文档面为 `openspec/` 与 `docs/specification/`；其「保留上游测试套件」不是正例——单测仅 2 文件约 18 例，可采纳断言仅键位编码与鼠标两类（主台账 `reapercanuk39/termux-kotlin-app` 行） |
| 自造 `svc #0` syscall 做 JNI 桥 | on3-terminal | **不得反向采纳**：绑定面走标准 JNI／libc 调用，自造 syscall 无跨设备可移植性 |
| `curl \| sh` 式引导安装 | shafinthedev/Ply（`src/main.rs:100-104`） | **不得反向采纳**：引导安装按 `bootstrap-install` 固定流程执行，不把远端脚本接进 shell 管道 |
| 渲染热路径内的 `measureText` | puppy | **不得反向采纳**：与既有台账行「在渲染热路径内改写终端状态」同口径，文本度量不得进每帧路径；其构建产物入库一并计入既有台账「仓库内提交 CMake 生成物与构建产物」行 |
| 治理文档膨胀（「文档海」） | termux-app-rafacodephi、termux-kotlin-app | **不得反向采纳**：文档面收敛为 `openspec/` 与 `docs/specification/`，研究过程记录不常驻 |
| 整包完整性校验（BLAKE3 与 sha256 双实现） | termux-app-rafacodephi（`BootstrapIntegrityVerifier` 走 BLAKE3；loader `BootstrapChecksumValidator` 走 sha256） | 不采纳：整包校验与 `PROHIBITED.md`「Bootstrap zip 的 sha256 sidecar 校验」同口径 |
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

### Requirement: 禁改区待维护者裁定项台账（只登记，不实施）

`docs/specification/TESTING.md` 要求而现行门禁缺席的四项，全部落在 AGENTS.md
禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者已裁定
「都不改，只登记」，落地方案记录于此，不擅自实施：本表各项 MUST 维持登记状态，
未经维护者明确同意 MUST NOT 实施。

| 项 | 规范依据 | 落地方案（未实施） |
| --- | --- | --- |
| aislop / jscpd 代码检查门禁 | TESTING.md 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 |
| rapidocr OCR 自动化 | TESTING.md 环境节 + 外部基准优先级 4 | `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用 |
| CI 产物哈希比对 | vercel-labs-wterm 研究结论 | `build.yml` 增产物 sha256 记录步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | GlobalTechInfo-RedTerm 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 |

#### Scenario: 禁改区项不被静默实施

- **WHEN** 本轮或后续复查接触到上述四项
- **THEN** MUST 维持登记状态并按维护者裁定执行，MUST NOT 以「规范要求」为由擅改
      禁改区文件
