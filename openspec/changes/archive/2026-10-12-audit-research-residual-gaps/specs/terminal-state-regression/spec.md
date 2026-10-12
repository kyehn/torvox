# Spec Delta

## ADDED Requirements

### Requirement: 网格状态断言直接针对网格输出

终端网格状态 MUST 由直接读取网格的断言覆盖：回滚区旧行顺序、输入回显与光标跟随、
简体中文显示宽度、字重与颜色文本，各 MUST 有断言其网格输出的用例，MUST NOT 仅断言
某一层的中间量或派生值。

期望值来源为本仓实现自身输出的网格快照装置 MUST NOT 引入：它把本仓的输出当成
本仓的正确答案，无法在实现出错时失败。期望值来自标准文本、上游协议文档或真机实测的
语料不在此限，按 `upstream-alignment` 的采纳门槛单独判定。

本能力的规范依据 MUST 以 `docs/specification/TESTING.md` 与 `upstream-alignment` 的实际
条款为准，MUST NOT 引用该文件中不存在的表述。

#### Scenario: 回滚区旧行按顺序断言

- **WHEN** 输出超出网格高度后回滚区被逐行读取
- **THEN** 断言逐行比对回滚区文本与顺序，新行出现在视口底部

#### Scenario: 字重与颜色断言网格呈现

- **WHEN** 终端以不同字重与前景色显示文本
- **THEN** 断言检查对应网格单元的样式与颜色，不止检查单元文本非空

#### Scenario: 不引入自验证快照装置

- **WHEN** 评估是否引入网格快照语料
- **THEN** 期望值由本仓实现自身产出的不引入，理由记为自验证；外部真相来源的语料
      单独按 `upstream-alignment` 的门槛评估

#### Scenario: 规范引用不得超出被引用文件

- **WHEN** 本能力引用 `docs/specification/TESTING.md` 作为某项取舍的依据
- **THEN** 被引用的表述在该文件中确实存在，否则改引 `upstream-alignment` 的对应条款
