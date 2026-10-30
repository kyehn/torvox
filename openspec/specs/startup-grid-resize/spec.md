# startup-grid-resize Specification

## Purpose

确保终端网格在冷启动、Surface 重建、字体度量就绪等所有时序下都按 Surface 真实尺寸与单元格度量收敛，不残留会话 spawn 默认 24×80。

## Requirements

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

### Requirement: PTY 建立时即使用真实网格

`TerminalRuntime` 在 spawn PTY MUST 先用原生字体度量与已知 Surface 像素尺寸算出真实网格（`initialSpawnGrid`），MUST NOT 先按默认 24×80 建立 PTY 再依赖 attach 之后的 resize 纠正；仅当度量或尺寸尚未就绪时退回默认网格。

用于计算网格的字体度量 MUST 取自**已经落定**的字体契约：字族、字号与光栅尺度 MUST 在网格计算之前下发。这三项是全局量（原生只维护一份字体管线，与会话无关），会话建立之前即可下发。

否则初始 winsize 按默认字体度量算出，真实字体会让 attach 之后多出一次 resize：shell 早已打印的 prompt 因 SIGWINCH 被重排，首屏多出一行输入提示符，视口多出一行内容时 prompt 还会被挤出可见区。

#### Scenario: 度量已就绪时新建会话

- **WHEN** 原生字体度量可用（26.4×53.2 px 单元格）、Surface 为 1080×2209、修饰键栏 132 px，新建会话 spawn
- **THEN** PTY 的 winsize 为 39×40，shell 的第一个 prompt 按 40 列排版；attach 之后的首次 resize 零变更

#### Scenario: 度量或尺寸尚未就绪

- **WHEN** `cellWidth`/`cellHeight` 仍为 0，或 Surface 尚未布局（宽或高为 0）
- **THEN** `initialSpawnGrid` 返回默认 24×80（与旧路径一致），零高度不会退化成 1 行 PTY；度量就绪后由 `syncGridDimensions` 重算并 resize

#### Scenario: 字号或字族与原生默认字体不同

- **WHEN** 用户设置的字号（或字族）与原生默认字体不同
- **THEN** spawn 出的初始 winsize 即为最终网格，attach 之后零变更，shell 不收到 SIGWINCH，首屏 prompt 不重复
