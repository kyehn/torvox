## ADDED Requirements

### Requirement: 切换活动会话清空全部会话态

切换活动会话 MUST 是一次完整的状态交割：上一会话留在全局会话态里的选择态、
搜索态、滚动偏移与输入目标 MUST 全部清空，渲染侧 MUST 丢弃非本会话的残留
（高亮框、Kitty 图集、末帧脏基线），新会话 MUST NOT 看到任何上一会话的可视状态。

会话态按会话键控而非全局单槽：跨会话的渲染残留以会话标识为键，进入新会话时
仅丢弃非本会话条目，MUST NOT 以「清空全部」代替（那会让同帧并存的合法条目
被误删，且掩盖键控缺失）。

#### Scenario: 切换后选择态与搜索态归零

- **WHEN** 在会话 A 有选区、搜索栏打开且滚动偏移非零时切换到会话 B
- **THEN** 选区清空、搜索栏关闭、滚动偏移归零，B 的画面不含 A 的选择高亮
      与搜索命中框

#### Scenario: 输入目标随会话切换

- **WHEN** 切换活动会话
- **THEN** 后续输入写入新会话的 PTY，MUST NOT 落入上一会话

#### Scenario: 异会话渲染残留被丢弃而非误删

- **WHEN** 渲染侧同时存在本会话与异会话的残留条目
- **THEN** 仅丢弃异会话条目，本会话条目保留

对照口径：ghostty-android-terminal `attachSession` 的跨会话清理清单（结束选区、
关搜索、清图缓存、重置输入、滚动归零）在本表的实现位置为：选择态清零
（`TerminalViewModel` 会话切换路径）、搜索栏关闭与滚动归零（`TerminalScreen`
的活动会话副作用）、图缓存与异会话渲染残留丢弃（`ffi.rs` `SessionScoped::enter_session`，
Kitty 组件按会话键控）、输入目标重置（`TerminalRuntime` 的 `inputTargetSessionId`）。
