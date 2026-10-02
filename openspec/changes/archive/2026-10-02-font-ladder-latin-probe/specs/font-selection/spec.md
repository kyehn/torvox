## MODIFIED Requirements

### Requirement: 主字体选择降级梯次

主字体选择 MUST 按序降级：声明的等宽文件按词干精确匹配（分隔符归一，再去空白）→
库内首个覆盖基本拉丁的等宽面（charmap 探测 `'m'`，跳过 emoji/符号这类被标记为
等宽却无拉丁字形的面）→ 库内首个覆盖拉丁的面 → 库内首个可用面；每级降级 MUST
输出日志。仅当库内没有任何可用面（等同 `fonts.xml` 不可用）时才 MUST 输出日志并
abort。`fonts.xml` 缺失或不可解析 MUST 保持 abort（DESIGN 不变）。

#### Scenario: 声明文件被 OEM 改包装

- **WHEN** `fonts.xml` 声明的等宽文件已加载但家族名与词干不匹配
- **THEN** 选择库内首个覆盖拉丁的等宽面并输出警告日志，不 abort

#### Scenario: 无拉丁字形的等宽面被跳过

- **WHEN** 库内等宽面不含拉丁字形（如 emoji 字体）
- **THEN** 该面被跳过，选择首个覆盖拉丁的面

#### Scenario: 全库无等宽面

- **WHEN** 放宽装库后库内仍无可用等宽面但至少有一个覆盖拉丁的面
- **THEN** 选择首个覆盖拉丁的面并输出错误日志，终端降级可用

#### Scenario: 库为空

- **WHEN** `fonts.xml` 全量声明文件无一可加载
- **THEN** 输出日志并 abort
