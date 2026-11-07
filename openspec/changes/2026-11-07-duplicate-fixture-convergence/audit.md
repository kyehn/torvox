# 审计留档

本轮只查重复率一项（依赖健康度与 vendor 核对见
`openspec/changes/archive/2026-11-05-code-health-audit/audit.md`，2026-11-05 的结论
在本轮未复核：直接依赖未增删、未改版本）。

## 一、重复率

### 方法与参数

```bash
npx --yes jscpd@4 --min-lines 12 --min-tokens 60 \
  --ignore "**/build/**,**/target/**,**/node_modules/**" native/src android/app/src
```

参数与 2026-11-05 的留档一致，前后数字可直接对照。扫描范围为 `native/src` 与
`android/app/src`（含主源码、单测、仪器测试与资源 XML）。

### 结果与处置

| 范围 | 扫描前 | 扫描后 | 处置 |
| --- | --- | --- | --- |
| 全部 | 11 处 / 191 行 / 0.23% | 8 处 / 127 行 / 0.15% | 收敛 3 处（另在更严阈值下收敛 2 处） |

### 逐条判定：扫描前的 11 处

1. **`ModifierBarRobolectricTest.kt:291 ↔ :266`（14 行 / 113 词）——真重复，已收敛。**
   两条抽屉用例各抄了一份「只计数抽屉键与粘贴键」的键栏搭建。收进同文件的
   `setDrawerCountingBar` + `DrawerCounters`：两条用例断言的是**互斥**行为（长按粘贴 /
   轻点开抽屉），计数口径必须同源——两份拷贝里任一份漏接回调，「长按不得打开抽屉」与
   「轻点不得粘贴」就会因为计数器没动而假绿。
2. **`FailsafeSessionRequestTest.kt:19 ↔ NotificationSessionSwitchTest.kt:17`
   （26 行 / 97 词）——真重复，已收敛。**
3. **`FailsafeSessionRequestTest.kt:147 ↔ NotificationSessionSwitchTest.kt:130`
   （27 行 / 222 词）——真重复，已收敛。**
   上面两处是同一份夹具的两半：运行期假件（会话表）与 ViewModel 构造（设置仓库）。
   收进 `TerminalViewModelFixture.kt` 的 `createTestRuntime` / `createTestViewModel`：
   「已有会话」在两处用例里必须意味着同一份运行期状态，两份拷贝一旦分叉，断言便各自为政。
4. **`DrawerSheetInsetRobolectricTest.kt:9 ↔ SessionDrawerNarrowTest.kt:9`（21 行）——保留。**
   命中段是 import 块加 Robolectric 用例脚手架（`@RunWith`/`@GraphicsMode`/`@Config`
   与 rule 声明）。两个用例的被测对象不同（`DrawerSheet` 的底色内缩 vs `SessionDrawer`
   的窄屏布局），可共享的只有外壳；把外壳抽出来会把「这条用例在测什么」藏进辅助函数。
5. **`ShellPtyInstrumentedTest.kt:1 ↔ VtCorrectnessInstrumentedTest.kt:1`（24 行）——保留。**
   import 块加类注释。import 列表天然相似，合并只能靠通配导入或门面对象，换来的是
   可读性与静态检查的退化；类注释是各自的判据说明。
6. **`ImePopupPixelInstrumentedTest.kt:6 ↔ SurfaceLossRecoveryInstrumentedTest.kt:7`
   （13 行）——保留。** import 块，理由同第 5 条。
7. **`HardKeyboardMultipleArrivalTest.kt:7 ↔ StickyCtrlInterruptInstrumentedTest.kt:6`
   （21 行）——保留。** import 块加类注释，理由同第 5 条。
8. **`BackspaceLatencyTest.kt:1 ↔ CjkComposingLatencyTest.kt:1`（15 行）——保留。**
   import 块，理由同第 5 条。
9. **`RenderPauseSemanticsTest.kt:3 ↔ VtCorrectnessInstrumentedTest.kt:6`（14 行）——保留。**
   import 块加类注解，理由同第 4 条。
10. **`launcher_monochrome.xml:8 ↔ logo_art.xml:18`（13 行）——保留。**
    两枚图标资源共用 `<vector>` 头与 `<group>` 变换；沿用 2026-11-05 的判定。
11. **`selection_search.rs:743 ↔ :661`（14 行）——保留。**
    两条钉的是**不同不变式**的 Kitty 放置回归（直接放置通道 vs 提示符行 scrollClear 路径），
    各自的断言消息即是文档；沿用 2026-11-05 的判定。

### 更严阈值下额外收敛的两处

阈值降到 `--min-lines 5 --min-tokens 50`（扫描前 103 处 / 1.06%，扫描后 100 处 / 1.02%）
时另有两处命中虽未进入上表，但属真重复，一并收敛：

- **`TerminalForegroundServiceTest.kt` 内三条用例各抄一份「建服务 → 启命令」**（8–12 行）。
  收进 `startControllerWith`，既有的 `startWith` 改为复用它。需要 `destroy()` 收尾的用例
  拿不到服务实例就无从收尾，这正是当初各自抄一份的原因；抽出控制器后理由消失。
- **`NotificationSessionSwitchTest.kt` 两条用例各抄一份「装切换 mock + 挂信号」**（10 行）。
  收进 `createSwitchRecordingRuntime`：「切换是否发生」是这两条用例的唯一判据，信号口径
  一旦分叉，断言就会等一个不会来的信号。

### 更严阈值的其余命中

100 处命中按片段内容分类：import 块 38 处，其余 62 处逐条读过，全部是用例脚手架、
短断言序列、参数/类型清单与解析器分支的重复体，没有真重复。判定保留的理由：

- **用例脚手架与短断言序列（5–11 行）**：每条用例的布置就是这条用例的判据前提，抽出后
  「这条用例到底测什么」不再一目了然；抽象收益低于信息损失。
- **参数/类型清单**（如 `native/src/android/ffi/session.rs:69 ↔ :16`）：JNI 导出的类型
  边界清单，收敛只能靠类型别名。
- **解析器分支的重复体**（如 `native/src/terminal/ghostty_terminal/internal/input_filter.rs:118
  ↔ :89`）：「找到结束位置 → 拷贝 → 推进游标」的分支体，收敛需引入捕获可变状态的闭包或
  辅助函数，而各分支的 `find_string_end` 参数（是否含中间字节）本就不同，合并反而掩盖差异。

## 二、外部依赖与 vendor

本轮未增删依赖、未改版本，`Cargo.toml` 与 `build.gradle.kts` 均未改动，2026-11-05 的
依赖健康度结论继续适用。仓内仍无 `vendor/`、无 `.gitmodules`、无上游 Zig 源码副本。
