# 修饰键栏触摸取消不触发

## 上下文

- `ModifierBar.kt:1177` `ExtraKeyButton` 手势处理三分支把 `!pressed` 当抬手触发；实测 `input motionevent DOWN`+`CANCEL` 会触发 `Key_CTRL`，`DOWN`+`MOVE`+`UP`（超 slop）不触发。
- Compose UI 1.12.1 `PointerEvent.motionEvent` 暴露原始事件，`actionMasked == ACTION_CANCEL` 可与抬手区分（View 系统同语义）。
- 键栏几何已有 `modifier_bar_bottom_position_above_gesture_zone` 测试守住（导航手势区之上）。

## 任务

1. [x] 仪器测试先红：View 派发 `DOWN`+`CANCEL` 断言 `Key_CTRL` 不触发；`DOWN`+`UP` 对照断言触发。
2. [x] 三分支识别取消并吞掉（含长按副动作）。
3. [x] 验证：`ModifierBarTest` 全绿、spotless/detekt/单元测试、release APK 设备矩阵（注入取消不触发、点按触发、真实上滑过键栏不触发、底部边缘上滑回桌面正常）。
4. [ ] `openspec validate`、提交推送，验证后同步 `openspec/specs` 并归档变更。
