# 建会话时的网格必须按最终字体度量算出

## Why

用户报障：终端首屏的 prompt 下方多出一行输入提示符；且概率性需要手动下滑才能看到
prompt。

复核后定位到一条确定的时序缺陷：**PTY 建立时用来算初始 winsize 的字体度量不是最终
度量**。

1. `spawnTerminalTimed` 先调 `refreshCellMetricsFrom(bridge)` 取度量，再据此算初始
   网格。此刻字体管线仍是原生默认字体（14.0 sp、默认字族），因为字族、字号与光栅
   尺度要到 spawn **之后**的 `applyRenderSettings` 才下发。
2. `applyRenderSettings` 下发真实字体后，度量变了；`attachPendingSurface` 随后的
   `recomputeGridFromFontMetrics()` 便按新度量算出另一套网格并 `resize`。
3. shell 这时早已打印了 prompt。SIGWINCH 让 readline 重排当前行，prompt 于是被打印
   两次：一次是 shell 自己打印的，一次是 readline 在窗口尺寸变化后重绘的。重绘落在
   下一行，看起来就是「prompt 下方多出一行输入提示符」；视口随之多出一行内容时，
   prompt 还会被挤出可见区，用户便要手动下滑才看得见。

初始 winsize 与最终网格不同源，是这条链路的起点：`initialSpawnGrid` 的前提「度量
已就绪」被满足，但那个度量并不是最终度量。

## What Changes

- 字体契约（字族 + 字号 + 光栅尺度）在 **spawn 之前**下发：这三项是全局量，原生侧只
  维护一份字体管线，会话建立之前即可应用。
- 网格计算随之读到的就是最终度量，初始 winsize 与最终网格一致，attach 之后零变更，
  shell 不再收到多余的 SIGWINCH。
- `Bridge` 的字体相关入口（`setFontFamily`/`setFontSizeInPlace`/`setRasterScale`/
  `loadFontFile`）不再走会话闸：它们在原生侧本就与会话无关，会话闸只会让它们在
  会话建立之前静默失效。
- `startup-grid-resize` 规范补上「算网格的度量必须已落定」这一前提。

## Impact

- `Bridge.kt`：字体相关入口改为全局下发。
- `TerminalRuntime.kt`：spawn 之前先落定字体契约；spawn 与契约下发共用同一份字族。
- 规范：`startup-grid-resize`。
- 行为：冷启动后不再有那次多余的 resize，首屏 prompt 不再重复，网格与字形度量
  从第一帧起就一致。
