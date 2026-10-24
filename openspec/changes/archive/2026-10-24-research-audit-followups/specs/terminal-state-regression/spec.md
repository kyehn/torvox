## ADDED Requirements

### Requirement: 缩窄重排保留正文，备用屏只裁剪

主屏缩窄 MUST 按新宽度重排软换行的整段文本，字符一个不少；备用屏缩窄 MUST 只按
新宽度裁剪可见内容，MUST NOT 重排——重排会让 TUI 的逐格绘制坐标整体错位。缩窄
路径 MUST NOT 因越界而崩溃（备用屏没有回滚区可借用）。

#### Scenario: 主屏缩窄后正文完整

- **WHEN** 36 个字符在 20 列网格上写满两行，随后缩窄到 10 列
- **THEN** 全文仍包含这 36 个字符的完整序列，只是折行位置改变
      （用例 `shrink_reflow_keeps_wrapped_text`）

#### Scenario: 备用屏缩窄只裁剪

- **WHEN** 备用屏写入一行长文本后缩窄列数
- **THEN** 可见内容仍是原文的连续片段、长度等于新列宽，退出备用屏后主屏正文不变
      （用例 `shrink_alt_screen_clips_without_reflow`）
