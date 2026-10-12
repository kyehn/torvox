# 任务：超长查询不截断

## 1. 测试（TDD 红→绿）

- [x] 在 `TerminalScreen` 所在模块的现有 JVM 单测体系内加一条：超长查询
      （129 字符）经 `onQueryChange` 后不调用 `bridge.searchSetNeedle`，
      且 `resultCount == 0`。先红（当前 `take` 截断会送原生）再绿。

  落点调整：`onQueryChange` 内联在 Compose 中，直接单测成本高，改为抽纯函数
  `coerceSearchQuery`（`ui/SearchQuery.kt`）+ 4 条 JVM 单测
  （`SearchQueryCoerceTest`）：短串/上限/超长返回 null/空串。实现改错后测试转红
  已验证判别力，恢复后全绿。

## 2. 实现

- [x] `onQueryChange` 去掉 `take`：超长时直接 `resultCount = 0` 并取消
      待执行的搜索，不送原生；查询框保留原文。
- [x] `specs/text-search/spec.md` 写 MODIFIED delta（UI 侧不截断一条）。

## 3. 验证

- [x] `openspec change validate 2026-10-12-fix-search-overlong-no-truncate` 通过。
- [x] 归档后核对目标规范含有新条目。
- [x] 新增单测 + 既有搜索单测全绿；`detekt` 通过。
