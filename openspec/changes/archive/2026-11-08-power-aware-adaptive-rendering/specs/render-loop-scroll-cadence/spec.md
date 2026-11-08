## ADDED Requirements

### Requirement: 帧时长窗口按真实显示周期结算

帧时长/阶段耗时的统计窗口 MUST 使用来自 `doFrame` 的实际
`frameTimeNanos` 差值结算，MUST NOT 以 60Hz 的常量 16.7ms 计算
FPS；FPS MUST 按窗口内实际帧间隔计算。没有产生新帧时，
该窗口不产出呈现帧或空帧。显示周期 MUST 由实测帧间隔的窗口统计
（平均/p95/最大）报出，MUST NOT 把常量当成实测帧率。

#### Scenario: 实际 vsync 周期结算

- **WHEN** 帧回调报告实际显示周期为 `t`
- **THEN** 该周期进入帧时长窗口，FPS 按实际帧间隔计算，
  不以 60Hz 固定值代替

#### Scenario: 没有帧回调时不造空帧

- **WHEN** 空闲期没有显示帧回调
- **THEN** 帧时长窗口不结算、不呈现空帧，也不以定时器
  伪造帧率
