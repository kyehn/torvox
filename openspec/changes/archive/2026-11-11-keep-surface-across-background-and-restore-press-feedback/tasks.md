# 任务

- [x] 终端 Surface 的生命周期改为跟随挂载（API 34+）：切后台不拆 surface、切回不重建
- [x] 同步更正宿主侧关于「切应用不调用 surfaceDestroyed」「SurfaceView 在后台保留窗口」的
      注释（真机 logcat 证明该假设不成立）
- [x] 修饰键栏的按下反馈改为按手势区延后启动，不再整个吞掉手势区内的反馈
- [x] 手势区外的按下反馈立即给出（延迟只用于可能被系统认领的触摸）
- [x] 更新 `render-stability` 的 surface 生命周期与切后台返回口径
- [x] 更新 `modifier-bar-gesture-cancel` 的手势区内反馈口径
- [x] 更新 `logcat-hygiene` 的平台告警归因（探测随表面重建次数出现）
- [x] 运行 Gradle 检查（`:app:compileDebugKotlin` 与键栏相关单测通过）
