# startup-grid-resize Specification

## Purpose

确保终端网格在冷启动、Surface 重建、字体度量就绪等所有时序下都按 Surface 真实尺寸与单元格度量收敛，不残留会话 spawn 默认 24×80。

## ADDED Requirements

### Requirement: Surface 生效前到达的尺寸必须重放

`ResizeManager.applyResizeNormal` 在 `SurfaceHolder.surface` 尚未有效时 MUST 记录待处理尺寸，并在 `surfaceCreated`（Surface 生效）后重放该尺寸，MUST NOT 直接丢弃。

#### Scenario: 冷启动布局早于 Surface 就绪

- **WHEN** `onSizeChanged(1080, 2209)` 在 `surfaceCreated` 之前触发，且此时 `surface.isValid == false`
- **THEN** 尺寸被暂存；`surfaceCreated` 触发后重放，网格按 1080×2209 与单元格度量收敛（45×48），而非停在 spawn 默认 24×80

#### Scenario: 无暂存尺寸时为空操作

- **WHEN** 未发生过被丢弃的尺寸（Surface 一直有效）
- **THEN** `applyPendingSurfaceResize()` 不做任何重配置（尺寸未变时 `applySurfaceResizeNow` 早退）

### Requirement: 会话运行时重挂载 Surface 后必须重算网格

`surfaceCreated` 在会话已运行分支调用 `attachSurface(width, height)` 后 MUST 依据该尺寸重算网格，MUST NOT 仅写入 `lastConfiguredWidth/Height` 而使后续 resize 全部早退。

#### Scenario: 冷启动会话先建、Surface 后挂载

- **WHEN** 会话由 `ensureDefaultSession()` 先创建，随后 `surfaceCreated` 以 1080×2209 挂载 Surface
- **THEN** 立即以 1080×2209 重算网格并 `resize`，屏幕无下半部空白

### Requirement: 单元格度量首次可用时必须重算网格

`TerminalRuntime.syncGridDimensions` 在 `cellWidth`/`cellHeight` 由 0 变为有效值时 MUST 调用 `recomputeGridFromFontMetrics()`，以补偿启动序列中早于字体度量就绪的早退。

#### Scenario: 启动同步早于 native 字体度量

- **WHEN** `attachPendingSurface` 的 `syncGridDimensions` 读到 `cellWidth == 0f`（度量未就绪）而早退
- **THEN** 后续任一次 `syncGridDimensions` 读到有效度量时触发网格重算，网格收敛到 45×48
