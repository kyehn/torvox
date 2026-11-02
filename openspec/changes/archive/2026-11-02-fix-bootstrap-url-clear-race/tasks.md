# 任务

## 一、缺陷核定

- [x] 确认缺陷在**时序**而非配置：待写值的到期时刻（上次编辑后 300ms）与停用窗口
      的长度（删除目录并重建 prefs 的耗时）互不相干。
- [x] 确认既有回归钉覆盖不到该时序：它先睡满 700ms（防抖在停用期间到期）才重新武装，
      测的是「停用窗口覆盖到期时刻」这一种时序。
- [x] 定位根因：作废「清除之前的待写值」这件事只由停用标志承担，而停用标志随清除
      结束即失效；写入重新武装后，那个清除之前编辑的值到期时被照常写盘。
- [x] 复现：新增用例在旧实现上判红（`ComparisonFailure`：期望空串，实际写回了
      清除前的 URL）。

## 二、实现

- [x] `SettingsRepository`：防抖流元素由裸字符串改为 `BootstrapUrlEdit`（值 + 清除轮次）。
- [x] `dropPendingBootstrapUrlEdits()`：递增清除轮次，使清除之前的待写值作废。
- [x] 落盘判据加轮次比对：轮次与当前不一致的编辑一律不写盘。
- [x] `recordBootstrapUrlEdit()` 取当前轮次；`latestBootstrapUrlEdit()` 取元素的值。

## 三、回归钉

- [x] `a dropped edit never lands when writes are rearmed before the debounce expires`：
      重新武装早于防抖到期时，清除前的 URL MUST NOT 落盘。
- [x] 既有 `dropped debounce edit never lands, rearmed edit does` 仍通过（停用窗口
      覆盖到期时刻这一种时序未回退）。
- [x] 既有 `slow write keeps the latest bootstrap url edit` 等用例不受影响。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.settings.SettingsRepositoryTest"`
      → BUILD SUCCESSFUL（15 tests, 0 failed）。
- [x] `:app:detekt` → 0 告警。
- [x] `:app:spotlessCheck` → 通过。
- [x] `openspec validate <change-id> --strict` → 通过。
