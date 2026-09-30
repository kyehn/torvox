# 修饰键栏内联终端列消除输入法重叠闪烁

## Why

真机取证（11 张截图逐像素分析，行距 44.6px 标定）：内容超出时输入法弹出，终端内容区与修饰键栏是两个独立位移——`TerminalContent` 按光标最小平移（165px），`ModifierBarOverlay` 按完整键盘高度（1026px）跟随。两者差拍 861px（约 19 行），终端行与键栏在 1328..1517 区互压，表现为内容重叠；Gboard 候选条 163px 间歇显隐使键盘高度在 1089px/926px 间跳变，位移跟随跳动即持续闪烁；键栏被终端 Surface 开孔压住后终端行透出即半透明。`onImeSettled` 只清 Surface 自位移，从未统一双位移，属结构性缺陷。

## What Changes

- 修饰键栏由外部 `ModifierBarOverlay` 改为终端 `Column` 内联末行：与终端同位移，差拍物理消失。
- 键盘弹出时 `Column` 按 `barPanPx` 全量上移（光标最小平移 `computeTerminalPanPx` 删除）；列内键栏行高度恒为 `modifierBarHeightPx`，网格行数不变。
- 空终端黑洞借 `TerminalScreen` 根背景（已是终端主题色）覆盖，不新增绘制。
- `TerminalImePanTest` 删除（函数已不存在）；`ImePopupPixelInstrumentedTest` 阈值不变（行为收敛到已验证的 m2 态）。

## Non-Goals

- 不改网格重排语义（行列数恒定，无 `SIGWINCH`）。
- 不引入 Compose 弹簧或 `WindowInsetsAnimation` 回调（系统插值已足够）。
- 稀疏会话防黑屏条款由全量上移替代（spec 同步更新）。
