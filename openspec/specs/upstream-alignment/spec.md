# upstream-alignment Specification

## Purpose

本仓的终端状态完全由上游承担（`libghostty-vt` 绑定 ghostty），因此「本仓用的是
哪一版上游」与「上游那些权威终端的测试资产能不能用」都不是一次性调研，而是需要
被持续证伪的断言：依赖停在旧 rev 会让上游修复悄悄落后，而上游语料的「不引入」
结论一旦前提变化就变成过期的借口。本能力记录依赖跟踪口径、采纳门槛，以及已被
复查过的结论与它们各自的阻塞点。

## Requirements

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
- **THEN** 该次复查 MUST 记录所钉 rev 携带的上游终端提交号，使「本仓用的 VT 实现
      落后上游多少」可核对而非靠记忆

### Requirement: 上游测试资产只在不产生自验证时采纳

引入任何上游项目的测试体系、测试数据或测试样例时，MUST 满足全部三条：

1. 期望值来自**外部真相**（标准文本、上游协议文档、真机实测），MUST NOT 是该上游
   自身实现的输出——那是自验证；
2. 不引入无程序消费者的二进制资产（截图基线、快照图），以免 vendor；
3. 采纳后净增覆盖或净减本地代码，MUST NOT 只是把既有断言换个出处。

不满足者 MUST NOT 引入，MUST 在本规范中记录不引入的理由，使结论可被复查。

#### Scenario: 已复查的不采纳结论

- **WHEN** 复查上游测试资产
- **THEN** 每项都给出违反上述哪一条的理由；不得写成「其断言是自身期望值」这类
      未经验证的概括——本仓库曾据此把 xterm.js 的 vttest 派生语料误判为自验证，
      而它实为从 xterm 手抄的外部真相

#### Scenario: 阻塞条件消失后必须重新评估

- **WHEN** 曾因上游能力缺失而阻塞的测试资产，其前置能力出现在**本仓实际钉住的**
      上游提交里
- **THEN** MUST 重新评估是否引入，MUST NOT 沿用旧结论

### Requirement: 判定上游语料必须实测通过数

判定一个上游语料是否值得采纳时，MUST 实测「今天有多少用例直接通过」，并把该数字
写入记录；MUST NOT 凭印象或凭语料规模下结论。语料「是外部真相」与语料「可以整包
采纳」是两个独立判断：前者只说期望值的来源，后者还要看期望值本身对不对。

实测过程 MUST 不留常红的探针：判定完成后删除探针，只把数字与差异聚类写进记录。

#### Scenario: 判定完成不留红测试

- **WHEN** 为判定某语料写了临时探针
- **THEN** 判定结束后该探针 MUST 从仓库删除，MUST NOT 以「已知失败」形式长期保留

#### Scenario: 实测暴露的是工具缺陷而非产品缺陷

- **WHEN** 实测出现大面积差异
- **THEN** MUST 先核对测试侧渲染口径（空单元格补空格、光标标记等）再判定为产品
      行为差异；口径错误会把数十例误报成 VT 缺陷

### Requirement: 按输入是否格式合规分流采纳

上游快照类语料常把「某实现的历史行为」一并固化，其中包含参数个数不合规的序列
（标准未规定处）与当年尚未实现的序列。采纳时 MUST 按输入分流：

- 输入格式合规且语义唯一的用例：可直接采纳为外部真相；
- 输入畸形或标准未规定的用例：MUST NOT 直接采纳（那会让本仓与上游的历史缺陷对齐），
  MUST 另按标准文本写断言——「对标标准」才是正确性测试，「对标快照」只是换了个
  出处的自验证。

#### Scenario: 快照记录了不合规的历史行为

- **WHEN** 某用例的输入是标准未规定的畸形序列，或该序列在上游快照成文时尚未实现
- **THEN** 该用例 MUST NOT 直接采纳；其差异 MUST 按标准文本裁定，并以标准为准写断言

#### Scenario: 已采纳语料的落地形态

- **WHEN** 采纳一份需要外部文件的上游语料
- **THEN** 语料 MUST 内联进测试源码的常量表，
      MUST NOT 随仓另放数据文件，也 MUST NOT 在测试时联网获取
- **THEN** 形式上的理由 MUST 满足 TESTING 的两条约束：测试不依赖状态（裸
      `cargo test` 与 `nix develop` 下结果必须一致），且不得联网获取测试输入。
      随仓放文件会制造需要人手工同步的第二份真相，并迫使仓库存放上游许可证；
      联网获取则把「没跑获取步骤」误报成「产品有 bug」。内联同时避开两者。
- **AND** 内联 MUST 是原样搬运：期望屏幕逐字对应上游 `.text`，输入字节逐字对应
      上游 `.in`，任何改写都意味着期望值不再是外部真相。来源 MUST 在注释里指向
      上游仓库、目录与**标签**（不用提交号：提交号随 rebase/force-push 失效且读者
      无法还原）
- **AND** 测试 MUST 直接从该常量表读取，MUST NOT 提供回退路径；缺前置时 MUST 失败
      而不是跳过

#### Scenario: 判定表必须覆盖语料全集

- **WHEN** 语料目录里的某个用例既不在断言表、也不在「已判定不采纳」表
- **THEN** 用例计数断言 MUST 失败，逼使逐例给出结论；MUST NOT 静默跳过未知用例

### Requirement: 未采纳的用例必须逐例写明理由并收敛到空

未能采纳的上游用例 MUST 在测试文件里逐例记录**具体理由**（哪一段输入畸形、哪个序列
快照成文时尚未实现、哪条边界语义尚未裁定），MUST NOT 只写一句「已知不支持」。

理由 MUST 是**实测得出的根因**，MUST NOT 是对行为的想定：写「本仓按标准取首参数」
而实测是整条序列被上游丢弃，会让后来者按错误的模型去判断能否采纳。每条理由 MUST
指出缺口所在层（本仓 / 绑定层 / 上游终端），使「能否本地修」不必重新调研。

「尚未裁定」的条目 MUST 保持为可见待办并把该表收敛到空列为目标；一条长期存在的
「尚未裁定」等同于隐藏的错误，且 MUST NOT 以「按当前行为断言」的方式钉死——那是
回归测试，不是正确性测试。

#### Scenario: 理由必须可复核到源码位置

- **WHEN** 某不采纳用例的理由写「上游未实现」或「上游丢弃该序列」
- **THEN** 理由 MUST 给出上游源码中的分发位置（文件与匹配分支），使复查者不必重新
      做一遍实测即可核对；仅有结论没有出处的理由等同未记录

#### Scenario: 上游缺口以特征化断言钉住并作为重新裁定触发器

- **WHEN** 某不采纳用例缘于上游引擎缺口（本仓无法本地修复）
- **THEN** MUST 以特征化断言钉住该缺口的当前行为并注明标准要求与上游出处，使上游
      补齐后该断言立即失败，从而回到不采纳表逐例重新裁定；MUST NOT 只把缺口留在
      注释里（那不会随上游前进而失效）

#### Scenario: 存在未裁定的语义分歧

- **WHEN** 某用例输入格式合规但行为与快照分歧
- **THEN** 该用例 MUST 记入待裁定清单并写明分歧边界（哪条标准条款、哪类边界行为），
      MUST NOT 以「按当前行为断言」的方式钉死

### Requirement: VT 能力缺口按绑定层与终端分别记录

判断某个 VT 特性是否可用时，MUST 区分两层并分别记录：终端上游（如 ghostty 的
`main`）与绑定层（如 `libghostty-vt` 所钉的终端提交）。只记「上游没有」会把
「绑定层没跟进」误判为不可逾越。

#### Scenario: 特性在上游终端已实现但绑定层未跟进

- **WHEN** 某 VT 特性在终端上游 master 存在、在绑定层所钉提交中不存在
- **THEN** 记录 MUST 写明是绑定层滞后，并给出可复核的两处提交号

### Requirement: 上游 VT 语义不由本仓测试覆盖

VT 解析与重排是上游 `libghostty-vt` 的职责（DESIGN 单一来源，`ghostty_terminal/tests.rs` 删除集合已列明）。TESTING 只测本项目功能、不做回归，故 xterm 语料、缺口触发器与重排往返三类断言 MUST NOT 留存本仓：`native/tests/vt_corpus.rs`、`native/tests/vt_conformance.rs`、`native/tests/vt_upstream_blockers.rs`、`native/tests/reflow_round_trip.rs` 已删除。

历史实测零本仓 bug：语料引入修的是测试侧空格口径（43 例误报），随后两次改动分别订正多参理由与自造输入，内联改动只搬运语料；重排三条只加测试无产品修复。同口径的删除先例见归档变更「删除回归金标语料与重复着色器校验测试」与「删除未发现本项目可解问题的上游测试资产」。BDD（TESTING 覆盖范围）、`concurrency.rs`（`EventQueue`）与快照管线等本地单元测试保留。

#### Scenario: 上游语义测试不再引入

- **WHEN** 某上游 VT 语料或重排回路可直接通过
- **THEN** MUST NOT 以此为由引入本仓测试；通过只说明上游行为未变，不说明本仓功能正确

### Requirement: 同类安卓终端的测试做法须成对比较

同类项目（尤其同架构的）在本仓作出测试取舍时构成对照面。比较 MUST 落到具体文件与
断言形态，不得停留在「谁测试多」的数量比较上——数量只说明覆盖面，不说明断言是否锚定
外部真相。

#### Scenario: 对照项被采纳或被否决都要留下判据

- **WHEN** 参照某个同类项目决定本仓的测试写法（断言风格、语料落地形态、构建集成方式）
- **THEN** 记录 MUST 同时给出该项目的做法与它在本仓不适用或更优的理由
- **AND** 对照项若在测试中存在**结构性弱点**（如断言复述被验表达式、反射读写被测类的
      私有字段因而与实现结构强耦合），该弱点 MUST 记为「不得反向采纳」

#### Scenario: 覆盖率低于对照项目不构成理由

- **WHEN** 本仓某模块的测试用例数低于某个对照项目
- **THEN** MUST NOT 以此为增补理由；MUST 先判定该对照项目的断言是否锚定外部真相。
      断言复述实现或依赖被测类内部结构的用例数量高，只说明自验证面大，不说明覆盖好

### Requirement: 同类安卓终端的既有断言按「覆盖面缺口」而非「数量」采纳

对照项目（尤其同架构的 Android 终端）已存在的**具体断言**若恰好覆盖本仓的覆盖面
缺口，且其期望值来自外部真相而非该项目的实现输出，则 MUST 采纳为判据来源，并在
注释里注明出处谱系。采纳 MUST NOT 以「对方用例更多」为理由——那只能说明覆盖面。

#### Scenario: 断言按判别力而非存在采纳

- **WHEN** 采纳某个对照项目的断言作为本仓测试的期望值
- **THEN** 该期望 MUST 可独立于本仓实现验证（外部标准、外部既有实现、或写入的原文）
- **AND** 采纳前 MUST 验证判别力：把被验行为改坏（或换成恒真断言）后该断言 MUST 变红
- **AND** 若期望值与本仓现状冲突，MUST 先判定本仓实现是错的一方，不得为迁就对照而改断言

#### Scenario: 失效语料不得采纳

- **WHEN** 某对照项目仓库内存在测试语料文件
- **THEN** MUST 先确认有代码实际引用它；引用为空即为失效语料，MUST NOT 采纳
- **AND** 本仓已有等价覆盖时，采纳只会制造第二份真相

#### Scenario: 自验证的对照断言不得反向采纳

- **WHEN** 对照项目的某组用例只断「解析/反序列化不抛异常」或「字段等于刚写入的值」
- **THEN** MUST 记为不得反向采纳：这类断言对实现的错误无判别力
- **AND** MUST 另行判定该对照项目针对的**边界形态**本仓是否存在；若不存在（如本仓走
      typed JNI 返回值而无裸指针布局协议），记为不适用而非「覆盖不足」

### Requirement: 已复查结论登记在本规范内

本规范 MUST 自带逐项结论台账：每项上游测试资产 MUST 写明采纳门槛的判定结果，并给出
**可复核依据**（上游源码位置、语料中实际存在的字节、或按上游自己的驱动代码确认的无消费者
文件），MUST NOT 只写「其断言是自身期望值」这类未经验证的概括，MUST NOT 把结论只留在归档
变更里——归档是历史台账，不承担规范义务。

判定结果 MUST 在「违反某条门槛」「无门槛违反（并说明为何不违反或阻塞于何处）」之间显式
二选一，MUST NOT 留空：留空既可能被读成「尚未复查」，也会让「每项都给出违反哪一条的理由」
这条 Scenario 形同虚设。

许可证 MUST NOT 作为判定依据：本仓不引入许可证文件也不检查依赖许可证，故「许可证排除」
不是可用理由；因许可被排除的资产 MUST 改列其真正违反的门槛。

MUST 采纳的资产 MUST 附实测数字（今日直接通过的用例数与差异聚类），实测探针 MUST 在判定
完成后删除。差异 MUST 归因到具体层（本仓 / 绑定层 / 终端上游），MUST NOT 以「上游行为不同」
笼统带过；拟归因于同一类边界时 MUST 穷举全部涉及的码位而非抽样——数量不止少数几个时说明
存在需要单独裁定的边界，不能用一句概括收尾。

#### Scenario: 结论可脱离归档独立复核

- **WHEN** 复查某项上游资产的采纳与否
- **THEN** 该项结论与依据 MUST 能在本规范内读到，MUST NOT 需要先翻归档变更

#### Scenario: 依据是可重放的事实而非概括

- **WHEN** 某项被判定为「期望值是该上游自身实现的输出」
- **THEN** MUST 给出该期望值的产出位置或产出方式，使读者打开上游即可核对；
      MUST NOT 以「其断言是自身期望值」收尾

#### Scenario: 失效语料被识别而非整包采纳

- **WHEN** 某语料目录内存在无同名输入的期望值文件
- **THEN** MUST 按上游驱动代码确认该文件是否被读取；无人读取者 MUST 记为失效语料，
      MUST NOT 计入可采纳规模

已复查结论台账如下。

| 资产 | 门槛 | 可复核依据 |
| --- | --- | --- |
| alacritty `ref` 语料（45 例） | 1 | 期望值由 `write_ref_test_results` 克隆**运行中**的网格后序列化落盘，回放测试再读该文件逐格比对——期望值即其自身实现的输出 |
| wezterm `k9::snapshot!` 快照 | 1 | 快照宏由 `k9` 依赖提供，置位环境变量即自动改写期望值，期望值可被实现自身覆盖 |
| kitty `kitty_tests` | 1 | 断言打在 C 扩展暴露的白盒钩子（如 `test_render_line`）上，断言对象是内部结构而非屏幕行为 |
| termux `terminal-emulator` JUnit（19 个文件） | 按断言形态分流，不整包判 | 实测该目录内 `assertLinesAre`/`assertLineIs` 手写屏幕文本断言 **271 处**，而 `MockTerminalOutput` 仅出现 **3 处**；6 个文件带外部规范出处（`vt100.net/docs/vt510-rm/` 引用 16 处，另有 Unicode 6.2、chromium issue 212704、Mozilla bug 746900 等）。故**屏幕文本类断言门槛 1 不成立**（锚定 VT510 等外部文档），**回写类断言（`TerminalTest`、`OperatingSystemControlTest` 打在 `mOutput` 与包级私有字段上）门槛 1 成立**。此前把整包判为门槛 1 属未经验证的概括，已按此更正 |
| contour `test/*.sh`（21 脚本） | 1 | 脚本只发序列并等待，无期望输出文件；其中 7 个以 `read` 等待人工按键，判定者是人眼 |
| xterm.js vttest 派生语料（76 例） | 门槛 1 不适用（期望值是外部真相），按输入分流不整包采纳 | 期望值是从 xterm 手抄的屏幕文本（外部真相），但同时固化了 xterm 当年的畸形行为：语料中确有多参数的 `CSI 2;3D`（CUB 只取首参数）与快照当年未实现的 `CSI 3 b`（ECMA-48 标准序列，本仓已实现并更合规）。按「按输入是否格式合规分流」，只可采纳格式合规且语义唯一的子集 |
| rio `rio-vt/tests/sixel`（3 组） | 3 | `.rgba` 期望值确由三种外部编码器产出（外部真相），但采纳它要求本仓先实现 sixel——DESIGN 未要求该功能，属净增功能而非净增覆盖 |
| foot `tests/` | 无门槛违反：仓库内无资产可评估 | 仅 `meson.build` 与 `test-config.c`，无 VT 或网格语料；「无资产」是结论而非遗漏，复查时确认这一点即可 |
| esctest2（559 个 `test_*` 用例方法） | 无门槛违反：阻塞于绑定层缺能力 | 期望值是手写的外部真相（门槛 1 不成立），阻塞在别处：屏幕断言经 DECRQCRA 取矩形校验和——`AssertScreenCharsInRectEqual` 对每个待验字符调 `GetChecksumOfRect`，后者发 `DECRQCRA` 并读 DCS 应答。实测：在绑定层构建时取到的终端源码树（即 `libghostty-vt-sys` 的 `build.rs` 中 `GHOSTTY_COMMIT` 常量所指提交）内检索 `DECRQCRA` **零命中**，而终端上游 master 已具备分发与校验和计算 → 缺口在**绑定层**，前提未具备则 MUST NOT 引入 |

实测记录如下。

| 语料 | 实测 | 结论 |
| --- | --- | --- |
| UAX #29 `GraphemeBreakTest`（Unicode 18.0.0） | 共 1547 簇，剔除 241 簇含控制码位（换行/制表/擦除副作用，无法写入网格）后实测 1306 簇。本仓两条自有字素通道——渲染路径 `CellData`（`codepoint` + `grapheme_extra`）与快照路径 `CellSnapshot.graphemes`——**0 例互相不一致**；其中 922 簇与 UAX 期望逐码位相等；384 簇差异全部落在「首码位为无基字符」一类，首码位穷举恰为 7 个（U+0300、U+0308、U+094D、U+1160、U+11A8、U+200C、U+200D，均为组合标记或格式字符，终端单元无基字符可附着），属终端上游的单元模型。最长簇 6 个码位，单格上限从未触发，无截断 | 本仓搬运层对上游字素簇零缺陷，该语料对本仓无净增覆盖，不留存 |
| xterm.js vttest 派生语料（76 例） | 本次实测的是失效语料一项：`t0031-HPB.text` 无同名输入文件，且上游驱动代码只按 `*.in` 取用例，该期望值无消费者 → 1 例不计入可采纳规模。其余 75 例的通过数是归档复查的历史数字，**本次未重测**，不作为当前判定依据 | 失效语料已剔除；历史数字仅作背景，重测前 MUST NOT 引用它下结论 |

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

### Requirement: Android 侧终端项目的复查台账

本规范 MUST 收录对 Android 侧终端项目的逐项复查台账，每项 MUST 写明：实质（它到底是终端
模拟器、UI 壳、环境 SDK，还是无源码的空仓库）、门槛判定结果、以及可复核依据。

判定 MUST 按断言的期望值来源分流，不得按「测试数量多」判断：手写屏幕文本并注明规范出处者
门槛 1 不成立；断言对象是自身回写字节流、或期望文件可被实现自身覆写者门槛 1 成立；
断言打在本仓不实现的组件上者门槛 3 不满足。

台账中 MUST 同时登记与 `DESIGN.md` 直接冲突、后续复查不得反向采纳的做法，即使其来自
同类项目且实现完整。

#### Scenario: fork 与重写项目的价值按内容差异判定

- **WHEN** 某项目自称是某上游的 fork 或重写版
- **THEN** MUST 以文件内容比对判定其相对上游的新增与改动，MUST NOT 以项目自述替代；
      若其测试目录与上游逐字节相同且零新增测试，则其价值等同于上游，MUST NOT 因「来自某个
      fork」而高估

#### Scenario: CI 显示绿灯但仓内无测试

- **WHEN** 某项目的持续集成声称运行单元测试
- **THEN** MUST 核实仓内实际测试文件数量；为零者 MUST 记为 CI 虚假绿灯，不得计为有测试资产

#### Scenario: 与 DESIGN 冲突的做法被登记为不可反向采纳

- **WHEN** 复查中发现某同类项目实现了 `DESIGN.md` 明确禁止或要求相悖的行为
- **THEN** MUST 在台账中点名文件与行号并标注「不得反向采纳」，
      MUST NOT 因该实现「更完备」而记为可参考点

实测台账如下。本次复查的 25 项 Android 侧终端项目逐条列在下面，门槛列显式二选一：
「违反 N」指门槛不成立；「无门槛违反」指门槛均成立，后接阻塞或不采纳理由。

**需改造后采纳（4 项）：**

| 项目 | 门槛 | 可复核依据 |
| --- | --- | --- |
| `sylirre/ghostty-android-terminal` | 门槛 1/2/3 均不违反，但须逐条验判别力 | 17 个文件 / 160 个 `@Test`，全在 `androidTest`。期望值锚定外部协议：`EmulatorVtTest.java:423` 断 DSR 应答字面量 `\u001b[1;1R`、`:647` 断 XTWINOPS `\u001b[8;5;20t`；大小写折叠表由脚本从 Python `unicodedata` 生成。与本仓同架构（libghostty-vt + JNI），是最正当的对照面；但本仓已有超链接、搜索、xtwinops、下划线覆盖，净增有限 |
| `reapercanuk39/termux-kotlin-app` | 门槛 1 不违反（仅限键位编码与鼠标两类断言） | `KeyHandlerTest.java:31-45` 断言 termcap 能力名到字节序列（`kb`→`\u007f`、`kB`→`\033[Z`、`kUP`→`\033[1;2A` 等），出处为 terminfo 规范；`TerminalTest.java:59-65` 断言越界点击按边缘钳位为 `\033[<0;1;1M` 并注明 termux-app issue 501。二者均属本仓自有功能（键位编码、鼠标），非上游 VT 语义 |
| `CocaKova/charon` | 门槛 1 不违反（仅限外部标准期望值子集）；其 corpus 门槛 1 与「可被实现覆写」均违反 | 外部标准类：`ModernContractTest`（DECRQM/DECRQSS/OSC52 按 ECMA-48）、`KittyKeyboard*`、`Utf8DecoderTest`（WHATWG 算法）、`TextSelection`；另有 `tools/conformance_vttest.py` 把外部套件跑在自己的 pty 上。反例：`CorpusTest.kt:26` 支持 `-Dcharon.regenGoldens` 用自身 `render()` 覆写期望，5 个 `.golden` 均属此类，MUST NOT 采纳 |
| `Lord1Egypt/AndroidThothTerm` | 门槛 1 不违反，但仅作判据来源登记 | `emulatorview/src/test` 3 个文件 / 22 个 `@Test`，期望值为人工手写外部真相（如控制字符串不得绘制、选区按逻辑序还原）。其引擎为 jackpal/androidterm 血统，无 sixel/kitty，能力低于本仓上游，断言只能当边界清单 |

**不可采纳（21 项）：**

| 项目 | 门槛 | 可复核依据 |
| --- | --- | --- |
| `termux/termux-app` | 屏幕文本类门槛 1 不违反、门槛 3 不满足；回写类门槛 1 成立 | 实测手写屏幕文本断言 271 处、`MockTerminalOutput` 仅 3 处、外部规范出处 16 处。门槛 3 不满足的原因是断言打在本仓不实现的上游 VT 引擎上 |
| `sohan-f/termux-monet` | 门槛 3 不满足 | 与官方 `terminal-emulator` 的差异经逐文件核验为纯格式化，`WcWidth` 的零宽区间表逐字节相同；新增仅 Monet 取色 14 例，断言的是刚写入的调色板值 |
| `ownerroc/termux-monet` | 门槛 1 成立、门槛 3 不满足 | 测试是官方 termux-app 的**旧快照**（少 `ApcTest.java`），fork 零新增测试；主源码缺 `ITermImage`/`TerminalSixel`/`TerminalBitmap`，以其为参照等于对齐更弱的上游 |
| `Autonomous-Agentic-Workflows/termux-app` | 门槛 3 不满足 | `terminal-emulator/src/test` 与官方逐字节相同、零原创测试；且它是官方的**弱化子集**（缺六个源文件，OSC 重构关键标识命中 18 处 vs 官方 37 处） |
| `rafaelmeloreisnovo/termux-app-rafacodephi` | 门槛 1 成立 | `terminal-emulator/src/test` 与官方逐字节相同；其 `rafaelia` 测试整条包在环境条件里，native 可用时**空跑**，且 `cycleAlwaysWithinRange` 只断自身取值的范围，零判别力 |
| `Rprop/AnsolePlus` | 门槛 3 不满足 | 停在 termux-app 旧 rev：`TerminalEmulator.java` 相对上游 595 删 / 65 增，`doDcs` 出现 0 次（上游 9）、`sixel` 0 次（上游 49），缺 `TerminalSixel`/`ITermImage`/`TerminalBitmap`；零测试 |
| `Adhi-hub07/on3-terminal` | 无门槛违反：零测试资产 | 全仓无任何 `src/test`；其 `TerminalEmulator.kt` 是 termux-app 的 Kotlin 改写（字段与算法逐行同构，**仓内无出处声明，此判断为结构推断**），不构成净增内容 |
| `frostre1997/kosh` | 无门槛违反：无本仓测试资产 | VT 全在 Maven 依赖 `terminal-emulator:v0.118.3`，仓内无实现；仅 2 个 Android Studio 模板测试；其 proot 树与 `ReTerminal` 经 `diff -rq` 完全相同，二者同源 |
| `shafinthedev/ReTerminal` | 无门槛违反：无本仓测试资产 | 同上，VT 由 `core/main/build.gradle.kts:90-91` 的 Maven 依赖提供，仓内仅 2 个模板测试 |
| `GlobalTechInfo/RedTerm` | 门槛 3 不满足 | VT 走上游 jar，仓内无 `TerminalEmulator`；329 个用例覆盖 SSH/SFTP/distro/proot，本仓无此功能；`AnswerLatchTest.kt:46-56` 以真实时钟与固定 sleep 断时序，违反本仓「没有不稳定的测试」 |
| `mirute02/anoterm` | 门槛 1 不违反、门槛 3 不满足 | 124 个 `@Test`，期望值确为外部真相（`CharWidth.kt:1-36` 注明取自 Unicode `EastAsianWidth.txt`，UTF-8 用标量规则，VT 语义手写规格）；但断言对象是它自研的 VT 组件，本仓不存在该组件 |
| `8dmusichannels-star/terminator` | 无门槛违反：零测试资产，且 CI 虚假绿灯 | 全仓 `@Test` / `org.junit` 零命中，三个 Gradle 模块无 `testImplementation`，而 `.github/workflows/build.yml:24-25` 仍有「Run unit tests / gradle test」；另携带 12 个 ncurses 二进制 terminfo 条目 |
| `pavelc4/Rin` | 门槛 1 成立、门槛 3 不满足 | 全仓 34 个 `.rs` 中 `#[test]` 仅 4 个，全在包管理器模块且断自身默认值；终端侧零测试；`src/renderer/screen.rs:90-99` 的 `render_cell` 是空壳，实际绘制在 Kotlin 的 `Canvas` |
| `imlunixdev/puppy` | 门槛 3 不满足 | `TerminalBufferTest.kt` 10 例中 truecolor、OSC133 用例确为手抄标准，但覆盖面极窄；核心解析含已确认的标准偏差（`TerminalBuffer.kt:104` 的 `CSI J` 漏 `Ps=1` 且额外重置光标，`:105` 的 EL 遇空参会抛） |
| `shafinthedev/Ply` | 门槛 1 不违反但零判别力 | 无 VT 解析，`src/main.rs` 只把 `/system/bin/sh` 的 stdout 原样转发；两个测试均坏：`test_help_command` 断言从未打印的字符串，`test_echo_command` 测的是宿主 shell |
| `irealashu/ApexShell` | 无门槛违反：无 VT 测试资产 | 以正则 `\u001B\[[0-9;]*[a-zA-Z]` 冒充 VT 解析（`engine/AnsiParser.kt:12`），无光标、无擦除、无真彩色；仅 3 个 Android Studio 模板测试 |
| `deathlegionteamlk/jolz` | 无门槛违反：无源码可评估 | `git ls-files` 仅 `README.md`，README 自述「源码私有，仅公开 APK 与本 README」；技术栈声明无源码可核实 |
| `Sunbr99/TerminalOS` | 无门槛违反：仓库内无资产 | 全仓仅 `README.md`、`LICENSE` 与 3 行 `examples/monitor.sh`；README 描述的 Go 项目结构与 `tests/shell_test.go` 在仓库中一个都不存在 |
| `TOYTTQE/PufiX` | 无门槛违反：仓库内无资产 | 13 个 Python 文件的桌面 REPL 玩具，只**产生** ANSI 而从不解析，无网格、无 PTY；零测试 |
| `Malik1stmkw/termux-clone` | 无门槛违反：仓库内无资产 | 全仓仅 1 个 59 字节的 `README.md`，无任何源码或二进制；「终端模拟器」这一主张在仓内无支撑 |
| `eggie-admin/hydra-shell-android` | 无门槛违反：仓库内无终端资产 | 204 个文件中 Android 与原生源码为 0（62 md / 40 json / 39 py / 26 yml / 18 sh），APK 钉在外部仓库；现有测试全为 HTTP 与 adb 相关 |

**新增失效语料记录：**

| 项目 | 失效语料 | 依据 |
| --- | --- | --- |
| `Lord1Egypt/AndroidThothTerm` | `tests/` 共 **122 个文件 / 11 个子目录 / 864K** | 全仓（含 `.github`、`build.gradle`、构建脚本）对 `tests/scripts`、`tests/wideChars` 等目录的引用数为 **0**。其中 `tests/scripts` 的期望写在脚本末尾 `#expected screen:` 注释里，判定者是人眼；`tests/wideChars` 是散文加手打屏幕 |

**不得反向采纳的做法：**

以下做法即使来自实现完整的同类项目，也与 `DESIGN.md` 相悖，后续复查 MUST NOT 记为可参考点：

| 做法 | 出处 | 与 DESIGN 的冲突 |
| --- | --- | --- |
| 双击选词、三击选行 | `ghostty-android-terminal` 的 `TerminalView.java:1076-1096`、`terminator` 的 `TerminalView.kt:1275-1279` | DESIGN 明令不支持多击选择 |
| OSC 8 链接做 scheme 白名单 | `terminator` 的 `TerminalView.kt:1482-1487` | DESIGN 要求不检查链接可用性，点击后经系统 API 跳转 |
| 屏幕更新回调里无条件清零滚动位置并全量重绘 | `on3-terminal` 的 `TerminalView.kt:249-260` | 与 DESIGN 的脏跟踪、跳过干净快照相悖 |
| 在渲染热路径内改写终端状态 | `puppy` 的 `TerminalView.kt:33` 每帧 `onDraw` 调 `buffer.resize` | DESIGN 禁止渲染路径副作用 |
| 把字符串长度当字节数传给写入接口 | `on3-terminal` 的 `TerminalEmulator.kt:204-206` 传 `data.length`（UTF-16 码元数）给 `write(ByteArray, offset, count)` | 任何非 ASCII 粘贴即越界或截断；属字符串到字节数组的长度换算缺测试锚点 |
| ED 顺手重置光标 | `puppy` 的 `TerminalBuffer.kt:104` 的 `Ps=2/3` 额外置 `cursorX=cursorY=0` | 规范 ED 不移动光标 |
| 仓库内提交 CMake 生成物与构建产物 | `terminator` 的 `terminal-emulator/src/main/cpp/Makefile`（内含作者本机绝对路径）、`on3-terminal` 的 `app/build/outputs/apk` | 构建卫生，且违反不得引入无程序消费者的二进制资产 |

### Requirement: 复查的否定结论不得用未经验证的概括

台账中「其断言是自身期望值」这类判断 MUST 以该项目的实际断言形态统计为依据，
MUST NOT 仅凭测试基类的存在即整包判定。

#### Scenario: 整包判定被实测推翻时必须更正

- **WHEN** 实测显示某项目的断言主体与其被判定的形态不符
- **THEN** MUST 更正既有台账行并写明更正前后的统计数字，
      MUST NOT 在保留旧结论的同时只补充新依据

#### Scenario: 台账正文 MUST 写在 Requirement 块内

- **WHEN** 向 openspec 提交规范 delta 并归档
- **THEN** 台账等正文 MUST 位于某个 `### Requirement` 块之内，MUST NOT 放在与
      `## ADDED Requirements` 同级的标题下——归档只同步 Requirement 块内正文，
      同级标题下的内容会被静默丢弃且校验仍然通过
- **AND** 归档后 MUST 核对目标规范文件确实含有所写入的条目数，MUST NOT 仅凭
      `openspec validate` 与 markdownlint 通过即认定内容已落地

---

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
| `terminal.rs`：`on_clipboard_write` 必须 `reply` | 采用（已修） | 上游文档「返回前必须 `reply`，否则拒绝写入」；本仓 `internal.rs` 回调此前零 `reply`。现满通道回 `Busy` 否则 `Ok` |
| `style::Underline` 五形状判定 | 采用（已修） | 三处 `matches!` 收敛为 `has_visible_underline`/`is_double_underline` 同源函数 |
| `Terminal::vt_write` 二进制安全 | 采用（已修） | `pty_write` 曾把 `>0xF7` 改空格，破坏 Kitty 直接 RGB 等二进制载荷；现仅剔除 NUL，与 `vt_write` 同策略 |
| 调色板与字节转浮点 | 采用（已修） | 渲染侧 `CATPPUCCIN_MOCHA_BACKGROUND` 与手写除法已删，以 `catppuccin_mocha_palette` 与 `byte_to_float` 为单源 |
| `selection` 交叉翻转归属 | 采用（已修） | 对标 termlib `applyHandleDrag` 五元组：回传新归属柄并收紧等值判据，调用方更新 `handleDragState` |
| `search::Search` 全文搜索 | 不采用 | DESIGN 要求「使用库（如 `regex`）实现文本搜索」；自研 `regex` 搜索合规，上游 `Search` 的视口导航语义与本仓批量查询不一致 |
| `selection/gesture` 手势、`select_line`/`select_output`、`format_selection_*` | 不采用 | DESIGN 禁双击三击多击选择；格式化器已选用 `fmt::Formatter`，双格式化器并存需注释说明而非迁移 |
| termlib `trimDetectedUrl`/括号计数/跨行拼接 | 不采用 | DESIGN「链接识别通过 `libghostty-vt` 查询，没有额外检查」即禁止移植；仅保留空白 trim 与协议白名单（属安全检查） |
| `on_bell`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_desktop_notification`/`on_progress_report` 等回调 | 不采用 | Kotlin 侧零消费者；DESIGN「不做不必要功能」，有消费者时再注册 |
| `track_grid_ref`、`width_px`/`height_px`、光标色实时查询、`set_mode`、`vt_write_until_ground` | 不采用 | 按需取用；现有轮询与原子镜像满足当前消费者，DESIGN 要求最小实现 |
| `key::Encoder` 按键编码 | 待办 | Kotlin `TerminalInputEncoder` 318 行手写表绕过上游编码器，kitty 键盘协议与应用模式丢失；阻塞于 Android 键码到上游 `Key` 的映射裁定与 JNI 桥形态，另立变更 |
| `Terminal::paste` 粘贴 | 待办 | 生产粘贴未走上游粘贴事件与 `is_safe` 拒绝流程；阻塞于 IME 提交路径与模式查询归属裁定，另立变更 |
