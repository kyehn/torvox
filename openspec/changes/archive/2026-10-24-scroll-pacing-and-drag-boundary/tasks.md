# Tasks

## 1. 滚动单路帧节拍

- [x] 1.1 帧回调记录 `lastVsyncNanos`，`SessionEntry` 新增该易变量
- [x] 1.2 纯函数 `activeLatchTimeoutNanos(vsyncFresh)`：健康 34ms、停摆 17ms，
      并新增 `RENDER_VSYNC_STALE_NANOS`（100ms），
      验证：`RenderLatchCadenceTest.activeLatchOutlastsOneVsyncWhenVsyncIsFresh`、
      `vsyncCallbackTimestampMarksTheChainFresh`
- [x] 1.3 渲染循环按新判据选择活跃闭锁超时，验证：`./gradlew :app:testDebugUnitTest`

## 2. 搜索高亮

- [x] 2.1 底色改为从原背景混合，交换只决定文字色，
      验证：`cargo test --lib search_highlight`
- [x] 2.2 选中命中保留全部物理行（`SearchViewportHighlights.selected` 改为 `Vec`），
      验证：`soft_wrapped_selection_colors_every_physical_row`
- [x] 2.3 新增判别力用例：高亮色等于前景色时其余命中与当前命中、与原背景都必须不同，
      验证：`other_match_stays_distinct_when_highlight_equals_foreground`

## 3. 选区拖动单一实时边界

- [x] 3.1 抽出 `resolveDragBounds`，快速路径与慢速路径共用，
      验证：`SelectionStateTest.slow path folds onto the pending boundary from the fast path`
- [x] 3.2 边缘滚动改走快速路径并按返回值重定位两只手柄，
      验证：`fast path folds onto the boundary written by the slow path`、
      `drag bounds are clamped to the grid on both paths`
- [x] 3.3 删除失去调用方的 `updateSelectionStart` 与 `syncDragSelectionToNativeThrottled`，
      验证：`./gradlew :app:detekt :app:spotlessCheck`

## 4. 规范与留档

- [x] 4.1 三个能力的规范增量落在本 change 的 `specs/` 下，验证：`openspec validate` 通过
- [x] 4.2 留档只写结论、判据与用例名，不写原始日志行、时间戳、PID 与设备标识

## 5. 集成验证

- [x] 5.1 Rust：`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [x] 5.2 JVM：`:app:testDebugUnitTest`（本轮涉及的用例类）、`:app:detekt`、`:app:spotlessCheck` 全绿
- [x] 5.3 仪器化复跑（x86_64 模拟器）：`SelectionDragQuantifiedTest`、
      `TextSearchEndToEndTest`、`ScrollBehaviorQuantifiedTest`、`SelectionEspressoTest` 全绿
- [ ] 5.4 真机复测（Mali）：滚动期间呈现间隔不低于一个显示周期，无 60ms 以上取帧空档

## Workflow follow-up

- `openspec validate 2026-10-24-scroll-pacing-and-drag-boundary --strict` 通过后归档，
  把三项增量同步进主 spec。
- 归档后执行 `openspec validate --all --strict`。
