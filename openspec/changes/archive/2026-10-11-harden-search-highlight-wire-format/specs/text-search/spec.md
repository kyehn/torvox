# Spec Delta

## ADDED Requirements

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
