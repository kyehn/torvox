# 会话列表按规范重设计

## 上下文

- `DESIGN.md` 侧边面板节与 `TESTING.md` "会话标题,OSC 7 工作目录读取"覆盖项为标准;`AGENTS.md` 要求修改前先建 changes。
- 实测结论(设备 mksh,40 列 PTY):PS1 内嵌 OSC 字节计入提示符宽度、触发水平滚动花屏 → 否决 PS1 内嵌方案,改 `cd` 包装(`command cd` 防递归、`&&` 守卫,均实测)。

## 任务

- [ ] 1.1 native:补充 OSC 9(`9;9`)pwd 事件测试,按实测修正 `public_api.rs` 注释(`cargo test` 目标用例)。
- [ ] 1.2 Kotlin:`ensureMkshPromptRc` 追加 `report_directory` + `cd` 包装 + 源时初发,marker 改为新内容特征,旧安装自愈。
- [ ] 1.3 `refreshSessionMetas`:去 `sameSet`/`lastMetaSessionIds`,非 force 2 秒节流、force 恒执行,只取 directory。
- [ ] 1.4 `SessionInfo` 移除 `title` 字段:抽屉副标题 = 缩写目录;全部构造点迁移;刷新不再调 `getTitle`。
- [ ] 1.5 `TerminalScreen` `drawerState.isOpen` 打开时 `refreshSessionMetas(force = true)`;移除 `SessionDrawer.onRefreshSessions` 与其内部 `LaunchedEffect`;`SessionDrawerNarrowTest` 适配。
- [ ] 1.6 验证(`cargo test` 目标用例、`testDebugUnitTest` 相关类、release APK 设备验证:cd 后抽屉目录、重开抽屉刷新、关闭后编号递增)。
- [ ] 1.7 `openspec archive` 归档(同步 `openspec/specs/session-list`,删除 change)。
