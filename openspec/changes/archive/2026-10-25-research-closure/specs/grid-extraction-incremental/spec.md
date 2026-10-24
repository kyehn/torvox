## ADDED Requirements

### Requirement: resize 使行布局失效后整缓存重建

网格尺寸变化使行布局失效时，行缓存 MUST 整缓存重建，输出实例数 MUST 等于当前网格的
单元数，MUST NOT 把旧网格的行残留在输出里。缓存容量按需增长且不复原（不足才重建），
因此「不累积」的判据是**输出内容**而非容量数字。

#### Scenario: 网格来回切换不累积旧行

- **WHEN** 网格在多种行列组合间反复变化，每次变化都重建一次
- **THEN** 每次输出的实例数都等于当前网格单元数（用例 `resize_cycles_do_not_accumulate_stale_rows`）

#### Scenario: 缩窄后不残留旧宽度的行

- **WHEN** 网格从宽变窄再变宽
- **THEN** 变宽后的实例数等于新网格单元数，MUST NOT 出现旧网格遗留的额外行
