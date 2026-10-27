# Proposal

## Why

`docs/STATUS.md` 的四条未解决问题里，两条的根因已经落到具体代码，但规范没有把
「按下反馈的时机」与「命中行被内容上移推出屏幕」这两条边界写死：

1. 修饰键的按下反馈在 `ACTION_DOWN` 就启动。全面屏底部上滑会先把触摸交给应用、
   再把同一个手势认领走并补发 `ACTION_CANCEL`；取消前的几帧里缩放与底色已经画了
   出来，用户看到「上滑触发了按键动画」（没有按键输出，只有动画）。
2. 搜索居中只算滚动。输入法弹出时内容为跟随键盘整体上移，靠近顶部的命中行即使
   把视口滚到最顶也仍在屏幕上边界之外；关闭输入法后上移量归零，同一条命中又
   「莫名可见」。

两条都不是新功能，而是既有规范在边界上的缺口。

## What Changes

- 修饰键按下反馈延迟到系统手势取消窗口之后启动；抬手复位不延迟，反馈仍短促。
- 搜索居中在命中行被上移量推出屏幕时，先把位移显出量拉满（上移量归零），再按新的
  可见带重算目标行并滚动。
- `docs/STATUS.md` 移除这两条已解决项。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `modifier-bar-gesture-cancel`：明确按下反馈不得在 `ACTION_DOWN` 立即启动，
  须覆盖系统手势取消窗口。
- `text-search`：明确命中行落在内容上移量之内时，居中须先归零上移量。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`
