# Proposal

## Why

真机 logcat 与 `docs/STATUS.md` 的未决问题指向同一条链路：`SurfaceView` 的 surface 生命周期
跟随**可见性**，于是切后台时平台拆掉 surface、切回前台再重建。

- 重建要重配交换链、重画整帧，而新窗口在拿到第一个缓冲前开孔是空的——用户看到的就是
  「切换后台返回终端闪烁」（STATUS 未决问题之一）。
- 每次重建都会让驱动重新枚举表面格式并试分配：真机每次留下四条 `mali_gralloc` 的
  「R8 格式不被支持」error 与一串 `SurfaceSyncer`「Failed to find sync」error。日志里的
  重建次数（logcat 2026-11-11 04-30 一轮 9 次、04-35 一轮 6 次）与这些 error 的条数
  一一对应——error 不是独立的缺陷，而是重建次数的倍数。
- 宿主侧的注释（「切应用不会调用 surfaceDestroyed」「SurfaceView 在后台保留窗口」）与真机
  行为相反：真机 logcat 每次切后台都出现 `ANativeWindow changed; rebuilding for the new
  window`。平台自 API 34 起提供 `SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT`，让 surface 跟随
  **挂载**：只在视图脱离窗口时销毁，切后台期间原窗口与上一帧都在。

另一条未决问题是修饰键栏底边那排按键「触发无动画反馈」：键栏底边贴着屏幕底边，全面屏
手势导航的 home 手势区就在那一带，现行实现把手势区内的按下反馈**整个吞掉**，于是底边
那排按键永远没有按下反馈（同一按键的上半部分有、下半部分没有）。现行规约把这条写成
「区内一律不启动反馈」，代价就是这条未决问题。

## What Changes

- 终端 Surface 的生命周期改为跟随挂载（API 34+）：切后台不再拆 surface、切回不再重建，
  闪烁与上述平台报错一并消失；更早的版本保持默认策略。
- 修饰键栏的按下反馈不再按手势区**吞掉**，改为按手势区**延后**：落在系统手势区内的触摸
  把反馈延后一段覆盖系统认领窗口的时间再启动，区外的触摸立即给反馈。手指在窗口内起滑时
  反馈尚未亮起，用户看到的仍是「上滑没有触发按键」；静止轻点照常给出反馈。
- 取消与滑出 `touchSlop` 的既有处理不变：按键不触发，复位瞬时且无反向位移。

## Capabilities

### Modified Capabilities

- `render-stability`：surface 生命周期的口径（跟随挂载而非跟随可见性）与切后台返回的
  表现。
- `modifier-bar-gesture-cancel`：系统手势区内的按下反馈口径（延后启动而非不启动）。
- `logcat-hygiene`：驱动格式探测与平台帧时间线告警随表面重建次数出现的归因补充。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`
- `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`
- `openspec/specs/render-stability/spec.md`
- `openspec/specs/modifier-bar-gesture-cancel/spec.md`
- `openspec/specs/logcat-hygiene/spec.md`

## 取舍与缺口

- 跟随挂载意味着后台期间交换链仍占着显存（渲染本就已在后台暂停）。这点显存换来的是零
  闪烁与零重建——重建一次要重配交换链、重画整帧，并让驱动重新做一轮格式试分配。
- 手势区内的反馈改为延后而非取消：手指静止停留超过该窗口后再起滑，按下反馈会先出现
  再复位。这与上游 termux 的同一取舍（termux 立即给反馈、被系统认领时复位）一致，且
  键栏底边那排按键不再没有反馈。系统何时认领手势取决于用户手指多快移动，没有上界，
  故延后只能覆盖常见的快速起滑。
- `docs/STATUS.md` 首行声明禁止修改本文件，故未在本次变更中改写该清单；两条未决问题的
  收口口径写在本文与主规约里。
