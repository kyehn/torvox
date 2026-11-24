## ADDED Requirements

### Requirement: 跨 JNI 扁平数据 MUST 用直接量而非 JSON

数据在 JNI 边界上的形状 MUST 由其自身决定，不因「顺手」而套用 JSON。判别标准是
**Kotlin 侧是否会形成一份与 Rust 结构同形的 schema 镜像**：

- 扁平原语集合（三个整数、整数序列、标识序列）MUST 用既有直接量约定回传：
  `IntArray`（`jint_slice_to_int_array`）或 `LongArray`（`u64_vec_to_long_array`）。
- 具名多字段结构、多态判别、可变长字符串集合 MAY 用一次 JSON 编解码，
  但 MUST 在下方登记保留理由，MUST NOT 出现「把失败静默改写为空集合」的回退。

#### Scenario: 扁平集合不得经 JSON

- **WHEN** 新增一项回传 N 个同类型原语的 JNI 查询
- **THEN** 该查询 MUST 返回 `IntArray`/`LongArray` 等直接量，MUST NOT 引入
  `serde_json::to_string` 与 Kotlin 侧可解析该结构的 DTO

#### Scenario: 保留的 JSON 必须写明理由

- **WHEN** 某项回传因字段数量、多态或可变长结构而保留 JSON
- **THEN** 下表 MUST 有对应行，给出字段数/判别方式与「为何直接量会更复杂」的实测对照

### Requirement: 保留 JSON 的边界登记

经实测对照后保留 JSON 的边界逐项记录理由与替代方案的规模，避免后来者重复调研；
保留项的字段一经变更 MUST 同步其单测，MUST NOT 把解码失败改写为空值：

#### Scenario: 保留项不得腐化

- **WHEN** 修改下表 JSON 边界的字段
- **THEN** 对应单测（`PollEventTest`、`ParseEventTest`）MUST 同步更新，
  且 MUST NOT 把反序列化失败改写为空值——空值与「真的没有」必须可区分

保留的边界逐项如下：

| 边界 | 形状 | 保留理由 | 直接量替代方案的实测规模 |
| --- | --- | --- | --- |
| `pollEvent` / `pollResult`（`event.rs`、`ffi.rs`） | 单次调用返回一个事件，3 种变体各自 2–3 个字段，`exit.code` 需表达 `null` 与 0 之别；剪贴板载荷长度不可预知 | 变体判别 + 「一次取出后即消费」的语义由一次 JSON 解码天然保证；拆成标量需跨调用保存槽位，而槽位要引入并发可见性与「取用顺序」两个新失败面 | 至少 1（判别）+ 4（文本/标识/码点/选择区）次调用，外加一处会话级槽位与其线程安全；剪贴板文本还须再加一段长度前缀与分段拉取协议 |
| `pollNotificationEvent`（通知队列） | 单次调用返回一个通知事件，含会话 id、标题、正文三个字段，负载长度不可预知 | 与事件队列同一套 `Event` 枚举、同一 Kotlin `PollEvent` 解码器；通知走独立队列只因后台泵不得取走退出事件，事件形状不变 | 若改为直接量，需新增「会话 id 数组 + 标题数组 + 正文数组」三段并行返回，三段长度必须同步，且 Kotlin 侧要重建同一 schema 镜像 |

已按本标准收敛的形状（供对照，勿回流 JSON）：主题（`theme-configuration`：
`getThemeCatalog` 一次取回整个目录，`u16` 主题数 + 逐条「标志 + 名称长度 +
名称 + 54 字节颜色」）、字体信息（`getFontInfo` 的 `u32` 标志 + 三个 `f32` +
`u16` 名称数 + NUL 分隔 UTF-8 名称，CJK 状态按判别序号传递）、搜索结果
（`searchSelect` 扁平 `IntArray`，命中总数由 `searchSetNeedle` 返回）、会话列表
（`listSessions` 扁平 `LongArray`）、会话标题（`getSessionTitles` 的字符串数组，
按传入顺序对应）。

### Requirement: JNI 边界不得静默截断或静默吞错

跨 JNI 的每个入参 MUST NOT 用 `as` 收窄到更窄的无符号类型：静默截断会让越界值
变成一个**合法但错误**的查询（模式号 65536 落到 DEC 模式 0、鼠标按钮落到「无按键」），
与「该模式确实关闭」无法区分。越界 MUST 抛 `IllegalArgumentException`。

「会话不存在」MUST 与其余故障分用异常类型（`IllegalStateException` 与
`RuntimeException`）：宿主只把前者当作会话竞态并回缺省值，其余异常照实上抛。二者
共用一个类型时，「PTY 写入失败」「主题不存在」「尺寸越界」等真实故障会被一并说成
「会话已销毁」而无声消失。

会话竞态之外的取数失败 MUST NOT 当作「取到了空值」继续：例如读 Java 字符串数组长度
失败若回落成空列表，本次的调用会**替换**掉已登记的数据（用户的字体目录被静默注销）。

#### Scenario: 越界模式号被拒绝

- **WHEN** 以超出 `u16` 的模式号查询终端模式
- **THEN** 抛 `IllegalArgumentException`，MUST NOT 截断后查询另一个模式

#### Scenario: 主题不存在不被说成会话已销毁

- **WHEN** 会话健在但传入未注册的主题标识
- **THEN** 该错误照实上抛，日志与异常都不指会话销毁

#### Scenario: 会话缺失仍按缺省值处理

- **WHEN** 检查之后、调用之前会话被销毁
- **THEN** 调用回缺省值并记一条会话竞态警告，不向上抛

### Requirement: 抛错后不得再调 JNI，且终止条件须与故障分级一致

JVM 侧读到待决异常后，MUST NOT 再调用任何 JNI 函数（JNI 规范只允许极少数调用，
`checkJNI` 下直接 abort 进程）。故「校验失败 → 抛异常」的路径上，返回值 MUST 是调用点
声明的失败常量，MUST NOT 再去构造空数组或取字符串。

抛出的异常类型 MUST 与故障性质一致：会话已退出是正常生命周期终态（与 EAGAIN 同款
记日志返回），MUST NOT 与真实 I/O 故障同抛——击键路径在 UI 线程上，抛出即带走进程。
设置数据失效按 DESIGN 的既定处置执行：记日志并把该设置落回默认值。

#### Scenario: 越界入参不再触发第二次 JNI 调用

- **WHEN** 某个跨 JNI 入参越界触发抛错
- **THEN** 随后的路径不再调用任何 JNI 函数，只交回调用点声明的失败值

#### Scenario: 会话已退出不使击键崩进程

- **WHEN** shell 已退出而退出事件尚未派发，此时按下按键
- **THEN** 该次写入记日志后丢弃，不向上抛异常
