# 任务

- [x] 建 change（实测取证 + 根因定位）
- [ ] `cell_builder::last_content_row`：视口最后一个有内容的行（空格/制表/NUL 不计）
- [ ] 宿主单测：全空视口、底部行有内容、中间行有内容、CJK 宽字符、越界 rows/cols
- [ ] `renderWithNewOutput` bit 49..63 搭载下沿行，函数文档位段注释同步
- [ ] `Bridge.RenderResult` 增字段 + 解包；`TerminalRuntime.lastContentRowFlow`（会话切换初始化）
- [ ] `computeImeSurfaceShift` 纯函数 + 单测（稀疏 0 / 满内容 = ime / 键盘吞满 / 退化输入）
- [ ] `TerminalScreen`：容器位移改为两个子节点各自 offset（同源同帧），去掉容器位移
- [ ] 仪器化用例改回「内容较少时终端无变化」并加条带墨迹可见断言
- [ ] cargo fmt / clippy / test，aarch64-linux-android check，testDebugUnitTest
- [ ] connectedDebugAndroidTest 跑 ImePopupPixelInstrumentedTest 验证真机行为
- [ ] 更新 openspec/specs 并归档 change
