---

## 未被解决的问题

基于 `2c964737` 提交

> 本文件此前顶部写着「任何情况下禁止对本文件进行任何修改」。该约束已被 2026-11-13 的
> 用户指令取代：那次指令明确要求修正本文件与实际状态不符之处，故本文件恢复为可按需
> 更新的状态清单。

- 输入法弹出时切换后台返回终端闪烁：`ON_PAUSE` 收起键盘后平台派发一次「输入法高度 0」，
  位移在后台被归零（实测 225→0），而开孔遮罩与原生保留的最后一帧都按离开时的位移拍下，
  回前台首帧画在错误位置、输入法回来再跳一次。已改为后台期间冻结 insets 采纳
  （`226ef094`），仪器化用例 `imeShiftFrozenWhileBackgrounded` 无修复时 225→0 失败、
  修复后保持 225。真机复验待做（模拟器切后台不拆 surface，端到端的遮罩/原生重建路径
  只在真机出现），复验前仍按未解决计。
- Android 13 的 `SurfaceSyncer` 同步组查询告警（`Failed to find sync for id=N`，真机日志
  33 条）：平台在 `SurfaceView.handleSyncNoBuffer` 把 syncId 记入 `mSyncIds` 之前就调用
  重绘回调，完成回调同步执行时该 id 在入账前即被同步组移除，此后每次
  `surfaceSyncStarted` 都用陈旧 id 查组失败并逐条打 E。已改为实现
  `SurfaceHolder.Callback2` 并把 `drawingFinished` 投递到下一轮 UI 循环执行
  （`321100c8`），仪器化用例 `SurfaceRedrawAsyncInstrumentedTest` 断言完成回调不得在
  `surfaceRedrawNeededAsync` 内同步执行（模拟器通过）。Android 13 真机复验待做——该告警
  只在 Android 13 出现（14 重写了该路径并删除 `mSyncIds`），模拟器（Android 15）复现
  不了平台侧的陈旧同步组，复验前仍按未解决计。
