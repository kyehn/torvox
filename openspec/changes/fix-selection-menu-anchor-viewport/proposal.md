# 选择菜单锚点不得落在视口之外

## Why

`menuAnchor` 的两处落点各只判**一侧**边界：上方只判 `aboveTop >= viewport.top`，
下方只判 `belowTop + menuHeight <= viewport.bottom`。选区整体滚出视口时（翻阅回滚后
选中区仍留在原处，或视口被 IME 上移），另一侧的判据对远离视口的 y 恒真，于是函数
返回一个**视口外**的锚点：

```text
选区 rows 0..2、视口顶行 943（残留滚动偏移）→ belowTop = (3+943) × 行高 + 手柄高 ≈ 19900
→ belowTop + 44 ≤ 视口底 ⇒ 成立 ⇒ 返回 y ≈ 19900
```

`PopupWindow.showAtLocation` 于是把菜单窗口添加到屏幕之外：不抛异常、不记日志，
用户与 UiAutomator 都看不到菜单，而 `selectAllShowsSelectionMenu` 只报一句
「Selection menu must appear after Select All」——判红原因与被测行为无关。

这条路径已由本次 CI run 的失败签名暴露（见 tasks），本变更修的是「锚点必须整体
落在视口内」这一几何不变量；两处判据因此收敛成同一条，纯函数也更短。

## What Changes

- `menuAnchor` 的两处落点共用「整体落在视口内」判据，越界即视为无处可放。
- `text-selection` spec 增补一条 Requirement：菜单 MUST 整体位于视口内，
  选区滚出视口时 MUST 隐藏。
- 取证能力（不改被测行为、不改保护文件）：`TerminalLogcatRule` 的标签清单改为
  匹配 logcat 里的**实际标签**，取回此前被过滤掉的渲染失效锚点；`SelectionEspressoTest`
  与 `BehaviorInstrumentedTest` 接入该规则；终端页面就绪门槛收口「系统无响应对话框」
  关闭；`diag.CursorPixelAcceptanceTest` 补用例收尾。详见 `tasks.md` 第 2 节。

## Non-goals

- 不改「上方优先 / 贴顶翻转到下方 / 贴右钳制」的既有锚定策略。
- 不改菜单内容、样式与一次点击即生效的契约。
- 不改任何断言阈值。

## Capabilities

### Modified Capabilities

- `text-selection`：菜单锚定与样式