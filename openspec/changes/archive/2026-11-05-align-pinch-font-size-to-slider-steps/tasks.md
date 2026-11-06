# 任务

## 一、单一收敛入口

- [x] 1.1 `SettingsRepository.clampFontSize` 改为「区间钳位 + 步长档位取整」，
      文档写明「可设档位 = 区间 ∩ 4sp 起、步长 2sp 的网格」，并说明为何区间内的
      非档位值也必须被挪动（滑块读数按整数打印、展示与生效分家、首次触摸改写）。
- [x] 1.2 `clampFontSize` 的取整不得越过上界：上界非档位时取不高于上界的档位。

## 二、捏合手势同源

- [x] 2.1 `zoomFontSize` 改走 `SettingsRepository.clampFontSize`，返回值必在档位
      网格上；KDoc 删去「只钳上下限」的旧口径。
- [x] 2.2 `onScaleEnd` 的落定判定改用收敛后的字号：删去未钳位的原始值与
      `ZOOM_FONT_SIZE_EPSILON_SP` 容差（基准与终值同为档位值，相等即未变化）。
- [x] 2.3 手势未改变字号时只撤销预览，不得落盘新值。

## 三、回归用例

- [x] 3.1 `TerminalSurfaceLogicTest`：捏合给出的每一档都落在调节条档位网格上；
      捏合落定值等于调节条可给出的值；预览值与落定值同源。
- [x] 3.2 `FontSizeRangeTest`：区间内的合法档位原样返回；区间内的非档位值被挪到
      最近档位；上界非档位时取整不越界；收敛幂等。
- [x] 3.3 仪器测试同源：`ZoomPreviewGestureInstrumentedTest` 的预览 tick 改走档位
      序列（插值出的非档位值会被收敛入口挪走，「精确落在请求值上」永远不成立）；
      `FontSizeReflowInstrumentedTest` 的目标字号经同一收敛入口取，避免
      「按目标预测的宽度」与实际生效的字号不是同一个值。

## 四、验证与归档

- [x] 4.1 `./gradlew --no-parallel --max-workers=1 :app:testDebugUnitTest` 全绿。
- [x] 4.2 `detekt`、`spotlessCheck`、`lintDebug`、`:app:compileDebugAndroidTestKotlin`
      全绿。
- [x] 4.3 同步 `openspec/specs/font-selection/spec.md` 并归档本变更。
