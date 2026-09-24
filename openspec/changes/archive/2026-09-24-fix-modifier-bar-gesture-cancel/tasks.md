# 修饰键栏触摸取消不触发

## 上下文

- `ModifierBar.kt:1177` `ExtraKeyButton` 手势处理三分支把 `!pressed` 当抬手触发；实测 `input motionevent DOWN`+`CANCEL` 会触发 `Key_CTRL`，`DOWN`+`MOVE`+`UP`（超 slop）不触发。
- Compose UI 1.12.1：`MotionEventAdapter` 对 action 3/4 直接返回 null，取消由 `processCancel` 合成**无 MotionEvent** 的全释放事件送达（红灯推翻了首版 `actionMasked == ACTION_CANCEL` 判定）——故按“见过原始事件 + 本事件无原始事件且全释放”识别。
- 键栏几何已有 `modifier_bar_bottom_position_above_gesture_zone` 测试守住（导航手势区之上）。

## 任务

1. [x] 仪器测试先红：View 派发 `DOWN`+`CANCEL` 断言 `Key_CTRL` 不触发；`DOWN`+`UP` 对照断言触发。
2. [x] 三分支识别取消并吞掉（含长按副动作）。
3. [x] 验证：`ModifierBarTest` 全绿、spotless/detekt/单元测试、release APK 设备矩阵（注入取消不触发、点按触发、真实上滑过键栏不触发、底部边缘上滑回桌面正常）。
4. [x] `openspec validate`、提交推送，验证后同步 `openspec/specs` 并归档变更。
