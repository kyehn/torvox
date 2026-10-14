# 生态参考与测试边界

外部项目经逐仓代码级审查后**留下的、可继续使用的结论**。已经用完的过程记录
（逐仓深读、复审轮次、收敛台账）不入本文件——本文件只保留日后仍需对照的判断、
禁止项与测试边界。

## 一、对标仓库与参照面

评级含义：S 为架构/UX 直接对标源；A 为有可落地的具体参考点；B 为对照阅读价值；
C 为仅记录在案或反面教材。「参照面」是本仓实现时该仓的**具体**可比对象。

| 仓库 | 技术栈 | 评级 | 参照面 |
| --- | --- | --- | --- |
| sylirre/ghostty-android-terminal | Java + 预编译 `libghostty-vt.a` + Canvas | S | 「不自研 VT」的 Java 落法；搜索上限 + 全局真值计数 + 免重扫的 `search_dirty`；选择手柄边缘翻转与拖拽 reorder；`ptyHangupForeground` 的前台进程组优先收尾；spawn 带像素 winsize 与 no-op resize 跳过 |
| termux/termux-app | Java + C，自研 VT + Canvas | S | 会话列表与文本选择的 UX 基准；`IUTF8` 退格一次一整字；常驻通知与退出按钮；Bootstrap 的 staging→rename 流程 |
| 8dmusichannels-star/terminator | Kotlin Compose + C 自研 VT | A | 设置下发的 live / spawn-time 二分法；DEC/xterm/ECMA-48 OSC 逐项矩阵；键盘动画期间暂停渲染 |
| CocaKova/charon | 纯 Kotlin/JVM 自研 VT | A | 「若必须自研 VT」的质量标杆；IME 全文本框镜像与五项填坑；选择空间负行号心智模型；语料回放作回归手段 |
| mirute02/anoterm | Kotlin Compose 自研 VT 子集 | A | CJK 宽度以 `Character.getType` + EAW 表而非单一码点范围判定；日语视角的两项 IME 补漏 |
| sohan-f/termux-monet | termux fork + Monet | A | 「刷新作用全部会话」的 bug 驱动教训；用户配置优先于动态取值 |
| pavelc4/Rin | Rust 引擎 + Compose，手写 VT 解析层 | A | 手写 ANSI 解析必为不完备子集——本仓「终端状态单一来源」的论据 |
| Lord1Egypt/AndroidThothTerm | JackPal 血统，BiDi 渲染 | B | 生态唯一 BiDi 实例（本仓规范不含）；Service 持有会话 |
| GlobalTechInfo/RedTerm | JitPack 复用 Termux 库 | B | 完成后替换 + 孤儿清理；搜索高亮覆盖层两层反色 |
| frostre1997/kosh、RohitKushvaha01/ReTerminal | Termux View + Compose 壳 | B | 文档提供器形态（ReTerminal 已停更） |
| reapercanuk39/termux-kotlin-app | termux 完整 Kotlin 化 | B | 迁移文档的组织方式；**其「保留上游测试套件」不是正例**（单测仅 2 文件约 18 例） |
| Adhi-hub07/on3-terminal | termite 范式，C PTY + Kotlin VT | B | `IUTF8`、像素 winsize、收尾 HUP→轮询→KILL 三例共识；反面：自造 `svc #0` syscall JNI、宽表缺 `isLowSurrogate` 校验、产物入库 |
| imlunixdev/puppy | Kotlin + C++ PTY | B | 合并投递防粘包；反面：`curl \| sh`、构建产物入库、`onDraw` 内 `measureText` + `resize` |
| rafaelmeloreisnovo/termux-app-rafacodephi | termux 重型 fork | B | 行尾孤立高代理越界的真实修复（本仓宽字符边界必须有 limit 检查）；反面：文档海、BLAKE3 整包校验 |
| Rprop/AnsolePlus | 旧版 termux 打包 | C | 反面：与上游同位置的孤立代理项越界未修 |
| Autonomous-Agentic-Workflows/termux-app | 旧基线 fork | C | 反面：删文件回传门控 + JNI 释放错对象（`ReleaseStringUTFChars` 写错对象） |
| vercel-labs/wterm | Zig/WASM + DOM 渲染，`@wterm/ghostty` 走 libghostty 的 WASM 封装 | C | 证明 libghostty 可作 WASM 后端，但本仓已直接用 `libghostty-vt-sys`；浏览器/WASM 形态无法作为 Android 依赖。其 WebSocket PTY 的重连与背压设计仅在日后开网络终端时可参考 |
| felixse/FluentTerminal（已停更）、lxqt/qterminal、KDE/konsole、KDE/yakuake、JetBrains/jediterm、borisfaure/terminology | 桌面壳 + xterm.js / QML | C | 桌面端复用 xterm.js，VT 与渲染都在别处，与本仓的单一来源正面冲突。可参考的只有产品面（主题导入、Shell Profile、键位编辑），而这些恰是本仓明禁项或未声明项 |
| irealashu/ApexShell、shafinthedev/Ply、Sunbr99/TerminalOS、TOYTTQE/PufiX、eggie-admin/hydra-shell-android、deathlegionteamlk/jolz、Malik1stmkw/termux-clone | — | C | 无终端实现、无 PTY 或玩具；`catch (_: Exception) {}` 通篇；su/Shizuku 提权 |

## 二、仍然有效的结论

- **终端状态单一来源**：生态内「复用 Ghostty」有 Java + 预编译库、自绑 Rust、
  复用 Termux 库三条路；「自研 VT」四条路全部是子集（Termux 自身也被 fork 删过
  `doDcs`/`sixel`）。本仓走自绑 Rust + wgpu，生态内无第二家。
- **GPU 渲染**：25 仓跨仓扫描，`wgpu` 零命中、完整 GPU 终端渲染路径仅本仓。
  Canvas 逐格系在 CJK 性能上先天不足；Termux 与 ghostty 都已批注「等宽 ASCII 的
  advance 恰为 cellWidth」这一前提，正是本仓字形图集 + 整形的职责边界。
- **会话不随 Activity 走**是跨生态共识（ghostty 进程单例、ThothTerm 前台 Service、
  本仓会话归 Rust）。ghostty `attachSession` 的跨会话清理清单值得对照自查：
  结束选区、关搜索、清图缓存、重置输入、滚动归零。
- **更完整的 VT 核不可换**：按成熟度 `alacritty_terminal`/`wezterm-terminal` 可 rival，
  含 GUI 的完整 libghostty 最全但 `ghostty_platform_e` 仅 `INVALID/MACOS/IOS`、
  对本仓（Android）无收益；`rio-vt` 须从 rio 仓库自建、不上 crates.io。对本仓
  确切约束（Android + Rust 宿主 + wgpu + 无头 + C ABI + kitty 图形 + 单一来源）
  无「更完整又能直接换」者。
- **减码空间**：本仓无死代码可白捡（`allow(dead_code)`/`todo!`/`unimplemented!`
  为 0），字体子系统是最大单块。降本只能靠重构而非换库。
- **新参考项目也买不到依赖**：跨形态对照后结论不变——Web/WASM 系（即便直接封装
  了 libghostty）与桌面壳系（复用 xterm.js / QML）都把 VT 解析与渲染放在别处，
  与本仓的单一来源正面冲突，能借鉴的只有产品面。

## 三、禁止项（勿从外部仓库抄入）

- 不 vendor、paste 或引入片段自 wezterm / rio / alacritty：可自动获取的文件不得
  vendor（`BUILD.md`），且引入许可证文件违反 `STYLE.md`。净省近零而维护负担大。
- 不为此更换 VT 核：违「终端状态单一来源」，且不贴合无头 C ABI 模型。
- 不等待完整 libghostty 支持 Android：当前仅 Apple 平台。

## 四、测试边界

- **不测**：VT 解析器与终端状态机本身的正确性——该职责整体委托 `libghostty-vt`，
  上游有权威 fuzz（`test/fuzz-libghostty` 含 parser/stream/osc 模糊测试）。
  再造穷举式 VT 一致性属「测试依赖行为」。esctest/vttest 的**全量**同理。
- **保留**：接缝与契约。写字节 → Ghostty → `RenderState` 快照 → 回读断言，
  测的是本仓接线。依赖 rev 一旦改动破坏本仓契约而失败，是正当信号；
  「依赖的小瑕疵」不是本仓 bug。
- **外部数据即基准**：期望值来自标准文本、上游协议文档或运行期现取的外部数据，
  按 `upstream-alignment` 的采纳门槛单独判定，数据不入库。

当前落地情况与逐条边界见 `openspec/specs/terminal-state-regression` 与
`openspec/specs/upstream-alignment`；字体表的真值对照见
`native/src/render/font/rasterization.rs` 与 `native/src/render/font/font_db.rs` 的
宿主字体库用例（数据取自开发环境固定的 fontconfig 字体集）。
