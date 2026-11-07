## ADDED Requirements

### Requirement: 部分帧只上传脏带实例

部分帧（脏带路径）MUST 只把脏带覆盖的实例切片上传到实例缓冲：字节偏移 MUST 与
绘制时使用的槽位序号同坐标系（清除段末尾 + 实例序号 × 步长），MUST NOT 上传
干净行的实例。全量帧 MUST 继续上传全部实例。

上传范围 MUST 覆盖本帧绘制的全部槽位：漏传的槽位会让 GPU 读到上一帧的实例
（画面错字或错位），MUST NOT 发生。

#### Scenario: 脏带切片按槽位偏移上传

- **WHEN** 部分帧有两个脏带，分别覆盖第 2–3 与第 10–12 个实例
- **THEN** 上传 MUST 只有两段，偏移 MUST 分别落在清除段末尾 + 2 × 步长与
      清除段末尾 + 10 × 步长（用例 `partial_instance_uploads_cover_only_dirty_bands`）

#### Scenario: 空切片与越界切片不产生非法上传

- **WHEN** 脏带的实例切片为空或整段落在本帧实例列表之外
- **THEN** 上传 MUST NOT 产生越界区间（用例 `partial_instance_uploads_skip_empty_and_out_of_range_bands`）

#### Scenario: 全量帧仍上传全部实例

- **WHEN** 本帧走全量重绘路径
- **THEN** 实例缓冲 MUST 收到整份实例，不按脏带裁剪
