# 帧/循环计时报告保留亚毫秒精度

## Why

真机（ZTE P720S20 / Android 13）logcat 的计时窗口全部退化为整毫秒截断的量化假象：

```text
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=2ms scrollback=0 rows
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=83ms ≈55fps
session 1 loop timing window (60 frames): avg=17ms p95=17ms max=92ms ≈58fps
session 1 loop timing window (60 frames): avg=15ms p95=17ms max=100ms ≈66fps
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=101ms ≈62fps
```

- `avg`/`p95`/`max` 由 `nanos / 1_000_000L` 整除，真机帧时长本就集中在 0–2ms，
  于是健康状态一律显示 `avg=0ms p95=0ms`——窗口报出 0 与 1ms 两种健康基线，
  而 `frameTimingTrend` 的「退化到基线 3 倍」与 `FRAME_TIME_WARN_*` 绝对阈值告警
  在这个分辨率下永远拿不到可读的真值。
- `≈fps` 由**截断后**的整毫秒均值再取整（`1_000L / 16` = 62、`/17` = 58、`/15` = 66），
  于是同一台设备在健康区间输出 55/58/62/66/71/76fps 六个值，全部是舍入产物，
  不含任何帧率信息。

结论是这份日志无法区分「渲染健康」与「渲染退化 2 倍」，即 DESIGN.md:79 要求的
「日志可见以便调试」在最需要它的场合失效。

## What Changes

- 帧窗口报告的 `avg`/`p95`/`max` 与基线毫秒值改由纳秒直接换算为亚毫秒精度输出。
- 循环窗口的 `fps` 改由 `averageNanos` 直接换算，不再经整毫秒截断的中间值。

## Non-goals

- 不改窗口长度、采样点与 `FrameTimingStats` 的存储结构（仍是纳秒 `LongArray`）。
- 不改 `SLOW_FRAME` 的阈值与判据。
