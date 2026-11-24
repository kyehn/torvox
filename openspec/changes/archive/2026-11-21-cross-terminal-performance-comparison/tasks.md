# 任务

## 一、一手调研

- [x] 核实两个对照对象的终端内容渲染路径（`termux/termux-app` v0.119.0-beta.3 的
      `TerminalView`／`TerminalRenderer`；`sylirre/ghostty-android-terminal` 的 `TerminalView`
      与 `CMakeLists.txt` 的预编译 `libghostty-vt`）
- [x] 核实两个对照对象的基准资产与官方性能数字（仓库树枚举：无 benchmark 模块；无可核实的
      性能数字）
- [x] 从 AOSP 源码取 `dumpsys SurfaceFlinger --latency` / `--latency-clear` 的确切语义
      （`SurfaceFlinger::dumpStats`、`SurfaceFlinger::clearStats`、`Layer::dumpFrameStats`）
- [x] 从 AOSP 源码取 `dumpsys gfxinfo` 的覆盖范围（`ThreadedRenderer.handleDumpGfxInfo`、
      `dumpArgsToFlags`）
- [x] 取 Perfetto 数据源约束（`linux.process_stats` 轮询下限与 `mem.*` 计数器、
      `android.surfaceflinger.frametimeline` 的 jank 类型、`android.log` 仅 userdebug、
      ATrace 每事件 1–10 微秒、`gpu.counters` 设备特定）
- [x] 取宏基准与退出归因的官方口径（`profileable` 的强制范围仅 API 29–30、模拟器与低电量报错、
      `FrameTimingMetric`／`StartupTimingMetric` 指标定义、`ApplicationExitInfo` reason 常量与
      `REASON_OTHER` 口径）
- [x] 实测设备侧采集通道（`/system/bin/perfetto`、`dumpsys thermalservice`、
      `cmd display set-user-preferred-display-mode`、
      `cmd power set-fixed-performance-mode-enabled`、`dumpsys battery`、
      `dumpsys activity exit-info`、`dumpsys SurfaceFlinger --list`、
      `dumpsys dropbox --print`、`adb emu` 控制台）

## 二、研究报告

- [x] 写 `docs/research/terminal-performance-comparison-2026.md`：摘要、研究方法与坐标、
      对照对象一手事实、可比性边界、跨应用统一帧面、采集面清单、12 类场景矩阵、环境固定化、
      失败归因规则、极端场景上限、无人工干预编排、数据落地格式、不采纳手段、与既有资产的关系
- [x] 报告内只写本项目事实与上游坐标，不写提交号、不引用对照对象自述性能数字

## 三、规范

- [x] `performance-measurement`：新增「跨应用性能对比的对照面与统一帧面」「全场景矩阵与
      环境固定化」「失败归因与采集失败」「极端场景的资源上限」四条要求
- [x] `performance-measurement`：扩充「不采纳的测量手段须记录理由」，逐条写明七项不采纳理由
- [x] `research-ledger`：补以报告文件名为键的索引行、逐份处置补齐行，在库计数 3 → 4
      资产（渲染路径、基准资产、官方性能数字）

## 四、验证

- [x] `openspec validate --all --strict`：通过
- [x] `semgrep scan --config .semgrep/rules.yml`：0 findings
- [x] `markdownlint-cli2 "**/*.md"`：0 issues
