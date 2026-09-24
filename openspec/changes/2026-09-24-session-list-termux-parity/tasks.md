# 会话列表对齐 termux 抽屉设计

## 背景与标准

- `docs/specification/DESIGN.md` 侧边面板节、`TESTING.md` 覆盖项、`REFERENCE.md` 为标准；termux 源码事实：行 = `[N] 名称` + 斜体标题、点击切换并关闭抽屉、无行内关闭按钮、长按重命名。
- 用户裁决：代码与文档都与 termux 一致；不实现重命名；保留行内关闭按钮与 `会话 N` 序号。

## 任务

- [ ] 1.1 Kotlin：`SessionInfo.directory` 改 `title`，`refreshSessionMetas` 查询 `getTitle`，抽屉第二行显示标题，删 `abbreviateDirectory`
- [ ] 1.2 mksh rc 去 OSC 7 发射，回退纯 `PS1='$ '`（marker 自愈 OSC 代旧内容）
- [ ] 1.3 测试：`SessionDrawerNarrowTest` 改字段、删 `AbbreviateDirectoryTest`、`VtCorrectnessInstrumentedTest` 删目录用例
- [ ] 2.1 native：删 cwd 通道、回调注册、查询、锁存、JNI 导出、OSC 7·9·1337 单测与 BDD 特性
- [ ] 3.1 `DESIGN.md` 侧边面板节、`TESTING.md` 覆盖项、`REFERENCE.md` termux 条目修正
- [ ] 4.1 验证：`cargo fmt`、ghostty_terminal 单测、`testDebugUnitTest`、`spotlessCheck`、`detekt`、`connectedDebugAndroidTest`（SessionDrawer、VtCorrectness）、`openspec validate --all`
- [ ] 5.1 归档 change、同步 specs、补 Purpose
