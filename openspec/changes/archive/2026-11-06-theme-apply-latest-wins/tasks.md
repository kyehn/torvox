# Tasks

## 一、实现

- [x] 1.1 `TerminalRuntime` 增补渲染契约下发的串行闸（`renderContractMutex`），
      两条下发路径（`applySettings`、`applyRenderSettings`）在同一把闸内下发
- [x] 1.2 闸内重读最新设置快照再解析主题名（`buildRenderContract`），
      字号与字族由调用方给定（建会话路径用 spawn 时的那一组）
- [x] 1.3 主题名不再随会话配置流转：`buildConfig()` 不再解析主题，
      `TerminalConfig` 去掉 `themeName`，强调色改由闸内设置
- [x] 1.4 `applySettings()` 不再调用 `buildConfig()`：字号与字族在闸内取，
      顺带免掉该路径上的前缀 Shell 探测与 `~/.mkshrc` 覆写

## 二、验证

- [x] 2.1 复现并定位：logcat 证实四个并发下发里两个带旧快照、两个带新快照，
      落地顺序由线程调度决定
- [x] 2.2 仪器化 `ThemeFollowSystemInstrumentedTest` 连跑 4 轮全绿，
      且 logcat 里同一批并发下发解析出的主题名全部为最新主题
- [x] 2.3 `:app:testDebugUnitTest` 全量通过
- [x] 2.4 `detekt`、`spotlessCheck` 通过
- [x] 2.5 `openspec validate --all --strict` 通过
- [x] 2.6 markdownlint 对本变更目录 0 issues
- [x] 2.7 新增仪器化回归用例 `ThemeApplyOrderInstrumentedTest`：以下发序列的单调性
      为判据，修复前判红、修复后连跑 3 轮全绿
