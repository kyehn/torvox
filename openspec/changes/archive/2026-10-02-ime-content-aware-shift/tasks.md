# 任务

- [x] 建 change（实测取证 + 根因定位）
- [x] `cell_builder::last_content_row`：视口最后一个有内容的行（空格/制表/NUL 不计）
- [x] 宿主单测：全空视口、底部行有内容、中间行有内容、CJK 宽字符、越界行号（6 例）
- [x] `renderWithNewOutput` bit 49..63 搭载下沿行，函数文档位段注释同步
- [x] `Bridge.RenderResult` 增字段 + 解包；`TerminalRuntime.lastContentRowFlow`（会话切换初始化）
- [x] `computeImeSurfaceShift` 纯函数 + 单测（稀疏 0 / 满内容 = ime / 上界 / 键盘吞满 / 单调，8 例）
- [x] `TerminalScreen`：容器位移改为两个子节点各自 offset（同源同帧），去掉容器位移
- [x] 仪器化用例改回「内容较少时终端无变化」并加条带墨迹可见断言
- [x] cargo fmt / clippy / test（512 例）、testDebugUnitTest、spotless、detekt、lint、semgrep
- [x] 真机实测（Android 15 x86_64 模拟器 1080x2400/density420，`dumpsys` ime frame
      `[0,1517][1080,2400]`）：稀疏内容 shift=0（提示符留在 y=132-162）；
      满内容上移且末行贴键栏顶（top 行被裁，末行为 `$ ▉`）；键栏底 1517 = 键盘顶
- [x] 更新 openspec/specs 并归档 change

## 落地过程中的实测修正

- 内容下沿 MUST 直接 `lastContentRowFlow.collect { }` 写入 placement 期读取的状态：
  `snapshotFlow { flow.value }` 只跟踪组合快照的读，对 StateFlow 的后续更新不触发
  （实测只发初值 -1），终端便完全不上移，网格填满时末行被键栏吞掉。
- 位移值取两个子节点各自的 placement：键栏 `Modifier.offset`、Surface
  `Modifier.layout`（需读 `placeable.height`）；实测两者在 ime 变化时逐帧重新求值。
