# Proposal

## Why

终端主题的并发下发会让**旧快照**压过**新快照**，终端停在用户早已改掉的主题上。

主题设置有五个入口（固定主题、日间主题、夜间主题、日夜切换开关、软件主题），
每个入口落盘后都调 `TerminalRuntime.applySettings()` 重下发主题。五个入口各起一条
IO 协程，彼此无序：每条协程在**自己被调度到的那一刻**读一次设置快照，按该快照
解析出主题名后下发。

于是同一次连续改设置会产生多个**新旧不一**的主题名，下发顺序由线程调度决定，
最后落地的那个未必是最新设置解析出来的那个。

实测 logcat（`ThemeFollowSystemInstrumentedTest` 的 setUp 连续改四个主题设置）：

```text
setTheme: dracula_plus      # 旧快照（theme_mode 尚未落盘，仍是 fixed）
setTheme: dracula_plus      # 旧快照
setTheme: catppuccin_latte  # 新快照
setTheme: catppuccin_latte  # 新快照
```

四条下发落在四条不同的线程上，落地顺序随机：收尾是 `dracula_plus` 时终端停在
默认夜间主题（日间底色判据只命中 345 像素，低于 2000 的下限），用例判红；
收尾是 `catppuccin_latte` 时通过。同一次改动的成败**全看运气**。

建会话路径（`applyRenderSettings`）与设置变化路径（`applySettings`）同样并发，
两者各自读到的快照也可能新旧不一，互相覆盖。

## What Changes

- 渲染契约（主题 + 光标样式 + 字体）的下发收进一条**串行闸**：
  - 两条下发路径（设置/配置变化、建会话）在同一把闸内串行下发，一次下发不会插进
    另一次下发的中间；
  - 闸内**重读**最新设置快照再解析主题名——后到的一次拿到的是不早于前一次的
    快照，故最后一次下发的就是最新主题。
- 主题名不再随会话配置（`TerminalConfig`）流转：`buildConfig()` 不再解析主题，
  主题名只在闸内解析一次，消灭「快照的载体」这一漂移来源。
- `applySettings()` 不再调用 `buildConfig()`：它只需要字号与字族，两者在闸内取，
  顺带免掉这条路径上的前缀 Shell 探测与 `~/.mkshrc` 覆写。

## Capabilities

### Modified Capabilities

- `theme-configuration`：主题应用 MUST 以最新设置为准，并发下发不得让旧快照压过
  新快照；新建会话 MUST 使用当前生效的主题。

## Impact

- Kotlin：`TerminalRuntime.kt`（串行闸、闸内解析主题、去掉主题在会话配置里的流转）、
  `Bridge.kt`（`TerminalConfig` 去掉 `themeName`）、两处测试夹具。
- 测试：新增仪器化回归用例 `ThemeApplyOrderInstrumentedTest.kt`，以主题下发序列的
  单调性为判据。
- 行为变化：并发的主题/配置变更不再出现「旧主题压过新主题」；其余主题行为不变。
- 不改依赖、不改协议、不改设置项、不改原生侧。
