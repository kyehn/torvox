# render-stability Specification

## Purpose

渲染稳定性基线：字形首帧完整性、IME 动画期间输出完整呈现、滚动无撕裂、启动首帧
及时呈现。本文件是 2026-09-20 那次变更的能力快照（归档件）；当前生效的规范见
`openspec/specs/render-stability/spec.md`。

## Requirements

### Requirement: 字形首帧完整性

任意 glyph（含合成斜体/粗体）首次渲染 MUST 完整可见，MUST NOT 缺失部分字形；
重复渲染结果 MUST 与首次一致（幂等）。MUST NOT 以 advance 宽度裁剪合成斜体位图的
有效部分（右上凸出）。

实现细节（任务 1）：

根因：`render_inner` 的 atlas 脏区上传（Phase 1）先于实例构建/字形光栅化
（Phase 3）执行；`render_cell_data` 内整帧构建时新光栅化的字形只写入
CPU 侧 `atlas_bitmap` 并登记 `dirty_rect`，GPU 纹理在本帧仍是空的。Idle
门控在无输入时不触发重绘，因此缺失字形一直保留到下次输出（“重复执行才
完整”的假象）。取证基线：14px 斜体合成字形位图 9px 宽、advance 8.43px，
右上悬空 0.57px 属字体固有 overhang，被 shader 的 advance 盒裁剪，
与 ghostty/termux 一致，不修 shader（既有测试 pin 住 ≤1px 边界）。

修复：`render_cell_data` 在实例构建完成、绘制命令提交之前补传脏区
（`take_dirty_rect` → `upload_atlas`）。`write_texture` 以调用顺序入队，
先于本帧绘制命令执行，保证首帧即按完整字形采样。ffi.rs Phase 1 上传
保留（覆盖 JNI 线程外部光栅化）。

单测：`first_build_leaves_pending_dirty_rect_covering_all_glyphs`
（构建后脏区覆盖全部新字形，幂等）、
`repeat_build_is_identical_and_produces_no_new_dirty_rect`
（重复构建命中缓存：无新脏区、实例逐字节一致、已上传像素不变）。

设备验证：`cat italic_test.txt` 输出首帧 OCR——修复前输出区 0 像素
（字形缺失，含一次 BACK 重绘后仍缺失），修复后完整读出
`italic ddd` / `abcdefghijklmnop`。

#### Scenario: 合成斜体首帧完整

- **WHEN** 终端首次渲染含合成斜体或粗体的字形
- **THEN** 该字形完整可见，MUST NOT 缺失部分笔画

#### Scenario: 重复渲染幂等

- **WHEN** 同一字形被重复渲染
- **THEN** 渲染结果与首次一致，MUST NOT 出现差异

### Requirement: IME 渲染恢复

IME 弹出/隐藏动画期间产生的 PTY 新输出，MUST 在 IME 稳定后完整呈现。渲染暂停
MUST 配对恢复，恢复后首选帧 MUST 强制重绘。

#### Scenario: IME 动画期间的输出完整呈现

- **WHEN** IME 弹出或隐藏动画期间 PTY 产生新输出
- **THEN** IME 稳定后该输出完整呈现，MUST NOT 缺段或错行

#### Scenario: 渲染暂停配对恢复

- **WHEN** 渲染因 IME 动画暂停后恢复
- **THEN** 首选帧强制重绘，MUST NOT 停留在暂停前的画面

### Requirement: 滚动流畅度

滚动渲染 MUST NOT 出现可见卡顿或撕裂。

#### Scenario: 滚动无撕裂

- **WHEN** 用户上下滚动终端内容
- **THEN** 渲染无可见卡顿与撕裂

### Requirement: 启动首帧

应用启动后终端内容 MUST 在合理时间内呈现，MUST NOT 长时间黑屏。

#### Scenario: 启动后及时呈现终端内容

- **WHEN** 应用冷启动
- **THEN** 终端内容在合理时间内呈现，MUST NOT 长时间黑屏
