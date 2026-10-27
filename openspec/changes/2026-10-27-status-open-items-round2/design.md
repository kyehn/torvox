# Design

## Context

### 修饰键按下反馈

`ExtraKeyButton` 在 `awaitFirstDown()` 后立刻把 `isPressed` 置真，缩放与底色由
`animateFloatAsState` / `animateColorAsState` 驱动。全面屏手势的 home 上滑从屏幕
底部起滑：系统先派发 `ACTION_DOWN`，判定为手势后向应用补发 `ACTION_CANCEL`。
逻辑层已经在超 `touchSlop` 或取消时吞掉按键输出，所以「仅动画」——但动画在取消
之前已经启动，视觉上就是误触。

### 搜索居中

`searchViewportTop` 用「可见带正中反解视口行」得到目标顶行，再 `scrollToRow`。
命中行在屏幕上的纵坐标是 `视口行 × 单元高 − 内容上移量`。命中行靠近内容顶部时，
目标顶行被 `coerceAtLeast(0)` 截到 0，滚动已到尽头；上移量仍把它推到负坐标，
屏幕上自然看不到。

## Goals / Non-Goals

- Goal：上滑取消时按下动画不曾启动；正常按住仍有反馈。
- Goal：命中行在键盘弹出时始终落在可见带内。
- Non-Goal：不改手势判定、按键输出与 `touchSlop` 语义。
- Non-Goal：不改搜索匹配、高亮与导航的上游来源。

## Decisions

### 决策一：延迟放在动画 spec，而不是手势状态

手势层延迟置 `isPressed` 会让轻点没有按下反馈（`UP` 可能早于首个 `MOVE`）。
把 `delayMillis` 放进 `tween`：`isPressed` 仍即时记录，动画在 40ms 后才开始；
取消发生在窗口内时目标值已回到 1，动画从未产生位移。抬手方向不延迟，反馈不拖沓。

- 备选「首个 `MOVE` 后才置按下」：轻点与静止按住都没有反馈，且三处分支各写一份。

### 决策二：搜索居中先滚动、再按实际视口判断、必要时归零上移量

先执行一次居中滚动（可能已足够）；随后用 `matchRow − 当前视口顶行` 求命中在视口
内的行号，若 `行号 × 单元高 < 上移量`，说明命中在屏幕上边界之外。此时把
`imeShiftRevealPx` 拉到容量（`shift` 归零），重新居中一次。

只在上移量真正大于命中纵坐标时才动显出量，避免每次导航都重置用户的显现状态。

## Risks / Trade-offs

- [延迟窗口太短，慢速上滑仍闪] → 40ms 覆盖系统从 `DOWN` 到认领的常规窗口；
  慢速上滑的位移本身也会先超 `touchSlop` 取消，动画同样不会启动。
- [归零上移量让光标被键盘遮挡] → 只在搜索导航且命中本不可见时发生；搜索场景
  以「看见命中」优先，关闭搜索后手势滚动会重新调整显出量。

## Migration Plan

无数据迁移。

## Open Questions

无。
