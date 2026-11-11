# render-stability Specification

## MODIFIED Requirements

### Requirement: 切后台返回时开孔必须被覆盖且重新出帧

`SurfaceView` 的 surface 在 `onStop` 之后被系统拆除时，组合层 MUST 立即回到「无缓冲区」
状态并在窗口层涂终端底色，直到新 surface 提交首帧。该状态 MUST NOT 依赖随生命周期
停止收集的订阅方式：写入发生在 `onStop` 之后，`ON_START` 重启收集与返回后的首帧绘制
会抢跑，首帧按「有缓冲区」绘制时开孔区就透出桌面或上一个应用。

返回前台后 MUST 重新出帧，MUST NOT 停留在渲染暂停状态形成永久黑屏。

平台在切后台后会拆掉本应用**窗口**的 surface（实测：转场动画结束后立即拆除），
`SurfaceView` 的 surface 随之被拆——`SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT` 只让
surface 跟随视图的挂载，不改变窗口 surface 在后台被拆这一事实，故返回前台时
交换链必然重建。重建期间开孔没有缓冲，窗口层 MUST 用**离开时的画面**遮住开孔
（而非仅一块平底色）：平底色与桌面都是「闪一下」，只是闪成另一种颜色。

#### Scenario: 切后台拆除 surface

- **WHEN** 应用切到后台且系统拆除了 `SurfaceView` 的 surface
- **THEN** 开孔覆盖状态 MUST 落回「无缓冲区」，窗口层涂终端底色

#### Scenario: 返回前台首帧前不透出上一个应用

- **WHEN** 应用回到前台，新 surface 尚未提交首帧
- **THEN** 窗口层 MUST 保持终端底色覆盖，MUST NOT 透出桌面或上一个应用

#### Scenario: 返回前台后重新出帧

- **WHEN** 应用回到前台且新 surface 已挂载
- **THEN** 终端 MUST 在 30s 内重新提交并呈现帧，覆盖状态随首帧撤下

#### Scenario: 切后台时把离开时的画面留作开孔遮罩

- **WHEN** 应用切到后台，渲染线程停下之前
- **THEN** 终端 MUST 把保留的上一帧回读成 CPU 像素并发布为开孔遮罩
- **AND** 回读 MUST 在渲染线程上完成（它是一次 GPU 同步等待），MUST NOT 放在主线程
- **AND** 遮罩的尺寸 MUST 与 surface 尺寸一致，不一致时 MUST 放弃遮罩（MUST NOT 画错位的内容）

#### Scenario: 遮罩在画面恢复后撤下

- **WHEN** 新 surface 提交了首帧
- **THEN** 遮罩 MUST 被撤下并释放像素

#### Scenario: 没有可回读的内容时不遮

- **WHEN** 回读失败、没有保留的帧或帧格式不是 RGBA8
- **THEN** 宿主 MUST 沿用平底色遮罩，MUST NOT 以空遮罩代替（空遮罩等于一块黑）

### Requirement: 终端 Surface 的表面策略跟随平台默认

终端 SurfaceView MUST 使用平台默认的表面生命周期策略，MUST NOT 以 `setSurfaceLifecycle`
改写它。

`SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT`（API 34+）只让 surface 跟随**视图的挂载**，
不改变平台在后台拆掉本应用**窗口** surface 这一事实：实测（API 35 模拟器）切后台时
`surfaceDestroyed` 仍会到达（紧随转场动画结束），返回前台时原生窗口指针已变
（`ANativeWindow changed; rebuilding for the new window`），交换链照样重建。改写该策略
既没有消除重建，也把「平台是否会替我们保留 surface」这种不可控的假设写进了代码，
故 MUST NOT 使用。

每次重建都会让驱动重新枚举表面格式并试分配，真机每次留下四条设备图形分配器的格式
探测 error 与一串平台帧时间线的同步组查询 error——这类条目是重建次数的函数（见
`logcat-hygiene`），减少它们只能靠减少重建，而重建本身不可免。

#### Scenario: 不改写表面生命周期策略

- **WHEN** 终端 SurfaceView 初始化
- **THEN** MUST NOT 调用 `setSurfaceLifecycle`，MUST 保持平台默认策略

#### Scenario: 视图脱离窗口时 surface 仍被拆除

- **WHEN** 终端 SurfaceView 脱离窗口（Activity 销毁或宿主换视图）
- **THEN** surface MUST 按平台回调拆除，GPU 表面 MUST 随之释放
