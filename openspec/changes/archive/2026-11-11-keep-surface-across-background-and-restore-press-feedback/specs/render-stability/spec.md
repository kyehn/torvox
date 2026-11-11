# render-stability Specification

> **后续修正（2026-11-11）**：本 delta 里「切后台 MUST NOT 触发 surface 的拆除与
> 重建」与实测相反（平台在后台拆窗口 surface，返回前台必然重建交换链），已由变更
> `2026-11-11-cover-the-terminal-hole-with-the-last-frame` 撤销并更正。

## ADDED Requirements

### Requirement: 终端 Surface 的生命周期跟随挂载而非可见性

终端 SurfaceView 的 surface 生命周期 MUST 跟随**挂载**（`SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT`），
MUST NOT 跟随可见性：切后台 MUST NOT 触发 surface 的拆除与重建。

跟随可见性时，平台在切后台拆掉 surface、切回前台再重建，重建要重配交换链并重画整帧，
而新窗口拿到第一个缓冲前开孔是空的——用户看到的就是「切换后台返回终端闪烁」。同时每次
重建都会让驱动重新枚举表面格式并试分配，真机每次留下四条设备图形分配器的格式探测
error 与一串平台帧时间线的同步组查询 error。

跟随挂载后，surface 只在视图脱离窗口时销毁：切后台期间原窗口与上一帧都在，切回即原样
显示，无需重建、无需重画。该策略自 API 34 起提供；更早的版本 MUST 保持平台默认策略
（跟随可见性），MUST NOT 以其他方式模拟。

跟随挂载期间渲染 MUST 仍然暂停（见 `app-lifecycle` 的前后台暂停与恢复渲染）：后台不做
任何 GPU 工作，保留下来的只是交换链本身的显存。

#### Scenario: 切后台不拆 surface

- **WHEN** 应用切到后台
- **THEN** 终端 Surface 的原生窗口 MUST NOT 被拆除，MUST NOT 发生交换链重建

#### Scenario: 切回前台直接沿用上一帧

- **WHEN** 应用从后台回到前台
- **THEN** 终端 MUST 沿用同一原生窗口继续呈现，MUST NOT 因窗口更换而重建交换链

#### Scenario: 视图脱离窗口时 surface 仍被拆除

- **WHEN** 终端 SurfaceView 脱离窗口（Activity 销毁或宿主换视图）
- **THEN** surface MUST 按平台回调拆除，GPU 表面 MUST 随之释放

## MODIFIED Requirements

### Requirement: 切后台返回时开孔必须被覆盖且重新出帧

`SurfaceView` 的 surface 在 `onStop` 之后被系统拆除时，组合层 MUST 立即回到「无缓冲区」
状态并在窗口层涂终端底色，直到新 surface 提交首帧。该状态 MUST NOT 依赖随生命周期
停止收集的订阅方式：写入发生在 `onStop` 之后，`ON_START` 重启收集与返回后的首帧绘制
会抢跑，首帧按「有缓冲区」绘制时开孔区就透出桌面或上一个应用。

返回前台后 MUST 重新出帧，MUST NOT 停留在渲染暂停状态形成永久黑屏。

surface 跟随挂载时（API 34+），切后台不拆除 surface，开孔始终保有上一帧：覆盖状态
MUST 保持「有缓冲区」，返回前台 MUST 直接沿用同一窗口出帧，MUST NOT 重建交换链。

#### Scenario: 切后台拆除 surface

- **WHEN** 应用切到后台且系统拆除了 `SurfaceView` 的 surface
- **THEN** 开孔覆盖状态 MUST 落回「无缓冲区」，窗口层涂终端底色

#### Scenario: 返回前台首帧前不透出上一个应用

- **WHEN** 应用回到前台，新 surface 尚未提交首帧
- **THEN** 窗口层 MUST 保持终端底色覆盖，MUST NOT 透出桌面或上一个应用

#### Scenario: 返回前台后重新出帧

- **WHEN** 应用回到前台且新 surface 已挂载
- **THEN** 终端 MUST 在 30s 内重新提交并呈现帧，覆盖状态随首帧撤下

#### Scenario: surface 未被拆除时覆盖状态不翻转

- **WHEN** 应用切到后台而 surface 未被系统拆除（跟随挂载）
- **THEN** 开孔覆盖状态 MUST 保持「有缓冲区」，返回前台 MUST 直接沿用上一帧
