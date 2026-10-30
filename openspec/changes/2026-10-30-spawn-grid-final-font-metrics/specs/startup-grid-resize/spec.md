## MODIFIED Requirements

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
