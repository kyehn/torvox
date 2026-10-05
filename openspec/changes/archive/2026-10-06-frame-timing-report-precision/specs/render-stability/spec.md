## ADDED Requirements

### Requirement: 帧与循环计时报告保留亚毫秒精度

帧窗口与循环窗口的汇总行 MUST 由纳秒直接换算为亚毫秒精度输出，MUST NOT 以整毫秒
截断的中间值参与换算：真机帧时长集中在 0–2ms，整除后健康状态一律报出
`avg=0ms p95=0ms`，窗口无法区分 0.1ms 与 0.9ms 两种健康基线，`frameTimingTrend`
的「退化到基线 3 倍」与 `FRAME_TIME_WARN_*` 绝对阈值告警在该分辨率下拿不到可读真值。
循环窗口的 `fps` MUST 由 `averageNanos` 直接换算；经截断均值换算的 `fps` 会把同一台
健康设备报成 55/58/62/66/71/76fps 六个值，全部是舍入产物而不含帧率信息。

#### Scenario: 健康窗口报出可读真值

- **WHEN** 真机静置一分钟，帧窗口内每帧渲染均在 2ms 以内
- **THEN** 汇总行的 `avg` 与 `p95` 为非零亚毫秒值（如 `avg=0.42ms p95=0.91ms`），
  而非 `avg=0ms p95=0ms`

#### Scenario: fps 不随舍入跳变

- **WHEN** 同一台设备循环周期稳定在 16ms 上下
- **THEN** 各窗口的 `fps` 取值一致，不出现 55/58/62/66 之间由整除产生的跳变
