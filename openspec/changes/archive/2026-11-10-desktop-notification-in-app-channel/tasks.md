# 任务

- [x] 确认平台提示窗口的三条约束在桌面通知路径上同样成立
- [x] `TransientMessageChannel` 改为依赖注入提供的进程内单例
- [x] `DesktopNotificationPlayer` 前台路径改走应用内通道，删除平台提示窗口的构造
- [x] 运行期与 ViewModel 接线：桌面通知与界面提示共用一条通道
- [x] 更新受影响的单元测试（通知投递器、系统通知投递、会话挂起判定、ViewModel 假件）
- [x] 仪器化验证：前台桌面通知经应用内通道呈现，且 logcat 无平台提示窗口条目
- [x] 定向运行 OpenSpec 严格校验，确认变更与主规约通过（`openspec validate`）

## Workflow follow-up

- 归档本 change 并同步主规约
