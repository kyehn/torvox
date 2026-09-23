# 会话列表按规范重设计

## 背景

- `DESIGN.md` 侧边面板节为标准:每一项含"会话序号、目录路径"(缩写参考 Termux)、关闭按钮;会话序号从 1 开始递增,列表改变时亦然;需要实现工作目录跟踪(OSC 7/9/1337)。
- 现状与标准相悖:
  1. 无任何 shell 上报工作目录:声明的唯一集成点 `ENV`(`.mkshrc`)仅改提示符,抽屉目录恒空。
  2. `refreshSessionMetas` 的 `sameSet` 短路使会话集合不变时元数据永不刷新(`lastMetaSessionIds` 相同直接返回),`cd` 后目录不更新;节流注释声明的"抽屉打开刷新"意图与实现相悖。
  3. 抽屉仅 `LaunchedEffect(Unit)` 首次组合时刷新,重开不刷新。
  4. `SessionInfo.title` 按创建时位置生成并经 `previousById` 复用,列表变更后过时编号可作为副标题显示;OSC 标题副标题未在规范声明。
  5. OSC 9:`public_api.rs` 注释称实测无事件,与上游文档(ConEmu CurrentDir)矛盾,且缺测试。
- 实测(设备 mksh,40 列 PTY):PS1 内嵌不可见 OSC 字节会被计入提示符显示宽度,第 9 个输入字符即触发水平滚动重绘(`\r` + `<` 标记)—— 即 `ensureMkshPromptRc` 注释要防的花屏根因。故 PS1 内嵌 `$(printf …)` 方案否决,改为 `cd` 函数包装 + 源时上报:`command cd` 防递归、`&&` 守卫失败不发射,均已在设备实测。

## 改动

- `.mkshrc` 内容追加 `report_directory` 函数、`cd` 包装、源时初发(OSC 7 `file://$PWD`,空主机);`PS1='$ '` 不变;marker 改为新内容特征,旧安装自愈。
- `refreshSessionMetas`:去 `sameSet`/`lastMetaSessionIds`;非 force 按 2 秒节流合并,force 恒执行;只取 directory,不再调 `getTitle`。
- `TerminalScreen` 的 `drawerState.isOpen` effect:打开时 `refreshSessionMetas(force = true)`;移除 `SessionDrawer.onRefreshSessions` 参数及其内部 `LaunchedEffect`。
- `SessionInfo` 移除 `title` 字段:抽屉副标题 = 缩写目录(仅此)。
- native:补充 OSC 9(`9;9` ConEmu 形式)工作目录跟踪测试,按实测修正 `public_api.rs` 注释。

## 非目标

- bash(bootstrap 默认)不读 `ENV`,不注入用户数据(`DESIGN.md` Shell 节已声明 bash 忽略 ENV);bash 会话目录由用户自行配置上报。
- 右滑关闭:规范为"关闭按钮(或支持右滑关闭)",关闭按钮已满足。
- 抽屉视觉布局:现有两行式条目与操作列已符合规范,本次不动。

## 影响

- 测试:native OSC 9 测试新增;`SessionDrawerNarrowTest` 适配 `SessionInfo` 新形状;`AbbreviateDirectoryTest`、`SessionDrawerInstrumentedTest` 按"会话 N"文本定位不受影响;`cargo test`、`testDebugUnitTest` 通过;release APK 设备验证(cd 后抽屉目录、重开抽屉刷新、关闭后编号递增)。
