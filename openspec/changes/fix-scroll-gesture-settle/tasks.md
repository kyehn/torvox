# 滚动手势收尾

## 上下文

- `TerminalSurface.kt:2862-2904` UP/CANCEL 分支不清滚动状态；`finishFlingAnimation:1453-1459` 与点抬手 `onSingleTapUp:1936-1947` 有完整收尾模板。
- maki 诊断 R2：慢速抬指必中泄漏。

## 任务

1. UP/CANCEL 分支内收尾滚动（`isScrolling=false`、余量归零推送、`onScrollingStateChanged(false)`），不重置行偏移。
2. 针对性单测：收尾语义可测部分落单测。
3. 真机验证：慢拖抬指后余量归零、新输出回底。
