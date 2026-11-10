# 任务

- [x] 定位平台提示窗口三类告警的根因（窗口独占、窗口生命周期、按包限额）
- [x] 新增应用内瞬时提示通道 `TransientMessageChannel`，补单元测试钉住「无订阅者即丢弃」
- [x] 字体应用与清除数据两条路径改经通道投递提示
- [x] 终端页接入提示条：订阅通道、以 Snackbar 呈现、提示条贴在键栏之上
- [x] 设置页接入提示条：订阅通道、以 Snackbar 呈现
- [x] 删除平台提示投递器 `ToastNotifier` 与其单元测试
- [x] 仪器化用例断言字族切换后提示可见
- [x] 定向运行 OpenSpec 严格校验，确认变更与主规约通过（`openspec validate`）

## Workflow follow-up

- 归档本 change 并同步主规约
