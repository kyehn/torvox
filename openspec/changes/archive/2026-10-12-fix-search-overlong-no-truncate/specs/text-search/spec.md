## MODIFIED Requirements

### Requirement: 查询长度与命中规模有上限

needle 字符数 MUST 不超过 `MAX_SEARCH_QUERY_CHARS`（128），超出直接判为无命中，
MUST NOT 截断——截断会让用户搜到另一个串的结果。命中规模上限以上游为准（上游
50k 窗口），本仓 MUST NOT 另设一份。

UI 侧 MUST NOT 用 `take` 截断查询串：超长查询 MUST 在 UI 侧直接判为无命中
（`resultCount = 0`），MUST NOT 送往原生。查询框仍显示用户输入的原文。

#### Scenario: 超长查询无命中

- **WHEN** 查询长度超过 128 字符
- **THEN** 返回 0 命中并出声记录，不进入扫描

#### Scenario: 超长查询不截断不送原生

- **WHEN** 用户在搜索框输入超过 128 字符的查询
- **THEN** UI 侧直接判为 0 命中，不调用原生搜索，查询框保留原文；
  用户看到的是「无命中」而非截断串的命中
