# 提案

## 为什么要做

用户给出 25 个终端类 Android / 开源仓库，要求逐个下载研究是否有可参考点、确保检查完整、
写入文档。

本规范的「已复查结论台账」此前只覆盖 9 项桌面/C 侧上游资产，Android 侧终端项目完全缺席。
而本规范同时写着两条对 Android 终端直接生效的条款——「同类安卓终端的测试做法须成对比较」
与「同类安卓终端的既有断言按覆盖面缺口而非数量采纳」——却没有台账支撑它们，
等于条款悬空。本次补齐这一段。

## 改什么

1. **更正既有台账的一行**：termux `terminal-emulator` JUnit 原被整包判为门槛 1，
   依据只举 `TerminalTestCase` 的 `MockTerminalOutput`。实测该目录内手写屏幕文本断言 271 处、
   `MockTerminalOutput` 仅 3 处、外部规范出处 16 处——整包判定属未经验证的概括，
   按断言形态分流改写。结论（不采纳）不变，依据变为可复核。
2. **新增 25 项台账**，逐条记录实质、门槛判定与可复核依据，其中：

   - **需改造后采纳（4 项）**：`ghostty-android-terminal` 的 `EmulatorVtTest`、
     `termux-kotlin-app` 的键位编码与鼠标边界断言、`charon` 的外部标准期望值子集、
     `AndroidThothTerm` 的 VT 单测作判据来源。
   - **不可采纳（21 项）**：其中 6 项属「无资产」（`jolz`、`TerminalOS`、`PufiX`、
     `termux-clone`、`hydra-shell-android`、`ReTerminal`），7 项是 termux-app 的 fork 或旧快照
     （价值等同官方或更弱），其余为自研 VT 或 UI 壳，与本仓「Ghostty 单一来源」对立。
   - **新增失效语料记录**：`AndroidThothTerm` 的 `tests/` 共 122 个文件、11 个子目录、
     864K，全仓（含 CI 与构建脚本）对它的引用数为 0。

3. **登记与 DESIGN 直接冲突、不得反向采纳的做法**，避免后续复查时被当成可抄对象：
   双击/三击选词、OSC 8 链接 scheme 白名单、`onScreenUpdated` 无条件清零滚动位置、
   渲染热路径内改写状态、ED 顺手重置光标。

## 影响

只改文档。`openspec/specs/upstream-alignment/spec.md` 新增一个 Requirement，
既有 Requirement 的措辞不变。不引入依赖、不 vendor 代码或测试数据、不改 `docs/specification/`。
