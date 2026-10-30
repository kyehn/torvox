# Tasks

## 1. Surface 归属认领

- [x] 1.1 `surfaceCreated` 的归属认领移到所有提前返回之前
- [x] 1.2 注释写明「认领被跳过时陈旧销毁会误伤当前持有者」的故障形态

## 2. 启动期换窗

- [x] 2.1 `start()` 的待绑定 Surface 登记移到「已在启动中」早退之前
- [x] 2.2 判活判据改为登记处里的最新一张
- [x] 2.3 spawn 的 PTY 初始 winsize 改按登记处那张的尺寸计算
- [x] 2.4 `attachPendingSurface` 返回是否真的绑上；绑不上时记错误日志

## 3. 验证

- [x] 3.1 主题跟随用例逐轮重新解析终端视图（Activity 重建后旧引用失效）
- [x] 3.2 主题跟随用例区分「画面在但底色不对」与「Surface 上没有画面」
- [x] 3.3 宿主编译、`detekt`、`spotlessCheck` 通过
- [x] 3.4 模拟器全量仪器化套件通过
- [x] 3.5 `check` 工作流重跑通过

## 4. 规范

- [x] 4.1 `render-stability` 新增「Surface 归属认领 MUST 先于任何提前返回」
- [x] 4.2 `render-stability` 新增「启动期间换窗 MUST NOT 丢弃新 Surface」
- [x] 4.3 `render-stability` 的「已取出的帧不得因 surface 竞态丢弃」补启动期换窗形态
