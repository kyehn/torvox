# 任务

## 一、取证与判定

- [x] 核对 ZTE 外屏 logcat（`logcat_2026-11-03_17-29-27.txt` 等）：`navigationBars`
      底部 inset 为 0，而键栏下仍有背景色空带——空带来自 `safeGestures` 附加内缩。
- [x] 确认旧判定的前提失效：`ExtraKeyButton` 已实现 `ACTION_CANCEL` 合成
      （`processCancel` + `isGestureCancelled`）与按下反馈延迟
      （`PRESS_FEEDBACK_DELAY_MS`），取消不再产生输出或残留动画；
      `ModifierBarTest` 的 Cancel 用例已钉住。
- [x] 确认上游 termux `ExtraKeysView` 不为手势区留白（同取舍）。

## 二、实现

- [x] `TerminalScreen.kt`：移除内容盒的 `.padding(bottom = gestureSafeBottom)` 与
      `gestureSafeBottom`/`safeGestures` 计算，底部只保留 `navigationBarsPadding`；
      就地注释改判依据。
- [x] 保留 `density`（`imeLift` 仍引用），清理失效 import。

## 三、回归钉

- [x] 新增 `ModifierBarBottomInsetInstrumentedTest`：内容盒底边 = 导航栏上沿、
      终端 Surface 与键栏覆盖层均贴内容盒底边（结构关系不随导航模式漂移）。

## 四、门禁

- [x] `:app:compileDebugKotlin` / `:app:compileDebugAndroidTestKotlin` 通过
- [x] `spotlessApply` 后无残余 diff
- [x] `openspec validate --all --strict` 通过
- [x] CI check + build 全绿（提交后由 CI 确认）
