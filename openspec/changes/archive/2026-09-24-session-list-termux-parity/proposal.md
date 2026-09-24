# 会话列表对齐 termux 抽屉设计

## 背景

`DESIGN.md` 侧边面板节称目录缩写"实现参考 Termux"，但 termux 抽屉（`TermuxSessionsListViewController` + `item_terminal_sessions_list.xml`）实测不显示目录：行 = `"[N] "+会话名`（粗体）+ 换行斜体终端标题（OSC 0/2），点击切换并关闭抽屉，无行内关闭按钮，长按重命名。本项目第二行显示目录路径，与参考实现相悖；`TESTING.md` 覆盖项"会话标题"过时（标题字段上轮已删）；`REFERENCE.md` 无会话列表条目；`openspec/specs/session-list` Purpose 为归档占位符。

用户裁决：代码与文档都与 termux 一致，第二行改为终端标题；不实现重命名功能，保留行内关闭按钮与 `会话 N` 序号（关闭按钮为 `DESIGN.md` 声明）。

## 改动

- 抽屉第二行改为该会话终端标题（`NativeBridge.getTitle` 查询，未设置标题时隐藏该行）；删除 `SessionInfo` 目录字段、目录缩写与 mksh rc 的 OSC 7 发射。
- 删除失去消费者的工作目录跟踪管线：native cwd 事件通道/`on_pwd_changed` 注册/查询与锁存、JNI `getCurrentDirectory`、OSC 7/9/1337 单测与 BDD 特性。
- `DESIGN.md` 侧边面板节：条目改"会话序号、终端标题"，删除工作目录跟踪声明；`TESTING.md` 覆盖项改会话序号与终端标题读取；`REFERENCE.md` 补 termux 会话抽屉事实条目。
- `session-list` spec：REMOVED 工作目录跟踪，MODIFIED 会话项内容与元数据刷新，归档后补 Purpose。

## 影响

- 测试：`SessionDrawerNarrowTest` 字段改名；删 `AbbreviateDirectoryTest`、`osc7工作目录.feature`、`VtCorrectnessInstrumentedTest` 目录用例（`osc0TitleQueryable` 与 `窗口标题.feature` 覆盖标题）。
- shell 自设 OSC 0/2 标题时第二行显示；应用不再上报目录。
- 不碰：点击切换、关闭按钮、编号递增、抽屉打开强制刷新与节流逻辑。
