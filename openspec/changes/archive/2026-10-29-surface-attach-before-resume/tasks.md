# Tasks

## 1. 挂载门闸

- [x] 1.1 新增 `SurfaceAttachGate`：Surface 销毁时领一次暂停，新窗口挂上时归还
- [x] 1.2 门闸接入运行期：销毁走 `onSurfaceDestroyed`，挂载走 `onSurfaceAttached`
- [x] 1.3 删除 `clearLifecycleRenderPause` 与 `clearSurfaceDestroyedRenderPause`
- [x] 1.4 门闸与账本只在主线程被调用：启动序列那条挂载路径切回主线程再解除

## 2. 恢复路径

- [x] 2.1 切前台恢复：门闸仍在等新窗口时不重启渲染线程
- [x] 2.2 延迟解除暂停：同样先判门闸状态
- [x] 2.3 强制出帧：只归还切后台那次持有

## 3. 验证

- [x] 3.1 门闸单测覆盖：销毁后只由挂载解除、重复挂载不重复解除、重复销毁计数非空
- [x] 3.2 宿主单测、detekt、spotless 全绿
- [x] 3.3 模拟器复跑「切后台 → 回前台」：新窗口挂载前无取帧，报错链不再出现
- [x] 3.4 logcat 逐条归因留档于 `evidence.md`

## 4. 规范

- [x] 4.1 `render-stability` 新增「渲染恢复 MUST 晚于新窗口挂载」要求
- [x] 4.2 `render-stability` 修改「强制出帧只作废生命周期持有」：只作废切后台那次持有
