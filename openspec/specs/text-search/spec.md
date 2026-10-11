# text-search Specification

## Purpose

终端文本搜索的能力边界。搜索须走外部 `regex` 库而非手写大小写折叠循环，命中规模与
查询长度须有上限以免拖住输入线程。

## Requirements

### Requirement: 搜索用外部 regex 库编译字面模式

`search_in_scrollback_all_impl` MUST 用外部 `regex` 库把查询编译为字面搜索模式
（`regex::escape` + `RegexBuilder`），大小写开关交由库的 `case_insensitive` 承载，
MUST NOT 手写逐字符大小写折叠循环。命中按 `find_iter` 取全部非重叠区间，字节偏移
MUST 转为字符列后才作为行列上报。

#### Scenario: 大小写开关生效

- **WHEN** 以不区分大小写搜索 `abc` 且回滚含 `ABC`
- **THEN** 命中该行；开启区分大小写则不命中

#### Scenario: 查询按字面处理

- **WHEN** 查询含正则元字符（如 `a.b`）
- **THEN** 按字面匹配，不把 `.` 当通配

### Requirement: 查询长度与命中规模有上限

查询字符数 MUST 不超过 `MAX_SEARCH_QUERY_CHARS`（128），超出直接无命中，避免正则
引擎与全回滚扫描的无谓开销。返回的可导航命中 MUST 不超过
`MAX_NAVIGABLE_MATCHES`（50,000，对标上游 50k 窗口），超出时保留最新的命中。

#### Scenario: 超长查询无命中

- **WHEN** 查询长度超过 128 字符
- **THEN** 返回空命中列表，不进入扫描

#### Scenario: 超量命中保留最新

- **WHEN** 回滚中的命中数超过 50,000
- **THEN** 返回最新一段命中，上一个/下一个可导航

### Requirement: 搜索高亮线格式的畸形输入必须被拒绝并出声

搜索高亮字节流（Kotlin `TerminalScreen` 打包、`NativeBridge.setSearchHighlights` 接收）
的解析 MUST 判定为显式错误而非静默容错。以下四类输入 MUST 被识别为错误并出声
（`log::error!`），MUST NOT 静默返回或静默截断：

| 畸形类别 | 判据 |
| --- | --- |
| 长度不足前缀 | 字节数 < 4（无法读出计数） |
| 计数为负 | 前 4 字节按 i32 LE 解释得负数 |
| 计数超出记录数 | 计数 > `(总长 - 4) / 16` |
| 尾部残缺记录 | 声明的条数均已凑满，但负载长度不是 16 的整数倍 |

判定顺序 MUST 为上表顺序：计数超出记录数是更根本的口径不一致，先于尾部残缺判定。
故「声明 2 条而只有 1 条完整记录加残缺尾巴」归入计数超出记录数，而非尾部残缺。

错误 MUST 以可区分的变体表达（不同错误类型/字段），MUST NOT 折叠为单一「解析失败」，
否则记录写不出是哪类输入出错。合法载荷 MUST 解析出与字节流逐字段相等的记录。

MUST NOT 在 JNI 边界 panic 或 abort：那会终止宿主应用且无法单测。错误 MUST 以日志
形式出声，使「高亮没画出来」可归因。

#### Scenario: 前缀不足四字节

- **WHEN** 传入 0 或 3 字节的载荷
- **THEN** 判定为前缀不足错误并记 error；MUST NOT 当作零条高亮静默返回

#### Scenario: 负计数不被抹平为 0

- **WHEN** 前 4 字节是 `0xFFFFFFFF`
- **THEN** 判定为负计数错误并记 error；MUST NOT 经 `max(0)` 变成「无高亮」而退出

#### Scenario: 计数超出实际记录数

- **WHEN** 前缀声明 5 条而负载只有 2 条完整记录
- **THEN** 判定为计数超出记录数错误并记 error；MUST NOT 静默取前 2 条继续渲染

#### Scenario: 尾部残缺记录

- **WHEN** 前缀声明 2 条而负载有 1 条完整记录加 7 字节残缺尾巴
- **THEN** 判定为尾部残缺错误并记 error；MUST NOT 只解析 1 条后静默 break

#### Scenario: 合法载荷逐字段解析

- **WHEN** 前缀声明 2 条且负载含 2 条完整记录
- **THEN** 解析出 2 条 `SearchHighlight`，其 `row`/`start_col`/`end_col_exclusive`/`color`
      与字节流逐字段相等

#### Scenario: 零条高亮的合法空载荷

- **WHEN** 载荷恰为计数 0 的 4 字节
- **THEN** 解析成功得到空列表（清除全部高亮是合法操作，不是畸形输入）

### Requirement: 定位保留上下文

上一个/下一个命中定位 MUST 在视口内保留上下文余量，被定位的命中行 MUST NOT 紧贴视口边缘，滚动后命中行上下保留可读上下文。

#### Scenario: 定位留上下文

- **WHEN** 通过下一个跳到位于屏幕外的命中
- **THEN** 视口滚动后该命中行不在首行或末行，上下保留上下文行

### Requirement: 导航不触发重扫

命中导航（上一个/下一个）MUST 复用已缓存的命中表，MUST NOT 因导航重新扫描回滚区；仅查询文本、大小写开关或终端内容变化时才重扫。

#### Scenario: 连续导航不重扫

- **WHEN** 连续点击下一个三次且期间无新输出
- **THEN** 三次定位均来自同一命中表，不重新扫描
