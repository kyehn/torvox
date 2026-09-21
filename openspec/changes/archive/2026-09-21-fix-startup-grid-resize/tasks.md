# Tasks

- [x] T1: `ResizeManager` 新增 `pendingRetryWidth/pendingRetryHeight` + `applyPendingSurfaceResize()`
- [x] T2: `applyResizeNormal` 在 `!surface.isValid` 分支记录待重放尺寸
- [x] T3: `surfaceCreated` 末尾重放暂存尺寸；`isRunning` 分支补 `applyGridResize(width, height)`
- [x] T4: `TerminalRuntime.syncGridDimensions` 在单元格度量首次可用时 `recomputeGridFromFontMetrics()`
- [x] T5: 运行 `ScrollDistanceTest` / `GridToScreenTest` / `InputBatchBufferTest`（37/37 通过）
^- [x] T6: release APK 实机冷启动验证：日志出现 `applyGridResize ... -> 45x48`，截图文本铺满整屏
^- [x] T7: openspec archive，git commit + push
