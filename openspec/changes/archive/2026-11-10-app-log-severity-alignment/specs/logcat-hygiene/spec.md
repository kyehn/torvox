## ADDED Requirements

### Requirement: 渲染线程的中断是关闭信号而非渲染错误

关闭路径（会话切换、运行期停止）中断渲染线程时，协程运行时以取消异常收尾，其 cause 是
`InterruptedException`。这类异常 MUST NOT 被记为渲染错误：MUST 按 debug 记，MUST NOT
输出完整栈，MUST NOT 计入渲染失败的连续计数与退避。

判据 MUST 取异常的 cause 而非异常类型：取消异常也可能来自协程作用域被取消，只有 cause
是 `InterruptedException` 时才说明是线程被中断。判据 MUST 抽成可单元测试的纯函数。

实现：`TerminalRuntime.kt` 的 `isRenderThreadInterruption` 为唯一判据，渲染循环的通用异常
分支先过该判据再决定记 error。

#### Scenario: 关闭会话时中断渲染线程

- **WHEN** 关闭路径中断渲染线程，渲染循环收到取消异常
- **THEN** 该次中断 MUST NOT 记为 error，MUST NOT 输出异常栈
- **AND** 渲染循环 MUST 干净退出

#### Scenario: 真正的渲染异常仍记 error

- **WHEN** 渲染循环抛出的异常不是中断引起
- **THEN** 该次异常 MUST 仍按 error 记，并计入渲染失败的连续计数

### Requirement: 宿主未就绪的路径必须按正常退化记

宿主未就绪的路径——平台收走原生窗口、宿主换新窗口、视图尚未附着——都是可自愈的正常退化，
MUST 按 info 或 debug 记，MUST NOT 记为 warn 或 error。连续出现说明恢复迟迟不来，那一档
MUST 留在 warn，MUST NOT 由应用侧自行吞掉。

同一情形在不同调用点 MUST 判成同一等级：原生侧的窗口判死与重挂载、宿主侧的换窗请求与
视图未附着，各有唯一判据且有单元测试钉住。

实现：原生侧判死记 debug（`SURFACE_INVALIDATION_LOG_LEVEL`）、重挂载记 info；宿主侧换窗
请求首次记 info、连续记 warn（`surfaceRecreateIsWarning`），重建信号记 debug；视图未附着
时跳过选择菜单记 debug。

#### Scenario: 平台收走原生窗口

- **WHEN** 平台收走原生窗口，渲染帧连续取不到纹理
- **THEN** 原生侧的窗口判死 MUST 记为 debug，MUST NOT 记为 error
- **AND** 宿主换新窗口的首次请求 MUST 记为 info，MUST NOT 记为 warn

#### Scenario: 新窗口迟迟不来

- **WHEN** 宿主连续请求换新窗口，新窗口仍未交付
- **THEN** 连续的那次请求 MUST 记为 warn
- **AND** 快速重试耗尽 MUST 仍记为 error

#### Scenario: 视图尚未附着

- **WHEN** 选择菜单因视图未附着而跳过
- **THEN** 该次跳过 MUST 记为 debug，MUST NOT 记为 warn

### Requirement: 同一时刻只允许一条提示窗口

平台对同一 UID 同时只允许一个提示窗口。应用 MUST NOT 在旧提示仍在显示时再挂一条：
MUST 先取消仍在显示的那条，MUST NOT 触发平台的窗口冲突告警。

提示 MUST 由单一投递点发出，MUST NOT 由各调用点各自构造。

实现：`ToastNotifier` 为唯一投递点，新的一条先取消仍在显示的那条；取消与展示都经主线程
消息队列，取消在前、展示在后。

#### Scenario: 连续弹出提示

- **WHEN** 应用在短时间内连续弹出两条提示
- **THEN** 第一条 MUST 在第二条挂上之前被取消
- **AND** 平台 MUST NOT 报出提示窗口冲突

#### Scenario: 提示投递不持有界面上下文

- **WHEN** 提示由单例投递点发出
- **THEN** 提示 MUST 用应用上下文构造，MUST NOT 因静态引用钉住 Activity
