## ADDED Requirements

### Requirement: 增量渲染与全量渲染输出一致

脏带（部分）路径只决定「哪些行重画」，MUST NOT 改变行内容：同一份单元数据与光标
下，增量构建的输出 MUST 与全量构建逐字节一致。判据落在实例输出上——CPU 层一致后，
GPU 侧只多出「画哪些行」，其像素正确性由滚动残留与脏带平铺范围用例分别覆盖。

#### Scenario: 只标一行脏的帧等于全量帧

- **WHEN** 先以全脏掩码填充行缓存，再对同一份单元数据只标记一行脏
- **THEN** 增量输出与全量输出逐字节相同（用例 `cached_build_matches_full_build_byte_for_byte`）

#### Scenario: 光标行参与增量

- **WHEN** 光标所在行被标脏
- **THEN** 该行连同光标四边形一并重建，输出仍与全量一致
