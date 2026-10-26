# Proposal

## Why

`docs/specification/STYLE.md` 要求「变量与函数一律使用完整描述性名称：禁止单字母
变量（如 `s`、`p`、`w`、`h`、`t`、`e`），禁止缩写（如 `config` 而非 `cfg`、
`background` 而非 `bg`、`application` 而非 `app`）」。原生侧已在上一轮按同一标准
清理完毕（`f158e21f`），Kotlin 侧尚未处理，扫描结果如下：

| 位置 | 现状 | 违反的条款 |
| --- | --- | --- |
| `runtime/LogUtil.kt` | `d` / `i` / `w` / `e` 四个日志函数 | 单字母变量 |
| `ui/TerminalSurface.kt` | `wm` / `vm` / `dx` / `dy` / `sr` / `sc` / `er` / `ec` / `maxR` / `maxC` / `px` / `py` / `e1` / `e2` / `loc` / `sel` / `bar` / `absX` / `absY` / `del` / `attr` / `rect` / `imm` | 单字母 + 缩写 |
| `ui/ModifierBar.kt` | `ev` / `slop` / `row1` / `row2` | 单字母 + 缩写 |
| `TerminalViewModel.kt` | `SessionInfo.id` / `switchSession(id)` / `cur` / `ids` / `dirs` / `meta` | 缩写 |
| `runtime/TerminalRuntime.kt` | `entry.id` / `switchSession(id)` / `closeSession(id)` / `px` / `fps` | 缩写 |
| `input/ComposingBaseline.kt` | `to` | 缩写 |
| `bridge/Bridge.kt` | `val id = sessionId` | 缩写 |
| `installer/*` | `abi` / `arch` / `pct` / `proc` / `bootstrapDirs` / `dirs` | 缩写 |
| `DocumentMutations.kt` / `DocumentQueries.kt` | `src` / `dstParent` / `ext` / `key` / `dir` | 缩写 |
| `bridge/Bridge.kt` / `bridge/NativeBridge.kt` | `xPx` / `yPx` | 缩写 |
| 测试代码 | `w` / `h` / `a` / `b` / `pa` / `pb` / `x` / `y` / `t` / `t0` / `dt` / `fd` / `s` / `ch` / `cw` / `h0` / `h1` / `e` / `up` / `out` / `enc` / `orch` / `frac` / `keyE` / `upAt` | 单字母 + 缩写 |

其中 `LogUtil.d` / `LogUtil.i` / `LogUtil.w` / `LogUtil.e` 尤其值得处理：它们是
全仓调用最频繁的工具函数之一，名字只有一个字母，调用点读起来是
`LogUtil.e("Runtime", ...)`，评审时既看不出级别也容易看错。

## What Changes

- 把 Kotlin 主源与测试源中的单字母标识符替换为完整描述性名称。
- 把缩写标识符（`wm`/`vm`/`sr`/`sc`/`er`/`ec`/`maxR`/`maxC`/`px`/`py`/`ev`/
  `loc`/`sel`/`bar`/`absX`/`absY`/`cur`/`ids`/`dirs`/`meta`/`abi`/`arch`/`pct`/
  `proc`/`src`/`dstParent`/`ext`/`slop`/`fps`/`imm`/`attr`/`rect`/`del`/`row1`/
  `row2`/`ch`/`cw`/`fd`/`h0`/`h1`/`t0`/`dt`/`up`/`id`/`lo`/`hi`/`sx`/`sy`/`ex`/
  `ey`/`xPx`/`yPx`/`bootstrapDirs`/`out`/`enc`/`orch`/`frac`/`keyE`/`upAt`/
  `minW`/`minH`/`maxW`/`maxH` 等）替换为完整名称。
- `LogUtil.d` / `i` / `w` / `e` 改名为 `debug` / `info` / `warn` / `error`。
- 只改标识符拼写：控制流、返回值、断言、日志文案与对外接口的形状全部不变。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无：本次只做等价重命名，不改变任何对外行为。

## Impact

- `android/app/src/main/java/terminal/emulator/**`：标识符重命名。
- `android/app/src/test/java/**`、`android/app/src/androidTest/java/**`：标识符重命名。
- 不改 `docs/specification/`、`docs/STATUS.md`、`build.gradle.kts` 等保护文件。
- 行为不变：由 Kotlin 编译、`detekt`、`spotlessCheck` 与既有单元测试保证。
