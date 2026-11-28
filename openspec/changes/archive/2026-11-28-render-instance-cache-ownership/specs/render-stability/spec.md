# render-stability Specification

## ADDED Requirements

### Requirement: 实例缓存 MUST 是本帧实例的唯一副本

渲染器 MUST NOT 为同一帧的单元实例保留第二份副本：行级实例缓存既是跨帧复用的增量
基线，也是本帧提交给 GPU 的那份实例。MUST NOT 每帧把整屏实例从缓存复制进另一块缓冲
再复制回来——一屏约 100KB，60fps 下即约 6MB/s 的纯搬运，且这份搬运不产生任何画面变化。

干净行 MUST 原地保留在缓存里；脏行 MUST 就地重建并替换，MUST NOT 先把整屏拷走再回填。

#### Scenario: 渲染器不持有第二份实例缓冲

- **WHEN** 检视 `Renderer` 的字段
- **THEN** MUST NOT 存在与实例缓存并列的实例缓冲字段

#### Scenario: 增量帧不产生整屏实例拷贝

- **WHEN** 一帧只有一行变脏
- **THEN** 干净行的实例 MUST 留在缓存原位，MUST NOT 经由第二块缓冲整体搬运
      （用例 `cached_incremental_rebuild_matches_full_build`）

### Requirement: 构建走哪条路径 MUST 由构建过程回报

实例构建走增量还是全量，MUST 只在构建过程里判定一次，并由构建过程把结果回报给调用方。
调用方 MUST NOT 用自己那套条件重新推断：两处条件只要不完全等价（掩码全脏、掩码短于
行数、图集搬迁、缓存尺寸不符），推断就会与实际不一致，而不一致的后果是拿全量重建的
实例去做稀疏上传——干净行的槽位留在上一帧的陈旧实例上，画面出现错字或错位。

构建过程 MUST 回报「走了增量」与「整帧重建」两种结果；转换失败 MUST 与这两种结果区分
开，调用方 MUST 据此报错而不是继续出帧。

#### Scenario: 增量帧回报增量

- **WHEN** 一帧只标记一行脏，且缓存尺寸、图集代际与掩码都匹配
- **THEN** 构建 MUST 回报走了增量路径（用例 `cached_incremental_rebuild_matches_full_build`）

#### Scenario: 降级为全量时回报全量

- **WHEN** 缓存的图集代际已陈旧、掩码短于行数、缓存尺寸不符，或预热引发图集搬迁
- **THEN** 构建 MUST 回报整帧重建，调用方 MUST 据此放弃稀疏上传
      （用例 `cached_stale_atlas_generation_forces_full_rebuild`、
      `cached_degraded_input_falls_back_to_full_rebuild`、
      `incremental_frame_falls_back_to_full_rebuild_when_warm_evicts`）

#### Scenario: 转换失败与路径判定区分开

- **WHEN** 单元数据无法转换为实例
- **THEN** 构建 MUST 回报失败，调用方 MUST 返回错误，MUST NOT 把它当成全量重建继续出帧
