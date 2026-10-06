# 任务：选择菜单锚点不得落在视口之外

## 1. 根因与修复

- [x] 定位：读 `TerminalSurface.menuAnchor` 的两处落点判据，确认各自只判单侧边界，
      选区整体在视口之外时返回视口外锚点（proposal 的几何推导）。
- [x] 修：两处落点共用 `fits(top)`（`top >= viewport.top && top + menuHeight <= viewport.bottom`）。
- [x] 补单测三条：选区整体在视口之上 → null；选区整体在视口之下 → null；
      选区下缘越过视口底但上方落点可用 → 仍锚在上方（既有策略不变）。
- [x] 既有五条 `menuAnchor` 用例全过（判据只收紧「另一侧」，不改变既有落点）。

## 2. 取证能力（不改动保护文件）

- [x] `SelectionEspressoTest` 接 `TerminalLogcatRule`：菜单缺席时把应用日志尾部附在
      失败信息上，`showSelectionMenu` 的四条缺席分支（未附着 / 无界 / 无处安放 /
      弹窗添加失败）各有一行日志，下次同类缺席即可判别分支。
- [x] `BehaviorInstrumentedTest` 接同一规则：该类在 CI 上以「节点查不到」判红且
      `connected-failure-message` 为空，无日志时无法区分应用没起 / 被系统弹窗盖住 /
      功能真的缺席。
- [x] `CursorPixelAcceptanceTest` 补 `@After cleanUpTerminalState()`：它是唯一没有收尾
      的像素类，选区、滚动偏移与回滚跨类留存会给后继像素用例留下错误前提。

## 3. 验证

- [x] 本地 CI 同款几何（`wm size 320x640` + `wm density 160`）单跑
      `SelectionEspressoTest` 5/5、`CursorPixelAcceptanceTest` 1/1 通过（改动前即通过，
      作为「未引入回归」的基线）。
- [x] `testDebugUnitTest`（含新增三条 `menuAnchor` 用例）全绿。
- [x] `detekt` + `spotlessCheck` 门禁通过。
- [ ] CI 复跑确认（build 工作流）——需三连绿为准，见 `audit-backlog-open-items` §33。