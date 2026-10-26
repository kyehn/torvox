# Design

## 上下文

原生侧上一轮（`f158e21f`）已把 Rust 里的单字母变量与缩写统一换成完整描述性
名称，使同一套风格在两侧一致。Kotlin 侧此前未处理，`LogUtil.d/i/w/e` 这类
单字母函数名尤其醒目。

## 决策

### 1. 重命名的判定标准

只处理下列两类标识符，其余保持原样：

- **单字母**：长度恰为 1 的局部变量、参数与函数名。
- **缩写**：长度 2–4 的、由更长单词截断而成的名字（`wm`/`cfg`/`pct`/`dirs`）。

不处理下列情况：

- 已是完整英文单词或领域标准术语：`row` / `col` / `rows` / `text` / `path` /
  `file` / `uri` / `url` / `json` / `mime` / `meta`（Android API 名 `metaState`
  除外）/`tag` / `lock` / `step` / `mask` / `info` / `item` / `body` / `code` /
  `home` / `host` / `name` / `size` / `stop` / `take` / `edit` / `exit` / `next` /
  `left` / `top` / `fit` / `fits` / `hold` / `poll` / `pump` / `raw` / `from` /
  `now` / `all` / `down` / `hung` / `room` / `key`（字典键）等。
- 平台回调签名固有的参数名：`onScroll(e1, e2, …)` 的 `e1`/`e2` 可改名但必须
  保持顺序与含义，改名为 `firstEvent` / `secondEvent`。
- 枚举 `ALT` / `DAY` / `CTRL` / `TAG` 等常量名。
- `it`：Kotlin 隐式 lambda 参数，无实际可读性损失。

### 2. `id` 的处理

`id` 在 Kotlin 侧指的都是**会话标识符**（`SessionInfo.id`、`switchSession(id)`、
`entry.id`）。统一改为 `sessionId`；`DocumentsProviderTest` 里的文档 id 保持
`id`（它不是会话，且 `documentId` 会与既有 API 混淆）。

### 3. `LogUtil` 四个单字母函数

改为 `debug` / `info` / `warn` / `error`。调用点数量大但替换是无歧义的
`LogUtil.d(` → `LogUtil.debug(` 形式，用带前缀的整词替换，不触碰 `Log.d(` 之类
第三方调用。

### 4. 测试代码同样适用

`docs/specification/STYLE.md` 的「通用」节不区分主源与测试源，故测试代码里
的 `w`/`h`/`a`/`b`/`pa`/`pb`/`x`/`y`/`t`/`dt`/`fd`/`s`/`ch`/`cw`/`h0`/`h1`/
`e`/`up` 一并处理。`UxTestUtils.kt` 的像素比较辅助函数额外抽出一个共用的
「取同尺寸两图像素数组」helper，消除 `changedPixelCount` 与
`changedBoundingBox` 之间重复的 8 行取像素样板。

### 5. 不做的事

- 不改函数签名中参数的**类型与顺序**，只改名字。
- 不改日志文案、断言文案、异常类型与抛出位置。
- 不借机重构逻辑：任何行为差异都属于本变更范围之外。
- 不动 `docs/specification/`、`docs/STATUS.md` 等受保护文件。

## 风险与对策

| 风险 | 对策 |
| --- | --- |
| 整词替换误伤字符串或注释 | 逐处人工核对替换点；日志/断言文案保持原样 |
| `id` 替换误伤其他语义 | 只在会话语义的文件内替换；文档提供器的 `documentId` 不动 |
| 重命名遗漏调用点 | 以 Kotlin 编译失败为准，逐个补齐；再跑 `detekt`（含未使用声明检查） |

## 验证

- `:app:compileDebugKotlin` 与 `:app:compileDebugUnitTestKotlin` 零错误。
- `:app:testDebugUnitTest` 全绿。
- `:app:detekt` 零告警（含 `UnusedPrivateMember`、`UnusedImport`）。
- `:app:spotlessCheck` 通过。
- 全仓再扫一次短标识符，确认无残留。
