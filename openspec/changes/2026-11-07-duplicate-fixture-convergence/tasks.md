# 任务

## 一、重复率

- [x] 用固定参数（`--min-lines 12 --min-tokens 60`）复扫 `native/src` 与 `android/app/src`：
      扫描前 11 处 / 191 行 / 0.23%。
- [x] 逐条判定扫描前的 11 处：3 处真重复收敛，8 处判定保留并写明理由。
- [x] 收敛 `FailsafeSessionRequestTest` 与 `NotificationSessionSwitchTest` 共用的运行期假件
      与 ViewModel 构造夹具（`TerminalViewModelFixture.kt`）。
- [x] 收敛 `ModifierBarRobolectricTest` 两条抽屉用例共用的键栏夹具。
- [x] 更严阈值（`--min-lines 5 --min-tokens 50`）复扫：另收敛前台服务用例的「建服务 →
      启命令」与通知用例的「装切换 mock + 挂信号」两处。
- [x] 更严阈值的其余 100 处命中逐条分类判定：import 块、用例脚手架、短断言序列、
      参数/类型清单与解析器分支重复体，无真重复，理由写进留档。
- [x] 复扫确认：8 处 / 127 行 / 0.15%。
- [x] 留档 `audit.md` 记录扫描参数、前后对比与逐条判定。

## 二、规约

- [x] `code-health` ADDED「审计留档必须覆盖当前代码树」。

## 三、验证

- [x] `:app:testDebugUnitTest` 全量通过（相关夹具涉及的 5 个用例类单独复跑通过）。
- [x] `detekt`、`spotlessCheck` 通过。
- [x] `openspec validate --all --strict` 通过。
- [x] 归档本 change 并同步主规约。
