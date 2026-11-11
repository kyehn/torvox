# Proposal

## Why

`docs/STATUS.md` 的未决问题「切换后台返回终端闪烁」尚未解决。此前的一次变更
（`2026-11-11-keep-surface-across-background-and-restore-press-feedback`，已归档）断言
「切后台不再拆 surface」，该断言被真机与模拟器实测证伪：

- 平台在切后台后会拆掉本应用**窗口**的 surface（实测 API 35 模拟器：
  `surfaceDestroyed` 紧随「Transition animation finished」到达）；
  `SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT` 只让 surface 跟随**视图的挂载**，不改变
  窗口 surface 在后台被拆这一事实。
- 返回前台时原生窗口指针已变（`ANativeWindow changed; rebuilding for the new
  window`），交换链照样重建——`setSurfaceLifecycle` 并没有消除重建。

重建不可免（平台行为），但**闪烁可以消除**：重建期间窗口的透明开孔没有缓冲，
此刻画的是一块平底色（用户看到的是「闪成另一种颜色」）。把离开时的画面留在开孔里，
用户看到的就与离开前一致。

## What Changes

- 撤销 `setSurfaceLifecycle(SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT)`：它既无效，又把
  「平台会替我们保留 surface」这种不可控的假设写进了代码。终端 Surface 回到平台
  默认策略。
- 新增「开孔遮罩」：切后台时（渲染线程停下之前）把保留的上一帧从 GPU 帧累加器回读
  成 CPU 像素，宿主在窗口开孔里画它，直到新 surface 提交首帧即撤下。
  - 原生侧新增帧累加器的像素回读（`read_cover_frame`）与 JNI 导出
    （`NativeBridge.readCoverFrame`）；
  - 运行期在渲染线程停下时回读并发布遮罩（新帧到达即撤），回读是一次 GPU 同步
    等待，故只在渲染线程上做；
  - 组合层在「还没有缓冲区」时画遮罩（没有遮罩才退回平底色）。
- 规约 `render-stability` 更正为实测机制：表面策略 MUST 保持平台默认；切后台时
  MUST 把离开时的画面留作开孔遮罩。

## Capabilities

### Modified Capabilities

- `render-stability`：表面策略回到平台默认；新增「切后台时把离开时的画面留作开孔
  遮罩」的要求与场景。
- `logcat-hygiene`：驱动格式探测与平台同步组查询条目的归因补上「重建不可避免，
  只能靠开孔遮罩消除闪烁」。

## Impact

- `native/src/render/context/cover.rs`（新增）、`native/src/render/context/mod.rs`
- `native/src/android/ffi/render/window.rs`
- `native/src/render/tests/cover.rs`（新增）、`native/src/render/tests/mod.rs`
- `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt`
- `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt`
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `android/app/src/main/java/terminal/emulator/runtime/RenderSupervisor.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` 的注释
- `android/app/src/androidTest/java/terminal/emulator/ui/SurfaceLossRecoveryInstrumentedTest.kt`
- `openspec/specs/render-stability/spec.md`
- `openspec/specs/logcat-hygiene/spec.md`

## 取舍与缺口

- 回读是一次 GPU 同步等待（约几毫秒），只在渲染线程停下之前做一次；遮罩是一张
  与 surface 同尺寸的位图（真机约 1.5MB），新帧到达即释放。
- 回读只支持 RGBA8 的帧累加器：其它格式放弃遮罩（平底色兜底），宁可少遮也不
  画错颜色。
- 重建本身（以及随之而来的驱动格式探测 error）无法消除：平台在后台拆窗口 surface
  是既定行为。本变更消除的是**用户可见的闪烁**，不是重建次数。
- 此前归档的变更 `2026-11-11-keep-surface-across-background-and-restore-press-feedback`
  里「切后台 MUST NOT 触发 surface 的拆除与重建」的断言与实测相反，本变更更正
  主规约，并在归档目录留修正说明。
