## Purpose

确保终端会话在 spawn 时即拥有正确的像素级 winsize，避免首帧折行与后续 resize 闪烁，支撑 90fps 稳定渲染。

## ADDED Requirements

### Requirement: Spawn 前同步 Winsize

系统 SHALL 在 `Session::spawn` 前基于当前 `PixelSize` 与 `CellSize` 预计算 `rows/cols`，并通过 `pty.resize` 同步设置，确保首次 `read` 前 winsize 已就绪。

#### Scenario: 首次启动无折行

- **WHEN** 用户在 1080x2400 设备以默认字体 (cell 12x24) 启动会话
- **THEN** 首帧渲染的 `rows == floor(availableHeight / cellHeight)` 且 `cols == floor(availableWidth / cellWidth)`，`stty size` 返回值与渲染网格一致

#### Scenario: 横竖屏旋转同步

- **WHEN** 设备从竖屏旋转至横屏且像素尺寸变化
- **THEN** 系统在 `Surface` 重建后 50ms 内调用 `setPixelSize` 并更新 `winsize`，`stty size` 变化与 UI 无黑屏

#### Scenario: 像素尺寸为零时不崩溃

- **WHEN** `setPixelSize` 传入 0x0（初始化竞态）
- **THEN** 系统 SHALL 保留上一次有效 winsize，不触发 `pty.resize`，并记录 debug 日志

### Requirement: Pixel 与 Cell 换算确定性

系统 SHALL 使用 `cellWidth/Height` 来自渲染管线的实时值（`fontInfo.cellWidthPx`），而非硬编码 6x12，确保高分屏与 CJK 回退字体下换算一致。

#### Scenario: CJK 回退字体换算

- **WHEN** 主字体不支持 CJK 且回退至 `Noto Sans CJK SC`
- **THEN** `cols` 计算使用回退后的 `cellWidth`，终端不出现半字符截断
