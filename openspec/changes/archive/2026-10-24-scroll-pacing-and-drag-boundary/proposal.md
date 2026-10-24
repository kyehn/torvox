# Proposal

## Why

`docs/STATUS.md` 仍列着三项「未被解决的问题」：上下滑动滚动缓慢、卡顿、闪烁；
文本搜索的匹配与当前匹配高亮颜色应当不同；长按选择后手柄反向越过对侧手柄时，
选区另一边界异常增加且手柄与边界不再对齐。

本轮的证据来自一份真机 logcat（Mali GPU，480×819 surface，Mailbox + latency=2）：
渲染循环的平均帧率约 72fps，明显高于 60Hz 显示刷新率；呈现时刻成对出现
（间隔 3–5ms），随后周期性出现 66–77ms 的取帧空档，单帧耗时几乎全部落在
`acquire`。三条缺陷各有可判定的根因，本 change 逐条修复并把判据写进规范。

## What Changes

- `render-loop-scroll-cadence`：活跃闭锁 MUST NOT 与 vsync 成为两路帧节拍。
  vsync 链健康时闭锁只作安全网（明显长于一个显示周期），链停摆时退回 17ms 兜底。
  循环据此把产出压到每个显示帧至多一帧，交换链不再排满。
- `text-search`：高亮底色的混合基准 MUST 是**原背景**。此前先交换再混合，
  默认主题的选区色回退等于前景色，`blend(前景, 前景)` 与 alpha 无关，
  当前命中与其余命中渲染成同一块颜色；同时选中命中 MUST 按**全部物理行**着色，
  软换行处不得退回「其余命中」的淡色。
- `text-selection`：拖动边界的计算 MUST 只有一份实时边界（未提交的拖动结果），
  快速路径与边缘滚动路径都以上一次结果为基准。此前快速路径不写 Compose 状态、
  慢速路径只读已提交状态，两条路径交替时未拖动的一侧会被弹回旧端点。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-loop-scroll-cadence`：活跃闭锁与 vsync 的单路节拍要求及判据。
- `text-search`：高亮混合基准（原背景）与选中命中跨软换行的着色范围。
- `text-selection`：拖动边界的单一真源（两条路径共用同一份实时边界）。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `native/src/render/cell_builder.rs`、`native/src/render/tests.rs`
- `native/src/android/ffi.rs`、`native/src/terminal/ghostty_terminal/`
- 测试：`RenderLatchCadenceTest`、`SelectionStateTest`、`cargo test --lib`
- 不改 `docs/specification/`。
