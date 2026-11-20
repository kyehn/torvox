## MODIFIED Requirements

### Requirement: 换新窗口触发 GPU 重建并恢复渲染

恢复 MUST 走「宿主换新 `SurfaceView` → `attachWindow` 重建 instance/adapter/device 与
`Renderer` → 清除丢失位 → 正常出帧」这一条链，MUST NOT 依赖进程重启。

`renderWithNewOutput` 的失效位 MUST 把 `device_lost` 与 `surface_invalidated` 合并上报
（同一动作：宿主换新窗口）；否则丢失后宿主等不到任何换窗口触发。重建 MUST 发生在收到
**新** `ANativeWindow` 的 `attachWindow` 内，且先于 `attach_surface`：`wgpu::Surface`
绑定 instance，旧 surface 不能配到新设备。

重建 MUST 搬运与设备无关的会话状态（暂停位、背景色、滚动偏移），MUST NOT 让恢复后的
首帧丢背景色或跳回顶部；重建失败 MUST 保持失效位并释放窗口引用，由宿主的重建预算继续
换窗口。

#### Scenario: 新窗口触发重建并恢复

- **WHEN** 丢失位已置且宿主送来新的 `ANativeWindow`
- **THEN** MUST 重建 instance/device 与 `Renderer`，清除丢失位，
      首帧 MUST 恢复出图（暂停位、背景色、滚动偏移保持）

#### Scenario: 重建失败继续换窗口

- **WHEN** 重建 instance/device 失败
- **THEN** 丢失位 MUST 保持，窗口引用 MUST 释放，失效位 MUST 继续上报，
      宿主按重建预算继续换窗口

#### Scenario: 恢复后新建会话照常出帧

- **WHEN** 设备丢失恢复后宿主新建会话
- **THEN** 新会话 MUST 复用已重建的 instance/device 并照常出帧，
      MUST NOT 挂到丢失前的死设备上（`SurfaceLossRecoveryInstrumentedTest`
      在恢复后新建会话并断言墨迹）

#### Scenario: 冷启动首帧后的设备丢失按有界恢复自愈

- **WHEN** 2026-11-20 12-45-04 真机日志（Mali-G57）出现冷启动首帧后的设备丢失：
      `first frame presented`（12:38:40.352）→ `GPU device lost`（12:38:40.357，
      首帧后 5ms；该条 error 由设备丢失回调在非渲染线程 tid 7885 上记出，
      渲染线程 tid 7887 同期只记了首帧呈现与随后的 `render skipped`）→ `render skipped`
      （12:38:40.369）→ `surface invalidated (attempt 1/5)`（12:38:40.370）→
      新窗口 `attachWindow`（12:38:40.476）→ `GPU rebuilt after device loss`
      （12:38:40.538）→ 新 surface 挂载（12:38:40.618）
- **THEN** 该次丢失 MUST 按有界恢复链自愈（换新 `SurfaceView` → 重建 instance/device →
      恢复出帧），从丢失到恢复出帧实测 261ms，MUST NOT 依赖人工干预或进程重启
- **AND** 本次丢失的底层根因 MUST 记为未定位：它发生在冷启动的正常渲染路径上
      （非切后台/换窗簇），应用配置（`PowerPreference`／present mode／
      `ANativeWindow_setBuffersGeometry`）仍无证据为成因；恢复是有界缓解而非消除，
      MUST NOT 表述为「已解决」
