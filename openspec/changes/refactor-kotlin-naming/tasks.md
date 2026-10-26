# Tasks

## 1. 日志工具

- [x] 1.1 `runtime/LogUtil.kt`：`d` / `i` / `w` / `e` → `debug` / `info` /
      `warn` / `error`，更新全仓调用点，
      验证：Kotlin 编译通过，日志级别与文案不变

## 2. 主源重命名

- [x] 2.1 `ui/TerminalSurface.kt`：`wm` / `vm` / `dx` / `dy` / `sr` / `sc` /
      `er` / `ec` / `maxR` / `maxC` / `px` / `py` / `e1` / `e2` / `loc` / `sel` /
      `bar` / `absX` / `absY` / `del` / `attr` / `rect` / `imm` / `lo` / `hi` /
      `sx` / `sy` / `ex` / `ey` → 完整名称，
      验证：编译通过，`clampSelection` 交换逻辑与菜单定位逻辑不变
- [x] 2.2 `ui/ModifierBar.kt`：`ev` / `slop` / `row1` / `row2` → 完整名称，
      验证：长按与自动重复阈值行为不变
- [x] 2.3 `TerminalViewModel.kt` / `runtime/TerminalRuntime.kt` /
      `bridge/Bridge.kt`：`id` → `sessionId`，`cur` / `ids` / `dirs` / `meta` /
      `fps` / `px` / `t` → 完整名称，
      验证：会话切换、关闭、选区提交与滚动余量语义不变
- [x] 2.4 其余主源：`input/ComposingBaseline.kt` 的 `to`、`FontUtils.kt` 的
      `abi`、`installer/*` 的 `arch` / `pct` / `proc` / `bootstrapDirs`、
      `DocumentMutations.kt` 的 `src` / `dstParent` / `key`、
      `DocumentQueries.kt` 的 `ext` / `dir`、
      `TerminalDocumentsProvider.kt` 的 `cols`、`bridge/*` 的 `xPx` / `yPx` →
      完整名称，
      验证：编译通过且各文件行为不变

## 3. 测试源重命名

- [x] 3.1 `androidTest` 与 `test` 下的 `w` / `h` / `a` / `b` / `pa` / `pb` /
      `x` / `y` / `t` / `t0` / `dt` / `fd` / `s` / `ch` / `cw` / `h0` / `h1` /
      `e` / `up` / `out` / `enc` / `orch` / `frac` / `keyE` / `upAt` /
      `minW` / `minH` / `maxW` / `maxH` → 完整名称，
      验证：编译通过，断言与判定逻辑不变
- [x] 3.2 `UxTestUtils.kt`：抽出共用的「同尺寸两图像素数组」helper，
      消除 `changedPixelCount` 与 `changedBoundingBox` 的重复样板，
      验证：两个函数的返回值口径不变

## 4. 集成验证

- [x] 4.1 Kotlin：`:app:compileDebugKotlin`、`:app:testDebugUnitTest`、
      `:app:detekt`、`:app:spotlessCheck` 全绿
- [x] 4.2 全仓再扫短标识符，确认无新增残留
- [x] 4.3 规范：`openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更。
