## ADDED Requirements

### Requirement: 增量的 UV 必须按当前图集解释

干净行复用的是**上一帧构建好的实例**，实例的 UV 只在图集搬迁后失效，而搬迁随时可能发生
（本帧脏行把干净行的字形挤出图集、抢占式预习的第一遍本身也会搬迁）。故单元构建 MUST
先预热本帧全部字形、再在代际不再推进的前提下发射，并且 MUST 只把「全程未发生搬迁的那一
遍」写回实例缓存；任一趟发射推进了代际时 MUST 降级为全量重建，MUST NOT 把搬前的 UV 连同
搬后的代际一起存入缓存——下一帧的增量判定会认这个代际为「未变」，干净行照着已被别人占用
的区域采样，同一个字符在屏幕上画出两种字形。

反之，若工作集本身大过图集容量，则遍数再多也不可能达成「不搬迁的一遍」（每次查询都在挤
走另一个字形）。此时 MUST 出声而 MUST NOT 静默无限重发射：实例表当场可用但 UV 无法自证
一致，屏幕上的字会在字符之间 variant，而唯一的归因线索就是这条告警。告警 MUST
按 logcat-hygiene 限频，且 MUST NOT 把那种帧写入实例缓存。

#### Scenario: 帧内搬迁后 UV 仍按当前图集解释

- **WHEN** 一帧里干净行沿用旧字形、脏行引入的新字形把前者挤出图集
- **THEN** 该帧每个实例的 UV MUST 等于当时字形缓存给出的位置，
      MUST NOT 沿用搬迁前的旧坐标（用例 `incremental_frame_with_midframe_eviction_has_current_uvs`）

#### Scenario: 写回缓存的那一趟没有搬迁

- **WHEN** 某一趟发射过程中图集代际发生了变化
- **THEN** MUST 全量重发一遍，且只有末趟代际未变时才写回实例缓存；
      一路不收敛时 MUST NOT 留下任何缓存条目

#### Scenario: 工作集超过图集容量时告警

- **WHEN** 一帧的去重字形数在图集里放不下，任意一遍发射都会挤走另一个字形
- **THEN** MUST 限频记一条 warn 说明图集在抖动，且 MUST NOT 写回实例缓存；
      MUST NOT 无限重发射——那是停顿而非修复

### Requirement: 字形缓存的取证 MUST 落在承载查表的缓存上

`(面, 码位)` 查表缓存的容量 MUST 容得下一屏之内的不同码位。取证 MUST 直接落在该缓存上，
MUST NOT 经 `glyph_information_styled` 间接取证：主字体覆盖汉字时该函数在更早的分支上
返回，查表一步都不走，间接断言会退化成「0 == 0」而仍然判绿。

#### Scenario: 一屏码位全部留存

- **WHEN** 同一面上查询一屏（200）个不同码位
- **THEN** 该缓存 MUST 恰好留下这 200 条，MUST NOT 因容量穿透而丢掉最久未用的那条

### Requirement: 字形 bearing 必须与 bearing 契约同源

实例 bearing MUST 等于 `glyph_bearing_y` 的结果：位图不高于单元格时取
`ascent × raster_scale − placement.top`（MUST NOT 居中），高于单元格时取居中回退
(`cell_height − glyph_height) / 2`。两个用例 MUST NOT 各执一端——那会让其中一侧在宿主
字体度量不同的环境里必然判红，而红的那侧并不反映实现是否漂移。

#### Scenario: 汉字位图高于单元格时按回退居中

- **WHEN** 宿主 CJK 字面的位图高度大于单元格高度（实测 16px / 14px）
- **THEN** 实例 bearing MUST 等于居中回退值，MUST NOT 被断言为基线值

#### Scenario: 位图不高于单元格时不居中

- **WHEN** 位图高度不大于单元格高度
- **THEN** 实例 bearing MUST 等于基线值，MUST NOT 居中
