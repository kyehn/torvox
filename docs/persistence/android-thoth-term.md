# AndroidThothTerm

## 来源与口径

来源：<https://github.com/Lord1Egypt/AndroidThothTerm>。取样于 2026-11-02 主线，仓库同时存在已发布的终端版与尚未实现的 Linux 版（`com.thothterm.linux`）。

本文记录该项目的架构与实现要点，作为本项目实现细节（`DESIGN.md`「未声明的实现细节」）的参照面，并给出逐项借鉴判定。

与 `openspec/specs/upstream-alignment/spec.md` 台账中 `Lord1Egypt/AndroidThothTerm` 行的差异：上游此后重构为 `term/`（终端版应用壳）与 `garden-*`（Linux 版 flavor）多模块，新增「上传到会话当前目录」、BiDi 与阿拉伯文渲染、`docs/security/THORNS.md` 安全契约、导出组件策略测试与 Golden 证据；`emulatorview` 单元测试由 3 文件 22 例增至 5 文件 34 例；`tests/` 语料目录由 122 文件增至 131 文件，仍无任何构建脚本引用。

## 仓库结构

| 模块 | 作用 |
| --- | --- |
| `emulatorview/` | JackPal 血统的 Java VT 引擎与 View 渲染（`TerminalEmulator`、`EmulatorView`、`PaintRenderer`、`TermSession`） |
| `term/` | 终端版应用壳：会话服务、窗口列表、修饰键栏、上传、日志、远程组件 |
| `term-ubuntu/`、`garden-common/`、`garden-debian/`、`garden-arch/`、`garden-security/` | Linux 版：PRoot 运行时、rootfs 生命周期、发行版安装界面、LAN 模式 |
| `libtermexec/` | PTY 相关的 JNI 执行封装 |
| `tests/` | 控制序列、宽字符、安全策略、PRoot 运行时等语料与脚本 |
| `docs/` | 产品文档，含 `docs/security/THORNS.md` 与 `docs/evidence/golden/` |

产品形态已拆成两支：终端版只连 Android 自身的 shell，不含 PRoot、apt 或任何 Linux rootfs；Linux 版仍在计划中。

## 终端引擎能力

自研 VT 解析（JackPal 血统），覆盖面窄，实测：

- OSC 只有 0/1/2（`doOsc` 中的标题与图标名），无 OSC 4/7/8/52/133 与 iTerm2 扩展。
- DA1 自述为 `ESC[?1;2c`（`sendDeviceAttributes` 中的 VT100 注释与字节序列）。
- DECSET 仅插入模式 4；鼠标跟踪 1000–1003 与格式 1006（SGR），无 1005/1015/1016、无焦点上报 1004、无 bracketed paste 2004、无同步输出 2026、无颜色方案上报 2031 与可见性上报 2033。
- 无 kitty 图形与键盘协议、无 sixel、无 iTerm2 图像。

与本项目对照：本项目解析唯一真源是 `libghostty-vt`，能力面见 `docs/persistence/terminal-protocols.md`。该引擎不可借鉴，只能作为「自研 VT 覆盖不全」的对照事实。

## 渲染与 BiDi

`PaintRenderer` 内有 `BidiLayout`、`BidiRun`、`BidiSegment` 三个私有类：逐行做逻辑列到视觉列的映射，RTL 段整段交给 Android 平台文本整形（`drawText` 保证阿拉伯字母连接形态存活），LTR 段保持单元格网格；`TranscriptScreen` 提供视觉列与逻辑列的双向映射，供光标与选择使用。

代价是渲染路径从「按单元格绘制」变为「按方向段绘制」，并依赖平台整形能力。

与本项目对照：本项目渲染为 wgpu 单元格实例化（`native/src/render/cell_builder.rs` 的 `build_instances_from_cell_data`），`DESIGN.md` 明确「支持单码位整形与复杂文种叠加」「不做跨单元连字」。阿拉伯文字的上下文连接天然跨单元格，采纳等于改动渲染架构并违背既有声明；上游 Ghostty 亦无 BiDi 能力。判定为不立项。

## 上传到会话当前目录

终端版新增「上传文件或文件夹到终端当前目录」，实现要点：

- 目标目录来自内核而非屏幕或 shell 配置：会话以 `setsid()` 起于自己的 PTY，`/proc/<leader>/stat` 字段 8 是该 PTY 的前台进程组，取该组任一成员的 `/proc/<pid>/cwd` 即为当前工作目录（`SessionDirectory`、`ProcFiles`）。提示符处是 shell，前台是 `vim` 时是 `vim`。
- 只增不改：写入先落到目标目录内的 `.thothterm-upload-<random>` 暂存名，日志先落（`StagingJournal`），`fsync` 后一步改名；取消、失败、断线、退出与下次启动都清理暂存（`AndroidUploadFs`）。
- 字节精确：无转码、无换行转换、无名称规范化；文件权限跟随会话的 umask；从不执行、不 source、不解压。
- 名称校验逐段进行，不做归一化（`UploadNames`）。

借鉴判定：功能本身本项目未声明（`DESIGN.md` 无「上传到会话工作目录」条目），按「不实现未声明的功能」不立项；但「用 `/proc` 前台进程组解析会话真实工作目录」是可靠做法，建议登记为将来若立项该功能时的技术判据。

## 安全契约

`docs/security/THORNS.md` 是一份十条规则的安全架构契约，开篇即要求：破坏任一规则的改动须在同一提交内写明理由，并留下钉住新行为的测试。

与本项目相关的条目：

- 导出面最小化：每个发行版只导出启动器，其余组件 `exported="false"`，并由策略文件核对。
- 外部 Intent 数据不可信：extras、data URI、ClipData、路径、MIME、包名一律按网络输入对待。
- 外部数据绝不键入交互终端：需要影响会话时只走结构化通道（工作目录、argv、环境变量），绝不在 shell 里敲命令。
- 路径边界：逐段检查外部提供的路径、不归一化、不跟随既有链接、不以写方式打开既有名字、以 `renameat2(RENAME_NOREPLACE)` 发布。
- fail closed：检查失败即拒绝，不落回宽松路径；超时必须真正终止被超时的工作。
- 每次漏洞修复必须留下永久回归测试，且不得为让发布通过而削弱测试。

与本项目对照：本项目的文档提供器已有等价的路径防护（`android/app/src/main/java/terminal/emulator/DocumentMutations.kt` 的 `canonicalFile` 比对与链接处理），但本项目没有把「外部数据不得键入 PTY」这类正面安全不变量写成规范条目；`PROHIBITED.md` 只列禁止项。建议把上述四条不变量（外部数据不入 PTY、路径逐段校验、fail closed、修复必留回归测试）补进本项目规范，作为低成本借鉴。

## 导出面策略测试

`tests/security/exported_components.py` 从构建出的 APK 读取合并清单（`aapt2 dump xmltree`），按包名选择 `tests/security/policy/<package>.json` 精确比对导出组件清单；清单有增删改或缺少策略文件即失败。

与本项目对照：本项目导出面为启动 Activity 与文档提供器两个组件（`android/app/src/main/AndroidManifest.xml`），规则简单，可用同样思路钉住「不得新增导出组件」。但本项目脚本规范只用 Nushell（`STYLE.md`），若采纳应写成 Nushell 脚本或 Gradle 检查，不宜新增 Python 体系。

## 不可借鉴清单

| 上游做法 | 不借鉴原因 |
| --- | --- |
| 自研 VT 引擎与 Canvas 渲染管线 | 违反「不重复实现 Ghostty 已有功能」；能力面窄于本项目上游 |
| 内嵌 PRoot 与预装发行版（garden 系列） | `PROHIBITED.md` 终端节明禁「内嵌 proot」「内嵌 bootstrap，预装发行版」 |
| LAN 模式（WebSocket 远程终端与浏览器上传） | 本项目未声明网络终端；体量与安全面大 |
| 日志落盘与日志界面（`logging/LogFileStore`） | `DESIGN.md` 要求日志只在 logcat 可见，不写文件不保存 |
| Unix 套接字服务与命令服务（`services/UnixSocketServer`、`CommandService`） | 等同 termux-api 面，`PROHIBITED.md` 明禁 |
| 自定义主题与主题编辑器 | `PROHIBITED.md` 设置节明禁 |
| 快捷方式插件与 `RUN_SCRIPT` 外部集成 | `PROHIBITED.md` 明禁 termux-api／termux-exec；外部命令面未声明 |
| 修饰键栏 NAV/SYM 双页与可编辑布局 | `DESIGN.md` 要求布局与 Termux 保持一致；`PROHIBITED.md` 明禁修饰键栏布局编辑器 |
| 运行时权限申请流程（`Permissions`） | `DESIGN.md` 要求声明文件权限、不提示 |
| `tests/controlSequences`、`tests/wideChars` 语料 | 全仓构建脚本零引用，属失效语料；期望值为人眼散文 |
| Golden 回滚清单与设备验收证据（`docs/evidence/golden/`） | 属发布流程资产；本项目已有 baselineprofile 与宏基准断言约束，无对应缺口 |

## 借鉴结论

| 项 | 成本 | 落点 | 判定 |
| --- | --- | --- | --- |
| 安全不变量四条（外部数据不入 PTY、路径逐段校验、fail closed、修复必留回归测试） | 低 | `docs/specification/` 与相关 openspec 规范 | 建议立项 |
| 导出面不得扩张的钉住检查 | 低 | 随导出面规则所在规范 | 可选，需先定检查体系（脚本只用 Nushell） |
| 会话真实工作目录解析（`/proc` 前台进程组） | 中 | 登记为将来「上传到会话目录」立项时的技术判据 | 暂不立项 |
| 上传文件或文件夹到会话工作目录 | 中高 | 需先在 `DESIGN.md` 声明功能 | 待裁定 |
| BiDi 与阿拉伯文整形 | 高 | 无 | 不立项（架构冲突且违背 `DESIGN.md` 声明） |

## 复核口径

- 克隆：`git clone --depth 1 https://github.com/Lord1Egypt/AndroidThothTerm.git`，取样于 2026-11-02 主线。
- 协议面：`emulatorview/src/main/java/jackpal/androidterm/emulatorview/TerminalEmulator.java` 的 `doOsc`、`sendDeviceAttributes`、`doSetMode`；鼠标格式见 `MouseReport`。
- 渲染面：`PaintRenderer` 的 `BidiLayout`、`drawRtlText`、`bidiCursorX`；列映射见 `TranscriptScreen`。
- 上传面：`term/src/main/java/com/thothterm/upload/` 的 `SessionDirectory`、`ProcFiles`、`UploadNames`、`StagingJournal`、`AndroidUploadFs`。
- 安全面：`docs/security/THORNS.md`；策略测试 `tests/security/exported_components.py` 与 `tests/security/policy/`。
- 语料判定：`emulatorview/src/test/` 5 文件 34 例；`tests/` 131 文件 36 目录，全仓构建脚本零引用。
