# code-health Specification

## Purpose

代码健康度的可复核口径：重复实现必须收敛为共享夹具而非各自拷贝；直接依赖必须处于
活跃维护状态，以自实现替代依赖前须先证明自实现覆盖了该依赖的维护状态；上游源码
不得进仓（vendor 只能是构建期拉取，不入库）；静态形状扫描的每一处命中必须逐条判定
并留档，未判定即视为未完成的审查。这些口径不靠「读代码觉得还行」判定，
而靠固定参数的扫描与外部事实（下载量、最近发布）判定，并留下可复算的留档。

## Requirements

### Requirement: 重复实现必须收敛为共享夹具

同一夹具（会话准备与销毁、就绪判据、可见性读数、锁持有与放行）在两处以上复制时，
MUST 收敛为一处共享实现，不得以拷贝形式各自演进。

共享实现 MUST 保留调用点原有的判据、超时与清理语义：收敛只允许消除复制，不得顺带
改变行为或放宽断言。

判定为「天然相似」而保留的重复（import 块、图标资源、判别不同不变式的并列回归钉）
MUST 在审计留档中逐条写明理由，不得默认忽略。

#### Scenario: 同一会话夹具在多个用例中复制

- **WHEN** 两个及以上用例复制了同一段「建会话 → 等就绪 → 销毁」代码
- **THEN** 该夹具 MUST 收敛为共享实现，就绪判据与超时只有一份

#### Scenario: 共享夹具收敛时不得改变行为

- **WHEN** 夹具收敛为共享实现
- **THEN** 每个调用点的判据、超时与「失败时是否清理」语义 MUST 与收敛前一致

#### Scenario: 保留的重复须逐条留档

- **WHEN** 某处重复经判定为天然相似或刻意并置
- **THEN** 审计留档 MUST 写明该重复的保留理由

### Requirement: 直接依赖必须处于活跃维护状态

引入或保留一个直接依赖前，MUST 核对该依赖的下载量与最近发布状态，并按同一口径
评估其余直接依赖。

依赖的采信口径 MUST 为：累计下载量 ≥ 100 万、近 90 天下载量 ≥ 10 万，且最近发布在
12 个月内。生态基线类型（仅作类型边界、无独立逻辑）可豁免「最近发布」一项，但
MUST 在留档中写明豁免理由。

以自实现替代既有依赖时，MUST 先证明自实现覆盖了该依赖的维护状态；否则 MUST 继续
使用该依赖。

#### Scenario: 候选依赖下载量过低或长期未更新

- **WHEN** 候选依赖的下载量或最近发布不满足采信口径
- **THEN** 该依赖 MUST NOT 被引入，且理由 MUST 写入审计留档

#### Scenario: 已有依赖不满足口径

- **WHEN** 复核发现某个直接依赖不满足采信口径
- **THEN** MUST 给出替换或退出方案，并在留档中记录其下载量与最近发布事实

#### Scenario: 生态基线类型的豁免

- **WHEN** 某个依赖仅承担类型边界而无独立逻辑，且维护停滞但使用面广
- **THEN** 该依赖可豁免「最近发布」一项，MUST 在留档中写明豁免理由

### Requirement: 扫描命中必须逐条判定并留档

对源码集运行静态形状扫描后，扫描器的每一处命中 MUST 逐条判定，并在审计留档中写明
位置、判据与结论。

命中不等于缺陷，也不等于结论：未判定的命中 MUST 视为未完成的审查。判定为「不是缺陷」
时，MUST 给出可复核的依据（陷阱文档的要求与现场代码的事实），不得只写「已确认安全」。

判定为真实缺陷时，MUST 修复或给出明确的处置结论；修复涉及收敛共享实现时，
MUST 保留原调用点的断言强度。

#### Scenario: 扫描命中但经判定不构成缺陷

- **WHEN** 某处扫描命中经阅读现场代码与陷阱文档后判定不构成缺陷
- **THEN** 审计留档 MUST 写明该处的位置、判据与依据

#### Scenario: 扫描命中尚未判定

- **WHEN** 某处扫描命中没有留下判定记录
- **THEN** 该处审查 MUST 视为未完成，不得以「扫描器未报缺陷」代替判定

#### Scenario: 收敛共享实现时不得弱化断言

- **WHEN** 为消除重复而把多份拷贝收敛为共享实现
- **THEN** 收敛后的实现 MUST 保留各调用点原有的断言强度

### Requirement: 审计留档必须覆盖当前代码树

重复率扫描的审计留档 MUST 与当前代码树的扫描结果一致：改动引入的重复命中 MUST 在留档中
逐条判定，真重复 MUST 收敛为共享实现。

留档 MUST 记录扫描参数与前后对比，使结论可复算；参数不同的两次扫描结果 MUST NOT 直接
互相比较。

#### Scenario: 改动引入新的重复命中

- **WHEN** 改动后的代码树在留档给定的扫描参数下出现留档未覆盖的命中
- **THEN** 该命中 MUST 在留档中逐条判定，且真重复 MUST 收敛为共享实现

#### Scenario: 留档可复算

- **WHEN** 复核者按留档给出的参数重跑扫描
- **THEN** 结果 MUST 可与留档中的数字逐项对照

### Requirement: 代码规模约束必须有统一口径与可复算留档

判定样式规范中的规模约束时，MUST 使用统一的源码行数口径：排除空行、纯注释行、块注释、
文档、配置、资源、构建产物与第三方代码；生产与测试按路径分列；MUST NOT 以磁盘字节、
仓库体积或未分列的混合行数代替。MUST 同时报出生产、测试与源码合计三档数字，并与两个
点名参考项目用同一脚本、同一参数重跑的结果对照。约束判定 MUST 留有可复算的基线数字；
复算与基线的偏差 MUST 给出解释。MUST NOT 通过删除测试来满足约束。

#### Scenario: 判定规模约束时按统一口径复算

- **WHEN** 需要判定代码量是否超过两个参考项目之和
- **THEN** MUST 按统一源码行数口径复算并同时报出生产、测试与源码合计三档，
      MUST NOT 以存储占用或未分列的混合行数代替

#### Scenario: 不同口径导出相反结论

- **WHEN** 同一份代码在生产口径下未越界、在源码合计口径下越界
- **THEN** 该判定 MUST 视为不可判定，MUST 先固定口径与留档位置再给出结论

#### Scenario: 复算结果偏离留档基线

- **WHEN** 重新统计的数字与审查留档的基线数字不一致
- **THEN** MUST 写明偏差来源与统计参数差异，MUST NOT 直接改写基线掩盖偏差

### Requirement: 超大文件阈值必须跨源码语言一致应用

文件规模治理 MUST 服从 `docs/specification/STYLE.md`：该规范把行数、嵌套层数与文件大小
列为仅风格相关的问题，明示可忽略且不得为其耗时。故本仓 MUST NOT 以文件大小或行数为由
登记未完成项，MUST NOT 为凑某个文件行数阈值而拆分源码文件。

拆分 MUST 仍按彼此独立的职责边界进行，MUST NOT 按行数机械切分，也 MUST NOT 制造仅用
一次的空壳抽象；职责确实混杂时按边界拆分，与行数无关。

代码量（相对两个点名参考项目之和）的约束由「代码规模约束必须有统一口径与可复算留档」
单独管辖，不受本条影响。

#### Scenario: 阈值只作用于单一语言

- **WHEN** 原生侧已按阈值拆分而界面与运行时侧仍存在更大的源码文件
- **THEN** MUST NOT 据此登记未完成项，MUST NOT 为行数发起拆分

#### Scenario: 按职责边界重组超文件

- **WHEN** 重组一个承载十余个关注面的超限文件
- **THEN** MUST 按彼此独立的职责边界切分，且每个调用点的行为与断言强度 MUST 保持不变

#### Scenario: 保留超限文件须留档

- **WHEN** 某个源码文件因职责内聚而保持较大
- **THEN** MUST NOT 为其逐条登记保留理由

### Requirement: 设计文档明列的参考项目必须有研究留档

设计文档明列的每个参考项目 MUST 在参考资料目录有一份对应留档，覆盖其分层方式、输入
顺序、生成物复现校验与基准体系。清点后缺失 MUST 记为资料齐备性缺口并写明补齐范围。
补齐落点为受保护目录时，MUST 在获得明确同意后进行；未获同意 MUST 登记为待办，
MUST NOT 修改受保护目录的实际内容。

#### Scenario: 清点明列参考项目

- **WHEN** 按设计文档清点明列的参考项目并核对参考资料目录
- **THEN** 每个 MUST 有对应留档，缺失者 MUST 记为资料齐备性缺口并写明补齐范围

#### Scenario: 补齐一份参考留档

- **WHEN** 为同类度最高的参考项目补齐研究留档
- **THEN** 留档 MUST 覆盖分层、输入顺序、生成物复现校验与基准体系四类事实

#### Scenario: 落点受保护且未获授权

- **WHEN** 补齐落点属于受保护目录且尚未获得明确同意
- **THEN** MUST 登记为待办并保留缺口记录，MUST NOT 改动受保护目录的实际内容

### Requirement: 规范声明的静态检查工具必须与验证门禁一致

验证脚本与持续集成实际执行的检查 MUST 与测试规范的声明逐条比对；不一致 MUST 登记为
未完成并写明差异位置与接入方式。声称某项检查通过时，证据 MUST 指向实际执行的命令，
MUST NOT 以规范声明代替执行记录。门禁实际内容 MUST 可复核并留档。

门禁内的静态检查工具 MUST NOT 留下「解析失败即跳过」的口子：解析失败的文件与区域
MUST 计入留档（数量与位置），MUST NOT 以「扫描器未报缺陷」当作已审查。缺口 MUST 被
消除；消除后重跑 MUST 无解析失败项。

未完成项的登记 MUST 落在「禁改区待维护者裁定项台账」要求的台账里，
台账 MUST 同时给出当次实测基线与实施时的验收判据。

#### Scenario: 声明的工具未进入门禁

- **WHEN** 测试规范声明的重复检测与代码质量扫描工具未出现在验证脚本与持续集成中
- **THEN** MUST 登记为未完成，写明差异位置，并注明接入方式由受保护文件的维护者决定

#### Scenario: 声称检查通过须有执行证据

- **WHEN** 审查结论声称某项静态检查通过
- **THEN** 证据 MUST 指向实际执行的命令及其输出，MUST NOT 以规范声明代替执行记录

#### Scenario: 门禁内容变更后重新比对

- **WHEN** 验证脚本或持续集成的检查项发生变化
- **THEN** MUST 重新逐条比对声明与实际，并更新留档中的门禁内容清单

#### Scenario: 扫描器解析失败留下覆盖缺口

- **WHEN** 门禁内的静态检查工具对某些文件报解析失败并跳过相应区域
- **THEN** 该缺口 MUST 计入门禁留档（文件与区域数量），MUST NOT 以「扫描器未报缺陷」
      视作已审查
- **AND** MUST 消除该缺口；消除后重跑 MUST 无解析失败项

### Requirement: 显示节拍禁止使用固定帧率和省电模式分支

代码审查时，显示节拍的实现细节 MUST 符合“以显示帧回调作
唯一帧源、定时闭锁只作安全网”的原则；禁止通过固定帧率声明
`setFrameRate()` 或 `PowerManager.isPowerSaveMode()` 分支去选择帧率
或渲染路径。前台服务的省电保活除外。渲染路径是否仍有帧率声明
或省电模式分支，MUST 由 `code-health` 的审查口径写入审计留档。

#### Scenario: 审查看到固定帧率声明

- **WHEN** 代码中出现帧率偏好的声明调用
- **THEN** 该调用 MUST 记为渲染策略破绽，或给出不采用的审计结论，
  不能静默通过

#### Scenario: 省电模式是否应可分支

- **WHEN** 代码基于省电模式开关决定帧率或渲染路径
- **THEN** 该分支 MUST 被标记为需要审查的破损点，或者
  在留档中写入“未见决定”而不是假装已判断

### Requirement: 审计留档的落点必须可解析

审计留档（重复率、静态形状扫描与依赖活跃度的核对记录）的落点 MUST 在规约中写明，
当前落点为 `openspec/changes/archive/2026-11-24-code-health-audit/audit.md`：重复率与
代码规模的实测数字、扫描参数与逐处判定写在该处；依赖活跃度的核对结论 MUST 写入同一
文件的「依赖活跃度」小节。

留档被删除或改名时，引用它的规约 MUST 同步更新落点，MUST NOT 保留失效引用。

#### Scenario: 规约引用的留档已不存在

- **WHEN** 规约要求在某份留档中记录，而该留档已从工作区删除
- **THEN** MUST 把该留档的落点写回现行规约，MUST NOT 保留指向已删除文件的引用

#### Scenario: 依赖活跃度核对有结论

- **WHEN** 核对了直接依赖的下载量与最近发布状态
- **THEN** 结论 MUST 写入审计留档，含该依赖的下载量与最近发布事实

### Requirement: 静态形状扫描审计结论

对源码集运行静态形状扫描后，扫描器的每一处命中 MUST 逐条判定；清洁运行 MUST 计入审计结论，
连续清洁运行计数 ≥ 5 为「连续 0 确认」的证据。判定为「合理构造」的命中 MUST 在审计留档的
「静态形状扫描」小节写明位置、判据与依据（如「gesture 只读状态，用 rememberUpdatedState」）。

#### Scenario: 扫描清洁运行不足

- **WHEN** 同一扫描器的清洁运行计数不足 5
- **THEN** 审计结论 MUST 继续计入并写明距够次数还差几次，不得以「一次 0 命中」声称完成

#### Scenario: 合理构造命中

- **WHEN** 命中经阅读现场代码与陷阱文档判定为合理构造
- **THEN** 审计结论 MUST 写明位置、判据与依据（如「gesture 只读状态，用 rememberUpdatedState」）

### Requirement: 平台输入的字符级行为必须可独立验证

被测代码处理平台输入事件（按键、触摸）时，字符级或参数级的决策 MUST 有不依赖平台
框架类的入口：平台事件级的过滤与字符级的处理 MUST 分开，回归用例 MUST 走字符级
入口验证字符级行为。

回归用例 MUST NOT 在测试进程里继承平台框架类来伪造输入。宿主 JVM 单测里，平台框架
类经 Robolectric 插桩后加载；继承它会与 mockk 的 inline agent 相撞，使 JVM 的字节码
转换失败并把测试工作进程带崩（`transform method call failed` 与
`NoClassDefFoundError: java.lang.StackTraceElement$HashedModules`），结果是整个测试
任务失败而非单条用例失败。

#### Scenario: 需要伪造返回特定字符的按键

- **WHEN** 回归用例需要一个返回特定 Unicode 字符（如死键字符）的按键输入
- **THEN** MUST 调用被测代码的字符级入口，MUST NOT 继承 `android.view.KeyEvent`
      等平台框架类来伪造该输入

#### Scenario: 平台框架类替身使测试工作进程崩溃

- **WHEN** 测试进程内出现继承平台框架类的替身，导致测试工作进程死亡
- **THEN** MUST 判为替身形态缺陷并改用字符级入口，MUST NOT 以跳过该用例、
      降低断言强度或调低日志级别处置

#### Scenario: 字符级入口不得改变平台路径的判定顺序

- **WHEN** 把字符级处理从平台事件处理中抽出
- **THEN** 平台路径的判定顺序、无内容时的返回值与死键被消费时的返回值 MUST 与
      抽出前逐条一致

### Requirement: 终端状态依赖跟踪上游 master

`libghostty-vt` / `libghostty-vt-sys` MUST 钉在上游 `Uzaaft/libghostty-rs` 的
master rev，MUST NOT 长期停在某个旧 rev 上——DESIGN 已要求「跟踪 git master rev，
无本地补丁」，停在旧 rev 会让上游修复与本仓长期分叉。

本仓 MUST NOT 为适配上游而打本地补丁；API 变化 MUST 直接在本仓代码里跟进。

#### Scenario: 上游 master 前进后本仓已跟进

- **WHEN** 上游 master 的 rev 高于本仓 `Cargo.toml` 中钉的 rev
- **THEN** 该差异被记录为待跟进项；跟进时全量 Rust 测试与经 JNI 驱动真实原生库的
      JVM 测试 MUST 保持全绿

#### Scenario: 记录钉住 rev 时所依赖的上游事实

- **WHEN** 依赖被钉住
- **THEN** 该次复查 MUST 记录所钉 rev 携带的上游事实（上游提交日期、与上游 master 的
      提交数差、本次实际消费的 API 面），使「本仓用的 VT 实现落后上游多少」可核对
      而非靠记忆

#### Scenario: rev 本身不进正文

- **WHEN** 记录上述事实
- **THEN** MUST NOT 把 git 长/短 hash 写进规范、提交消息或注释（`STYLE.md` 明禁）；
      rev 的唯一载体是 `Cargo.toml` 的依赖钉——那是构建清单的功能性声明，不是正文。
      「落后多少」由提交日期与提交数差表达，与 hash 无关

**2026-11-15 依赖版本复查**：

- 绑定层：`libghostty-vt` 所钉提交与上游 master 提交数差为 0（当日 master），上游最新提交日期
  2026-11-24。本次前进的提交中，两项把快照解码器的 kitty 图像计数与待传字节总量迁到
  `IncrementalDecoder` 上、一项把可选 `allocator_api` feature 改用标准库分配器 trait、一项
  抬高构建用 rustc。本仓未消费 `snapshot` 解码接口，也未启用 `allocator_api`；实际消费的
  API 面（`key`、`mode`、`color`、`style`、`selection`、`search`、`screen`、`render`、`point`、
  `paste`、`mouse`、`kitty::graphics`、`grid_ref`、`terminal`、`effect`、`report`、`set_logger`）
  无签名或语义变化，全量 Rust 测试与其之上的 Android 仪器测试保持全绿。
- crates.io：`nix` 为 0.32（最新稳定），其余直接依赖已是当日最新稳定版本，锁文件已刷新到各
  依赖的最新修订；`fontdb` 仍受 `cosmic-text` 的 `^0.23` 要求约束（见依赖活跃度表）。
- Gradle/Android：Gradle wrapper 9.9.3、KSP 2.3.14、Robolectric 4.18 为当日最新稳定版本。
  `androidx.test`、Espresso 与 Benchmark 存在更新的 alpha，但当前所用为稳定版，按「最新稳定
  版本」口径保持不动；AGP 9.6.0-alpha06、lifecycle 2.12.0-alpha06、activity 1.14.0-alpha03、
  detekt 2.0.0-alpha.6、LeakCanary 3.0-alpha-9 属「已用不稳定版本且无更高稳定版本」，保持该线
  最新不稳定版本。

#### Scenario: 上游改的是语义而非签名

- **WHEN** 上游改动使既有调用的**含义**改变而签名不变（如回滚字节预算的 `None`
      由「无上限」变为「恢复默认上限」）
- **THEN** 本仓 MUST 逐条核对该调用点的既有取值是否仍表意，MUST 用既有本地用例
      钉住行为（去掉修复即失败），MUST NOT 只以「能编译」判定跟进完成

### Requirement: 同一上限 MUST 只由一个具名常量表达

同一处能力上限（例如单格字素簇码位数）MUST 由一个具名常量表达，**上限的应用点**（把上限
施加到数据上的取数处）MUST 引用该常量或其派生值，MUST NOT 在第二条路径上把派生值重写为
字面量——否则调整常量会让两条路径静默分叉，而分叉不会被任何测试发现。

零值构造点（默认值数组的字面长度）不属于应用点：其长度由字段类型决定，常量一旦调整即
编译失败而非静默分叉，故 MUST NOT 为其扩大常量可见性（基准目标同样无法引用 crate 私有常量）。

#### Scenario: 两条通道共用同一上限

- **WHEN** 同一字素簇上限同时用于渲染数据与快照数据
- **THEN** 两处 MUST 都由同一具名常量派生；调整该常量 MUST 同时改变两条通道的行为

#### Scenario: 零值构造点保持紧凑

- **WHEN** 某处只需构造一份全零的字素簇默认值
- **THEN** MAY 直接写字面长度；MUST NOT 为此把常量提升为公开可见

### Requirement: 上游 API 取用判定登记

本仓以 Ghostty 为终端状态单一来源：上游已有的能力 MUST 直接取用，MUST NOT 重实现；
上游有而本仓不用的能力 MUST 在此登记「不采用」的 DESIGN 依据或「待办」的阻塞点，
MUST NOT 留空使后来者重复调研或把合规分叉移植回来。每项 MUST 给可复核依据
（上游源码位置、本仓文件行号或 DESIGN 条款）。

#### Scenario: 结论可脱离审计报告独立复核

- **WHEN** 复查某项上游 API 的取用与否
- **THEN** 该项结论与依据 MUST 能在本规范内读到，MUST NOT 需要先翻审计报告或归档变更

上游 API 取用判定如下（上游位置均指 `Cargo.toml` 所钉 rev 的 `crates/libghostty-vt/src`）。

| 上游能力 | 结论 | 可复核依据 |
| --- | --- | --- |
| `terminal.rs`：`on_clipboard_write` 必须 `reply` | 采用（已修） | 上游文档「返回前必须 `reply`，否则拒绝写入」；本仓 `internal/run.rs` 的 `on_clipboard_write` 回调此前零 `reply`。现满通道回 `Busy` 否则 `Ok` |
| `style::Underline` 五形状判定 | 采用（已修） | 三处 `matches!` 先收敛为同源包装（`has_visible_underline`/`is_double_underline`），后删包装直接按枚举比较：`Underline::None` 判定绘制位、`Underline::Double` 判定双下划线（`rendering.rs`） |
| `Terminal::vt_write` 二进制安全 | 采用（已修） | `pty_write` 曾把 `>0xF7` 改空格，破坏 Kitty 直接 RGB 等二进制载荷；现仅剔除 NUL，与 `vt_write` 同策略 |
| 调色板与字节转浮点 | 采用（已修） | 渲染侧 `CATPPUCCIN_MOCHA_BACKGROUND` 与手写除法已删，以 `theme` 单源（构建期 alacritty-theme）为唯一入口，按标识取用（`theme::default_dark`／`theme::by_name`）；中间 epoch 用过的 `GhosttyTerminal::default_palette` 已随「主题改按标识在原生单点解析」删除 |
| `selection` 交叉翻转归属 | 采用（已修） | 对标 termlib `applyHandleDrag` 五元组：回传新归属柄并收紧等值判据，调用方更新 `handleDragState` |
| `search::Search` 全文搜索 | 采用（2026 改判） | 自研 `regex` 搜索曾被判定合规，实为重复实现：软换行跨行、随输出增量同步、命中总数与导航滚动上游全已提供。现 VT 线程持有 `search::Search`（同生命周期的局部变量，非自引用结构），needle/总数/导航/视口命中均走上游，本仓只做「上游选区 → 按物理行网格区间」转换。代价：上游仅 ASCII 折叠、无区分大小写开关，`DESIGN.md` 修饰键栏节原「大小写匹配」与「使用库（如 `regex`）实现文本搜索」两条**已由维护者改写**（删去大小写匹配按钮、改写为使用 libghostty-vt Search，与上游口径一致）；`regex` 依赖随之删除。软换行/宽字符/CJK/真实空格的用例语义按上游口径重写，见 `text-search` 规约 |
| `selection/gesture` 手势、`select_line`/`select_output`、`format_selection_*` | 不采用 | 连击语义与 DESIGN.md 终端页面节的「不得支持 双击 三击 多击选择」「不得支持 双击 三击 多击选择」正面冲突，该族主体（拖动手势 + 边缘自动滚动）另有两条理由：其自动滚动按定时器在边缘区持续滚动，与 `text-selection` 的 Scenario「下缘拖动逐行滚动」（每次触点移动视口滚动 1 行）与「手指静止不滚动」相反；其手势状态若由 VT 线程持有，则每个触摸帧一次跨线程往返，使拖动快路径的无跨语言调用优势尽失，并与视图侧手势所有权形成双状态机。四项上游优势的覆盖情况：词派生、跟踪选区、随滚动跟随、复制格式化已走上游；宽字符吸附的事实取自上游网格单元宽度；边缘滚动节奏不采纳；单一来源已由 `terminal.rs` 承担。 |
| termlib `trimDetectedUrl`/括号计数/跨行拼接 | 不采用 | DESIGN「链接识别通过 `libghostty-vt` 查询，没有额外检查」即禁止移植；仅保留空白 trim 与协议白名单（属安全检查） |
| `on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report` 等回调 | 不采用 | Kotlin 侧零消费者；DESIGN「不做不必要功能」，有消费者时再注册。`on_bell` 曾被 7 处注释误记为已接线（`ghostty_terminal/commands.rs` 的 `RunConfig` 悬空附着于 `cell_size_px`、`types.rs` 的 `EVENT_CHANNEL_CAPACITY`、以及 `session/` 的 5 处收割路径文档），实际该通道只承载 OSC 52 剪贴板写入；注释已删除。`on_bell` 与 `on_desktop_notification` 现已单独接线，见下两行 |
| `Terminal::on_bell`（BEL 回调） | 采用（2026-11-03 改判） | `DESIGN.md` Shell 节要求 `bell-character` 三态与 termux-app 一致，响铃事件必须有来源。回调只置无锁标志（`ghostty_terminal/internal/run.rs` 的 `on_bell` 注册），每帧收割为 `Event::Bell { session_id }`（`native/src/android/ffi/render/pump.rs`），Kotlin 按 `bell-character` 三态限频执行（`runtime/BellPlayer.kt`），限频窗口内的重复响铃静默丢弃 |
| `Terminal::on_desktop_notification`（OSC 9/777 回调） | 采用（2026-11-05 改判） | `DESIGN.md` 软件节要求处理 OSC 9 等通知（前台 Toast、后台系统通知、点击切换会话），通知必须有来源。回调注册在 `ghostty_terminal/internal/run.rs`；负载是程序给出的原始字节（上游文档写明不保证 UTF-8），按替换字符口径解码后按会话归属投递；Kotlin 侧按应用前后台状态分派（`runtime/DesktopNotificationPlayer.kt`），规约见 `desktop-notification` |
| `track_grid_ref`、`width_px`/`height_px`、光标色实时查询、`set_mode`、`vt_write_until_ground` | 不采用 | 按需取用；现有轮询与原子镜像满足当前消费者，DESIGN 要求最小实现 |
| `key::Encoder` 按键编码 | 采用（已修） | Android 键码到上游 `Key` 为机械对应表（`keymap.rs`），桥形态与 `Query::EncodeMouseEvent` 同构（编码在 VT 线程）；`TerminalInputEncoder` 仅保留输入法文本路径 |
| `Terminal::paste` 粘贴 | 不采用 | 含换行文本返回 `Error::Rejected` 并要求确认框后 `allow_unsafe` 重试，PROHIBITED 明禁粘贴确认对话框；恒置 `allow_unsafe` 则检查失去意义 |
| `paste::encode` 粘贴编码 | 采用（已修） | 无终端依赖的纯函数：bracketed 标志取活体模式 2004，控制字节中化与括号包裹由上游承担；`encodePaste` 查询跑在 VT 线程，Kotlin 只做截断与字节流切块 |
| `paste::is_safe` | 不采用 | 其拒绝流程同样要求确认对话框，PROHIBITED 明禁 |
| `device_attributes` 回调 | 不采用 | 缺省 trampoline 已按默认属性应答（实测 DA1=`?62;22c`，`test_output_capture_da` 钉住）；自定义 132 列等属性需先验证 DECCOLM 通路，暂无消费者要求 |
| `xtversion` 回调 | 不采用 | 缺省回退 `libghostty` 已应答；自定义名字无功能增益 |
| `color_scheme` 回调 | 不采用 | 注册 None 等价于未注册；无 OS 级来源，不猜 |
| `report::ColorScheme::encode`（明暗主动上报） | 采用（2026-11-15 改判） | 主题/覆盖生效且子进程开启 `Mode::COLOR_SCHEME_REPORT` 时，VT 线程主动推 `CSI ? 997 ; 1\|2 n`；序列由上游 `report::ColorScheme::{Dark,Light}` 编码（`commands.rs` 的 `report_color_scheme_on_change`），替换手写字面量；明暗按覆盖后背景亮度判定（`colors-configuration`「明暗上报」规约），不是注册 OS 回调 |
| `set_terminfo_name` | 不采用 | 未设置则 TN 查询不报告；声称值需先审计能力一致性，暂无消费者要求 |
| Kitty 文件/共享内存介质 | 不采用 | 直传已覆盖主路径；文件介质是新能力面（沙箱路径裁决），需产品确认，另立变更 |
| `kitty::graphics::RustPngDecoder` | 采用（已修） | 本仓原先自写 60 行 `KittyPngDecoder`，注释称「上游 RustPngDecoder 无公开构造器」——该结论与上游源码不符：`pub const fn new()` 存在，且本仓已开 `png` 特性。自写版按未经上限校验的 PNG 头尺寸在全局堆分配，绕过的正是上游分配器上限 |
| `style::RgbColor::parse` / `perceived_luminance` | 采用（已修） | 主题色值解析与深浅分组原为本仓手写（`#RGB` 展开、Rec.601 加权 0.299/0.587/0.114）；两者都取自上游，阈值语义与 ghostty 一致 |
| `on_clipboard_read` | 不采用 | 上游要求在回调内同步应答（VT 流暂停）。本仓的应答链必须异步：剪贴板内容来自 Android 系统剪贴板，经 `pollEvent` 上报后再回写 PTY；若改走上游回调，VT 线程要阻塞到 Kotlin 应答，终端在应答期间冻结。识别仍需自研扫描（上游回调装上后才拦截得到），故净收益为负 |
| `fmt::Formatter` 取全量纯文本 | 不采用 | 文档写明只覆盖 active screen，而本仓 `ReadAllText` 的口径是「回滚 + 视口」；上游补回滚参数后才可能跟进 |
| `fmt::Formatter` 取选区文本 | 采用（已修） | 选区文本经上游格式化器提取（换行感知、CJK 宽字符不被切开）；上游把选项改为活选项后，装指定选区须走 `set_selection`（上游标为 unsafe），本仓在 `internal/selection.rs` 的 `selection_text_impl` 处按 SAFETY 约定装同一终端活动屏的网格快照 |
| `Terminal::set_scrollback_max_bytes` | 采用（已修） | 回滚以行数为唯一约束，字节预算须显式解除；上游语义由「`None` 解除」改为「`None` 恢复默认的 10 000 字节预算」，故改传 `Some(usize::MAX)`，否则深缓冲回滚被截断到几百行（`scrollback_deep_buffer_does_not_collapse` 钉住） |
| `Terminal::new(cols: u16, rows: u16)` 网格尺寸入参 | 采用（2026-11-21 改判） | 上游只收 `u16`。本仓此前把 `GhosttyTerminal` 构造器的行列放宽为 `u32`，再用 `as u16` 收窄：70000 列被静默截成 4464 列（截断后仍是合法网格，无日志可循），而 resize 侧会如实拒绝——同一尺寸在两条路径上得到两种结果。现构造器直接收 `u16`（越界在类型上不可表达），零行/零列显式失败（`TerminalError::InvalidDimensions`）；会话侧 `u32` 入参仍由 `session::validated_grid` 收窄并显式拒绝（`session_spawn_rejects_dimensions_outside_the_u16_grid` 钉住） |
| `mouse::Encoder::set_size` | 采用（已修） | 上游改为返回 `Result`：尺寸写不进编码器时该事件不可用，本仓记错误并丢弃事件，不沿用旧尺寸换算坐标 |

### Requirement: 不采用的上游回调不得留有通道描述

判定为不采用的上游回调 MUST NOT 在代码注释、结构体字段文档或常量说明中以「事件通道」
「每次振铃一个空消息」等形态描述为已存在的实现。若某回调已判定不采用而代码中仍见其
通道描述，MUST 删除该描述，MUST NOT 顺手接线回调——接线需先有 `DESIGN.md` 声明与
消费者，缺一即属未声明功能。

本仓的 OSC 回调通道有两条：剪贴板通道承载 OSC 52 剪贴板写入，通知通道承载 OSC 9/777
桌面通知。两条通道的容量常量与字段文档 MUST 只描述各自承载的回调。

#### Scenario: 悬空通道注释被删除

- **WHEN** 结构体中某字段的文档注释描述了一条并不存在的上游回调通道
- **THEN** 该描述被删除，相邻字段保留其自身文档，不因删除而并入邻字段

#### Scenario: 事件通道容量说明只写剪贴板

- **WHEN** 查阅剪贴板事件通道容量的说明
- **THEN** 说明只提到剪贴板写入，不出现未接线的其他回调

#### Scenario: 清理不以接线收尾

- **WHEN** 因注释失真而删除某回调的通道描述
- **THEN** 该回调仍保持未接线，且没有新增任何消费者或事件变体

### Requirement: 选择手势类上游 API 的不采用依据须覆盖往返成本

上游 `selection::gesture` 一类以指针事件驱动选区、要求与终端变更串行的 API，其采纳
判定 MUST 以**拖动路径的跨线程往返成本**为主要判据，MUST NOT 仅以「DESIGN 禁多击选择」
为由——连击计数只是该族的一小部分，拖动与边缘自动滚动才是主体。

此类 API 与本仓的交互模型通常冲突：手势状态机不可跨线程，而拖动的每一帧都必须把结果
推到渲染侧才能重定位手柄；若每次 MOVE 都要一次带超时的跨线程查询，本仓现有的
「本地算边界 + 节流推送 + 松手提交」模型即被推翻。故评估时 MUST 先核对拖动路径当前的
往返形态，再判断该 API 是否能以低频事件（而非逐 MOVE）的形态接入。

MUST 同时逐项核对上游能力的语义优势是否已被现有实现覆盖；已被覆盖者不构成采纳理由。

#### Scenario: 不采用依据不是多击限制

- **WHEN** 记录上游手势 API 的不采用结论
- **THEN** 依据包含拖动路径的往返成本，且不只写「不支持多击选择」

#### Scenario: 低频接入点单独评估

- **WHEN** 评估上游手势 API 中仅在低频定时事件触发的部分（如边缘自动滚动）
- **THEN** 该部分与逐 MOVE 部分分开评估，不因整体不采用而一并排除

### Requirement: 规范内的路径指针必须指向现存文件

规范正文、表格与索引行里以代码路径形式给出的**本仓文件**指针 MUST 指向工作树中
现存的文件，MUST NOT 指向已删除或从未存在的路径——读者按图索骥落空时，规范就把
「去哪里看」这件事说错了。

历史性陈述（已删除文件的历史路径、用于说明「已随 X 移入 Y」的旧路径）不在此限，
但那类陈述 MUST 带「已删除／已退场／已移入」等状态标记，MUST NOT 读作现役指针。

判定与依据本就写在规范内时（台账行、索引行即是载体），失效指针 MUST 改为指向规范
内的判定载体，MUST NOT 靠一句「详见某文件」把结论托管给不存在的文件。

#### Scenario: 失效指针被发现即订正

- **WHEN** 规范里的本仓路径指针在仓库中不存在
- **THEN** MUST 订正为现存路径，或改写为不指向具体文件的表述

#### Scenario: 历史路径不冒充现役指针

- **WHEN** 规范提到一个已删除文件的历史路径
- **THEN** 该处 MUST 带「已删除／已退场／已移入」等状态标记

#### Scenario: 结论的载体是规范本身

- **WHEN** 某项判定与依据已完整写在规范的台账行或索引行内
- **THEN** 该行 MUST NOT 以「详见某文件」把结论托管给规范之外的文件

### Requirement: 禁改区待维护者裁定项台账

`docs/specification/TESTING.md` 要求而现行门禁缺席的，以及门禁工具覆盖面小于代码树的项，
全部落在 AGENTS.md 禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者
已裁定「都不改，只登记」，故这些项 MUST 维持登记状态，未经维护者明确同意 MUST NOT 实施。

登记 MUST NOT 只有落地方案而无事实基线：日后获授权实施的人 MUST NOT 需要重新调研
「改之前是什么样」。故每项 MUST 同时登记**当次实测基线**与**实施时的验收判据**，使门禁的
判定以基线为准而非以「无输出」为准。

台账落点为审计留档 `openspec/changes/archive/2026-11-24-code-health-audit/audit.md` 的
「禁改区待维护者裁定项台账」小节。

#### Scenario: 实测基线与验收判据

- **WHEN** 记录规范要求但尚未落地的门禁项
- **THEN** MUST 给出规范依据、落地方案与当次实测基线，不得以计划代替实测
