# Design

## 上下文

系统手势（底部上滑回桌面/多任务、侧缘返回）由框架在窗口层拦截：起滑点落在系统
手势区内时，框架认领该手势，向应用补发 `ACTION_CANCEL`，此后的触摸事件不再送达
应用。Compose 的 `MotionEventAdapter` 会丢弃这个 `ACTION_CANCEL`，改为合成一个
「全部抬起」的指针事件——但前提是应用仍在接收指针事件流。

## 决策

### 决策一：键栏底边避开系统手势区，而不是只避开导航栏

手势区不是导航栏 inset 的子集：全面屏手势导航下导航栏只占很窄的一条，系统的
强制手势区（`mandatorySystemGestures`）与普通手势区（`systemGestures`）可能比它
更高。只按导航栏留白，键栏底边仍可能落在手势区里。

底部留白取**导航栏 inset 与 `safeGestures`（系统手势区 ∪ 强制手势区）的较大者**。
留白加在承载终端与键栏的同一个 Box 上：终端 Surface 随之变矮，网格按新高度重算，
键栏与网格的预留口径保持一致（网格本来就为键栏预留 `MODIFIER_BAR_HEIGHT_DP`）。

代价：全面屏手势导航设备上终端少几行可用高度。这与 Termux 把附加键栏放在手势区
之上的做法一致，是「不把可交互控件放在系统手势区内」的代价。

### 决策二：只有按下方向插值，抬起与取消瞬时复位

既有规范要求取消时「不产生动画位移」。实现上把插值规格按方向分开：

- 按下：`tween(90ms, delay=40ms)`——延迟覆盖系统认领窗口后才启动反馈。
- 抬起/取消：`snap()`——瞬时回到未按下外观。

`animateFloatAsState` 的规格随 `isPressed` 改变即重启目标动画，故方向切换不需要
额外的状态机。抬起时瞬时复位不影响短促感：按下方向的插值已给出反馈。

底色同理：按下走 `tween`，抬起/取消走 `snap`。

### 被否决的方案

- **把取消延迟加大到覆盖任意起滑速度**：系统认领时机取决于用户手势速度，没有
  固定上界；延迟过大则正常点按的反馈变迟钝。
- **在按下反馈上做超时看门狗**：合法长按（按住 CTRL 锁键）期间本就没有新事件，
  看门狗会把仍在按住的手指误判成被认领，长按语义被破坏。
- **恢复 `ACTION_CANCEL` 的原始事件**：取消事件由框架在窗口层消费，应用侧无从
  取得；Compose 的合成释放事件只在指针事件流仍在时送达。

## 验证

- `ModifierBarTest#modifier_bar_bottom_position_above_gesture_zone`：键栏底边必须
  高于 `displayHeight − max(navigationBars, systemGestures, mandatorySystemGestures)`。
- `ModifierBarTest#modifier_bar_touch_cancel_does_not_trigger_key`：被认领的手势
  不触发按键（既有用例）。
- `ModifierBarTest#modifier_bar_view_tap_triggers_key`：正常点按仍触发（既有对照）。
