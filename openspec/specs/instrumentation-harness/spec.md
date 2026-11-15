# instrumentation-harness Specification

## Purpose

仪器化用例共享 JUnit 规则，而规则会驱动真实的 `MainActivity` 与真实的原生会话。
一条多余的 Activity 规则就会多拉起一个 Activity 实例：两个实例同时轮询同一套
原生事件通道，退出事件、渲染事件被另一个实例消费，用例于是偶发失败且失败信息
指向别处。本能力固定「哪类用例用哪个基类」，把这条竞态挡在编写阶段。

## Requirements

### Requirement: Compose 规则只属于 UI 用例基类

需要 Compose 语义树的仪器化用例 MUST 继承 `ComposeTerminalLogcatTest`；不驱动
界面的用例 MUST 继承 `TerminalLogcatTest`，后者 MUST NOT 声明
`createAndroidComposeRule`。

UI 用例 MUST NOT 再自行声明 `createAndroidComposeRule`。确需自行管理 Activity
的用例（例如只用 UiAutomator 驱动系统界面的用例）MUST 只声明一条 Activity
规则，且 MUST 继承 `TerminalLogcatTest`。

#### Scenario: 非 UI 用例不启动 Activity

- **WHEN** 用例只经 JNI、`startService` 或文件系统验证行为
- **THEN** 它继承的基类 MUST NOT 拉起 `MainActivity`
- **AND** 原生会话事件 MUST 只由用例自身的 `pollEvent` 消费

#### Scenario: UI 用例只有一条 Activity 规则

- **WHEN** 用例通过 `composeTestRule` 断言界面
- **THEN** 规则链中 MUST 只有一条 Activity 启动规则
- **AND** `TerminalLogcatRule` MUST 位于其外层，使 Activity 启动失败同样携带日志尾部

### Requirement: 读终端 Surface 的用例 MUST 逐轮重取视图

仪器化用例需要从终端 Surface 取像素时，MUST 在每一轮轮询里重新解析视图，
MUST NOT 复用轮询开始时取到的 `SurfaceView` 引用。

系统重建 Activity（主题切换、输入法重排、旋转）后，旧视图的 Surface 已失效，
对着它做像素拷贝只会一直失败，而画面本身并无问题。用例 MUST 把「读回失败」
与「读回成功但颜色不符」分成两路计数并各自进断言信息，否则失败信息指向
错误的根因。

#### Scenario: Activity 重建后仍能读到当前画面

- **WHEN** 轮询期间 Activity 被重建，旧的 `SurfaceView` 引用已失效
- **THEN** 用例 MUST 读到重建后视图上的像素，MUST NOT 因旧引用恒定失败

#### Scenario: 失败信息区分两种形态

- **WHEN** 断言底色不符
- **THEN** 失败信息 MUST 同时给出读回失败次数与命中的最多像素数

### Requirement: 注入真实手势的用例必须把事件投递给目标视图并按实际布局取落点

向终端注入真实手势的仪器化用例，MUST 把 MotionEvent 投递给终端表面本身，MUST NOT
投递给窗口装饰视图（`decorView`）。

落点 MUST 由被测对象的实际布局推出（网格行列数与单元格像素尺寸），MUST NOT 取窗口
几何中心：终端表面之上覆盖着修饰键栏等组合覆盖层，窗口几何中心并不落在终端内容上，
事件会被覆盖层先行命中而永远到不了终端表面。

用例 MUST 先把输入法收起并确认已收起，再取网格与落点：输入法可见时网格与屏幕坐标
不是一一对应关系。

失败信息 MUST 区分「手势没有到达终端」与「到达了但菜单被错误收起」，MUST NOT 让前者
以后者的措辞出现。

#### Scenario: 长按落在终端内容上

- **WHEN** 用例按网格单元格中心取落点并把事件投递给终端表面
- **THEN** 长按 MUST 到达终端表面，MUST NOT 被修饰键栏覆盖层拦截
- **AND** 选区菜单 MUST 出现并留存到松手之后

#### Scenario: 输入法收起后才取落点

- **WHEN** 输入法可见
- **THEN** 用例 MUST 先收起输入法并等到窗口 inset 中输入法确实不可见，再取网格与落点

#### Scenario: 不向装饰视图投递手势

- **WHEN** 用例注入真实手势
- **THEN** 事件 MUST 投递给终端表面，MUST NOT 投递给 `decorView`

### Requirement: 改写共享会话状态的用例必须还原

仪器化套件跨用例共用同一个原生会话（同一块网格、同一份 VT 状态）。用例为了稳住
自己的断言而改写会话状态时，MUST 在离开前还原，且还原 MUST NOT 依赖断言成功——
断言失败时同样要执行。

光标可见性最容易漏：像素比对期间要藏起光标（块光标闪烁是逐像素噪声源），却忘了
还原。后继用例靠帧内光标定位起点（退格、组字两条用例），光标一直藏着时起点判据
整段失效，红灯落在后继用例的措辞上，根因却在更早的用例——只跑后继用例即通过，
这正是跨用例污染的指纹。

#### Scenario: 藏光标的用例还原光标

- **WHEN** 用例为像素比对藏起光标（DECTCEM）
- **THEN** 它 MUST 在离开前恢复光标可见
- **AND** 还原 MUST 放在收尾钩子里，断言失败时同样执行
- **AND** 后继用例 MUST 能重新取到帧内光标
- **AND** 还原 MUST 走共享夹具：序列与出口只有一份，多个用例各写一份时一处改动即让污染复发

#### Scenario: 共用清场恢复光标基线

- **WHEN** 用例经共用清场（`cleanUpTerminalState`）准备基线
- **THEN** 清场 MUST 把光标可见性恢复成可见
- **AND** 清场 MUST NOT 依赖前序用例已经还原
- **AND** 清场 MUST 与用例收尾共用同一份光标显示序列

光标可见性是跨用例存活的会话状态，且隐藏光标时 `cursorViewportPacked` 返回 -1：
光标一直藏着，靠光标位置落格的用例整段失效（实测报「标记必须落格于」），
红灯措辞落在后继用例上，根因却在更早的用例。共用清场 MUST 把这条基线一并恢复，
使后继用例不依赖前序用例的卫生。
