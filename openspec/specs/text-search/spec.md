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
