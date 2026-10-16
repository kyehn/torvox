## ADDED Requirements

### Requirement: 研究报告逐份索引与文档可移除性

`docs/research` 的每份研究报告 MUST 在本规范内有一条**以报告文件名为键**的索引行，
写明该报告的实质、判定结果与依据位置（本规范某张表、其他 spec 的能力名、或已落地代码）。
索引行 MUST 能独立回答「该仓有无可用资产、结论是什么、为什么」，MUST NOT 以
「其余若干份已核对」这类群体概括收尾。

索引完整是本仓删除研究文档的**前置条件**：任取一份报告，其全部可执行项的去向
MUST 已在索引行内可读到；满足该条件后报告本体 MUST 可被删除而不丢失任何仍需遵守的结论。
本仓现已删除 `docs/research` 全部报告，索引表是这批研究的**唯一常驻载体**；
日后若重新引入研究文档，索引行 MUST 与文档同批提交。

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
与各台账取代；表中依据列写作「生态参照面组行」的行，指同一组「形态或技术栈与本仓
无交集、仓内无可用资产」的报告行，其共同事实依据即上段生态扫描与 `DESIGN.md` 的
未声明口径，不再单设评级表。

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

- **WHEN** 遍历 `docs/research/` 曾有的任一份报告文件
- **THEN** 索引表内有以该文件名为键的行，且该行的判定与依据可在本规范内读到

#### Scenario: 删除报告不丢结论

- **WHEN** 某份报告被删除（现已整体删除）
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
| `8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 子集 + C PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；其 OSC 注释清单作生态参照面，OSC 52 安全姿态归 OSC 52 条款；多击选词／OSC 8 白名单／提交构建产物三做法属不得反向采纳 | 「Android 侧终端项目的复查台账」terminator 行；「OSC 覆盖逐项核对」；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `DanteAlighierin-foot.md` | C/Wayland 自研 VT 极简终端 | 不采纳：仓内无 VT／网格语料（「无资产」而非门槛违反）；自研 C 解析器违单一来源，box-drawing／组合宽度只作结构对照 | 「已复查结论台账」foot 行 |
| `GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 发行版运行器 | 不采纳（门槛 3）：仓内无本仓组件，329 例属 SSH/SFTP/proot 面；16KB 页对齐登记禁改区 | 台账不可采纳行；「禁改区待维护者裁定项台账」；参照面「搜索高亮两层透明度」归 `text-search`，备份「完成后替换 + 孤儿清理」归「参照面与反面做法收敛补齐行」 |
| `JetBrains-jediterm.md` | JVM/Swing 可嵌入终端库（自研 VT + Pty4J） | 不采纳（门槛 1）：before/after 期望即其自身输出；vttest 派生子集按输入分流逐例判定 | 「已复查结论台账」vttest 行；「按输入是否格式合规分流采纳」 |
| `KDE-konsole.md` | C++/Qt 桌面终端（Vt102Emulation） | 不采纳：桌面 C++ 自研 VT；profile/session 编辑撞 PROHIBITED，DBus／i18n／tysend 未声明 | 「能力裁定补齐行」DBus、i18n 与 ZMODEM/tysend 行；「语料判定补齐行」konsole 行；「不得反向采纳的做法」 |
| `KDE-maui-station.md` | Qt6/Kirigami 移动终端（MauiKit） | 不采纳：QML 集成在 Android 无对应形态；仓内无 VT 资产（其 `split` 指 Service/UI 分层进程模型，非分屏） | 本索引行 |
| `KDE-yakuake.md` | Konsole KPart 下拉式壳 | 不采纳：下拉 UX 无 Android 对应，无新仿真 | 本索引行 |
| `Kuddev-pebrel.md` | Rust+GPUI 桌面 GPU 终端（vte 核） | 部分采纳：release profile（LTO／codegen-units／strip）已落地；录制回放语料不采纳（门槛 1）；PTY 事件循环与 redraw anchor 已被本仓 VT 线程 + 行级脏跟踪越过；GPL-3.0 不引入代码 | `native/Cargo.toml`；「语料判定补齐行」pebrel 行；「能力裁定补齐行」分屏与 vi-like 行 |
| `PaNDa2code-zerotty.md` | Zig+Vulkan 早期终端 | 不采纳：Zig 转换对 Android 无用途，自研 VT 违单一来源 | 生态参照面组行 |
| `PerBothner-DomTerm.md` | C/Java/JS WebSocket+DOM 终端 | 不采纳：Web UI 栈形态，无 Rust 依赖可嵌入 | 「能力裁定补齐行」WebSocket 行 |
| `RohitKushvaha01-ReTerminal.md` | Compose 壳 + Termux View | 不采纳（门槛 3）：VT 走上游 jar；proot 禁入 | Android 台账（kosh／ReTerminal 同源行）；`documents-provider`（其文档提供器形态已落地） |
| `TermoraDev-termora.md` | Kotlin/JVM Swing 终端 + SSH（自研 KTerm） | 不采纳：自研 VT 违单一来源；KeyEncoder／Selection／Find 模型本仓已有等价实现 | 生态参照面组行 |
| `TragicWarrior-libvtty.md` | C VT 库（libvterm fork） | 不采纳：VT 核不可换；C 库经 NDK 引入净增 FFI 且违单一来源 | 生态参照面组行；「语料判定补齐行」libvtty 行 |
| `Yazelix-mars.md` | Rust/wgpu（Rio 派生，alacritty 核） | 不采纳：VT 核冲突；字形基线与图集本仓已有等价实现 | 生态参照面组行 |
| `ademmenh-zebra.md` | C+GTK3+VTE 极简示例 | 不采纳：无新洞察、无资产 | 本索引行 |
| `alacritty-termbenchbot.md` | 终端基准自动化工具 | 仅参照：指标维度（cursor_motion／dense_cells 等）作方法论，无可执行项 | 「语料判定补齐行」termbenchbot 行 |
| `aligrudi-fbpad.md` | C framebuffer 极简终端 | 不采纳：framebuffer 单头、无 Android 落点（许可未声明不作判据） | 本索引行 |
| `benyblack-ntilde.md` | .NET/Avalonia+Skia 终端 + conformance 矩阵 | 仅参照：差异矩阵与回放调试思路；语料为该实现自身输出，不采纳 | 「语料判定补齐行」ntilde 行 |
| `borisfaure-terminology.md` | C/EFL 桌面终端 | 不采纳：shell 回归以人眼读 diff（无期望文件）；tysend 文件传输未声明；主题格式本仓以构建期 alacritty-theme 为唯一入口 | 「已复查结论台账」门槛 1 判据；`theme-configuration`；「能力裁定补齐行」ZMODEM/tysend 行 |
| `contour-terminal-contour.md` | C++23+OpenGL 桌面终端（自带 vtconformance） | 不采纳（门槛 1）：脚本只发序列等待、7 个靠人工按键；图集与下划线几何本仓已有等价实现 | 「已复查结论台账」contour 行；同步刷新走「不采用的上游回调不得留有通道描述」（`on_render_hold` 不接线） |
| `cutefishos-terminal.md` | C++/Qt/QML（内嵌 `qmltermwidget` 控件，包 VTE/Konsole 系实现） | 不采纳：QML/C++ 形态在 Android 无对应；所嵌控件非本仓组件、未逐 crate 确认可移植性；无测试数据 | 本索引行 |
| `dakra-ghostel.md` | Zig + libghostty-vt 的 Emacs 动态模块（同引擎第四家，已支持 Android/Termux） | 仅参照：同引擎 FFI 模式（分配器／Stream／screen pages／pin tracking）与脏跟踪已由本仓 wrapper 覆盖，代码不移植（GPL-3.0-or-later + Emacs 模块形态） | 本索引行；「语料判定补齐行」ghostel 行 |
| `elementary-terminal.md` | Vala/GTK4 + VTE 桌面终端 | 不采纳：VTE 与单一来源冲突；不安全粘贴对话框撞 PROHIBITED；VTE shell 设置集成（GApplication settings schema）无 Android 落点 | 「能力裁定补齐行」；PROHIBITED 粘贴确认对话框；「语料判定补齐行」elementary 行 |
| `felixse-FluentTerminal.md` | UWP+C#/WebView（xterm.js），已停更 | 不采纳：VT 与渲染都在别处；主题导入撞「构建期读取、主题列表固定」；`TerminalBridge` 事件桥属 WebView host 模式、本仓无对应形态 | `theme-configuration`；「语料判定补齐行」felixse 行（`.itermcolors` 样例）；生态参照面组行 |
| `floooh-chipz-kitty.md` | Zig 游戏 demo，用 Kitty 图形协议 | 不采纳：Kitty 图形由上游承担；无终端核心内容 | 「上游 API 取用判定登记」Kitty 行 |
| `fyne-io-terminal.md` | Go/Fyne 终端 widget | 不采纳：Go 不可用于 Android Rust 层；`test_data` 属其自研解析器期望 | 「语料判定补齐行」fyne 行 |
| `ghreprimand-odytty.md` | Rust+wgpu 自研 VT | 不采纳：自研 VT 违单一来源；wgpu 渲染组织本仓已越过；native GPU 测试套件属其自身期望，GPL-3.0-only 前置 | 生态参照面组行；「语料判定补齐行」odytty 行 |
| `godot-pty-gpty.md` | Rust+Godot 扩展（alacritty PTY，AI 向） | 不采纳：MCP 明禁、事件总线与平铺多路复用未声明；portable-pty 无 Android 后端；GPL-3.0-only 前置 | 「能力裁定补齐行」MCP 明禁行与分屏行；「语料判定补齐行」gpty 行 |
| `harmoninux-HiSH.md` | HarmonyOS + QEMU 跑完整 Linux VM | 不采纳：非终端模拟器；QEMU 作宿主过重 | 生态参照面组行 |
| `hugomyb-Lume.md` | Tauri/SolidJS+xterm.js（远程／AI） | 不采纳：Web 栈；replay ring 与会话持久化撞 PROHIBITED；命令面板、远程会话、AI 集成均未声明；OSC 133 命令块仅作上游能力对照 | 「能力裁定补齐行」会话持久化行、命令面板行、远程会话行、AI 集成行、OSC 133 行 |
| `hugopl-batata.md` | Crystal/GTK4+VTE 桌面平铺终端 | 不采纳：Crystal/VTE 与 Android 无交集；分屏未声明 | 「能力裁定补齐行」分屏行 |
| `iewnfod-lumina-terminal.md` | Tauri/Rust+React+xterm.js（含 MCP server） | 不采纳：WebView 形态；MCP 明禁、命令面板未声明；portable-pty 不适用 Android | 「能力裁定补齐行」MCP 明禁行、命令面板行 |
| `ii8-havoc.md` | C/Wayland 极简终端（libtsm） | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行 |
| `isene-glass.md` | x86_64 汇编 + X11 极简终端 | 不采纳：仅 x86_64、成熟度低、无 Android 落点 | 本索引行 |
| `ismail-yilmaz-Bobcat.md` | C++17/U++ 桌面终端（外部 TerminalCtrl） | 不采纳：VT 为外部依赖且非 Ghostty；U++ 非 Android；vi-like 选择器与按键绑定未声明 | 生态参照面组行；「能力裁定补齐行」vi-like 行；「OSC 覆盖逐项核对」1337 行 |
| `javaterminal-TerminalFX.md` | Java/JavaFX 终端 widget（core／local 分离） | 不采纳：JavaFX 无 Android 目标；core/PTY 解耦模型本仓 JNI 边界已有等价 | 生态参照面组行 |
| `joshka-qwertty-term.md` | Rust 重写 Ghostty 语义 + Metal 渲染 | 不采纳（门槛 1）：差分期望即其自身实现输出；本仓 VT 语义整体交上游 | 「上游 VT 语义不由本仓测试覆盖」 |
| `kemokempo-iced_term.md` | Rust/ICED(wgpu) + alacritty 核 | 不采纳：alacritty 核违单一来源；iced 非 Android UI 目标；cell 绘制与本仓渲染已有等价 | 生态参照面组行；「能力裁定补齐行」第三方渲染实现行 |
| `ketraterm-KetraTerm.md` | Kotlin/JVM Swing 终端库 + xterm oracle | 不采纳：JVM/Swing 无 Android 目标；oracle 为自身实现输出，帧租约本仓渲染线程已持有 | 生态参照面组行；「语料判定补齐行」KetraTerm 行；「能力裁定补齐行」第三方渲染实现行 |
| `kmscon-libtsm.md` | C 可复用 VT 状态机库 | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行；「语料判定补齐行」libtsm 行 |
| `kshivang-BossTerm.md` | KMP 终端库（jEditerm 派生）+ Compose | 不采纳：VT 核冲突；Compose 接线本仓已有同栈实现（其 `split` 指字形拆分宽度，非分屏）；LGPL-3.0 许可审查前置 | 生态参照面组行；「语料判定补齐行」BossTerm 行 |
| `leoberbert-zashterminal.md` | Python/GTK4+VTE 桌面终端 | 不采纳：VTE 冲突；会话管理与广播属桌面形态；文件管理器侧栏、快捷提示、语法高亮与 AI 集成未声明 | 生态参照面组行；「能力裁定补齐行」文件管理器行与 AI 集成行 |
| `lihop-godot-xterm.md` | Godot 终端控件（libtsm/node-pty） | 不采纳（PROHIBITED + 人工目检）：asciicast 回放撞会话持久化；colortest 靠人眼 | 「语料判定补齐行」godot-xterm 行；「能力裁定补齐行」Terminal API 行与第三方渲染实现行 |
| `lmorg-ttyphoon.md` | Go/Wails 多媒体终端（自研 VT） | 不采纳：自研 VT + WebView；输出块分组与命令退出状态着色、结构化输出块、AI 集成未声明 | 生态参照面组行；「语料判定补齐行」ttyphoon 行；「能力裁定补齐行」结构化输出行与 AI 集成行 |
| `lxqt-qterminal.md` | C++/Qt6（qtermwidget） | 不采纳：桌面 C++；DBus／i18n 未声明 | 「能力裁定补齐行」DBus 与 i18n 行 |
| `marc2332-term.md` | Rust+Freya Flatpak 桌面终端 | 不采纳：桌面 GUI；JSON 会话持久化撞 PROHIBITED，分屏未声明 | 「能力裁定补齐行」会话持久化行与分屏行 |
| `marshallku-copad.md` | Rust+原生 UI 跨平台 AI 终端（alacritty） | 不采纳：alacritty 核冲突；插件／事件总线、comux 工作树集成未声明；报告无 MCP（此前误记已更正） | 「能力裁定补齐行」插件行与 worktree 行 |
| `midasdf-zt.md` | Zig 极简多后端终端 | 不采纳：row-map 归上游；内嵌字体与「按需加载宿主字体」路线相悖 | 「能力裁定补齐行」row-map 行 |
| `migueldeicaza-SwiftTerm.md` | Swift 自研 VT（含 Ghostty fuzz 对照） | 不采纳（门槛 1）：期望即其自研实现输出；fuzz 语料对照已由本仓 `external_benchmarks` 落地 | 「语料判定补齐行」SwiftTerm 行 |
| `mintty-mintty.md` | C/Win32+Cygwin 终端 | 不采纳：宽度表归上游；Sixel 已登记、BiDi 未声明 | 「能力裁定补齐行」BiDi 行；「语料判定补齐行」mintty 行 |
| `mirbehnam-RtlTerminal.md` | C#/WPF Windows RTL 终端 | 不采纳：RTL 整形与 layoutDirection 均未声明 | 「能力裁定补齐行」RTL 两行 |
| `missdeer-mostty.md` | Zig + libghostty 桌面原生终端 | 不立项：单帧 deadline／quantum 属过度设计，本仓读链已有界；PTY→UI 模式已有等价物 | 「能力裁定补齐行」mostty 两行（含错裁更正） |
| `mrlima4095-OpenTTY-J2ME.md` | J2ME MIDlet + RISC-V 模拟器 | 不采纳：与终端组件无现实关联 | 生态参照面组行 |
| `nihiL7331-sfte.md` | C 单头文件极小终端（st 衍生） | 不采纳：仓内仅 fuzz 宏无语料；设备性能门禁登记在案 | 「语料判定补齐行」sfte 行；「能力裁定补齐行」设备性能自动化门禁行 |
| `not-elm-orzma.md` | Rust 自研 VT + WebView 混合终端 | 不采纳（门槛 1+3）：第三方 VT 差分；WebView 超范围 | 「语料判定补齐行」orzma 行 |
| `nowledge-co-con-terminal.md` | Rust+GPUI 全量 Ghostty 的 AI 终端 | 不采纳（门槛 1）：conformance 期望为自身输出；完整 libghostty 无 Android 平台 | 「语料判定补齐行」con 行；本条「禁止项三条保留」段（不等待完整 libghostty 支持 Android） |
| `omnipty-terminal-omnipty.md` | Rust+GPUI+alacritty 桌面终端 | 不采纳：GPUI 非 Android；事件循环本仓已有等价 | 生态参照面组行 |
| `orhun-ratty.md` | Rust/Bevy/wgpu 3D GPU 终端 | 不采纳：present 管线早于本仓字形图集 + cell 实例化；Bevy 过重 | 生态参照面组行（含 font/atlas 悬问闭合） |
| `paulrobello-par-term-emu-core-rust.md` | Rust VT 引擎核 + C/Python 绑定 | 不采纳（门槛 1+3）：第三方 VT 差分；Ghostty 覆盖更广 | 「语料判定补齐行」par-term 行 |
| `paulrobello-par-term.md` | Rust/wgpu/egui GPU 终端 | 采纳（已落地）：GPU surface 尺寸钳制到设备上限；egui／agent API、命令面板、ACP 分诊等不采纳 | `render-stability`「Surface 尺寸钳制到设备上限」；「能力裁定补齐行」命令面板行与 AI 集成行；「语料判定补齐行」 |
| `pg83-shitty.md` | C++/Vulkan 桌面终端（自研 vterm） | 不采纳（门槛 1 + 穷举扩张禁止）；许可双轨悬问已闭合；damage 驱动渲染与字形回退本仓已有等价 | 「语料判定补齐行」shitty 行；「能力裁定补齐行」第三方渲染实现行 |
| `pop-os-cosmic-term.md` | Rust/iced + alacritty 终端 | 不采纳：alacritty_terminal 作替代核违单一来源 | 「能力裁定补齐行」 |
| `rricharz-Tek4010.md` | C/SDL Tek 图形仿真 | 不采纳：Tek 序列未声明（Ghostty 无该协议） | 「能力裁定补齐行」Tek4010 行 |
| `sanohiro-bcon.md` | Rust/OpenGL ES bare-TTY 终端 | 不采纳（门槛 1+3）：vte crate 作 test shim 属第三方 VT 差分 | 「语料判定补齐行」bcon 行 |
| `sudo-tee-hollow.md` | Zig/LuaJIT + Ghostty 核桌面终端 | 仅参照：同引擎 wrapper 回调生命周期已有等价实现，代码不移植；插件/mux 测试非 VT 语料 | 「能力裁定补齐行」同引擎组行与插件行；「语料判定补齐行」hollow 行 |
| `techsenger-jeditermfx.md` | JavaFX/JediTerm xterm 仿真器 | 不采纳（门槛 1）：before/after 期望与其实现同源；type-ahead 未声明 | 「语料判定补齐行」jeditermfx 行；「能力裁定补齐行」 |
| `tenox7-aclock.md` | C 终端文字时钟玩具 | 不采纳：玩具级 VT 绘制；其 escape 绘制样例可作渲染器兼容性人工对照目标，非语料 | 生态参照面组行 |
| `tn5250j-tn5250j.md` | Java IBM 5250 主机终端 | 不采纳：非 ANSI/xterm 域 | 「能力裁定补齐行」5250 行 |
| `tuzig-terminal7.md` | TS/xterm.js/Capacitor 远程终端 | 不采纳：Web-first 无 Android 落点、网络终端未声明；webexec 超 local-only 范围 | 「语料判定补齐行」terminal7 行；「能力裁定补齐行」WebSocket 行 |
| `veops-go-ansiterm.md` | Go 屏幕模拟／命令提取 | 不采纳（门槛 3）：Go 库无 Android 落点，断言未编目 | 「语料判定补齐行」go-ansiterm 行 |
| `vercel-labs-wterm.md` | Zig/WASM + DOM 渲染（封装 libghostty） | 不采纳：WASM/DOM 无法作为 Android 依赖；无障碍/文本捕获未声明（朗读形态撞「无障碍朗读」明禁）；WS 重连／背压未声明；CI 产物哈希登记禁改区 | 「语料判定补齐行」wterm 行；「能力裁定补齐行」WebSocket 行与无障碍行；「禁改区待维护者裁定项」 |
| `wilsonglasser-oryxis.md` | Rust/iced + alacritty SSH 客户端 | 不采纳：alacritty 核冲突；config vault／会话恢复撞 PROHIBITED；crate tests 未编目 | 「能力裁定补齐行」会话持久化行与 ZMODEM/tysend 行；「语料判定补齐行」oryxis 行 |
| `z-hunter-Scanline-term.md` | Tauri/React/xterm.js 复古终端 | 不采纳：无 VT conformance 套件；MCP 明禁、CRT 特效未声明、会话恢复明禁 | 「语料判定补齐行」Scanline 行；「能力裁定补齐行」MCP 明禁行 |
| `ecosystem.md` | 生态参考与测试边界（参照面评级、禁止项、测试边界） | 内容已并入：参照面评级 → 本表「实质」列（评级语义与「生态参照面组行」定义见本条正文）；禁止项 → 本条「禁止项三条保留」段；测试边界 → `terminal-state-regression` 与本能力「上游 VT 语义不由本仓测试覆盖」；参照面表的可落地项按名落点（`pty-lifecycle` 收 `IUTF8`／像素 winsize／前台组收尾，`text-search` 收搜索上限与两层高亮，`settings-propagation` 收 live／spawn-time，`render-stability` 收键盘动画期不暂停，通知常驻归 `DESIGN.md` 软件节），其余 → 「参照面与反面做法收敛补齐行」 | 本表；`pty-lifecycle`；`text-search`；`settings-propagation`；`render-stability`；「参照面与反面做法收敛补齐行」；`terminal-state-regression` |
| 用户指定 commit 的编号文档集（`00`–`30`，共 31 份，已不在工作区） | 逐仓深研编号集，现行 73 份报告的前身 | 已处置：结论并入现行报告集与本规范各表，曾用于立项 `bootstrap-install`、PTY 收尾清单、搜索上限、文档提供器形态等 | `openspec/changes/archive/` 台账；本规范各表 |
