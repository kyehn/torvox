# Proposal

## Why

CI 的 `build` 工作流在 `ThemeFollowSystemInstrumentedTest` 上失败：用例断言
「日间主题底色没有出现在终端 Surface 上」。本地全量仪器化套件 235 例通过，
差异只在 CI 的模拟器更慢。逐条审查后找到三处**运行期 Surface 竞态**，
它们都能让终端停在空白画面，且全程零异常、零自愈——CI 的失败只是它们的一个
侧面：Activity 重建后旧 `SurfaceView` 引用失效，用例对着失效视图取像素，
自然读不到底色。

三处缺陷同属一条链：**谁持有 Surface、哪一张 Surface 是最新的**这两个事实，
在启动与视图重建的交错时序下被算错了。

1. `TerminalRuntime.start()` 把待绑定的 Surface 登记在「已在启动中」早退**之后**。
   启动进行中视图重建，新 Surface 只经这条路径抵达运行期，却被早退丢弃；
   正在进行的启动随后绑定的是旧（已销毁）Surface 或空——终端从此没有画面。
2. `TerminalRuntime.start()` 用**入参**那张 Surface 判活、用**入参**尺寸算 PTY
   初始 winsize。启动期间换过窗时，入参那张早已作废：拿它判活会让整条启动
   白白中止，拿它的尺寸 spawn 会让 PTY 网格与实际窗口不符。
3. `TerminalSurface.surfaceCreated()` 在 0 尺寸／无效 Surface 的提前返回**之后**
   才认领归属。认领被跳过时，旧视图迟到的 `surfaceDestroyed` 仍被当作当前
   持有者处理：它清空待绑定的 Surface 并领走一次渲染暂停，而新窗口的绑定
   已经发生，此后无人归还那次暂停——终端停在空白画面。

## What Changes

- `TerminalRuntime.start()`：待绑定 Surface 的登记移到「已在启动中」早退之前；
  判活与 spawn 网格尺寸一律取登记处里的最新一张。
- `TerminalRuntime.attachPendingSurface()` 改为返回是否真的绑上；有 Surface
  却绑不上时记错误，使「会话活着却没有画面」这一形态在 logcat 里可见。
- `TerminalSurface.surfaceCreated()`：归属认领移到所有提前返回之前。
- `ThemeFollowSystemInstrumentedTest`：每轮轮询重新解析终端视图（Activity
  重建后旧引用失效），并把「画面在但底色不对」与「Surface 上根本没有画面」
  分开计数，失败时进断言信息。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`: 新增「Surface 归属认领 MUST 先于任何提前返回」与
  「启动期间换窗 MUST NOT 丢弃新 Surface」两条要求；原「已取出的帧不得因
  surface 竞态丢弃」一条补上启动期换窗这一形态。
- `instrumentation-harness`: 新增「读终端 Surface 的用例 MUST 逐轮重取视图」
  一条要求，把「Activity 重建后旧视图引用失效」这条实现细节固定下来。

## Impact

- 运行期：`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`、
  `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`。
- 测试：`android/app/src/androidTest/java/terminal/emulator/ThemeFollowSystemInstrumentedTest.kt`。
- 规范：`openspec/specs/render-stability/spec.md`。
- 不涉及原生代码、依赖与对外 API。
