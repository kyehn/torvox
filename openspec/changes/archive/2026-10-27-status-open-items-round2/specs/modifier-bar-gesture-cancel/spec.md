# modifier-bar-gesture-cancel Specification

## ADDED Requirements

### Requirement: 按下反馈不得早于系统手势取消窗口

按键的按下反馈（缩放、底色）MUST NOT 在 `ACTION_DOWN` 立即启动。系统底部上滑先
派发 `ACTION_DOWN`、再把同一手势认领走并补发 `ACTION_CANCEL`；反馈若在取消之前
已经画出一帧，用户看到的是「上滑触发了按键动画」，尽管没有任何按键输出。

反馈的启动 MUST 延迟一段覆盖该取消窗口的时间；取消或滑出 `touchSlop` 时 MUST
回到未按下状态且不产生动画位移。抬手复位 MUST NOT 延迟，反馈仍须短促。

#### Scenario: 底部上滑取消

- **WHEN** 用户在按键上按下后，触摸被系统底部上滑认领并收到 `ACTION_CANCEL`
- **THEN** 按下反馈从未启动，屏幕上看不到缩放或底色变化

#### Scenario: 正常按住

- **WHEN** 用户按住按键超过延迟窗口且未滑出 `touchSlop`
- **THEN** 按下反馈正常出现，松手后立即复位
