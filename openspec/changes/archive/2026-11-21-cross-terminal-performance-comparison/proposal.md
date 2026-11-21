# Proposal

## Why

本仓已有两条性能路径：CI 内的微基准（`native/benches`）与模拟器上的 Android 宏基准
（`android/benchmark`）。两者都只覆盖本仓，且都停在「本应用自己和自己比」的层面。跨终端
性能对比目前没有可执行口径，原因有三：

1. **端到端帧率不具跨应用可比性。** 两个现实对照对象（`termux/termux-app`
   v0.119.0-beta.3、`sylirre/ghostty-android-terminal` main）的终端内容都经 HWUI `Canvas`
   绘制（两者的 `TerminalView` 均继承 `View`），而本项目用 wgpu 直绘 SurfaceView。同一个
   「60 fps」在两侧量的不是同一件事，而两者的仓内均无 benchmark 模块，也未发布过可核实的
   官方性能数字——对比必须自建 harness，且不能引用任何一方的自述性能。
2. **缺少跨应用统一的帧面。** `dumpsys gfxinfo` 的入口是
   `ThreadedRenderer.handleDumpGfxInfo`（`dumpGlobalProfileInfo` 加
   `WindowManagerGlobal.dumpGfxInfo`），只统计经 ViewRootImpl/RenderThread 绘制的帧；本项目
   终端内容由原生线程直绘 SurfaceView，根本不在其中。系统侧的图层时序
   （`dumpsys SurfaceFlinger --latency <layer>`）才是唯一的统一面：AOSP 的
   `SurfaceFlinger::dumpStats` 先写 `getPacesetterVsyncPeriod().ns()`，再对同名图层调用
   `Layer::dumpFrameStats` 输出逐帧 `desired/actual/ready` 纳秒三列，
   `SurfaceFlinger::clearStats` 对应 `--latency-clear`。
3. **没有全场景与归因口径。** 现有宏基准覆盖启动、滚动与三类界面动画，缺持续输出、突发
   输出、宽字符行、输入洪水、前后台切换与资源极端；失败时也没有「这是本应用的问题还是系统
   的问题」的判定规则——`ApplicationExitInfo` 的 reason、FrameTimeline 的 jank 类型、logcat
   与 dropbox 四源都在设备侧可得，却无人按统一口径取用。

极端场景还缺资源上限：现有场景一旦卡住，只能靠外层超时兜底，没有「超限即终止并记录」的
约定，也没有「不为失败场景继续消耗设备资源」的纪律。

## What Changes

- 新增「跨应用性能对比的对照面与统一帧面」要求：统一帧面取系统侧图层时序；`gfxinfo` 帧
  统计只用于 HWUI 界面；对照对象须先核实基准资产与官方性能数字，无则自建 harness。
- 新增「全场景矩阵与环境固定化」要求：12 类场景各有固定负载、指标、采集面、判据与硬上限；
  同一场景给各应用完全相同的字节流与网格尺寸；环境漂移必须记录。
- 新增「失败归因与采集失败」要求：四类归因（应用／系统／人为与编排／采集失败），
  `REASON_OTHER` 的官方口径，证据不足记「未定」。
- 新增「极端场景的资源上限」要求：wall-clock 上限、输出字节上限、采样间隔下限、制品配额
  与单被测应用串行。
- 扩充「不采纳的测量手段须记录理由」：宏基准指标作跨应用判据、`gfxinfo` 作终端内容帧判据、
  Perfetto `android.log`、跨设备 `gpu.counters`、跨设备 `PowerMetric`、`screenrecord` 作判据、
  引用对照应用自述性能数字，逐条写明理由。
- 新增研究报告 `docs/research/terminal-performance-comparison-2026.md`，并在
  `research-ledger` 登记索引行与逐份处置行、在 `upstream-alignment` 登记两个对照对象的性能
  对比资产事实。

## 不做什么

- 不改 `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts` 等受保护文件。
- 不在 `scripts/` 下新增跨设备多应用编排脚本：该目录属受保护目录，须维护者明确同意后实施；
  本次只登记落点建议（现有可复用落点为 `android/benchmark` 仪器测试模块与
  `scripts/test-emulator.nu`）。
- 不改动既有规约口径：模拟器只作数量级、帧级数据面按绘制主体选源、采集为空即判红、设备侧
  命令不含 shell 语法、不得用固定等待替代就绪信号等一律保持。
- 不引入对照应用的安装包到仓库（外部下载，不入库）。

## Impact

- Affected specs: `performance-measurement`（新增四条要求，扩充一条不采纳要求）、
  `research-ledger`（索引行、逐份处置行、在库计数）、`upstream-alignment`（性能对比资产
  登记表）。
- Affected code: 无（本次只出方法学与规范，不改实现）。
- 行为变化：无。
- 验证方式：`openspec validate --all --strict`、`semgrep scan --config .semgrep/rules.yml`、
  `markdownlint-cli2`。
