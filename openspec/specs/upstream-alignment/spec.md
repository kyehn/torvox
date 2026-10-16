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
- **THEN** 该次复查 MUST 记录所钉 rev 携带的上游事实（上游提交日期、与上游 master 的
      提交数差、本次实际消费的 API 面），使「本仓用的 VT 实现落后上游多少」可核对
      而非靠记忆

#### Scenario: rev 本身不进正文

- **WHEN** 记录上述事实
- **THEN** MUST NOT 把 git 长/短 hash 写进规范、提交消息或注释（`STYLE.md` 明禁）；
      rev 的唯一载体是 `Cargo.toml` 的依赖钉——那是构建清单的功能性声明，不是正文。
      「落后多少」由提交日期与提交数差表达，与 hash 无关

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
| `CocaKova/charon` | 门槛 1 不违反（仅限外部标准期望值子集）；其 corpus 门槛 1 与「可被实现覆写」均违反 | 外部标准类：`ModernContractTest`（DECRQM/DECRQSS/OSC52 按 ECMA-48）、`KittyKeyboard*`、`Utf8DecoderTest`（WHATWG 算法）、`TextSelection`；另有 `tools/conformance_vttest.py` 把外部套件跑在自己的 pty 上。反例：`CorpusTest.kt:26` 支持 `-Dcharon.regenGoldens` 用自身 `render()` 覆写期望，5 个 `.golden` 均属此类，MUST NOT 采纳；取其 IME 契约经验时，`updateSelection` 经 `javap` 实测为框架内部 @hide（API 33/36/37 公开 `InputConnection` 均无此符号，仅 IME 侧类含之），应用不可覆写，记为不采纳 |
| `Lord1Egypt/AndroidThothTerm` | 门槛 1 不违反，但仅作判据来源登记 | `emulatorview/src/test` 3 个文件 / 22 个 `@Test`，期望值为人工手写外部真相（如控制字符串不得绘制、选区按逻辑序还原）。其引擎为 `jackpal/Android-Terminal-Emulator` 血统，无 sixel/kitty，能力低于本仓上游，断言只能当边界清单 |

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
| `RohitKushvaha01/ReTerminal` | 无门槛违反：无本仓测试资产 | 同上，VT 由 `core/main/build.gradle.kts:90-91` 的 Maven 依赖提供，仓内仅 2 个模板测试 |
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
| 把自研 VT 核留作「Ghostty 集成失败时的回退核」 | `kshivang/BossTerm`（`bossterm-core-mpp`，报告原话「if Ghostty integration fails, BossTerm is fallback」） | DESIGN 锁定终端状态单一来源，不设第二 VT 核；引入回退核即违单一来源 |
| WebView 远程终端作为移动端形态 | `marshallku/copad` 的 `copad-mobile` | DESIGN 无网络终端面（未声明）；终端状态与渲染须在本机（本仓 kotlin + Rust 本地栈） |

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
| 调色板与字节转浮点 | 采用（已修） | 渲染侧 `CATPPUCCIN_MOCHA_BACKGROUND` 与手写除法已删，以 `theme` 单源（构建期 alacritty-theme）为唯一入口，按标识取用（`theme::default_dark`／`theme::by_name`）；中间 epoch 用过的 `GhosttyTerminal::default_palette` 已随「主题改按标识在原生单点解析」删除 |
| `selection` 交叉翻转归属 | 采用（已修） | 对标 termlib `applyHandleDrag` 五元组：回传新归属柄并收紧等值判据，调用方更新 `handleDragState` |
| `search::Search` 全文搜索 | 采用（2026 改判） | 自研 `regex` 搜索曾被判定合规，实为重复实现：软换行跨行、随输出增量同步、命中总数与导航滚动上游全已提供。现 VT 线程持有 `search::Search`（同生命周期的局部变量，非自引用结构），needle/总数/导航/视口命中均走上游，本仓只做「上游选区 → 按物理行网格区间」转换。代价：上游仅 ASCII 折叠、无区分大小写开关，`DESIGN.md` 修饰键栏节原「大小写匹配」与「使用库（如 `regex`）实现文本搜索」两条**已由维护者改写**（删去大小写匹配按钮、改写为使用 libghostty-vt Search，与上游口径一致）；`regex` 依赖随之删除。软换行/宽字符/CJK/真实空格的用例语义按上游口径重写，见 `text-search` 规约 |
| `selection/gesture` 手势、`select_line`/`select_output`、`format_selection_*` | 不采用 | 连击语义与 `DESIGN.md:130`「不得支持 双击 三击 多击选择」正面冲突，该族主体（拖动手势 + 边缘自动滚动）另有两条理由：其自动滚动按定时器在边缘区持续滚动，与 `text-selection`「每次触点移动恰好滚 1 行、静止不滚」相反；其手势状态若由 VT 线程持有，则每个触摸帧一次跨线程往返，使拖动快路径的无跨语言调用优势尽失，并与视图侧手势所有权形成双状态机。四项上游优势的覆盖情况：词派生、跟踪选区、随滚动跟随、复制格式化已走上游；宽字符吸附的事实取自上游网格单元宽度；边缘滚动节奏不采纳；单一来源已由 `terminal.rs` 承担。 |
| termlib `trimDetectedUrl`/括号计数/跨行拼接 | 不采用 | DESIGN「链接识别通过 `libghostty-vt` 查询，没有额外检查」即禁止移植；仅保留空白 trim 与协议白名单（属安全检查） |
| `on_bell`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_desktop_notification`/`on_progress_report` 等回调 | 不采用 | Kotlin 侧零消费者；DESIGN「不做不必要功能」，有消费者时再注册。`on_bell` 曾被 7 处注释误记为已接线（`ghostty_terminal/commands.rs` 的 `RunConfig` 悬空附着于 `cell_size_px`、`types.rs` 的 `EVENT_CHANNEL_CAPACITY`、以及 `session.rs` 的 5 处收割路径文档），实际该通道只承载 OSC 52 剪贴板写入；注释已删除，未新增回调 |
| `track_grid_ref`、`width_px`/`height_px`、光标色实时查询、`set_mode`、`vt_write_until_ground` | 不采用 | 按需取用；现有轮询与原子镜像满足当前消费者，DESIGN 要求最小实现 |
| `key::Encoder` 按键编码 | 采用（已修） | Android 键码到上游 `Key` 为机械对应表（`keymap.rs`），桥形态与 `Query::EncodeMouseEvent` 同构（编码在 VT 线程）；`TerminalInputEncoder` 仅保留输入法文本路径 |
| `Terminal::paste` 粘贴 | 不采用 | 含换行文本返回 `Error::Rejected` 并要求确认框后 `allow_unsafe` 重试，PROHIBITED 明禁粘贴确认对话框；恒置 `allow_unsafe` 则检查失去意义 |
| `paste::encode` 粘贴编码 | 采用（已修） | 无终端依赖的纯函数：bracketed 标志取活体模式 2004，控制字节中化与括号包裹由上游承担；`encodePaste` 查询跑在 VT 线程，Kotlin 只做截断与字节流切块 |
| `paste::is_safe` | 不采用 | 其拒绝流程同样要求确认对话框，PROHIBITED 明禁 |
| `device_attributes` 回调 | 不采用 | 缺省 trampoline 已按默认属性应答（实测 DA1=`?62;22c`，`test_output_capture_da` 钉住）；自定义 132 列等属性需先验证 DECCOLM 通路，暂无消费者要求 |
| `xtversion` 回调 | 不采用 | 缺省回退 `libghostty` 已应答；自定义名字无功能增益 |
| `color_scheme` 回调 | 不采用 | 注册 None 等价于未注册；无 OS 级来源，不猜 |
| `set_terminfo_name` | 不采用 | 未设置则 TN 查询不报告；声称值需先审计能力一致性，暂无消费者要求 |
| Kitty 文件/共享内存介质 | 不采用 | 直传已覆盖主路径；文件介质是新能力面（沙箱路径裁决），需产品确认，另立变更 |
| `kitty::graphics::RustPngDecoder` | 采用（已修） | 本仓原先自写 60 行 `KittyPngDecoder`，注释称「上游 RustPngDecoder 无公开构造器」——该结论与上游源码不符：`pub const fn new()` 存在，且本仓已开 `png` 特性。自写版按未经上限校验的 PNG 头尺寸在全局堆分配，绕过的正是上游分配器上限 |
| `style::RgbColor::parse` / `perceived_luminance` | 采用（已修） | 主题色值解析与深浅分组原为本仓手写（`#RGB` 展开、Rec.601 加权 0.299/0.587/0.114）；两者都取自上游，阈值语义与 ghostty 一致 |
| `on_clipboard_read` | 不采用 | 上游要求在回调内同步应答（VT 流暂停）。本仓的应答链必须异步：剪贴板内容来自 Android 系统剪贴板，经 `pollEvent` 上报后再回写 PTY；若改走上游回调，VT 线程要阻塞到 Kotlin 应答，终端在应答期间冻结。识别仍需自研扫描（上游回调装上后才拦截得到），故净收益为负 |
| `fmt::Formatter` 取全量纯文本 | 不采用 | 文档写明只覆盖 active screen，而本仓 `ReadAllText` 的口径是「回滚 + 视口」；上游补回滚参数后才可能跟进 |

### Requirement: 六大上游对标结论按「能力 → 本仓现状 → 差距 → 阻塞层」登记

对标对象固定为六个：`sylirre/ghostty-android-terminal`、`termux/termux-app`、
`mrndstvndv/termux-ghostty`、`connectbot/termlib`、`libtermux/libtermux-android`、
`Uzaaft/libghostty-rs`（后者以 `Cargo.toml` 所钉 rev 为准）。每项 MUST 记录上游的
**实测做法**（文件与符号级），本仓现状，差距，以及差距可修的层；MUST NOT 以
「上游更好」这类概括代替定位。

#### Scenario: 复查结论可逐项复核

- **WHEN** 读到某项对标结论
- **THEN** 该项的上游做法、本仓现状、差距与阻塞层都能在表内读到，不需重新克隆上游

2026-10-12 复查台账（上游均取当日 `main`/master 浅克隆）：

| 能力 | 上游实测做法 | 本仓现状 | 差距 | 阻塞层 |
| --- | --- | --- | --- | --- |
| 全文搜索实现 | `ghostty-android-terminal` 在 `app/src/main/cpp/terminal_jni.c` 自研：`run_search` 全屏扫描、`search_cs` 保留区分大小写、`case_fold.h` 生成 BMP 折叠表、`search_dirty` 触发重扫、`MAX_MATCHES` 封顶；其注释写明「libghostty-vt has no built-in search」（所钉版本早于上游引入 search） | 走上游 `search::Search`（本规范「采用（2026 改判）」行） | 区分大小写：上游仅 ASCII 折叠、无开关 | 上游（`SearchOption` 只有 NEEDLE/SELECT_*，需上游新增才可能跟进） |
| 搜索命中高亮 | 同上：当前命中装为终端选区（`search reuses the selection slot`），高亮走选区通道 | 独立 `render_state.search_highlights` 通道，由原生按上游命中直接生成 | 多一条高亮通道 | 本仓（可选：复用选区槽可再删通道，代价是命中与选区互斥） |
| 搜索导航上下文 | 同上：导航从 Java 侧重跑扫描，陈旧屏幕坐标不需跟踪引用 | 导航走上游 `select_*`，视图侧把命中行居中 | 无 | 无 |
| 格子数据跨语言传输 | `termux-ghostty` 把脏行序列化为直连 JNI `ByteBuffer`（UTF-16 长度/折行标志/连续 cell 起点长度宽度/u64 样式/UTF-16 字符，8 字节对齐） | 网格数据全程在 Rust 内（flume 通道 + 脏跟踪），不经 JNI | 不适用 | 无（我们的路径本就不跨 JNI，该优化无对象） |
| 修饰键栏 | `termux-app` 的 `termux-shared/.../extrakeys/`（`ExtraKeysInfo`/`ExtraKeysView`/`ExtraKeysConstants`/`SpecialButton`）含布局编辑器与按键配置 | 仅采纳默认布局与粘滞语义 | 布局编辑器 | PROHIBITED（明禁） |
| 选区拖动手柄 | `termlib` 的 `applyHandleDrag` 锚点语义 + 交叉翻转 | 已采纳（同源实现与单测） | 无 | 无 |
| Linux 用户态 SDK | `libtermux-android` 提供 `core`/`os`/`terminal-view`/`bootstrap-*`/`shizuku` 模块（README 自述实验阶段） | 自持 bootstrap 安装与终端视图 | Shizuku 集成 | PROHIBITED（明禁）；其余能力已由本仓自实现覆盖 |
| 其余 libghostty-rs 能力 | `gesture`/`select_output`/`format_selection_*`/`snapshot decoder`/`on_bell`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report`/`on_desktop_notification`/`device_attributes`/`xtversion`/`color_scheme`/`set_terminfo_name`/`track_grid_ref`/`width_px`/`height_px` | 均未采纳 | 无消费者 | 本仓（按需取用，见「上游 API 取用判定」表） |
| Kitty 图像 PNG 解码 | `kitty/graphics.rs:997` 的 `RustPngDecoder`（`pub const fn new`）经 `set_png_decoder` 注册；解码前先按 libghostty 分配器取定长缓冲，未受该上限约束的尺寸不会被 PNG 头放大 | 已采纳（本次改判）：删除本仓 60 行等价实现与 `png` 直接依赖 | 无 | 无 |
| 颜色语法与深浅判定 | `style.rs:390` `RgbColor::parse`（3/6/9/12 位 hex、X11 名、`rgb:`/`rgbi:`）与 `style.rs:415` `perceived_luminance`（文档写明「背景亮度超过 0.5 即视为浅色」） | 已采纳：主题色值与深浅分组都走上游，本仓不再手写 `#RGB` 展开与 Rec.601 公式 | 无 | 无 |
| 终端模式查询 | `terminal.rs:1380` `Mode::BRACKETED_PASTE`、`terminal.rs:1367` `Mode::FOCUS_EVENT` 具名常量 | 已采纳：`Query::ModeGet` 携带上游 `Mode`，删掉 `(u16, u8)` 裸号与各调用点的数字 | 无 | 无 |
| `fmt::Formatter` 全量纯文本 | `fmt.rs:163` `Formatter::new(terminal, opts)` 文档写明「a terminal's **active screen**」，只覆盖活动屏 | 不采纳：本仓的 `ReadAllText` 覆盖回滚 + 视口（`scrollback_len + grid_rows`），格式化器取不到已滚出的行 | 逐格 FFI 扫描（每格 `grid_ref`→`cell`→`codepoint`）在回滚填满时单趟可越过查询超时，靠 `all_text` 缓存兜住 | 上游（格式化器需补回滚范围参数） |
| `on_clipboard_read` 同步回调 | `terminal.rs:2754` 回调文档：应答须在回调返回前完成，回调期间 VT 流暂停；不装回调则 OSC 52 读取被忽略 | 不采纳：应答链是「VT 线程 → 会话队列 → `pollEvent` → Kotlin 读系统剪贴板 → JNI 回写 PTY」的异步链，改用回调须让 VT 线程阻塞等待 Android 侧剪贴板应答 | 读取请求的识别自研逐字节状态机（`output_processor.rs`），应答由本仓拼 OSC 52 帧 + base64 | 本仓（架构取舍：宁可不走上游回调，也不冻结 VT 线程） |

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

仅剩一处经实测对照后保留 JSON，逐项记录理由与替代方案的规模，避免后来者重复调研；
保留项的字段一经变更 MUST 同步其单测，MUST NOT 把解码失败改写为空值：

| 边界 | 形状 | 保留理由 | 直接量替代方案的实测规模 |
| --- | --- | --- | --- |
| `pollEvent` / `pollResult`（`event.rs`、`ffi.rs`） | 单次调用返回一个事件，3 种变体各自 2–3 个字段，`exit.code` 需表达 `null` 与 0 之别；剪贴板载荷长度不可预知 | 变体判别 + 「一次取出后即消费」的语义由一次 JSON 解码天然保证；拆成标量需跨调用保存槽位，而槽位要引入并发可见性与「取用顺序」两个新失败面 | 至少 1（判别）+ 4（文本/标识/码点/选择区）次调用，外加一处会话级槽位与其线程安全；剪贴板文本还须再加一段长度前缀与分段拉取协议 |

#### Scenario: 保留项不得腐化

- **WHEN** 修改上述 JSON 边界的字段
- **THEN** 对应单测（`PollEventTest`、`ParseEventTest`）MUST 同步更新，
  且 MUST NOT 把反序列化失败改写为空值——空值与「真的没有」必须可区分

已按本标准收敛的形状（供对照，勿回流 JSON）：主题（`theme-configuration`：
`getThemeCatalog` 一次取回整个目录，`u16` 主题数 + 逐条「标志 + 名称长度 +
名称 + 54 字节颜色」）、字体信息（`getFontInfo` 的 `u32` 标志 + 三个 `f32` +
`u16` 名称数 + NUL 分隔 UTF-8 名称，CJK 状态按判别序号传递）、搜索结果
（`searchSelect` 扁平 `IntArray`，命中总数由 `searchSetNeedle` 返回）、会话列表
（`listSessions` 扁平 `LongArray`）、会话标题（`getSessionTitles` 的字符串数组，
按传入顺序对应）。

### Requirement: 不采用的上游回调不得留有通道描述

判定为不采用的上游回调 MUST NOT 在代码注释、结构体字段文档或常量说明中以「事件通道」
「每次振铃一个空消息」等形态描述为已存在的实现。若某回调已判定不采用而代码中仍见其
通道描述，MUST 删除该描述，MUST NOT 顺手接线回调——接线需先有 `DESIGN.md` 声明与
消费者，缺一即属未声明功能。

本仓的 OSC 回调事件通道只承载 OSC 52 剪贴板写入，相关容量常量与字段文档 MUST 只描述
剪贴板写入。

#### Scenario: 悬空通道注释被删除

- **WHEN** 结构体中某字段的文档注释描述了一条并不存在的上游回调通道
- **THEN** 该描述被删除，相邻字段保留其自身文档，不因删除而并入邻字段

#### Scenario: 事件通道容量说明只写剪贴板

- **WHEN** 查阅 OSC 回调事件通道容量的说明
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

### Requirement: OSC 52 读取请求的收割与应答须对每个会话有界

OSC 52 读取请求进入的会话队列 MUST 每帧被排空——**包括后台会话**：只排活跃会话时，
后台程序持续发读请求会让队列进一条、从不取出（内存无界增长），且那些程序永远收不到
应答。队列字段文档 MUST 只写「每帧排空」这一约束，MUST NOT 声称别的机制（如下游上限）
能替代它。

削减分支（单会话未作答数超限）MUST 另设**每帧**回写上限：削减本身要同步写 PTY，
子进程不读 stdin 时每次写入可等到数十毫秒，而该代码跑在渲染线程上；一个 8KB 输出块
即可塞约 900 条读请求，不封顶等于把渲染线程整帧交给病态远端。超出每帧上限的请求
MUST 不作答（不写即拒绝）。

#### Scenario: 后台会话的读请求被应答

- **WHEN** 后台会话的输出里含超过单会话上限的 OSC 52 读取请求
- **THEN** 请求队列被排空，且 PTY 上出现空串应答（削减路径的外部可见结果）

#### Scenario: 削减回写每帧有上界

- **WHEN** 单帧内的削减回写次数远超每帧上限
- **THEN** 实际回写次数不超过上限，且至少发生过一次（削减分支并未整体失效）

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

### Requirement: OSC 覆盖逐项核对

外部终端生态的 OSC checklist（xterm/ECMA-48 常见段：0/1/2、4、7、9、10/11/12、
52、104、133、1337、777）MUST 在本规范内有一张逐项核对表，写明每一段在本仓的
行为归属（上游解析并观察 / 上游处理且有测试 / 默认色归内嵌方走原生 setter /
本仓收割应答 / 不接）与可复核依据（文件与行号）。表 MUST 覆盖各段而非概括表述；
上游不处理或本仓不消费的段 MUST 注明理由，MUST NOT 留空。

生态参照面为 `8dmusichannels-star/terminator` 的 OSC 注释清单（该仓 CI 虚假绿灯、
零测试资产，仅其 checklist 构成对照）；判定口径与「上游 API 取用判定登记」一致——
本仓不自建 OSC 解析器（`output_processor.rs` 模块文档），除 OSC 52 读取请求的
收割外不再自研识别。

#### Scenario: 某段 OSC 的行为可脱离代码检索直接读到

- **WHEN** 复查某个 OSC 段在本仓的行为
- **THEN** 在「OSC 覆盖逐项核对」表内读到该段的行为归属与文件行号依据，
      MUST NOT 需要先翻归档变更或全仓 grep

#### Scenario: 上游不处理的段注明归属

- **WHEN** 某 OSC 段的语义上游不处理（如默认色查询）
- **THEN** 表内 MUST 写明其归属（内嵌方）与本仓的置色入口（原生主题 setter），
      MUST NOT 记为「上游处理」

核对表如下。

| OSC 段 | 本仓行为 | 依据 |
| --- | --- | --- |
| 0 / 2（图标名 / 窗口标题） | 上游解析；标题经 `Query::Title` 与快照 `title` 观察 | `internal.rs` 标题查询与快照 title；`tests.rs` 的 OSC 0/2 标题可读用例（含跨分块重组） |
| 1（图标名查询） | 上游解析；本仓无消费者，不另设查询 | 标题查询已承载用途；`on_title_changed` 类回调不采用见「上游 API 取用判定登记」 |
| 4（调色板覆盖） | 上游处理且有测试钉住生效路径 | `internal.rs` 初始主题注释；`osc4_palette_override_reaches_dumped_grid` |
| 7（工作目录） | 上游解析，本仓不消费；透传行为有测试 | `output_processor.rs` 模块文档与 `osc7_and_osc8_pass_through`；`on_pwd_changed` 不采用见「上游 API 取用判定登记」 |
| 8（超链接） | 上游解析；本仓经 `hyperlink_at` 查询网格单元 URI | `internal.rs` `hyperlink_at_impl`（4KiB 内联缓冲 + 越界精确重读） |
| 9（iTerm2 通知） | 不接：零消费者 | 与 `on_desktop_notification` 同一判定（DESIGN 未声明通知语义） |
| 10 / 11 / 12（默认前背景色 / 光标色查询与设置） | 上游不处理，默认值归内嵌方；本仓经原生主题 setter 置默认色 | `internal.rs` 初始主题段落三处注释（默认色归内嵌方、OSC 4 覆盖归上游、故走原生 setter 而非手写 OSC 序列） |
| 52（剪贴板读写） | 写走上游回调；读取请求由本仓扫描器收割并拼帧应答 | `upstream-alignment`「OSC 52 读取请求的收割与应答须对每个会话有界」；`output_processor.rs` `ReadScan` 状态机 |
| 104 / 110 / 111 / 112（色重置） | 上游处理；跨分块重组有测试 | `tests.rs` OSC color reset 跨分块用例 |
| 133（shell 集成标记） | 上游解析（若支持）；本仓不消费 | DESIGN 未声明 prompt 标记语义；`upstream-alignment` 预测线条目判「未声明，需先补 DESIGN」 |
| 1337 / 777（媒体 / 桌面通知） | 不接：零消费者 | 同 OSC 9 判定 |

#### Scenario: 新增 OSC 消费者时先补 DESIGN

- **WHEN** 拟为某个 OSC 段新增本仓消费者（如 cwd 展示、桌面通知）
- **THEN** MUST 先在 `DESIGN.md` 声明该功能再接线，
      MUST NOT 以「上游已解析」为由直接消费

### Requirement: 研究报告逐份结论登记在本规范内

已退役的 `docs/research` 全部研究报告（编号文档集 31 份 + 逐仓报告 73 份 + `ecosystem.md`）
的可执行项 MUST 在本规范内有逐份可复核的结论：
每项 MUST 写明判定结果（采纳 / 不采纳及违反哪条采纳门槛 / 未声明不立项 /
PROHIBITED 明禁）与依据，MUST NOT 只以「其余若干份已核对」这类群体概括收尾，
MUST NOT 把逐份结论只留在归档变更里。

判定口径：外部语料的期望值若为其自身实现的输出（可被实现覆写、或固化了该实现
当年的偏差），门槛 1 成立，不采纳；断言打在本仓不实现的第三方 VT 组件上，门槛 3
不满足，不采纳；功能若 `DESIGN.md` 未声明，按 `STYLE.md` 不实现，登记为不立项；
撞 `PROHIBITED.md` 者登记为明禁。许可证 MUST NOT 作为判定依据。

#### Scenario: 任何一份研究报告都有可复核行

- **WHEN** 复查任一份已退役 `docs/research` 报告的结论
- **THEN** 在本规范的本表或相邻台账表内读到该报告的判定与依据，
      MUST NOT 需要先翻归档变更

#### Scenario: 漏登文档被发现时补行而非改口径

- **WHEN** 复查发现某份报告在既有台账中无逐份结论（如成文早于台账归档而漏登）
- **THEN** MUST 补该报告的判定行并说明此前漏登的事实，
      MUST NOT 把它并入「其余若干份」的群体概括

#### Scenario: 错裁按事实更正

- **WHEN** 某报告被群体口径错裁（如把同引擎对标仓按「自研 VT」裁掉）
- **THEN** MUST 按该报告真实内容重新判定并写明更正前后结论

### Requirement: 语料判定补齐行台账

下表补齐既有「已复查结论台账」未覆盖的语料资产。既有表中的行继续有效。
本表每行 MUST 写清资产、判定与可复核依据；复查中发现未登记的语料资产 MUST 补行，
MUST NOT 以概括表述代替逐项判定。

| 资产 | 判定 | 可复核依据 |
| --- | --- | --- |
| ghostty `corpus/stream-initial` 01–24 | **采纳**（已落地） | 上游自家规范种子，期望按种子名声明的意图外部可推；落地为 `native/tests/external_benchmarks/upstream_corpus.rs`，按发布标签取数、清单按名排序并断条数，断言本仓接线回读 |
| migueldeicaza/SwiftTerm 175 个 Swift 测试文件 | 不采纳（门槛 1） | 期望值是其自研 VT 实现的输出；与 Ghostty 的 fuzz 语料对照思路已由上条落地 |
| techsenger/jeditermfx `src/test/resources/testData` before/after 对与 vttest drift | 不采纳（门槛 1） | JediTerm 为第三方 VT，期望文件即其自身输出；与 `JetBrains/jediterm` 的 vttest 语料同口径 |
| pg83-shitty `tst/`（自述 >5000 例）及其收获清单（esctest/xterm vttest/libvterm 等） | 不采纳（门槛 1 + 穷举扩张禁止） | lib/vterm 为第三方自研 VT，黑盒快照属自验证；esctest/vttest 全量维持「不扩张成穷举 VT 一致」（terminal-state-regression） |
| not-elm/orzma_vt `tests/` | 不采纳（门槛 1 + 3） | 第三方 VT 差分，非上游资产 |
| paulrobello/par-term 与 par-term-emu-core 的 `tests`/`fuzz` | 不采纳（门槛 1 + 3） | 同上，第三方 VT 差分；其 fuzz 语料非上游资产 |
| midasdf/zt `/tests` sequence 输入 | 不采纳（门槛 1 + 3） | 第三方 Zig VT 差分 |
| lmorg/ttyphoon Go VT 测试 | 不采纳（测依赖行为） | 本仓 VT 语义不由自研测试覆盖（上游 VT 语义不由本仓测试覆盖 Requirement） |
| nihiL7331/sfte fuzz 宏 | 不采纳（无可评估资产） | 仓内仅 fuzz 宏定义，无语料 |
| veops/go-ansiterm integration tests | 不采纳（门槛 3 + 未编目） | 报告自述断言未编目；Go 库无 Android 落点，无补目必要 |
| nowledge-co/con `con-test` conformance harness | 不采纳（门槛 1） | 期望值为其自身实现输出；报告自述未复核 |
| z-hunter/Scanline-term vitest | 不采纳（无 VT 资产） | 文档自陈无 VT conformance suite、无测试矩阵 |
| lihop/godot-xterm `test/scenes`、asciicast 样例、`colortest.sh` | 不采纳（PROHIBITED + 人工目检） | asciicast 录制回放撞会话持久化禁止；`colortest.sh` 属人工目检；报告自述可执行命令未实际运行 |
| sanohiro/bcon 的 vte crate「for test/shimmer」 | 不采纳（门槛 1 + 3） | 与第三方 VT 差分同口径（qwertty 语料先例） |
| benyblack/ntilde gap matrix / conformance report CI 比对 | 仅参照（不引入） | 与 Ghostty 的差异矩阵思路可对照；语料本身是 .NET VT 自身输出 |
| alacritty/termbenchbot 指标维度（cursor_motion/dense_cells/scrolling/unicode） | 仅参照（不引入） | 性能方法论对照；本仓基准在 `native/benches`，设备侧宏基准见本节能力行 |
| KDE/konsole `tests/`、`autotests/`（emulation/screen） | 不采纳（门槛 1） | 期望值取自其自研 Vt102Emulation，报告自陈「需外部真相审计」；与 contour 行同口径 |
| Kuddev/pebrel `nebula_terminal/tests` 录制回放（`.recording` + expected grid JSON） | 不采纳（门槛 1 + 3） | 期望网格为其自身实现产物；其 release profile 借鉴已落地 `native/Cargo.toml` |
| kmscon/libtsm `test/` 单元与验收套件 | 不采纳（门槛 1 + 3） | libtsm 为可替换 VT 核（违单一来源），期望即其自身行为；「与 Ghostty 快照对照 harness」建议不引入（上游 fuzz 已覆盖） |
| ketraterm/KetraTerm 402 个测试文件与 JMH 基准 | 不采纳（门槛 1 + 3） | JVM/Swing 无 Android 落点；`tools/xterm-oracle` 为差异对照工具、非本仓语料，改写为 JNI 调用方可用 |
| mintty `testbidi.tc` 与生成脚本 | 不采纳（无消费者） | RTL/BiDi 未声明不立项（见能力裁定补齐行），宽度用例非本仓语料 |
| sudo-tee/hollow `src/lua/tests`、`src/mux/tests.zig`、bench 脚本 | 不采纳（门槛 3） | Emacs 插件与 Zig mux 侧断言，非 VT 语义语料；无 ConPTY 集成测试 |
| tuzig/terminal7 Vitest 与 Playwright e2e | 不采纳（门槛 3） | Web/xterm.js 集成测试，非 VT conformance |
| vercel-labs/wterm Vitest 矩阵与 Playwright e2e | 不采纳（门槛 3） | 覆盖 WASM bridge／DOM 渲染／WS transport，均 Web 形态，无 Android 落点 |
| wilsonglasser/oryxis crate tests（含 alacritty VT 套件） | 不采纳（门槛 1 + 未编目） | 断言范围未展开；第三方 VT 差分，口径同 go-ansiterm |
| kshivang/BossTerm「additional oracle」测试 | 不采纳（门槛 1） | jEditerm 派生 VT 的自身输出；另有 LGPL-3.0 许可审查前置 |
| felixse/FluentTerminal 的 `AdventureTime.itermcolors` 配色样例 | 不采纳（无落点） | iTerm 主题导入撞 `PROHIBITED.md` 设置节「自定义终端主题」；主题列表固定且构建期读取（`theme-configuration`） |
| dakra/ghostel 的 `test/hypothesis` 渲染 JSON corpora 与 `src/tests.zig` | 不采纳（门槛 1 + 3） | 期望值出自其自身渲染投影，断言对象为 Emacs 模块组件 |
| TragicWarrior/libvtty 的 `test/*.suite` 与 `.reference` | 不采纳（门槛 1 + 3） | libvterm fork 的黑盒参考输出即第三方 VT 自身行为（与 kmscon/libtsm 行同口径） |
| ghreprimand/odytty 的 native GPU tests、`fuzz/`、`benches` | 不采纳（门槛 1 + 3） | 自研 VT 的自身期望；GPL-3.0-only 仅作许可前置事实，不作判定依据 |
| fyne-io/terminal 的 `test_data` 与 `*_test.go` | 不采纳（门槛 1 + 3） | Go 自研解析器的期望；Go 模块无 Android Rust 落点 |
| godot-pty/gpty 的 crates tests 与 Godot 集成测试说明 | 不采纳（门槛 1 + 3） | alacritty 第三方 VT 差分；GPL-3.0-only 前置 |
| elementary/terminal 的 `src/tests`（Application、UnsafePasteTest 两例） | 不采纳（无可评估资产） | 仅应用启动与不安全粘贴对话框用例，后者撞 `PROHIBITED.md`「粘贴确认对话框」 |
| borisfaure/terminology 的 `tests/`（color、cursor shape、link detection、selection、OSC、DSR、tbc、lf margins） | 不采纳（门槛 1 + 未编目） | 报告自陈回归脚本「reference xterm behavior, can be adapted with expected diff」——期望改写属待做而非现成资产，判定言人眼读 diff；与 contour 行同口径 |
| PaNDa2code/zerotty 的 `tests/` shell tests | 不采纳（门槛 3 + 未编目） | 报告只记「shell tests」未展开断言；自研 VT 的 Zig 实现差异，无 Android 落点 |
| TermoraDev/termora 的 `src/test`（25 个测试）与 `issue-564`／`issue-1055` 回归数据 | 不采纳（门槛 1 + 3） | 断言对象为其自研 KTerm；回归数据属其自身 VT 语料；ssh/transfer 类用例依赖 SSHD mock |
| Yazelix/mars 的 `specs/` 计分卡、benchmarks 与 alacritty cell grid／Rio 渲染测试 | 不采纳（门槛 1 + 3） | Rio/alacritty 血统的第三方 VT 期望；benchmark 与 spec 用例须在目标设备重算，且其性能门禁为桌面口径（`29` 篇已裁定 spike 不做） |
| isene/glass 的 `test` 目录与内嵌 emoji 缓存 | 不采纳（无可评估资产） | 报告只记目录存在，未列断言；x86_64 汇编形态无 Android 落点 |
| cutefishos/terminal 的测试 | 不采纳（无可评估资产） | 报告自陈「None」（无测试数据） |
| hugopl/batata 的 `spec/` Crystal specs | 不采纳（门槛 3 + 未确认） | 报告自陈「whether they execute in a current runtime was not confirmed here」；GTK/VTE 形态无 Android 落点 |
| rricharz/Tek4010 的 `tektests` 与动画样例 | 不采纳（无落点） | Tek 协议未声明（Ghostty 无该协议），其语料属该协议专属 |

#### Scenario: 未登记的语料资产补行

- **WHEN** 复查中发现某仓的测试语料未在本表与既有「已复查结论台账」内
- **THEN** MUST 补行写明判定（含门槛号）与可复核依据，MUST NOT 以概括表述代替

### Requirement: 能力裁定补齐行台账

研究报告中出现、此前未逐条裁定的功能面，判定如下；「未声明」指 `DESIGN.md`
无对应功能声明，按 `STYLE.md` 不实现。判「PROHIBITED」时 MUST 引 `PROHIBITED.md`
条目，MUST NOT 以「未声明」笼统代替；错裁 MUST 按事实更正并保留更正前后结论。

| 功能面 | 出处 | 判定 |
| --- | --- | --- |
| Model Context Protocol | godot-pty-gpty、lumina、ntilde、par-term、par-term-emu-core-rust、ratty、Scanline-term | **PROHIBITED 明禁**（`PROHIBITED.md` 终端节「Model Context Protocol」）；口径更正：先前记「未声明」属错裁，按 BiDi 更正同例改判；出处更正：先前引 copad、Lume 属误引（两报告无 MCP），改引实测命中者，同时补入 gpty |
| 插件系统、事件总线、mux daemon | ratty、par-term、copad、hollow | 未声明，不立项；出处更正：先前引 Scanline-term 属误引（该报告无插件/mux 内容），已按实测命中移除 |
| 无障碍/文本捕获（`readText` API） | vercel-labs/wterm | `DESIGN.md` 无无障碍面，**未声明，不立项**；朗读形态另撞 `PROHIBITED.md` 终端节「无障碍朗读」；口径更正：先前整行记「PROHIBITED 明禁」系以朗读条目覆盖整个无障碍面，按报告实测（仅可访问性/文本捕获）更正 |
| WebSocket/网络终端形态（服务端拆分、DOM 单元布局、Electron/Qt 集成、WebView 远程终端） | DomTerm、terminal7、wterm、copad（`copad-mobile`） | 未声明，不立项（本仓无网络终端；wterm 的重连与背压设计仅在日后开网络终端时参考） |
| 分屏／平铺面板 | batata、godot-pty-gpty、marc2332-term、par-term、pebrel | 未声明，不立项；出处更正：先前引 BossTerm、maui-station 属误引（两报告的 `split` 分别指字形拆分宽度与 Service/UI 分层，非分屏），已替换为实测命中者 |
| git worktree 侧栏／集成 | marc2332-term、copad | 未声明，不立项（`DESIGN.md` 无 git 集成面） |
| ZMODEM/tysend 类文件传输 | terminology、konsole、termora、oryxis | 未声明，不立项 |
| 命令面板／shell 补全 overlay | Lume、lumina、par-term | 未声明，不立项 |
| 远程会话（设备配对、隧道、移动端控制） | Lume | 未声明，不立项 |
| AI 助手集成与 ACP/crash 分诊 | Lume、par-term、ttyphoon、zashterminal | 未声明，不立项（MCP 通道另见上表明禁行）；出处更正：lumina 报告只有 MCP server、无 AI 助手面，已移出 |
| vi-like 选择模式／可配置按键绑定 | Bobcat、pebrel | 未声明，不立项（「实体键盘快捷键设置」另有 PROHIBITED 明禁） |
| 文件管理器侧栏、快捷提示、语法高亮 | zashterminal | 未声明，不立项 |
| 结构化输出块／可点击树部件 | ttyphoon | 未声明，不立项 |
| 第三方渲染实现（cell 绘制、damage 驱动刷新、字形回退与 glyph cache、渲染帧租约、cell 批量上传） | iced_term、shitty、KetraTerm、godot-xterm | 不采纳：本仓字形图集 + 行级脏跟踪 + 渲染线程帧所有权已有等价实现（`native/src/render/`），代码不移植 |
| Terminal API 作 Compose contract／selection 映射 ViewModel | godot-xterm | 不采纳：本仓 `host-linkage` 的 JNI 边界与 `text-selection` 已有等价模型 |
| 会话恢复/持久化、配置保险库、daemon 常驻会话 | Scanline-term、marc2332-term、wilsonglasser、contour（独立 daemon 持久会话） | PROHIBITED 明禁（`PROHIBITED.md` 终端节「会话数据持久化 / 恢复」）；出处更正：copad 报告实为 AI 会话 detach/resume（运行时编排，非数据持久化），已移出本行 |
| CRT/scanline 视觉特效 | Scanline-term | 未声明，不立项 |
| 输出分块与命令状态着色（OSC 133 语义） | hugomyb/Lume | 上游 OSC 133 解析已登记；本仓不消费，DESIGN 未声明 prompt 标记语义；出处更正：先前引 ttyphoon、terminator 属误引（两报告无 OSC 133），ttyphoon 的输出块/退出状态着色归下方「结构化输出块」行 |
| DBus 远程控制 | qterminal | 未声明（Android 无对应形态），不立项 |
| 界面多语言/i18n 组织 | qterminal | 本仓界面仅简体中文（`STYLE.md`），不做 i18n；其余语言支持未声明，不立项 |
| 用户自定义主题覆盖与优先级链（用户文件 > 自动取色 > 默认） | termux-monet | **PROHIBITED 明禁**（`PROHIBITED.md` 设置节「自定义终端主题」）；本仓主题列表固定、构建期读取（`theme-configuration`） |
| RTL/BiDi 文本整形 | RtlTerminal、ThothTerm | **未声明，不立项**；口径更正：`PROHIBITED.md` 无 BiDi 条目，「已禁止」的表述无依据，正确口径是未声明 |
| 界面 layoutDirection RTL | RtlTerminal | 未声明，不立项 |
| copy-on-selection（松手即复制） | godot-xterm | 不采纳：选择交互以菜单点击复制为唯一入口（`text-selection`） |
| bell 冷却/节流 | godot-xterm | 不接线（`on_bell` 无消费者，既有台账行），连带节流不立项 |
| Tektronix 4010/4014 | rricharz-Tek4010 | 未声明，不立项（Ghostty 无该协议） |
| 5250 主帧协议 | tn5250j | 不适用（非 ANSI/xterm 域） |
| O(1) row-map 滚动、内嵌 59k Nerd Font | midasdf/zt | 不需要/未声明：滚动归上游；内嵌字体与「按需加载宿主字体」的字体路线相悖 |
| 单帧处理 deadline/quantum、edge-triggered 唤醒 | missdeer-mostty | 不立项：本仓读链已有界通道（`OUTPUT_CHANNEL_BOUND` × 8KB）封顶，宿主轮询 `new_output`（`AtomicBool`，经 `take_new_output` 读取并清除）已实现对等语义；帧预算属过度设计嫌疑 |
| 「同引擎对标仓」的 PTY→UI 交接模式 | missdeer-mostty | 仅审计参照（本仓已有等价物）；该报告被「自研 VT 全部禁止」群体口径错裁，按事实更正——它与本仓同为 libghostty 唯一 VT 核 |
| Zig libghostty wrapper 的 callback 生命周期模式 | sudo-tee-hollow | 仅审计参照：本仓 wrapper 已有等价脏跟踪与状态转换实现；代码移植被许可未确认阻断，仅模式对照 |
| type-ahead 输入 | techsenger-jeditermfx | 未声明，不立项（`InputBatchBuffer` 已保序，与 JetBrains/jediterm 同口径） |
| IME 组字内外删除分流、组字光标模式 | anoterm | 仅参照（本仓已覆盖）：删除与改写面归 `ime-editor-contract`「替换类编辑必须到达终端」「已提交词的改写经重锚送达终端」，光标锚点归「组字光标位置回报给输入法」，无新增引入 |
| Unicode 17 graphemes/跨单元连字 | pg83-shitty | DESIGN 明示不做跨单元连字（单码位整形与复杂文种叠加除外） |
| alacritty_terminal 作替代 VT 核 | pop-os-cosmic-term | 不采纳（终端状态单一来源） |
| 设备性能自动化门禁 | nihiL7331-sfte | 登记现状：设备侧宏基准已存在并接入 CI（`scripts/test-emulator.nu` 调 `:benchmark:connectedBenchmarkReleaseAndroidTest`，`build.yml` 的模拟器步骤触发），缺的是阈值判定——基准只输出指标、阈值需真机标定（CI 为 swiftshader 软件渲染，帧率不作回归判据）；阈值自动化留待维护者裁定 |
| CI 产物哈希/二进制一致性比对 | vercel-labs-wterm | 登记待维护者裁定：`.github/` 属禁改区，本轮不改 |

#### Scenario: 功能面判定可溯源

- **WHEN** 读到某个功能面的判定
- **THEN** MUST 能在本表读到出处报告与依据（`DESIGN.md` 未声明、`PROHIBITED.md` 条目，或本仓既有实现）

### Requirement: 参照面与反面做法收敛补齐行

编号文档集（`00`–`30`）已从工作区删除，其参照面与反面做法 MUST 在本规范各表有可读去向；
已退役的 `docs/research/ecosystem.md` 与逐仓报告的参照面／反面做法同此要求。
反面做法 MUST 点名出处仓库与做法并标注「不得反向采纳」或「不采纳」，
MUST NOT 要求读者恢复已删除的研究报告才能复核。

| 项 | 出处 | 判定与依据 |
| --- | --- | --- |
| PTY 输出合并投递与 UTF-8 粘包保留解码 | puppy、anoterm | 仅参照：本仓会话与输出管道在 Rust，字节直传上游 VT stream，无自研粘包解码；跨块半扫由 `output_processor.rs` 用例覆盖 |
| 迁移文档的问题清单组织 | termux-kotlin-app | 仅参照（文档思路，无代码落点）：本仓文档面为 `openspec/` 与 `docs/specification/`；其「保留上游测试套件」不是正例——单测仅 2 文件约 18 例，可采纳断言仅键位编码与鼠标两类（主台账 `reapercanuk39/termux-kotlin-app` 行） |
| 自造 `svc #0` syscall 做 JNI 桥 | on3-terminal | **不得反向采纳**：绑定面走标准 JNI／libc 调用，自造 syscall 无跨设备可移植性 |
| `curl \| sh` 式引导安装 | shafinthedev/Ply（`src/main.rs:102` 的 `curl -s {} \| sh`，包管理安装流程） | **不得反向采纳**：引导安装按 `bootstrap-install` 固定流程执行，不把远端脚本接进 shell 管道 |
| 渲染热路径内的 `measureText` | puppy | **不得反向采纳**：与既有台账行「在渲染热路径内改写终端状态」同口径，文本度量不得进每帧路径；其构建产物入库一并计入既有台账「仓库内提交 CMake 生成物与构建产物」行 |
| 治理文档膨胀（「文档海」） | termux-app-rafacodephi、termux-kotlin-app | **不得反向采纳**：文档面收敛为 `openspec/` 与 `docs/specification/`，研究过程记录不常驻 |
| 整包完整性校验（BLAKE3 与 sha256 双实现） | termux-app-rafacodephi（`BootstrapIntegrityVerifier` 走 BLAKE3；loader `BootstrapChecksumValidator` 走 sha256） | 不采纳：整包校验与 `PROHIBITED.md` 终端节「Bootstrap zip 的 sha256 校验文件」同口径 |
| 删减式回退（缺源文件、删 Sixel 分支、去 API 版本守卫） | Autonomous-Agentic-Workflows/termux-app | **不得反向采纳**：跟 master 并保留上游能力；既有实测台账已记「官方的弱化子集（缺六个源文件）」 |
| JNI 字符串释放错对象（`ReleaseStringUTFChars`） | Autonomous-Agentic-Workflows/termux-app、Rprop/AnsolePlus、reapercanuk39/termux-kotlin-app（三仓 `termux.c:164-165` 同为 `(cmd, cmd_cwd)`；上游为 `(cwd, cmd_cwd)`） | **不得反向采纳**：JNI 字符串与数组释放 MUST 对象成对，错位即漏释放加错释放 |
| 选择空间以负行号寻址回滚区的心智模型 | CocaKova/charon（`TextSelection.kt` 行号 `coerceIn(-scrollbackSize, rows-1)` 经 `ScreenBuffer.relativeLine` 解析） | 仅参照：本仓选择空间与回滚寻址归上游，`text-selection` 只声明可观测契约，不下沉自研；其 IME「全文本框镜像」不采纳——本仓口径是组字基线为输入法可见文本的唯一镜像（`ime-editor-contract`） |
| 常驻通知（显示会话数）与退出按钮 | termux/termux-app | 已落地为 `DESIGN.md` 软件节要求（`TerminalForegroundService.kt`），研究行仅为出处对照 |
| 备份「完成后替换 + 孤儿清理」 | GlobalTechInfo/RedTerm | 未声明，不立项：本仓无备份／恢复面（`DESIGN.md` 无声明）；其搜索高亮两层透明度归 `text-search` |
| 行尾孤立高代理项越界读（宽表缺 `isLowSurrogate`／长度守卫） | Rprop/AnsolePlus（与上游同位置未修）、Adhi-hub07/on3-terminal（宽表缺 `isLowSurrogate` 校验） | **不得反向采纳**：`cjk-rendering`「行尾孤立代理项不越界」要求长度守卫，宽度判定 MUST NOT 越界读取码元；rafacodephi 的真实修复为主台账正例行 |
| 以空 `catch` 吞掉全部异常 | irealashu/ApexShell、Sunbr99/TerminalOS 等无终端实现组 | **不得反向采纳**：与「JNI 边界不得静默截断或静默吞错」同口径，错误 MUST 可观测，不得静默降级 |
| 以 su／Shizuku 提权换取 shell 能力 | eggie-admin/hydra-shell-android 等提权组 | **不得反向采纳**：撞 `PROHIBITED.md` 设置节「Shizuku 集成开关」明禁；本仓不引入提权通道 |

#### Scenario: 反面做法可独立复核

- **WHEN** 复查编号集某仓的反面做法
- **THEN** MUST 在本表读到出处仓库、做法与「不得反向采纳／不采纳」判定，
      MUST NOT 需要恢复已删除的编号报告

### Requirement: 禁改区待维护者裁定项台账（只登记，不实施）

`docs/specification/TESTING.md` 要求而现行门禁缺席的四项，全部落在 AGENTS.md
禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者已裁定
「都不改，只登记」，落地方案记录于此，不擅自实施：本表各项 MUST 维持登记状态，
未经维护者明确同意 MUST NOT 实施。

| 项 | 规范依据 | 落地方案（未实施） |
| --- | --- | --- |
| aislop / jscpd 代码检查门禁 | TESTING.md 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 |
| rapidocr OCR 自动化 | TESTING.md 环境节 + 外部基准优先级 4 | `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用 |
| CI 产物哈希比对 | vercel-labs-wterm 研究结论 | `build.yml` 增产物 sha256 记录步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | GlobalTechInfo-RedTerm 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 |

#### Scenario: 禁改区项不被静默实施

- **WHEN** 本轮或后续复查接触到上述四项
- **THEN** MUST 维持登记状态并按维护者裁定执行，MUST NOT 以「规范要求」为由擅改
      禁改区文件

### Requirement: 研究阶段残留项与覆盖缺口登记

退役研究报告（编号集 `28`／`29`／`30` 三篇）中**未闭合**的条目 MUST 在本表留有可读去向，
与已落地项分开登记：已落地者 MUST 给出代码或能力落点，未查清者 MUST 写明「未查清」并说明
为何不凭猜测处理，覆盖缺口 MUST 写明缺的是哪半边与替代判据。MUST NOT 把未查清或不可测的
条目写成已解决，也 MUST NOT 因「做不到」而删除条目。

| 项 | 判定与依据 |
| --- | --- |
| `get_capabilities` 在模拟器上失败的真因 | **未查清，维持不处理**：无模拟器反馈闭环，加重试属凭猜测（编号集 `28`）。代码侧已落地的是同根因的可见缺陷修复——`attach_surface` 只在重建**确实成功**后才清失效位与连续失败计数（否则失败路径替成功路径还愿，自愈阈值永远攒不满，渲染停摆，`native/src/render/context.rs`）；该修复带 `#[cfg(target_os = "android")]`、宿主无 seam，判别力用例会被迫把被测逻辑抄进测试体（第一版恒通过），故**据实登记为覆盖缺口**，不冒充已覆盖 |
| 焦点上报写出的那半边 | **覆盖缺口，维持不冒充**：`focus_event_bytes` 写出需在宿主单测里等一次 VT 线程往返，并发跑全量测试时该耗时无上界，写成断言即不稳定用例（编号集 `28`）。可确定覆盖的半边已钉在 `key-input-encoding`「查询窗口宽于 VT 空闲 tick」；写出半边只保留结构性保证（`Session::focus_event_bytes` 直写子 PTY） |
| 37 项过度复杂特性嫌疑 | **仅记录，不立即简化**（编号集 `28`）：含高风险六项（上游对标语料裁定流程、备用屏输入位移状态机、表面失效自愈限流、亚毫秒计时、渲染帧调度锁存群、字号上下限换算体系）。简化须单独立项并复现 `DESIGN.md` 保证，本轮不立项 |
| L2–L6 减码建议 | **有界重构建议（P1/P2），不扩大为变更**（编号集 `29`）：L1 批量合并已执行；红线不变——不减测试、不降覆盖、不弱化断言、不删 `DESIGN.md` 功能；想大幅减码只能改 `DESIGN.md` 收窄规格，属产品决策 |
| `sugaloaf`／`libsugaloaf` 承接 render／font 的 spike | **维持不做**（编号集 `29`）：须先有 lavapipe 基准且能复现 `DESIGN.md` 字体保证才动；`librio` 的 render-state pull API 与 staticlib 打包仅作设计参照 |
| esctest／vttest 真机精选子集 | **维持「不扩张成穷举 VT 一致」**（编号集 `30`）：全量一致套件不搬入本仓（违「不测试依赖行为」）；若引入只放真机 `connectedDebugAndroidTest` 且取精选子集。`esctest2` 另阻塞于绑定层缺 `DECRQCRA`（「已复查结论台账」esctest2 行） |
| 真实字体／bootstrap 数据扩面（原优先级 4） | **已由既有基准承担**：字体侧真度量由 `native/src/render/font/rasterization.rs` 的字体表基准（`cell_metrics_match_the_selected_face_own_tables` 按所选面自身表换算并核对格宽行高）覆盖，不再另立 hmtx 扩面；bootstrap 侧真实 zip 与装后可用性由设备侧用例覆盖；金图 OCR 属禁改区待维护者裁定（见「禁改区待维护者裁定项台账」） |
| debug 档 `cargo test` 链的是未优化 Zig 解析器 | **口径登记**（编号集 `30`）：上游 `libghostty-vt-sys` 构建脚本按 cargo 的 `DEBUG` 选 Zig `-Doptimize`，故 debug 档耗时结论 MUST NOT 外推 release；以大量组合符回放为前提的用例在 debug 档会顶穿 `flush` 上限，本条目的抽样上限写在用例常量旁并附实测数字 |
| kitty 键盘协议 16 处上游与规范不一致 | **已逐条钉在用例内**（编号集 `30`）：方向键／Home／End／F1/F2/F4 在 kitty 模式下仍发 legacy 形式，媒体与音量键不发字节，只开消除歧义时 CapsLock/NumLock 不发字节；按「不把上游小瑕疵转为本仓测试失败」钉实测值并注明分歧，MUST NOT 按规范断言 |
| CI connected 用例失败的归因口径 | **已更正为「一次一个确定根因」**（编号集 `28`）：初稿判「整体性随机不稳定」是错的——「每次用例都不同」不等于随机，逐次归因后每次都有一个确定根因，修好一个露出下一个。教训登记：判定「是否已修」MUST 核对修复提交时间与 CI 运行时间，MUST NOT 拿过期失败继续追查 |
| Kotlin 侧格式门禁的漏跑 | **教训登记**（编号集 `28`）：`scripts/check.nu` 含 `detekt spotlessCheck` 等 Kotlin 门禁，只跑 Rust 侧门禁会漏掉 Kotlin 侧格式违规；跨语言改动 MUST 跑整套门禁 |
| 报道指出但本仓已自行解决的项 | **已核销，不留待办**（编号集 `28`）：PTY 互斥量与无界写入争锁（现无互斥量）、`clipboard_text` 队列无上界（已封顶丢最旧）、VT 线程 resize 失败永久分歧（已有跨线程失败标记）、每次安装新建 `OkHttpClient`（已进程内缓存）、注释「500ms 防抖」漂移（现为具名常量 300ms）、`closeNotifyThread` 从不 `quitSafely`（复核为设计而非缺陷）；`InputBatchBuffer.close()` 已同批摘回调并复位排程标记 |
| 无可解析码位的负缓存推理 | **口径已更正**（编号集 `28`）：报告原话成立——本仓 `Pipeline::load_font_file`（`native/src/render/font/pipeline.rs`）的全部调用点都在测试模块内，生产无调用；易混的是 `font_db.rs` 中的 `font_database.load_font_file(...)`，那是 fontdb crate 的 `Database` 同名方法，非同一物。故负缓存放进 `clear_identity_caches` 后，其失效面与既有的 `cjk_glyph_cache` 完全相同，不存在「漏一处即字形消失」。用例钉两半（失败后入缓存、换字号后清空），各经一次变异验证 |
| kitty 图形上游测试载荷的退场 | **已退场，非遗漏**：20×15 与 128×96 上游载荷随「删除未发现本项目可解问题的上游测试资产」清理；现行 kitty 用例只保留 1×1 与内联 PNG。日后复查 MUST NOT 因「载荷不见了」重建它们 |
| 外部基准与原计划的口径差异 | **登记在案**（编号集 `30`）：优先级 1 原写「进 BDD」，改为 Rust 集成测试目标（接缝相同，逐条代码化期望塞进 Gherkin 只多一层无收益的间接）；优先级 2 的「采样」二字作废（可打印码位全量比对）；优先级 3 改为「规范表当真值 + 分歧登记」（字节断言基本落在上游，本仓判据是增强请求是否到达编码器）；优先级 4／5 见上两行 |

#### Scenario: 未查清项不被写成已解决

- **WHEN** 复查上表任一条目
- **THEN** MUST 读到它是「未查清」「覆盖缺口」还是「已核销」，MUST NOT 出现
      「无反馈闭环却声称已修」的表述

#### Scenario: 口径更正的旧结论被废弃

- **WHEN** 读到 CI 稳定性或 debug 档耗时的旧口径
- **THEN** MUST 以更正后的归因为准（一次一个确定根因、debug 结论不外推 release）

### Requirement: 研究报告逐份索引与文档可移除性

`docs/research` 的每份研究报告 MUST 在本规范内有一条**以报告文件名为键**的索引行，
写明该报告的实质、判定结果与依据位置（本规范某张表、其他 spec 的能力名、或已落地代码）。
索引行 MUST 能独立回答「该仓有无可用资产、结论是什么、为什么」，MUST NOT 以
「其余若干份已核对」这类群体概括收尾。

索引完整是本仓删除研究文档的**前置条件**：任取一份报告，其全部可执行项的去向
MUST 已在索引行内可读到；满足该条件后报告本体 MUST 可被删除而不丢失任何仍需遵守的结论。
本仓现已删除 `docs/research` 全部报告，索引表是这批研究的**唯一常驻载体**；
日后若重新引入研究文档，索引行 MUST 与文档同批提交。

生态扫描中仍作为判断依据保留的事实：跨 25 仓扫描 `wgpu` 零命中、完整 GPU 终端渲染
路径仅本仓；Canvas 逐格体系在 CJK 性能上先天不足，Termux 与 ghostty 均已批注
「等宽 ASCII 的 advance 恰为 cellWidth」前提，本仓字形图集 + 整形正是该前提的职责边界；
与本仓同用 libghostty-vt 的对照组为 `mostty`／`hollow`／`con`／`ghostel`
四家壳（`ghostel` 为 Emacs 动态模块且已支持 Android/Termux）；
本仓无死代码可白捡（`allow(dead_code)`／`todo!`／`unimplemented!` 计 0），减码只能靠重构。
禁止项三条保留：不 vendor 第三方 VT 代码片段、不更换 VT 核、不等待完整 libghostty 支持 Android。
换核对照结论一并保留：`alacritty_terminal`／`wezterm-terminal` 体量可 rival 但违单一来源；
完整 `libghostty` 的 `ghostty_platform_e` 仅 `INVALID/MACOS/IOS`、对 Android 无收益；
`rio-vt` 未上 crates.io、须自 rio 仓库自建——在本仓确切约束（Android + Rust 宿主 + wgpu +
无头 + C ABI + kitty 图形 + 单一来源）下无「更完整又能直接换」者。
测试边界由 `terminal-state-regression` 与该能力「上游 VT 语义不由本仓测试覆盖」承担：
VT 解析归上游，上游权威 fuzz 为 `libghostty` 的 `test/fuzz-libghostty`（parser／stream／osc），
本仓只测接缝与契约，外部数据即基准、不入库。

原 `ecosystem.md` 的 S/A/B/C 参照面评级是研究期分流工具，其作用已由索引表的「判定」列
与各台账取代；表中依据列写作「生态参照面组行」的行共享同一事实依据（上段生态扫描
与 `DESIGN.md` 的未声明口径）：这些报告的形态或技术栈与本仓无交集，其可用内容止于
语言级或算法级对照，且已由本仓既有实现越过或撞明禁/未声明——各行「判定」列 MUST
写明该行自身不可采纳的具体理由，不得只以组名代替；不再单设评级表。

生态内其余仍需对照的结论：终端状态单一来源无第二家（「复用 Ghostty」三条路——Java + 预编译库、
自绑 Rust、复用 Termux 库——本仓取自绑 Rust 且唯一在 wgpu 上落地；「自研 VT」四条路全部是子集，
Termux 自身也被 fork 删过 `doDcs`／`sixel`）；会话不随 Activity 走是跨生态共识
（ghostty 进程单例、`ThothTerm` 前台 Service、本仓会话归 Rust），ghostty `attachSession`
的跨会话清理五项已由 `session-list` 登记；Web/WASM 系（即便直接封装 libghostty）与
桌面壳系（复用 xterm.js／QML）都把 VT 解析与渲染放在别处，参照面只剩产品面，而产品面
多属本仓明禁项或未声明项。

两处研究期悬问已实地闭合：`pg83/shitty` 许可为双轨——2023-07-11 导入的基线（含
`lib/vterm`）保持 GPL-3.0-or-later，此后一手贡献 MIT/GPL3 双选，本仓按自研 VT 与
不 vendor 双规则不引入代码；`orhun/ratty` 的 font/atlas 悬问——`src/` 全部 20 个条目
内无 font/atlas 模块，字体与图集随 Bevy 默认栈，原「本仓已越过该层」的判断维持。
字体表的真值对照保留在 `native/src/render/font/rasterization.rs` 与 `font_db.rs`
的宿主字体库用例（数据取自开发环境固定的 fontconfig 字体集），外部数据不入库。

#### Scenario: 索引覆盖到文件

- **WHEN** 遍历 `docs/research/` 曾有的任一份报告文件
- **THEN** 索引表内有以该文件名为键的行，且该行的判定与依据可在本规范内读到

#### Scenario: 删除报告不丢结论

- **WHEN** 某份报告被删除（现已整体删除）
- **THEN** 其提出的可执行项去向（已落地代码位置、被否的门槛理由、未声明/PROHIBITED 判定）
      仍可在索引行与所指向的表内读到，MUST NOT 需要恢复该报告才能复核

#### Scenario: 重新引入报告须同批补索引

- **WHEN** 日后重新引入任何研究文档
- **THEN** 同一批改动 MUST 同时补上其索引行；缺少索引行的报告 MUST NOT 入库

#### Scenario: 索引不重开已裁定事项

- **WHEN** 为某报告补索引行
- **THEN** 该行 MUST 引用既有判定，MUST NOT 借索引重开已裁定的能力面或改写门槛

索引表如下。

| 报告 | 实质 | 判定 | 依据 |
| --- | --- | --- | --- |
| `8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 子集 + C PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；其 OSC 注释清单作生态参照面，OSC 52 安全姿态归 OSC 52 条款；其 `NativePty.c` 一类 C PTY 桥无落点——本仓 PTY 在 Rust 侧（`nix::pty`），仓内无 C 源；多击选词／OSC 8 白名单／提交构建产物三做法属不得反向采纳 | 「Android 侧终端项目的复查台账」terminator 行；「OSC 覆盖逐项核对」；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `DanteAlighierin-foot.md` | C/Wayland 自研 VT 极简终端 | 不采纳：仓内无 VT／网格语料（「无资产」而非门槛违反）；自研 C 解析器违单一来源，box-drawing／组合宽度只作结构对照 | 「已复查结论台账」foot 行 |
| `GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 发行版运行器 | 不采纳（门槛 3）：仓内无本仓组件，329 例属 SSH/SFTP/proot 面；16KB 页对齐登记禁改区 | 台账不可采纳行；「禁改区待维护者裁定项台账」；参照面「搜索高亮两层透明度」归 `text-search`，备份「完成后替换 + 孤儿清理」归「参照面与反面做法收敛补齐行」 |
| `JetBrains-jediterm.md` | JVM/Swing 可嵌入终端库（自研 VT + Pty4J） | 不采纳（门槛 1）：before/after 期望即其自身输出；vttest 派生子集按输入分流逐例判定 | 「已复查结论台账」vttest 行；「按输入是否格式合规分流采纳」 |
| `KDE-konsole.md` | C++/Qt 桌面终端（Vt102Emulation） | 不采纳：桌面 C++ 自研 VT；profile/session 编辑撞 PROHIBITED，DBus／i18n／tysend 未声明 | 「能力裁定补齐行」DBus 远程控制行、界面多语言/i18n 组织行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」konsole 行；「不得反向采纳的做法」 |
| `KDE-maui-station.md` | Qt6/Kirigami 移动终端（MauiKit） | 不采纳：QML 集成在 Android 无对应形态；仓内无 VT 资产（其 `split` 指 Service/UI 分层进程模型，非分屏） | 生态参照面组行；「能力裁定补齐行」分屏／平铺面板行（该行已记其误引更正） |
| `KDE-yakuake.md` | Konsole KPart 下拉式壳 | 不采纳：无新仿真（emulation 全在 Konsole，本行与 `KDE-konsole.md` 同口径）；下拉 UX 无 Android 对应；其 profile/session 复用撞 `PROHIBITED.md`「会话持久化」 | 生态参照面组行；`PROHIBITED.md` 会话持久化条；`KDE-konsole.md` 索引行 |
| `Kuddev-pebrel.md` | Rust+GPUI 桌面 GPU 终端（vte 核） | 部分采纳：release profile（LTO／codegen-units／strip）已落地，其「hot path O3」**不采纳**——移动端 release 以 `opt-level = "z"` + LTO 为体积目标，逐点 O3 与 DESIGN 的体积取向相悖；录制回放语料不采纳（门槛 1）；PTY 事件循环与 redraw anchor 已被本仓 VT 线程 + 行级脏跟踪越过；GPL-3.0 不引入代码 | `native/Cargo.toml`；「语料判定补齐行」pebrel 行；「能力裁定补齐行」分屏／平铺面板行与 vi-like 选择模式／可配置按键绑定行 |
| `PaNDa2code-zerotty.md` | Zig+Vulkan 早期终端 | 不采纳：Zig 转换对 Android 无用途，自研 VT 违单一来源；CI 与环境搭建方式无可移植结论 | 生态参照面组行；「语料判定补齐行」zerotty 行 |
| `PerBothner-DomTerm.md` | C/Java/JS WebSocket+DOM 终端 | 不采纳：Web UI 栈形态，无 Rust 依赖可嵌入 | 「能力裁定补齐行」WebSocket 行 |
| `RohitKushvaha01-ReTerminal.md` | Compose 壳 + Termux View | 不采纳（门槛 3）：VT 走上游 jar；proot 禁入 | Android 台账（kosh／ReTerminal 同源行）；`documents-provider`（其文档提供器形态已落地） |
| `TermoraDev-termora.md` | Kotlin/JVM Swing 终端 + SSH（自研 KTerm） | 不采纳：自研 VT 违单一来源；KeyEncoder／Selection／Find 模型本仓已有等价实现；`issue-564`／`issue-1055` 回归数据属其自身 VT 语料；ssh/transfer 测试依赖 SSHD mock，与本仓无落点 | 生态参照面组行；「语料判定补齐行」termora 行 |
| `TragicWarrior-libvtty.md` | C VT 库（libvterm fork） | 不采纳：VT 核不可换；C 库经 NDK 引入净增 FFI 且违单一来源；按键映射表与颜色缓存算法只作结构对照，不引入 C 代码 | 生态参照面组行；「语料判定补齐行」libvtty 行 |
| `Yazelix-mars.md` | Rust/wgpu（Rio 派生，alacritty 核） | 不采纳：VT 核冲突；字形基线与图集本仓已有等价实现；`sugarloaf`/`libsugaloaf` 承接 render/font 的 spike 维持不做（`29` 篇 P2 有界 spike，须复现 DESIGN 字体保证才动）；`teletypewriter` 的 PTY 在 Android 仍须 JNI 桥，无直接落点；其 config spec 清单只作 UX 元数据对照，设置面以 `settings-propagation` 为准 | 生态参照面组行；「语料判定补齐行」mars 行 |
| `ademmenh-zebra.md` | C+GTK3+VTE 极简示例 | 不采纳：VTE on GTK3 的极简 demo（`main.c` + Makefile），报告自陈「no new insights」且「几乎无可借鉴」；无测试数据 | 生态参照面组行 |
| `alacritty-termbenchbot.md` | 终端基准自动化工具 | 不采纳：`bench.sh` 依赖桌面环境（报告自陈 Android 须另建 NDK harness，不可直接用）；输入生成脚本与阈值建议可作方法论，本仓基准在 `native/benches` | 「语料判定补齐行」termbenchbot 行；「能力裁定补齐行」设备性能自动化门禁行 |
| `aligrudi-fbpad.md` | C framebuffer 极简终端 | 不采纳：framebuffer 单头、无 Android 落点（许可未声明不作判据） | 生态参照面组行 |
| `benyblack-ntilde.md` | .NET/Avalonia+Skia 终端 + conformance 矩阵 | 仅参照：差异矩阵与回放调试思路；语料为该实现自身输出，不采纳 | 「语料判定补齐行」ntilde 行 |
| `borisfaure-terminology.md` | C/EFL 桌面终端 | 不采纳：自研 terminal state 违单一来源，EFL 不在 Android；`tests/` 的 shell 回归脚本期望写在脚本内、以人眼读 diff 为主（报告自陈「可改写为 expected diff」属待做而非现成资产）；link／cursor／OSC 等用例的期望值非现成外部真相；tysend 文件传输未声明；主题格式本仓以构建期 alacritty-theme 为唯一入口 | 「语料判定补齐行」terminology 行；`theme-configuration`；「能力裁定补齐行」ZMODEM/tysend 行 |
| `contour-terminal-contour.md` | C++23+OpenGL 桌面终端（自带 vtconformance） | 不采纳（门槛 1）：脚本只发序列等待、7 个靠人工按键；图集与下划线几何本仓已有等价实现；其 Vi 式选区与图像协议（sixel/regis）未声明；其列举的 synchronized output／reflow／OSC 8 三项本仓均不需自建——前两者归上游 `libghostty-vt`（见本条「上游 VT 语义不由本仓测试覆盖」），OSC 8 亦为上游解析、本仓只经 `hyperlink_at` 查询 | 「已复查结论台账」contour 行；同步刷新走「不采用的上游回调不得留有通道描述」（`on_render_hold` 不接线）；「能力裁定补齐行」vi-like 行 |
| `cutefishos-terminal.md` | C++/Qt/QML（内嵌 `qmltermwidget` 控件） | 不采纳：QML/C++ 形态在 Android 无对应；所嵌控件的 VTE/Konsole 血统未逐 crate 编译确认（报告自陈不能声明为 Android 可移植）；无测试数据 | 生态参照面组行；「语料判定补齐行」cutefishos 行 |
| `dakra-ghostel.md` | Zig + libghostty-vt 的 Emacs 动态模块（同引擎第四家，已支持 Android/Termux） | 仅参照：同引擎 FFI 模式（分配器／Stream／screen pages／pin tracking）与脏跟踪已由本仓 wrapper 覆盖，代码不移植（GPL-3.0-or-later + Emacs 模块形态）；其 PTY resize 的像素字段语义已归 `pty-lifecycle`；预编译模块的分发流程对 APK 原生库无对应形态 | 上段生态扫描的同引擎对照组事实；「语料判定补齐行」ghostel 行；`pty-lifecycle`；`native/src/terminal/ghostty_terminal/` |
| `elementary-terminal.md` | Vala/GTK4 + VTE 桌面终端 | 不采纳：VTE 与单一来源冲突；不安全粘贴对话框撞 PROHIBITED；VTE shell 设置集成（GApplication settings schema）无 Android 落点 | 「能力裁定补齐行」；PROHIBITED 粘贴确认对话框；「语料判定补齐行」elementary 行 |
| `felixse-FluentTerminal.md` | UWP+C#/WebView（xterm.js），已停更 | 不采纳：VT 与渲染都在别处；主题导入撞「构建期读取、主题列表固定」；Shell Profile 模型与自定义会话撞 `PROHIBITED.md`（启动入口由 `shell-entry` 固定，不设 profile 编辑器）；托盘 IPC／更新器属桌面形态；`TerminalBridge` 事件桥属 WebView host 模式、本仓无对应形态；其测试组织（xunit／Moq／AutoFixture／FluentAssertions）属 .NET 栈，本仓测试走 `cargo test` 与 Gradle 目标，无落点 | `theme-configuration`；`shell-entry`；「语料判定补齐行」felixse 行（`.itermcolors` 样例）；生态参照面组行 |
| `floooh-chipz-kitty.md` | Zig 游戏 demo，用 Kitty 图形协议 | 不采纳：Kitty 图形由上游承担；无终端核心内容 | 「上游 API 取用判定登记」Kitty 行 |
| `fyne-io-terminal.md` | Go/Fyne 终端 widget | 不采纳：Go 不可用于 Android Rust 层；`test_data` 属其自研解析器期望 | 「语料判定补齐行」fyne 行 |
| `ghreprimand-odytty.md` | Rust+wgpu 自研 VT | 不采纳：自研 VT 违单一来源；wgpu 渲染组织本仓已越过；box-drawing 算法与无障碍控制未声明（朗读形态撞 `PROHIBITED.md` 无障碍朗读明禁）；native GPU 测试套件属其自身期望，GPL-3.0-only 前置 | 生态参照面组行；「语料判定补齐行」odytty 行；「能力裁定补齐行」无障碍行 |
| `godot-pty-gpty.md` | Rust+Godot 扩展（alacritty PTY，AI 向） | 不采纳：MCP 明禁、事件总线与平铺多路复用未声明；portable-pty 无 Android 后端；GPL-3.0-only 前置 | 「能力裁定补齐行」Model Context Protocol 行与分屏／平铺面板行；「语料判定补齐行」gpty 行 |
| `harmoninux-HiSH.md` | HarmonyOS + QEMU 跑完整 Linux VM | 不采纳：QEMU 全虚拟机编排（ArkTS/C++ 壳 + `libqemu-system-aarch64`），仓内无 VT 解析器与渲染，仅由 guest 终端体现；QEMU 作 Android 宿主属「内嵌用户态/虚拟机」明禁方向且过重；无终端测试 | 生态参照面组行 |
| `hugomyb-Lume.md` | Tauri/SolidJS+xterm.js（远程／AI） | 不采纳：Web 栈；replay ring 与会话持久化撞 PROHIBITED；命令面板、远程会话、AI 集成均未声明；OSC 133 命令块仅作上游能力对照 | 「能力裁定补齐行」会话恢复/持久化行、命令面板／shell 补全 overlay 行、远程会话行、AI 助手集成行、输出分块与命令状态着色行 |
| `hugopl-batata.md` | Crystal/GTK4+VTE 桌面平铺终端 | 不采纳：Crystal/VTE 与 Android 无交集；分屏未声明；其 VTE hyperlink match-setup（`match_add_regex`／hover 模式）无落点——本仓链接经上游查询（`text-selection` 口径）；`spec/` 的 Crystal specs 能否在当前运行时执行未确认，不计可采纳资产 | 「能力裁定补齐行」分屏行；「语料判定补齐行」batata 行 |
| `iewnfod-lumina-terminal.md` | Tauri/Rust+React+xterm.js（含 MCP server） | 不采纳：WebView 形态；MCP 明禁、命令面板未声明；portable-pty 不适用 Android | 「能力裁定补齐行」Model Context Protocol 行、命令面板／shell 补全 overlay 行 |
| `ii8-havoc.md` | C/Wayland 极简终端（libtsm） | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行 |
| `isene-glass.md` | x86_64 汇编 + X11 极简终端 | 不采纳：仅 x86_64、成熟度低、无 Android 落点；X11 wire 协议与内嵌 emoji 缓存的极简二进制思路无可移植结论（本仓图集与字体栈已越过该层） | 生态参照面组行；「语料判定补齐行」glass 行 |
| `ismail-yilmaz-Bobcat.md` | C++17/U++ 桌面终端（外部 TerminalCtrl） | 不采纳：VT 为外部依赖且非 Ghostty；U++ 非 Android；vi-like 选择器与按键绑定未声明 | 生态参照面组行；「能力裁定补齐行」vi-like 行；「OSC 覆盖逐项核对」1337 行 |
| `javaterminal-TerminalFX.md` | Java/JavaFX 终端 widget（core／local 分离） | 不采纳：JavaFX 无 Android 目标；core/PTY 解耦模型本仓 JNI 边界已有等价 | 生态参照面组行 |
| `joshka-qwertty-term.md` | Rust 重写 Ghostty 语义 + Metal 渲染 | 不采纳（门槛 1）：差分期望即其自身实现输出；本仓 VT 语义整体交上游；其 `examples/frame-capture` 的 bytes→PNG 无程序消费者，撞门槛 2（不引入无消费者的二进制资产） | 「上游 VT 语义不由本仓测试覆盖」 |
| `kemokempo-iced_term.md` | Rust/ICED(wgpu) + alacritty 核 | 不采纳：alacritty 核违单一来源；iced 非 Android UI 目标；cell 绘制与本仓渲染已有等价 | 生态参照面组行；「能力裁定补齐行」第三方渲染实现行 |
| `ketraterm-KetraTerm.md` | Kotlin/JVM Swing 终端库 + xterm oracle | 不采纳：JVM/Swing 无 Android 目标；oracle 为自身实现输出，帧租约本仓渲染线程已持有 | 生态参照面组行；「语料判定补齐行」KetraTerm 行；「能力裁定补齐行」第三方渲染实现行 |
| `kmscon-libtsm.md` | C 可复用 VT 状态机库 | 不采纳：libtsm 为可替换 VT 核，违单一来源 | 生态参照面组行；「语料判定补齐行」libtsm 行 |
| `kshivang-BossTerm.md` | KMP 终端库（jEditerm 派生）+ Compose | 不采纳：VT 核冲突；Compose 接线本仓已有同栈实现（其 `split` 指字形拆分宽度，非分屏）；LGPL-3.0 许可审查前置；其「Ghostty 集成失败时的回退核」定位**不得反向采纳**（DESIGN 单一来源不设回退核），JNI-free 的 Kotlin 直调 PTY 与本仓会话归 Rust 相悖 | 生态参照面组行；「语料判定补齐行」BossTerm 行；「不得反向采纳的做法」回退核一条 |
| `leoberbert-zashterminal.md` | Python/GTK4+VTE 桌面终端 | 不采纳：VTE 冲突；会话管理与广播属桌面形态；文件管理器侧栏、快捷提示、语法高亮与 AI 集成未声明 | 生态参照面组行；「能力裁定补齐行」文件管理器行与 AI 集成行 |
| `lihop-godot-xterm.md` | Godot 终端控件（libtsm/node-pty） | 不采纳（PROHIBITED + 人工目检）：asciicast 回放撞会话持久化；colortest 靠人眼 | 「语料判定补齐行」godot-xterm 行；「能力裁定补齐行」Terminal API 行与第三方渲染实现行 |
| `lmorg-ttyphoon.md` | Go/Wails 多媒体终端（自研 VT） | 不采纳：自研 VT + WebView；输出块分组与命令退出状态着色、结构化输出块、AI 集成未声明 | 生态参照面组行；「语料判定补齐行」ttyphoon 行；「能力裁定补齐行」结构化输出行与 AI 集成行 |
| `lxqt-qterminal.md` | C++/Qt6（qtermwidget） | 不采纳：桌面 C++；DBus／i18n 未声明 | 「能力裁定补齐行」DBus 远程控制行与界面多语言/i18n 组织行 |
| `marc2332-term.md` | Rust+Freya Flatpak 桌面终端 | 不采纳：桌面 GUI；JSON 会话持久化撞 PROHIBITED，分屏未声明 | 「能力裁定补齐行」会话恢复/持久化行与分屏／平铺面板行 |
| `marshallku-copad.md` | Rust+原生 UI 跨平台 AI 终端（alacritty） | 不采纳：alacritty 核冲突；插件／事件总线、comux 工作树集成未声明；报告无 MCP（此前误记已更正）；其 `copad-mobile` 走 WebView 远程终端，在 Android 本地终端约束下无对应形态 | 「能力裁定补齐行」插件系统行与 git worktree 行；「能力裁定补齐行」WebSocket/网络终端形态行 |
| `midasdf-zt.md` | Zig 极简多后端终端 | 不采纳：row-map 归上游；内嵌字体与「按需加载宿主字体」路线相悖 | 「能力裁定补齐行」row-map 行 |
| `migueldeicaza-SwiftTerm.md` | Swift 自研 VT（含 Ghostty fuzz 对照） | 不采纳（门槛 1）：期望即其自研实现输出；fuzz 语料对照已由本仓 `external_benchmarks` 落地 | 「语料判定补齐行」SwiftTerm 行 |
| `mintty-mintty.md` | C/Win32+Cygwin 终端 | 不采纳：宽度表归上游；Sixel 已登记、BiDi 未声明 | 「能力裁定补齐行」BiDi 行；「语料判定补齐行」mintty 行 |
| `mirbehnam-RtlTerminal.md` | C#/WPF Windows RTL 终端 | 不采纳：RTL 整形与 layoutDirection 均未声明 | 「能力裁定补齐行」RTL/BiDi 文本整形行与界面 layoutDirection RTL 行 |
| `missdeer-mostty.md` | Zig + libghostty 桌面原生终端 | 不立项：单帧 deadline／quantum 属过度设计，本仓读链已有界；PTY→UI 模式已有等价物 | 「能力裁定补齐行」单帧处理 deadline/quantum 行与「同引擎对标仓」的 PTY→UI 交接模式行（含错裁更正） |
| `mrlima4095-OpenTTY-J2ME.md` | J2ME MIDlet + RISC-V 模拟器 | 不采纳：**非终端仿真器**（RV32IM ELF 模拟器 + 内嵌 Lua + 虚拟 VFS 的 J2ME 平台）；其 `tests/check_*` 无终端仿真用例，LCDUI 测试对 Compose 无意义；平台与运行环境整体不匹配 | 生态参照面组行 |
| `nihiL7331-sfte.md` | C 单头文件极小终端（st 衍生） | 不采纳：仓内仅 fuzz 宏无语料；设备性能门禁登记在案 | 「语料判定补齐行」sfte 行；「能力裁定补齐行」设备性能自动化门禁行 |
| `not-elm-orzma.md` | Rust 自研 VT + WebView 混合终端 | 不采纳（门槛 1+3）：第三方 VT 差分；WebView 超范围 | 「语料判定补齐行」orzma 行 |
| `nowledge-co-con-terminal.md` | Rust+GPUI 全量 Ghostty 的 AI 终端 | 不采纳（门槛 1）：conformance 期望为自身输出；完整 libghostty 无 Android 平台；其 config string patch 与「sanitized font family」设计无落点——本仓主题固定且构建期读取（`theme-configuration`），字体族由 `font-selection` 管辖 | 「语料判定补齐行」con 行；`theme-configuration`；`font-selection`；本条「禁止项三条保留」段（不等待完整 libghostty 支持 Android） |
| `omnipty-terminal-omnipty.md` | Rust+GPUI+alacritty 桌面终端 | 不采纳：GPUI（Zed 引擎）非 Android UI 目标；VT 源头 alacritty 属核冲突；其自定义字节流事件循环报告自陈「仍属设计素材、未经复核验证」，而本仓会话与输出管道已在 Rust 落地；`tests/` 报告未展开框架与覆盖 | 生态参照面组行 |
| `orhun-ratty.md` | Rust/Bevy/wgpu 3D GPU 终端 | 不采纳：present 管线早于本仓字形图集 + cell 实例化；Bevy 过重 | 生态参照面组行（含 font/atlas 悬问闭合） |
| `paulrobello-par-term-emu-core-rust.md` | Rust VT 引擎核 + C/Python 绑定 | 不采纳（门槛 1+3）：第三方 VT 差分；Ghostty 覆盖更广 | 「语料判定补齐行」par-term 行 |
| `paulrobello-par-term.md` | Rust/wgpu/egui GPU 终端 | 采纳（已落地）：GPU surface 尺寸钳制到设备上限与 no-op configure 跳过；egui／agent API、命令面板、ACP 分诊不采纳；Kitty graphics geometry（`c`／`r`／偏移）由上游承担，无本仓落点 | `render-stability`「Surface 尺寸钳制到设备上限」；「上游 API 取用判定登记」Kitty 行；「能力裁定补齐行」命令面板行与 AI 集成行 |
| `pg83-shitty.md` | C++/Vulkan 桌面终端（自研 vterm） | 不采纳（门槛 1 + 穷举扩张禁止）；许可双轨悬问已闭合；damage 驱动渲染与字形回退本仓已有等价 | 「语料判定补齐行」shitty 行；「能力裁定补齐行」第三方渲染实现行 |
| `pop-os-cosmic-term.md` | Rust/iced + alacritty 终端 | 不采纳：alacritty_terminal 作替代核违单一来源 | 「能力裁定补齐行」 |
| `rricharz-Tek4010.md` | C/SDL Tek 图形仿真 | 不采纳：Tek 序列未声明（Ghostty 无该协议）；`tektests`／动画样例属其自身协议的语料，无落点 | 「能力裁定补齐行」Tektronix 4010/4014 行；「语料判定补齐行」Tek4010 行 |
| `sanohiro-bcon.md` | Rust/OpenGL ES bare-TTY 终端 | 不采纳（门槛 1+3）：vte crate 作 test shim 属第三方 VT 差分 | 「语料判定补齐行」bcon 行 |
| `sudo-tee-hollow.md` | Zig/LuaJIT + Ghostty 核桌面终端 | 仅参照：同引擎 wrapper 回调生命周期已有等价实现，代码不移植；插件/mux 测试非 VT 语料 | 「能力裁定补齐行」Zig libghostty wrapper 的 callback 生命周期模式行与插件系统行；「语料判定补齐行」hollow 行 |
| `techsenger-jeditermfx.md` | JavaFX/JediTerm xterm 仿真器 | 不采纳（门槛 1）：before/after 期望与其实现同源；type-ahead 未声明 | 「语料判定补齐行」jeditermfx 行；「能力裁定补齐行」 |
| `tenox7-aclock.md` | C 终端文字时钟玩具 | 不采纳：玩具级 VT 绘制；其 escape 绘制样例可作渲染器兼容性人工对照目标，非语料 | 生态参照面组行 |
| `tn5250j-tn5250j.md` | Java IBM 5250 主机终端 | 不采纳：非 ANSI/xterm 域 | 「能力裁定补齐行」5250 行 |
| `tuzig-terminal7.md` | TS/xterm.js/Capacitor 远程终端 | 不采纳：Web-first 无 Android 落点、网络终端未声明；webexec 超 local-only 范围 | 「语料判定补齐行」terminal7 行；「能力裁定补齐行」WebSocket 行 |
| `veops-go-ansiterm.md` | Go 屏幕模拟／命令提取 | 不采纳（门槛 3）：Go 库无 Android 落点，断言未编目 | 「语料判定补齐行」go-ansiterm 行 |
| `vercel-labs-wterm.md` | Zig/WASM + DOM 渲染（封装 libghostty） | 不采纳：WASM/DOM 无法作为 Android 依赖；无障碍/文本捕获未声明（朗读形态撞「无障碍朗读」明禁）；WS 重连／背压未声明；CI 产物哈希登记禁改区；其 DOM 脏行跟踪／部分重绘思路已由本仓行级脏跟踪与部分帧带越过，Ghostty 封装层的页面零初始化修复属上游内部实现 | 「语料判定补齐行」wterm 行；「能力裁定补齐行」WebSocket 行与无障碍行；「禁改区待维护者裁定项」 |
| `wilsonglasser-oryxis.md` | Rust/iced + alacritty SSH 客户端 | 不采纳：alacritty 核冲突；config vault／会话恢复撞 PROHIBITED；crate tests 未编目 | 「能力裁定补齐行」会话恢复/持久化行与 ZMODEM/tysend 类文件传输行；「语料判定补齐行」oryxis 行 |
| `z-hunter-Scanline-term.md` | Tauri/React/xterm.js 复古终端 | 不采纳：无 VT conformance 套件；MCP 明禁、CRT 特效未声明、会话恢复明禁 | 「语料判定补齐行」Scanline 行；「能力裁定补齐行」Model Context Protocol 行 |
| `ecosystem.md` | 生态参考与测试边界（参照面评级、禁止项、测试边界） | 内容已并入：参照面评级 → 本表「实质」列（评级语义与「生态参照面组行」定义见本条正文）；禁止项 → 本条「禁止项三条保留」段；测试边界 → `terminal-state-regression` 与本能力「上游 VT 语义不由本仓测试覆盖」；参照面表的可落地项按名落点（`pty-lifecycle` 收 `IUTF8`／像素 winsize／前台组收尾，`text-search` 收搜索上限与两层高亮，`settings-propagation` 收 live／spawn-time，`render-stability` 收键盘动画期不暂停，通知常驻归 `DESIGN.md` 软件节），其余 → 「参照面与反面做法收敛补齐行」 | 本表；`pty-lifecycle`；`text-search`；`settings-propagation`；`render-stability`；「参照面与反面做法收敛补齐行」；`terminal-state-regression` |
| `00-汇总与可参考点收敛.md` | 编号集总纲：25 仓汇总、P0/P1/P2 分级与 openspec 落点 | 已处置：P0 由各能力承接、`bootstrap-install` 单列立项；P1/P2 为实现指导不新立项 | `bootstrap-install`；「参照面与反面做法收敛补齐行」；本表 `01`–`25` 各行 |
| `01-sylirre-ghostty-android-terminal.md` | Java+C 壳，VT 全委托预编译 `libghostty-vt.a`，Canvas 拉快照过 JNI | 需改造后采纳（同引擎同架构最正当对照面）：可采纳类为锚定外部协议的 VT 断言；Java 持会话、CPU/GL 回退、SAF 复制字体、自定义主题编辑器不采纳 | 「Android 侧终端项目的复查台账」`sylirre/ghostty-android-terminal` 行；「不得反向采纳的做法」 |
| `02-termux-termux-app.md` | 上游基准：自研 VT + Canvas，约 4.8 万行 | 不采纳（门槛 3）：断言打在本仓不实现的上游 VT 引擎；行为参考归各能力（PTY 双线程、退出保留现场、键矩阵、回滚 2000） | 复查台账 `termux/termux-app` 行；`pty-lifecycle`；`shell-entry`；`terminal-state-regression` |
| `03-sohan-f-termux-monet.md` | termux fork + Monet 运行时取色引擎 | 不采纳（门槛 3）：新增仅 14 例断言刚写入的调色板值；Monet 动态取色与自定义主题撞 `PROHIBITED.md` 设置节 | 复查台账 `sohan-f/termux-monet` 行；「能力裁定补齐行」用户自定义主题行；`theme-configuration` |
| `04-8dmusichannels-star-terminator.md` | Kotlin/Compose 自研 VT 全矩阵 + C 真 PTY 桥 | 不采纳：零测试资产且 CI 虚假绿灯；OSC 52 门控归 OSC 52 条款；多击选词／OSC 8 白名单／提交产物不得反向采纳 | 同仓现行索引行 `8dmusichannels-star-terminator.md`；「OSC 52 读取请求的收割与应答须对每个会话有界」；「不得反向采纳的做法」三行 |
| `05-deathlegionteamlk-jolz.md` | 仅 README 与 Release APK，源码私有 | 无门槛违反：无源码可评估，其声称细节不得引用；两处声称与本仓方向一致且已由本仓规范覆盖——宿主环境继承归 `shell-env`（本仓取固定透传名单而非全量继承）、logcat 可见崩溃报告归 `DESIGN.md` 日志要求 | 复查台账 `deathlegionteamlk/jolz` 行；`shell-env`；`DESIGN.md` |
| `06-GlobalTechInfo-RedTerm.md` | 复用 Termux jar 的 proot 多发行版运行器 | 不采纳（门槛 3）：329 例属 SSH/SFTP/proot 面；单源启动脚本、通知退出、备份替换归本规范各表 | 同仓现行索引行 `GlobalTechInfo-RedTerm.md`；复查台账 `GlobalTechInfo/RedTerm` 行；`bootstrap-install` |
| `07-rafaelmeloreisnovo-termux-app-rafacodephi.md` | termux 重型 fork：会话加固 + 自研 bootstrap 治理 | 不采纳（门槛 1）：`terminal-emulator/src/test` 与官方逐字节相同；正例为会话加固（先进程组收尾） | 复查台账 `rafaelmeloreisnovo/termux-app-rafacodephi` 行；「参照面与反面做法收敛补齐行」整包完整性校验（BLAKE3 与 sha256 双实现）行 |
| `08-pavelc4-Rin.md` | Rust 引擎 + Compose 壳，手写 545 行 ANSI 解析 | 不采纳（门槛 1 + 3）：手写解析必然不完备（反衬 Ghostty 单源）；Canvas 指针进 Rust 绘制违渲染分层 | 复查台账 `pavelc4/Rin` 行；「禁止项三条保留」段（不换 VT 核） |
| `09-Lord1Egypt-AndroidThothTerm.md` | JackPal 血统 Java 仿真器 + BiDi 渲染 | 门槛 1 不违反但仅作判据来源；`tests/` 122 文件对全仓零引用属失效语料；BiDi 未声明；交互侧可参考项本仓已有等价或更强实现——长按取词与拖拽手柄归 `text-selection`、虚拟键布局归 `modifier-bar-height-reservation`、会话列表标题监听归 `session-list`、常驻通知归 `DESIGN.md` 软件节、staging 与失败清理归 `bootstrap-install` | 复查台账 `Lord1Egypt/AndroidThothTerm` 行；「新增失效语料记录」行；「能力裁定补齐行」RTL/BiDi 行；`text-selection`；`modifier-bar-height-reservation`；`session-list`；`bootstrap-install` |
| `10-ownerroc-termux-monet.md` | 同源换皮的 termux fork | 不采纳（门槛 1 成立、门槛 3 不满足）：测试为官方旧快照、零新增；缺 `ITermImage` 等属更弱上游 | 复查台账 `ownerroc/termux-monet` 行 |
| `11-irealashu-ApexShell.md` | Compose 壳：每命令 `ProcessBuilder`，正则染色冒充 VT | 无门槛违反：无 VT 测试资产；逐命令执行／提权／历史持久化撞明禁 | 复查台账 `irealashu/ApexShell` 行；「参照面与反面做法收敛补齐行」空 catch 行 |
| `12-Sunbr99-TerminalOS.md` | 仓内仅 README、LICENSE 与 3 行脚本；README 声称的 Go 结构不存在 | 无门槛违反：无资产可评估 | 复查台账 `Sunbr99/TerminalOS` 行 |
| `13-imlunixdev-puppy.md` | Kotlin + C++ PTY，自研 VT 子集 + proot | 不采纳（门槛 3）：核心解析含已确认标准偏差；合并投递与粘包解码归本仓 Rust 侧 | 复查台账 `imlunixdev/puppy` 行；「参照面与反面做法收敛补齐行」渲染热路径内的 measureText 行 |
| `14-CocaKova-charon.md` | 纯 Kotlin 自研 VT + SSH，自研界质量标杆 | 门槛 1 部分不违反（外部标准期望值子集）；corpus golden 可被实现覆写 MUST NOT 采纳；`updateSelection` 经 `javap` 实测为框架 `@hide` | 复查台账 `CocaKova/charon` 行；`ime-editor-contract`「组字光标位置回报给输入法」 |
| `15-RohitKushvaha01-ReTerminal.md` | Termux 库复用 + proot，Material 3 壳（已停更） | 无门槛违反：VT 在 Maven 依赖、仓内仅模板测试；文档提供器与透传名单形态归本仓能力；可配登录 shell 的形态收敛到本仓启动入口框（绝对路径二进制、不做存在性检查、不设脚本路径） | 复查台账 `RohitKushvaha01/ReTerminal` 行；`documents-provider`；`shell-env`；`shell-entry` |
| `16-eggie-admin-hydra-shell-android.md` | Godot 壳 + 云集成，无终端代码 | 无门槛违反：Android 与原生源码为 0；提权与云集成撞明禁 | 复查台账 `eggie-admin/hydra-shell-android` 行；「参照面与反面做法收敛补齐行」提权行 |
| `17-Autonomous-Agentic-Workflows-termux-app.md` | 旧基线 fork，含真实回退（缺源文件、删 Sixel、去版本守卫） | 不采纳（门槛 3）：弱化子集；跟 master 并保留上游能力 | 复查台账 `Autonomous-Agentic-Workflows/termux-app` 行；「参照面与反面做法收敛补齐行」删减式回退行 |
| `18-mirute02-anoterm.md` | Kotlin/Compose 自研 VT 子集 + SSH | 不采纳（门槛 3）：断言对象为其自研组件；宽度与解码口径与本仓一致处归各能力 | 复查台账 `mirute02/anoterm` 行；`cjk-rendering`；`text-search`；「能力裁定补齐行」IME 组字内外删除分流、组字光标模式行 |
| `19-TOYTTQE-PufiX.md` | 桌面 Python 文字交互壳 | 无门槛违反：只产生 ANSI 从不解析，无网格、无 PTY | 复查台账 `TOYTTQE/PufiX` 行 |
| `20-Malik1stmkw-termux-clone.md` | 空仓库（一句 README，无源码） | 无门槛违反：无资产 | 复查台账 `Malik1stmkw/termux-clone` 行 |
| `21-Adhi-hub07-on3-terminal.md` | termite 范式：C PTY + Kotlin 自研 VT | 无门槛违反：零测试资产；PTY 子进程核对清单归 `pty-lifecycle`；自造 `svc #0` syscall 不得反向采纳 | 复查台账 `Adhi-hub07/on3-terminal` 行；`pty-lifecycle`；「参照面与反面做法收敛补齐行」自造 `svc #0` syscall 做 JNI 桥行 |
| `22-frostre1997-kosh.md` | ReTerminal 同系 + 预装 Alpine rootfs | 无门槛违反：VT 在 Maven 依赖；预装 rootfs 与共享存储启动撞明禁 | 复查台账 `frostre1997/kosh` 行 |
| `23-shafinthedev-Ply.md` | Rust 行循环 REPL + TextView 壳 | 门槛 1 不违反但零判别力：两个测试均坏；管道式引导安装不得反向采纳 | 复查台账 `shafinthedev/Ply` 行；「参照面与反面做法收敛补齐行」`curl \| sh` 式引导安装行 |
| `24-Rprop-AnsolePlus.md` | 旧版 Termux 打包（剥离图像协议） | 不采纳（门槛 3）：停在旧 rev、图像族全缺；JNI 字符串释放错位不得反向采纳 | 复查台账 `Rprop/AnsolePlus` 行；「参照面与反面做法收敛补齐行」JNI 字符串释放错对象行 |
| `25-reapercanuk39-termux-kotlin-app.md` | termux 完整 Kotlin 化 + AI 代理运行时 | 门槛 1 不违反（仅键位编码与鼠标两类断言）；AI/MCP 撞明禁；宽表体量佐证不自研宽字符表 | 复查台账 `reapercanuk39/termux-kotlin-app` 行；「能力裁定补齐行」Model Context Protocol 行 |
| `27-改进点收敛与openspec落点.md` | 改进点收敛四类表（已覆盖/在途/允许/明确拒绝）与落点 | 已处置：初判「新增能力 0」经复核自证伪并改正，落 `grid-extraction-incremental`；其余 P1/P2 为实现指导 | `grid-extraction-incremental`；本表 `01`–`25` 各行 |
| `28-复审循环与残留问题收敛.md` | 二十余轮复审循环记录：已修／误报／不改／仍未修逐条 | 已处置：可执行项逐条落代码与判别力用例；两处原判「仍未修」已由后续裁定收口——IME 组字锚点经**公开** `InputMethodManager.updateCursorAnchorInfo` 实现（`InputConnection` 上的同名符号才是 `@hide`），rev 哈希张力按「hash 只在依赖钉里」裁定；其余未查清与覆盖缺口归「研究阶段残留项与覆盖缺口登记」；`UnsupportedQueueFamily` 的定性（surface 丢失的后果而非独立缺陷，`get_capabilities` 失败即报此错）已落 `native/src/render/context.rs` 重建慢路径的注释与自愈阈值 | `ime-editor-contract`「组字光标位置回报给输入法」；本能力「rev 本身不进正文」；「研究阶段残留项与覆盖缺口登记」；`native/src/render/context.rs` |
| `29-减码杠杆与libghostty家族结论收敛.md` | 减码 L1–L6 分级与红线；libghostty-vt 对比 librio | 采纳并已执行（L1）：JNI 批量合并按 `DESIGN.md` 背书落地（标题／单元格宽高／主题目录／会话计数收敛为批量查询），`getMode` 导出删除；L2–L6 维持「有界重构建议，不扩大为变更」；红线（不减测试、不删 DESIGN 功能）为约束；不换核、不等完整 libghostty 上 Android 归禁止项三条保留 | 「禁止项三条保留」段；「研究阶段残留项与覆盖缺口登记」L2–L6 行；`openspec/changes/archive/` 台账 |
| `30-VT测试边界与外部基准接入收敛.md` | VT 测试边界与外部基准接入；优先级 1–3 已落地 | 已落地：`native/tests/external_benchmarks/`（UAX #11 全量宽度、ghostty fuzz 语料回放、kitty 键盘协议）；边界与「不扩张成穷举 VT 一致」一致 | `terminal-state-regression`；本能力「上游 VT 语义不由本仓测试覆盖」；「语料判定补齐行」`corpus/stream-initial` 行 |
| `终端模拟器生态深度研究报告.md` | 生态深度报告：S/A/B/C 评级、五条 VT 路线、跨仓扫描 | 已处置：评级语义归本条「生态参照面组行」定义；路线与扫描结论归本条「生态内其余仍需对照的结论」段 | 本条「生态扫描中仍作为判断依据保留的事实」段与「原 `ecosystem.md` 的 S/A/B/C 参照面评级」段；本表各行 |

上表末 31 行键为另一份退役编号集的文件名（`00`–`30` 去掉 `26` 的 30 份编号文档
加 `终端模拟器生态深度研究报告.md`；`26` 为 API 清单，成文时即移出研究目录，不在
本次退役范围）；该编号集曾用于立项 `bootstrap-install`、PTY 收尾清单、搜索上限与
文档提供器形态，其逐仓结论已由「Android 侧终端项目的复查台账」承接、其余结论按
上表各行指向表内可读位置。
