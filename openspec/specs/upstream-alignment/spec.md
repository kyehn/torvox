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

**2026-11-15 依赖版本复查**：

- 绑定层：`libghostty-vt` 所钉提交与上游 master 提交数差为 0（当日 master），上游最新提交日期
  2026-11-14。本次前进的提交中，两项把快照解码器的 kitty 图像计数与待传字节总量迁到
  `IncrementalDecoder` 上、一项把可选 `allocator_api` feature 改用标准库分配器 trait、一项
  抬高构建用 rustc。本仓未消费 `snapshot` 解码接口，也未启用 `allocator_api`；实际消费的
  API 面（`key`、`mode`、`color`、`style`、`selection`、`search`、`screen`、`render`、`point`、
  `paste`、`mouse`、`kitty::graphics`、`grid_ref`、`terminal`、`effect`、`report`、`set_logger`）
  无签名或语义变化，全量 Rust 测试与其之上的 Android 仪器测试保持全绿。
- crates.io：`nix` 由 0.31 升到 0.32（最新稳定，2026-11-14 发布），其余直接依赖已是当日最新
  稳定版本；`fontdb` 仍受 `cosmic-text` 的 `^0.23` 要求约束（见依赖活跃度表）。
- Gradle/Android：Gradle wrapper 9.9.1、KSP 2.3.14、Robolectric 4.18 为当日最新稳定版本。
  `androidx.test`、Espresso 与 Benchmark 存在更新的 alpha，但当前所用为稳定版，按「最新稳定
  版本」口径保持不动；AGP 9.6.0-alpha03、lifecycle 2.12.0-alpha05、activity 1.14.0-alpha03、
  detekt 2.0.0-alpha.6、LeakCanary 3.0-alpha-9 属「已用不稳定版本且无更高稳定版本」，保持该线
  最新不稳定版本。

#### Scenario: 上游改的是语义而非签名

- **WHEN** 上游改动使既有调用的**含义**改变而签名不变（如回滚字节预算的 `None`
      由「无上限」变为「恢复默认上限」）
- **THEN** 本仓 MUST 逐条核对该调用点的既有取值是否仍表意，MUST 用既有本地用例
      钉住行为（去掉修复即失败），MUST NOT 只以「能编译」判定跟进完成

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

- **WHEN** 采纳一份需要外部文件的上游语料（快照对、规范正文、模糊种子、权威表）
- **THEN** 语料 MUST NOT 随仓入库，MUST NOT 存放上游 `LICENSE` 或任何许可证文本；
      MUST 在测试运行期从上游取数，取数失败 MUST 带 URL 与原因判红
- **THEN** 形式上的理由 MUST 满足 TESTING 的两条约束：测试不依赖仓库状态，且缺前置
      时必须失败而不是跳过。随仓放文件会制造需要人手工同步的第二份真相，并迫使仓库
      存放上游许可证；而运行期现取会把上游行为变成本仓的判红条件，故本仓当前**不引入**
      需要外部取数的上游语料——见「上游 VT 语义不由本仓测试覆盖」的「外部基准不入仓」
      Scenario；日后若恢复取数，MUST 集中在一处且 `ureq` 只进 dev-dependencies。
- **AND** 来源 MUST 在注释与常量里指向上游仓库、目录与**不可变定位符**——发布标签
      （kitty `v0.49.2`、ghostty `v1.3.1`）或权威版本的稳定 URL（UCD `latest`，
      期望值随同一份数据推导），MUST NOT 用提交号（提交号随 rebase/force-push 失效
      且读者无法还原）
- **AND** 取回的正文 MUST 原样使用：宽度真值按 UCD 数据推导（终端与 UAX #11 的既定
      分歧逐条钉在显式覆盖表里，上游一改即判红待复核）、键盘序列按规范正文推导、
      模糊种子按原始字节回放，MUST NOT 手工转录成第二份真相；体量小且与输入同处一表
      的载荷（kitty 1×1 PNG 等）才是内联进测试源码常量表的形态
- **AND** 本 Scenario 只覆盖「本仓功能断言所需的外部数据」；上游 VT 语义类语料
      （xterm 快照对等）MUST NOT 借本行重新引入，其去向见「上游 VT 语义不由本仓
      测试覆盖」
- **AND** 测试 MUST 直接从取回的数据或该常量表读取，MUST NOT 提供回退路径：MUST NOT
      回退到本地副本、MUST NOT 静默跳过；缺前置时 MUST 失败而不是跳过

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
- **THEN** 记录 MUST 写明是绑定层滞后，并给出可复核的两处上游事实——提交日期与
      提交数差；MUST NOT 写 hash（见「rev 本身不进正文」，rev 的唯一载体是
      `Cargo.toml` 的依赖钉）

#### Scenario: 终端上游与绑定层同时缺失

- **WHEN** 某 VT 特性在终端上游 master 与绑定层所钉提交中都不存在
- **THEN** 记录 MUST 写明是上游缺口，并给出两处可复核事实（上游 master 的源码
      事实与绑定层所钉提交的源码事实），MUST NOT 记为绑定层滞后
- **AND** 应用 MUST NOT 以绕过上游的方式补齐该能力（如自研 VT 解析、自建共享
      内存通道）；协议定义了应答时 MUST 按协议回明确失败，未定义应答时 MUST NOT
      伪造应答

### Requirement: 上游 VT 语义不由本仓测试覆盖

VT 解析与重排是上游 `libghostty-vt` 的职责（DESIGN 单一来源，`ghostty_terminal/tests/` 删除集合已列明）。TESTING 只测本项目功能、不做回归，故 xterm 语料、缺口触发器与重排往返三类断言 MUST NOT 留存本仓：`native/tests/vt_corpus.rs`、`native/tests/vt_conformance.rs`、`native/tests/vt_upstream_blockers.rs`、`native/tests/reflow_round_trip.rs` 已删除。

#### Scenario: 外部真值类断言的同口径

- **WHEN** 复查某项断言是否值得保留在本仓测试里
- **THEN** 以外部基准或重复 Gherkin 套件承载的断言 MUST 保持删除状态，同类断言 MUST 只在本地单测保留一条

外部真值类断言同口径 MUST NOT 留存：以 Unicode UCD 宽度表、kitty 规范字节、ghostty fuzz 语料为期望的三类外部基准（`native/tests/external_benchmarks/`，9 例）已删除，`ureq` 随之退出 dev-dependencies。逐条复述既有本地单测的 Gherkin 套件（`native/tests/bdd/`、`native/tests/features/`，7 特性 12 场景）同批删除，`cucumber`／`tokio` 退出 dev-dependencies——TESTING「不保留无意义或低价值测试」之下，与本地单测重复的断言 MUST 只留一条。

历史实测零本仓 bug：语料引入修的是测试侧空格口径（43 例误报），随后两次改动分别订正多参理由与自造输入，内联改动只搬运语料；重排三条只加测试无产品修复；外部基准三项实测分歧（kitty 16 处、宽度 268 个码位）全部落在上游与终端惯例，产品修复为零。同口径的删除先例见归档变更「删除回归金标语料与重复着色器校验测试」与「删除未发现本项目可解问题的上游测试资产」。`concurrency.rs`（`EventQueue`）与快照管线等本地单元测试保留。

#### Scenario: 上游语义测试不再引入

- **WHEN** 某上游 VT 语料或重排回路可直接通过
- **THEN** MUST NOT 以此为由引入本仓测试；通过只说明上游行为未变，不说明本仓功能正确

#### Scenario: 外部基准不入仓

- **WHEN** 候选用例的期望值由上游决定（UCD 宽度表、kitty 规范正文、ghostty fuzz 语料、上游 `key::Encoder` 产出的字节、上游快照行为）
- **THEN** MUST NOT 引入：判红只说明上游 rev 升级改了行为，与本仓功能正确性无关；上游若真改坏本仓契约，由既有本地单测的接缝断言判红
- **WHEN** 候选用例的断言与 `native/src` 内既有单测逐条重复，只是换成 Gherkin 或另起 harness 叙述
- **THEN** MUST NOT 引入：唯一断言 MUST 只留一条，多一层间接不构成覆盖

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

#### Scenario: 实测数字与差异归因

- **WHEN** 采纳上游资产或登记其差异
- **THEN** MUST 附今日实测的通过与差异数字、把差异归因到具体层，且临时探针 MUST 在判定后删除

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
| `Lord1Egypt/AndroidThothTerm` | 门槛 1 不违反，但仅作判据来源登记 | 2026-11-02 复查：`emulatorview/src/test` 5 个文件 / 34 个 `@Test`，期望值为人工手写外部真相（如控制字符串不得绘制、选区按逻辑序还原）。其引擎为 `jackpal/Android-Terminal-Emulator` 血统，OSC 只有 0/1/2、DA1 自述 `ESC[?1;2c`、DECSET 仅插入模式 4、鼠标只有 1000–1003 与 1006，无 sixel/kitty，能力低于本仓上游，断言只能当边界清单。逐项判定见本表下方「能力裁定补齐行」与「参照面与反面做法收敛补齐行」 |

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

**性能对比资产登记：**

| 项目 | 终端内容渲染路径 | 基准资产 | 可核实的官方性能数字 |
| --- | --- | --- | --- |
| `termux/termux-app` | HWUI `Canvas`（`TerminalView` 继承 `View`，`TerminalRenderer` 以 `canvas.drawTextRun` 绘制） | 仓内无 benchmark 模块 | 无 |
| `sylirre/ghostty-android-terminal` | HWUI `Canvas`（`TerminalView` 继承 `View`，`canvas.drawText` 逐 run 绘制） | 仓内无 benchmark 模块 | 无 |
| 本项目 | wgpu 直绘 SurfaceView（终端内容不经 HWUI） | `android/benchmark` 宏基准模块与 `native/benches` 微基准 | 无（模拟器数据只作数量级） |

两者的终端内容帧都经 HWUI 管线，本项目的终端内容帧由原生线程直绘 SurfaceView，故端到端帧率
不具跨应用可比性；跨应用帧面是系统侧帧面且分三层——逐图层 `dumpsys SurfaceFlinger --latency`
给逐帧三列（精确名匹配任何承载内容帧的图层，含本项目终端 SurfaceView 层），显示级
`dumpsys SurfaceFlinger --frametimeline` 给 display frame 级时序并按 `Owner Pid` 归属到内容
图层，见 `performance-measurement` 的「跨应用性能对比的对照面与统一帧面」「跨应用性能对比
的帧面分层与覆盖面」「终端内容直绘 SurfaceView 时的帧面取数」「SurfaceView 内容帧的 jank 判定与 N/A 兜底」各条。两者既无基准资产，也未
发布可核实的性能数字，故对比必须自建 harness，MUST NOT 引用其自述性能数字。

**新增失效语料记录：**

| 项目 | 失效语料 | 依据 |
| --- | --- | --- |
| `Lord1Egypt/AndroidThothTerm` | `tests/` 共 **131 个文件 / 36 个子目录 / 924K**（2026-11-02 复查） | 全仓（含 `.github`、`build.gradle`、构建脚本）对 `tests/scripts`、`tests/wideChars` 等目录的引用数仍为 **0**。其中 `tests/scripts` 的期望写在脚本末尾 `#expected screen:` 注释里，判定者是人眼；`tests/wideChars` 是散文加手打屏幕 |

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
| 安全契约的正面不变量（外部 Intent 数据不得键入 PTY、只走工作目录／argv／环境变量等结构化通道；外部路径逐段校验且不跟随既有链接；检查失败即拒绝不落回宽松路径；每次漏洞修复留永久回归测试） | `Lord1Egypt/AndroidThothTerm`（`docs/security/THORNS.md`） | 已采纳为正面不变量：`app-lifecycle` 的「外部 Intent 的数据只走结构化通道」、`documents-provider` 的「外部寻址一律按根内包含性判定」与 `bootstrap-install` 的「引导归档的来源路径必须限定在应用私有目录内」三条 Requirement 承接；本仓对外入口为导出的启动 Activity 与文档提供者，两者各自的处置见上述条目；归档入口原先放开任意路径，已按私有目录包含性收紧并留回归用例 |
| 导出面策略测试（从构建产物读合并清单并与策略文件精确比对，多一项即失败） | `Lord1Egypt/AndroidThothTerm`（`tests/security/exported_components.py` 与 `tests/security/policy/`） | 仅参照：本仓导出面为启动 Activity 与文档提供器（`android/app/src/main/AndroidManifest.xml`），可用同样思路钉住不得新增导出组件；若采纳，检查体系须只用 Nushell（`STYLE.md`），不得新增 Python 体系 |
| 上传到会话当前目录（用 `/proc` 前台进程组解析会话真实工作目录，暂存加一步改名，只增不改） | `Lord1Egypt/AndroidThothTerm`（`term/src/main/java/com/thothterm/upload/` 的 `SessionDirectory`、`ProcFiles`、`StagingJournal`） | 未声明，不立项：本仓 `DESIGN.md` 无「上传到会话工作目录」条目；前台进程组解析为可靠技术判据，该判据的登记处即本行（原研究笔记所在的 `docs/persistence/android-thoth-term.md` 已删除） |

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
| 格子数据跨语言传输 | `termux-ghostty` 把脏行序列化为直连 JNI `ByteBuffer`（UTF-16 长度/折行标志/连续 cell 起点长度宽度/u64 样式/UTF-16 字符，8 字节对齐） | 网格数据全程在 Rust 内（最新帧邮箱 + 脏跟踪），不经 JNI | 不适用 | 无（我们的路径本就不跨 JNI，该优化无对象） |
| 修饰键栏 | `termux-app` 的 `termux-shared/.../extrakeys/`（`ExtraKeysInfo`/`ExtraKeysView`/`ExtraKeysConstants`/`SpecialButton`）含布局编辑器与按键配置 | 仅采纳默认布局与粘滞语义 | 布局编辑器 | PROHIBITED（明禁） |
| 选区拖动手柄 | `termlib` 的 `applyHandleDrag` 锚点语义 + 交叉翻转 | 已采纳（同源实现与单测） | 无 | 无 |
| Linux 用户态 SDK | `libtermux-android` 提供 `core`/`os`/`terminal-view`/`bootstrap-*`/`shizuku` 模块（README 自述实验阶段） | 自持 bootstrap 安装与终端视图 | Shizuku 集成 | PROHIBITED（明禁）；其余能力已由本仓自实现覆盖 |
| 其余 libghostty-rs 能力 | `gesture`/`select_output`/`format_selection_*`/`snapshot decoder`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_progress_report`/`device_attributes`/`xtversion`/`color_scheme` 回调/`set_terminfo_name`/`track_grid_ref`/`width_px`/`height_px` | 均未采纳 | 无消费者 | 本仓（按需取用，见「上游 API 取用判定」表；`report::ColorScheme::encode` 已采用见同名行） |
| 桌面通知回调 | `terminal.rs:3475` 的 `on_desktop_notification` 经 C ABI 的 `GhosttyTerminalDesktopNotification` 回调暴露；上游 `osc9.zig` 与 `rxvt_extension.zig` 分别把 OSC 9 与 OSC 777 的 `notify` 归到同一条 `show_desktop_notification` 命令 | 已采纳（2026-11-05 改判）：注册上游回调，通知按会话归属投递，前台应用内提示、后台系统通知、点击切换会话 | 无 | 无 |
| Kitty 图像 PNG 解码 | `kitty/graphics.rs:997` 的 `RustPngDecoder`（`pub const fn new`）经 `set_png_decoder` 注册；解码前先按 libghostty 分配器取定长缓冲，未受该上限约束的尺寸不会被 PNG 头放大 | 已采纳（本次改判）：删除本仓 60 行等价实现与 `png` 直接依赖 | 无 | 无 |
| 颜色语法与深浅判定 | `style.rs:390` `RgbColor::parse`（3/6/9/12 位 hex、X11 名、`rgb:`/`rgbi:`）与 `style.rs:415` `perceived_luminance`（文档写明「背景亮度超过 0.5 即视为浅色」） | 已采纳：主题色值与深浅分组都走上游，本仓不再手写 `#RGB` 展开与 Rec.601 公式 | 无 | 无 |
| 终端模式查询 | `terminal.rs:1380` `Mode::BRACKETED_PASTE`、`terminal.rs:1367` `Mode::FOCUS_EVENT` 具名常量 | 已采纳：`Query::ModeGet` 携带上游 `Mode`，删掉 `(u16, u8)` 裸号与各调用点的数字 | 无 | 无 |
| `fmt::Formatter` 全量纯文本 | `fmt.rs` `Formatter::new(terminal)` 文档写明只覆盖「a terminal's **active screen**」，只覆盖活动屏 | 不采纳：本仓的 `ReadAllText` 覆盖回滚 + 视口（`scrollback_len + grid_rows`），格式化器取不到已滚出的行 | 逐格 FFI 扫描（每格 `grid_ref`→`cell`→`codepoint`）在回滚填满时单趟可越过查询超时，靠 `all_text` 缓存兜住 | 上游（格式化器需补回滚范围参数） |
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
| 0 / 2（图标名 / 窗口标题） | 上游解析；标题经 `Query::Title` 与快照 `title` 观察 | `internal/dispatch.rs` 的 `Query::Title` 与快照 `title`；`tests/grid_text.rs`、`tests/osc_notification.rs`、`tests/selection_search.rs` 的 OSC 0/2 标题可读用例（含跨分块重组） |
| 1（图标名查询） | 上游解析；本仓无消费者，不另设查询 | 标题查询已承载用途；`on_title_changed` 类回调不采用见「上游 API 取用判定登记」 |
| 4（调色板覆盖） | 上游处理且有测试钉住生效路径 | `internal/run.rs` 初始主题注释；`internal/tests.rs` 的 `osc4_palette_override_reaches_dumped_grid` |
| 7（工作目录） | 上游解析，本仓不消费；透传行为有测试 | `output_processor.rs` 模块文档与 `osc7_and_osc8_pass_through`；`on_pwd_changed` 不采用见「上游 API 取用判定登记」 |
| 8（超链接） | 上游解析；本仓经 `hyperlink_at` 查询网格单元 URI | `internal/selection.rs` 的 `hyperlink_at_impl`（4KiB 内联缓冲 + 越界精确重读） |
| 9（iTerm2 通知） | 上游解析并观察：本仓注册 `on_desktop_notification` 回调，通知按会话归属投递 | `DESIGN.md` 软件节要求处理 OSC 9 通知（前台 Toast、后台系统通知、点击切换会话）；回调注册见 `ghostty_terminal/internal/run.rs`，分派见 `runtime/DesktopNotificationPlayer.kt`，规约见 `desktop-notification` |
| 10 / 11 / 12（默认前背景色 / 光标色查询与设置） | 上游不处理，默认值归内嵌方；本仓经原生主题 setter 置默认色 | `internal/run.rs` 初始主题段落三处注释（默认色归内嵌方、OSC 4 覆盖归上游、故走原生 setter 而非手写 OSC 序列） |
| 52（剪贴板读写） | 写走上游回调；读取请求由本仓扫描器收割并拼帧应答 | `upstream-alignment`「OSC 52 读取请求的收割与应答须对每个会话有界」；`output_processor.rs` `ReadScan` 状态机 |
| 104 / 110 / 111 / 112（色重置） | 上游处理；跨分块重组有测试 | `tests.rs` OSC color reset 跨分块用例 |
| 133（shell 集成标记） | 上游解析（若支持）；本仓不消费 | DESIGN 未声明 prompt 标记语义；`upstream-alignment` 预测线条目判「未声明，需先补 DESIGN」 |
| 777（rxvt 桌面通知） | 上游解析并观察：走上游 `on_desktop_notification` 回调（同 OSC 9） | 上游 `rxvt_extension.zig` 把 `777;notify;标题;正文` 归到 `show_desktop_notification`，与 OSC 9 同一条命令 |
| 1337（iTerm2 媒体 / 通知） | 不接：零消费者 | 上游 `iterm2.zig` 只实现 `Copy` 与 `CurrentDir`，通知类负载（`RequestAttention`）上游记为未实现；本仓不消费 |

#### Scenario: 新增 OSC 消费者时先补 DESIGN

- **WHEN** 拟为某个 OSC 段新增本仓消费者（如 cwd 展示、桌面通知）
- **THEN** MUST 先在 `DESIGN.md` 声明该功能再接线，
      MUST NOT 以「上游已解析」为由直接消费

### Requirement: 研究报告逐份结论登记在研究台账能力内

`docs/research` 的全部研究报告（编号文档集 31 份 + 逐仓报告 73 份 + `ecosystem.md`，
另加 6 份综合研究报告，合计 111 份，已全部退役）的可执行项 MUST 在 `research-ledger`
能力内有逐份可复核的结论：
每项 MUST 写明判定结果（采纳 / 不采纳及违反哪条采纳门槛 / 未声明不立项 /
PROHIBITED 明禁）与依据，MUST NOT 只以「其余若干份已核对」这类群体概括收尾，
MUST NOT 把逐份结论只留在归档变更里。

判定口径：外部语料的期望值若为其自身实现的输出（可被实现覆写、或固化了该实现
当年的偏差），门槛 1 成立，不采纳；断言打在本仓不实现的第三方 VT 组件上，门槛 3
不满足，不采纳；功能若 `DESIGN.md` 未声明，按 `STYLE.md` 不实现，登记为不立项；
撞 `PROHIBITED.md` 者登记为明禁。许可证 MUST NOT 作为判定依据。

#### Scenario: 任何一份研究报告都有可复核行

- **WHEN** 复查任一份已退役 `docs/research` 报告的结论
- **THEN** 在 `research-ledger` 的本表或相邻台账表内读到该报告的判定与依据，
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

#### Scenario: 语料资产逐项判定

- **WHEN** 判定某上游语料资产是否采纳
- **THEN** MUST 按下表给出判定与可复核依据，采纳规模 MUST 以仍可读取的输入为准

| 资产 | 判定 | 可复核依据 |
| --- | --- | --- |
| ghostty `corpus/stream-initial` 01–24 | **采纳后撤回**（不测上游） | 曾落地为 `native/tests/external_benchmarks/upstream_corpus.rs`（按发布标签取数、清单按名排序并断条数）；后随外部基准整体删除——期望由上游语义决定，通过只说明上游未变。日后复查 MUST NOT 重建，见「外部基准与 BDD 套件的退场」行 |
| migueldeicaza/SwiftTerm 175 个 Swift 测试文件 | 不采纳（门槛 1） | 期望值是其自研 VT 实现的输出；与 Ghostty 的 fuzz 语料对照思路曾由上条落地、后随外部基准撤回 |
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

#### Scenario: 功能面逐条裁定

- **WHEN** 生态扫描命中某个功能面
- **THEN** MUST 按下表给出出处与判定；`DESIGN.md` 未声明者 MUST 记为不立项

| 功能面 | 出处 | 判定 |
| --- | --- | --- |
| Model Context Protocol | godot-pty-gpty、lumina、ntilde、par-term、par-term-emu-core-rust、ratty、Scanline-term | **PROHIBITED 明禁**（`PROHIBITED.md` 终端节「Model Context Protocol」）；口径更正：先前记「未声明」属错裁，按 BiDi 更正同例改判；出处更正：先前引 copad、Lume 属误引（两报告无 MCP），改引实测命中者，同时补入 gpty |
| 插件系统、事件总线、mux daemon | ratty、par-term、copad、hollow | 未声明，不立项；出处更正：先前引 Scanline-term 属误引（该报告无插件/mux 内容），已按实测命中移除 |
| 无障碍/文本捕获（`readText` API） | vercel-labs/wterm | `DESIGN.md` 无无障碍面，**未声明，不立项**；朗读形态另撞 `PROHIBITED.md` 终端节「无障碍朗读」；口径更正：先前整行记「PROHIBITED 明禁」系以朗读条目覆盖整个无障碍面，按报告实测（仅可访问性/文本捕获）更正 |
| WebSocket/网络终端形态（服务端拆分、DOM 单元布局、Electron/Qt 集成、WebView 远程终端） | DomTerm、terminal7、wterm、copad（`copad-mobile`）、ThothTerm（LAN 模式） | 未声明，不立项（本仓无网络终端；wterm 的重连与背压设计仅在日后开网络终端时参考） |
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
| RTL/BiDi 文本整形 | RtlTerminal、ThothTerm | **未声明，不立项**；口径更正：`PROHIBITED.md` 无 BiDi 条目，「已禁止」的表述无依据，正确口径是未声明。2026-11-02 复查：ThothTerm 已在 `PaintRenderer` 内实现 `BidiLayout`／`BidiRun` 与整段 RTL 整形（逻辑列↔视觉列映射见 `TranscriptScreen`），其做法为 Canvas 按方向段绘制并依赖平台整形；本仓渲染为 wgpu 单元格实例化（`native/src/render/cell_builder.rs`），`DESIGN.md` 明令「不做跨单元连字」，阿拉伯文字连接天然跨单元格，采纳即改渲染架构并违背既有声明，上游 Ghostty 亦无 BiDi。**不得反向采纳**其整段整形改造 |
| 界面 layoutDirection RTL | RtlTerminal | 未声明，不立项 |
| copy-on-selection（松手即复制） | godot-xterm | 不采纳：选择交互以菜单点击复制为唯一入口（`text-selection`） |
| bell 冷却/节流 | godot-xterm | 不另立一面：响铃已在 `runtime/BellPlayer.kt` 内按上游 `BellHandler.MIN_PAUSE` 同理由限频（`termux-configuration`「bell-character 的三态执行」），上游的独立冷却面无第二消费者，不重复实现 |
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

#### Scenario: 反面做法点名与归属

- **WHEN** 评审参考实现的具体做法
- **THEN** MUST 按表点名出处并标明可参照或不得反向采纳

| 项 | 出处 | 判定与依据 |
| --- | --- | --- |
| PTY 输出合并投递与 UTF-8 粘包保留解码 | puppy、anoterm | 仅参照：本仓会话与输出管道在 Rust，字节直传上游 VT stream，无自研粘包解码；跨块半扫由 `output_processor.rs` 用例覆盖 |
| 迁移文档的问题清单组织 | termux-kotlin-app | 仅参照（文档思路，无代码落点）：本仓文档面为 `openspec/` 与 `docs/specification/`；其「保留上游测试套件」不是正例——单测仅 2 文件约 18 例，可采纳断言仅键位编码与鼠标两类（主台账 `reapercanuk39/termux-kotlin-app` 行） |
| 自造 `svc #0` syscall 做 JNI 桥 | on3-terminal | **不得反向采纳**：绑定面走标准 JNI／libc 调用，自造 syscall 无跨设备可移植性 |
| `curl \| sh` 式引导安装 | shafinthedev/Ply（`src/main.rs:102` 的 `curl -s {} \| sh`，包管理安装流程） | **不得反向采纳**：引导安装按 `bootstrap-install` 固定流程执行，不把远端脚本接进 shell 管道 |
| 渲染热路径内的 `measureText` | puppy | **不得反向采纳**：与既有台账行「在渲染热路径内改写终端状态」同口径，文本度量不得进每帧路径；其构建产物入库一并计入既有台账「仓库内提交 CMake 生成物与构建产物」行 |
| 治理文档膨胀（「文档海」） | termux-app-rafacodephi、termux-kotlin-app | **不得反向采纳**：文档面收敛为 `openspec/` 与 `docs/specification/`，研究过程记录不常驻 |
| 整包完整性校验（BLAKE3 与 sha256 双实现） | termux-app-rafacodephi（`BootstrapIntegrityVerifier` 走 BLAKE3；loader `BootstrapChecksumValidator` 走 sha256） | 不采纳：`DESIGN.md` 要求 bootstrap 不做 SHA-256 或其他完整性校验，`PROHIBITED.md` 另禁 sha256 独立校验文件；整包校验两种形态均不落地 |
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

`docs/specification/TESTING.md` 要求而现行门禁缺席的，以及门禁工具覆盖面小于
代码树的，共六项，全部落在 AGENTS.md 禁改区（`scripts/`、`.github/`、
`android/app/build.gradle.kts`）。维护者已裁定「都不改，只登记」，落地方案记录于此，
不擅自实施：本表各项 MUST 维持登记状态，未经维护者明确同意 MUST NOT 实施。

登记 MUST NOT 只有落地方案而无事实基线：日后获授权实施的人 MUST NOT 需要重新调研
「改之前是什么样」。故每项 MUST 同时登记**实测基线**与**实施时的验收判据**，
使门禁的判定以基线为准而非以「无输出」为准。

#### Scenario: 实测基线与验收判据

- **WHEN** 记录规范要求但尚未落地的门禁项
- **THEN** MUST 给出规范依据、落地方案与当次实测基线，不得以计划代替实测

| 项 | 规范依据 | 落地方案（未实施） |
| --- | --- | --- |
| aislop / jscpd 代码检查门禁 | TESTING.md 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 |
| rapidocr OCR 自动化 | TESTING.md 环境节 + 外部基准优先级 4 | `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用 |
| CI 产物哈希比对 | vercel-labs-wterm 研究结论 | `build.yml` 增产物 sha256 记录步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | GlobalTechInfo-RedTerm 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 |
| `cargo-nextest` / `cargo-mutants` / `cargo-tarpaulin` 测试工具链 | `rust-dependency-survey-2026.md` 研究结论 | `scripts/check.nu` 的 `cargo test --no-fail-fast` 改用 nextest 运行；变异测试与覆盖率阈值另配 CI 步骤 |
| detekt 对仪器化测试源集的覆盖面 | 本仓 90 个仪器化用例文件（`TESTING.md`「仪器化用例」的覆盖面） | `scripts/check.nu` 的 `detekt` 改列 `detektDebugAndroidTest`（或等价聚合任务），使仪器化源集与生产、单测源集同受 detekt 规则约束 |

实测基线（本轮实跑所得，工具版本可能前进，重跑 MUST 以当日实测覆盖本列）：

- **aislop**（须以 `npx aislop@latest` 调用，本仓 nix 环境未内置该可执行文件。
  2026-11-20 重测时 `latest` 解析到的 0.22.2 已可正常下载运行，其结果与 0.22.1
  完全一致（`score` 100、label `Healthy`、0 errors / 0 warnings）；本次基线取自
  `npx aislop@0.22.1`，并以 0.22.2 交叉复核一致）：
  `npx aislop@0.22.1 scan --json native/src` 退出码 0，`score` 100、label `Healthy`、
  `coverage.supportedFiles` 116、`unsupportedFiles` 0，`summary` 0 errors /
  0 warnings / 0 fixable，`diagnostics` 为空；
  `findingAssessment.byKind` 为 `style-policy` 0 / `confirmed-defect` 0。
  全仓扫描同版本同为 0 errors / 0 warnings，
  `coverage.supportedFiles` 117、`unsupportedFiles` 208、
  `dominantUnsupported` 为 `Kotlin`、`scoreable` 为真。
  **覆盖面限制**：该工具扫不到本仓 Android 侧 Kotlin 代码，
  故本项 MUST NOT 被当作全仓门禁；它只对 Rust 侧有判别力。
  **评分可用性**：该工具仅在 `unsupportedFiles < 10` 或
  `unsupportedFiles ≤ supportedFiles × 3` 时给出分数；本仓当前 208 ≤ 117 × 3，
  全仓扫描亦可评分，但分数基线仍 MUST 取 Rust 子目录扫描。
  **与上一列基线的偏差来源**：上一列唯一 `file-too-large` 命中在
  `internal/input_filter.rs`（1159 行），其主体是 `#[cfg(test)] mod tests`（646 行）；
  用例已移到同目录 `input_filter/tests.rs`（与 `pty.rs` / `pty/tests.rs` 同一形状），
  生产文件回到 622 行、测试文件 641 行，均低于该工具 1100 行的告警阈，
  命中随之归零（`function-too-long` 仍为 0）。更早一次偏移是
  `public_api.rs`（1123 行）按职责拆成 `public_api/` 目录模块。`supportedFiles`
  由 115 升至 116 是新增该测试文件所致。
    （2026-11-21 本轮重测同值：`npx aislop@0.22.2 scan native/src` 与 `scan .` 均退出码 0，
  `score` 100、label `Healthy`、`summary` 0 errors / 0 warnings / 0 fixable、`diagnostics` 为空、
  `findingAssessment.byKind` 四类全 0；native/src 覆盖 116 supported / 0 unsupported，
  全仓覆盖 117 supported / 208 unsupported（`dominantUnsupported` 为 `Kotlin`）。
  本轮变更只在 Rust 侧，覆盖面与基线一致。）
  验收判据：接入后 `confirmed-defect` 与 `errors` MUST 维持 0，
  `file-too-large` / `function-too-long` 的条数 MUST NOT 上升（基线 0 / 0）；
  该工具归类为 style-policy 的项 MUST NOT 阻断构建，
  只有 error 级或 `confirmed-defect` 才阻断。
- **jscpd**：`npx jscpd@latest --min-lines 8 --min-tokens 60` 对 `native/src` 与
  `android/app/src/main` 实测退出码 0，2 处克隆 / 23 行重复 / 0.03%，jscpd 报告 sources 258
  （kotlin 103 / markup 13 / rust 142）；扫描范围内受版本控制的源文件 262 个
  （Rust 143、Kotlin 103、res XML 15、AndroidManifest 1），两者相差的 4 个是 jscpd 的格式
  探测器切不出 token 的文件（实测 `native/src/render/pass/mod.rs` 只含模块声明与 use，
  `res/mipmap/` 两个文件与 `res/drawable/` 中的一个文件同样不计入），不是扫描遗漏。
  （2026-11-21 重测：克隆数、重复行数、重复率与残留位置均与上一列基线同值；
  本轮新增的一处 Rust 克隆——两个主字体选择用例重复的「族名去空白 + .ttf」构造——已提取为
  `stem_target` 辅助函数消除，Rust 侧克隆归零。）
  2026-11-21 本轮重测：仍为 2 处克隆 / 23 行重复 / 0.03%，残留位置与逐条判定同上一列
  （kotlin 1 处、markup 1 处、Rust 0 处）；本轮变更只在 Rust 侧。
  测量取自已提交的树快照，不取工作区状态（工作区可能同时存在他人进行中的改动）。
  前一列基线为 2 处克隆 / 26 行重复 / 0.04% / 115 个文件：其第二处
  `native/src/terminal/ghostty_terminal/internal/mod.rs` 内 `process_query` 与
  `drain_queries` 的同形十参形参表已随该模块重组消失；基线记录之后新增的代码引入
  了另外三处克隆（JNI 导出的注册表查找骨架、会话测试夹具、Kitty 放置撤除用例的
  送图与清屏流程），已分别收敛为 `with_session_entry`、`mock_pty::mock_session` 与
  `mock_pty::mock_arc_session`、`ed2_clears_direct_kitty_placement`。
  现存两处残留逐条判定如下：
  ① `android/app/src/main/res/drawable/launcher_monochrome.xml` 与 `logo_art.xml`
  的同形 `pathData`（14 行），不修的具体理由是 Android drawable 不支持跨文件
  引用几何，单色版必须自带路径。
  ② `android/app/src/main/java/terminal/emulator/ui/SettingsAppearanceSection.kt` 与
  `SettingsLogLevelSection.kt` 的同形包声明与 import 段（9 行），不修的具体理由是
  同包同层级的两个界面文件各自按需 import 同一组 Compose API，合并即产生
  只为转发符号的中间层（违 `STYLE.md` 的一次性抽象禁令），属天然相似。
  测试源码集另测（`android/app/src/test` 与 `android/app/src/androidTest` 分目录各跑一次，
  2026-11-21）：单测源码集 3 处克隆 / 34 行重复 / 0.18%（18560 行），仪器测试源码集
  21 处克隆 / 220 行重复 / 1.17%（18796 行），合计 24 处克隆 / 254 行重复 / 0.68%。
  逐处判定：24 处中 23 处是跨文件的同形 import 段（各文件按需 import 同一组 JUnit、
  Compose 与测试 API，合并即产生只为转发符号的中间层，违 `STYLE.md` 的一次性抽象禁令）；
  余下 1 处是 `BackgroundResumeFlickerInstrumentedTest` 与
  `TerminalHoleTransitionFillInstrumentedTest` 各自的 `@Before` 就绪壳（就绪动作已收敛为
  `TestUtils.kt` 的 `awaitTerminalReady`，两个测试类的前置动作不同故各自保留壳）。
  验收判据：重复率 MUST NOT 高于该基线；新增克隆 MUST 逐处给出
  「不修」的具体理由（被测行为不同、脚手架样板、或传状态必需的形参列表），
  MUST NOT 以「先记着」放行。
- **clean-run 计数（2026-11-21）**：`scripts/check.nu` 连续 5 次退出码 0（938 passed / 0 failed）；
  jscpd 5 次均一致为 2 处克隆 / 23 行重复 / 0.03%；aislop 5 次均 score 100 / Healthy /
  0 errors / 0 warnings / 0 confirmed-defect；semgrep 5 次均 0 findings。满足 code-health
  「连续清洁运行计数 ≥ 5」的「连续 0 确认」证据门槛。本轮唯一修复：默认主字体取正体面
  （`pipeline/families.rs`、`font_db.rs`），斜体不再退化为直立；openspec/font-selection 同步。
  （2026-11-21 本轮：修复「终端构造器静默截断行列」后重跑 `./scripts/check.nu` 仍退出码 0，
  `cargo test` 940 passed / 0 failed（新增回归用例 1 例，计数由 938 变为 940）、
  openspec validate 55/55、`markdownlint` 0 命中；连续清洁运行计数累加，不重置。
  本轮另测 aislop 与 jscpd 均与基线同值，见上两条。）
- **footgun-scan**（累计 13 次运行，0 确认缺陷）：2026-11-21 重扫实测的主源码集、单测源码集与
  仪器测试源码集的文件数、命中数与逐处判定见下方「静态形状扫描」表；同日再测与该基线同值
  （主源码集 119 文件 / 6 处命中，单测源码集 145 文件 / 3 处命中，仪器测试源码集
  91 文件 / 0 处命中）。
- **detekt 的源集覆盖面**（2026-11-21 实测）：`scripts/check.nu` 调的是 `detekt`，它覆盖生产
  与单测源集（探针：在 `src/test` 放一处未使用 import，`detekt` 退出码 1），**但不覆盖仪器化
  源集**（同一探针放到 `src/androidTest` 时 `detekt` 退出码 0，而 `detektDebugAndroidTest`
  退出码 1）。为不让 90 个仪器化用例文件长期处于静态检查之外，本轮以
  `./gradlew :app:detektDebugAndroidTest` 作临时口径全量清理：清理前 94 条命中，按规则为
  `UnusedImport` 38 / `NoUnusedImports` 11（后者与前者对同一批 import 重复报告）/
  `UnusedVariable` 8 / `LibraryCodeMustSpecifyReturnType` 15 / `ImplicitDefaultLocale` 6 /
  `UseOrEmpty` 4 / `UseDataClass` 3 / `AbstractClassCanBeConcreteClass` 2 /
  `InjectDispatcher`、`HasPlatformType`、`CollapsibleIfStatements`、`UseCheckNotNull`、
  `RedundantExplicitType`、`VarCouldBeVal`、`EmptyFunctionBlock` 各 1；清理后 0 条。
  8 处 `UnusedVariable` 均为测试内死绑定（如 `SurfaceLossRecoveryInstrumentedTest` 的
  `inkBefore` 与 `shot`）；断言仍在被调用的 `awaitInk` / `awaitBridge` 内部，删绑定不削弱断言。
  三处按「规则例外 + 就地理由」保留：无 `@Test` 的抽象基类不得改 `open`（否则会被 JUnit
  当测试类枚举）、真机用例的 `Dispatchers.Default` 不得换成测试调度器（会让真实等待失去意义）、
  `clipRead` 的显式可空类型被轮询体赋可空值（去掉后赋值不成立）。
  验收判据：接入后 `detektDebugAndroidTest` MUST 维持 0 命中；新增命中 MUST 逐条修复，
  或以「规则例外 + 就地理由」保留，MUST NOT 以「先记着」放行。
- **detekt 对仪器化测试源集的覆盖面**（2026-11-21 实测）：`scripts/check.nu` 与
  `check.yml` 跑的 `detekt` 只分析生产与单测源集，不分析 `src/androidTest`。判别实验：
  在 `src/androidTest` 放入只含未使用 import 的探针类后，`:app:detekt` 退出码 0 而
  `:app:detektDebugAndroidTest` 退出码 1；同形态探针放进 `src/test` 时 `:app:detekt`
  直接报错——证明单测源集在覆盖面内、仪器化源集不在。逐类任务名是
  `detektDebugAndroidTest`（写成 `:app:detektAndroidTest` 会被 Gradle 判为歧义前缀）。
  该源集一次性扫描实测 94 处命中：`UnusedImport` 38、`LibraryCodeMustSpecifyReturnType` 15、
  `NoUnusedImports` 11（与 `UnusedImport` 同处重复计数）、`UnusedVariable` 8、
  `ImplicitDefaultLocale` 6、`UseOrEmpty` 4、`UseDataClass` 3、
  `AbstractClassCanBeConcreteClass` 2、`InjectDispatcher`／`HasPlatformType`／
  `CollapsibleIfStatements`／`UseCheckNotNull`／`RedundantExplicitType`／`VarCouldBeVal`／
  `EmptyFunctionBlock` 各 1。已全部修至 0，其中 4 处以 `@Suppress` 就地写明规则不适用：
  两个无 `@Test` 的抽象基类（改成具体类会被 JUnit 当测试类枚举）、一个持有 `IntArray`
  的测试持有类（data class 的 equals/hashCode 会退化为引用相等）、一处刻意用真实调度器
  的采集协程（注入测试调度器会让 300ms 真实等待失去意义）。
  验收判据：接入后 `detektDebugAndroidTest` MUST 退出码 0；新增命中 MUST NOT 以扩充
  `@Suppress` 消解，只能按本表口径逐处判定。
- **rapidocr**：`flake.nix` 引导调用 `scripts/download-rapidocr-models.nu` 把模型
  下载到 `/tmp/.rapidocr-models`，模型环境已就绪；该下载与其他 shellHook 初始化步骤
  （AOSP testkey、`/tmp/alacritty-theme` 克隆）共享同一把排他锁，多个 shell 并发进入
  时 MUST 串行执行，MUST NOT 并发写同一 `/tmp` 路径；但
  `scripts/test-emulator.nu` 跑 `connectedDebugAndroidTest`、
  `connectedBenchmarkReleaseAndroidTest` 与 `connectedBenchmarkNixAndroidTest`，
  仓内无任何 OCR 消费方，即
  `TESTING.md` 的「拉截图 → 跑 rapidocr → 断言」闭环仍只存在于人工步骤。
  验收判据：实施后 MUST 有程序化断言比对识别文本，MUST NOT 只落截图或只打印识别结果。
- **CI 产物哈希比对** 与 **16KB 页对齐**：本轮未实测（MUST NOT 以推定数字入表）。
  前者实施判据以产物 `sha256` 记录步骤存在且随每次构建产出为准；
  后者须以 AGP 官方指南与实际打包产物校验为准，MUST NOT 以本仓既有配置反推。

`code-health` 要求的审计留档（静态形状扫描与依赖活跃度）同记在本节，事实与判定如下。

**门禁实际内容清单**（`code-health`「规范声明的静态检查工具必须与验证门禁一致」与
「门禁内容变更后重新比对」要求的留档，2026-11-20 逐条比对 `scripts/check.nu` 与
`.github/workflows/` 的实际内容）：

- `scripts/check.nu`（CI `check.yml` 的唯一检查工作流调用它）逐步为：`cargo fmt --all -v
  -- --check` → `cargo clippy --all-targets -- --deny warnings` → `cargo ndk -t arm64-v8a
  clippy --all-targets -- --deny warnings` → `cargo machete --with-metadata` →
  `cargo test --no-fail-fast` → `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` →
  `cargo bench --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick` →
  `cargo build --package native --profile release` → `semgrep scan --error
  --dataflow-traces --time --config .semgrep/rules.yml` → `markdownlint-cli2 "**/*.md"` →
  `openspec validate --all --strict` → `./gradlew --continue detekt spotlessCheck
  app:dokkaGenerate lintDebug lintVitalRelease assembleDebugAndroidTest
  testDebugUnitTest benchmark:compileBenchmarkReleaseKotlin
  baselineprofile:compileNonMinifiedReleaseKotlin
  -Dorg.gradle.internal.test.results.binary.enabled=false`。
- 另两个工作流不属检查门禁：`fmt.yml` 是自动修复并回推（`nix flake update`、
  `markdownlint-cli2 --fix`、`cargo fmt`、`cargo clippy --fix`、Gradle wrapper 升到
  `latest`、`spotlessApply`、`detekt --auto-correct`、`nix fmt`，随后 amend 提交并强推），
  `build.yml` 是构建与设备侧验证（先 `scripts/build-android-libs.nu arm64-v8a x86_64`，
  再 `scripts/build-apk.nu --debug`／`--nix`／`--release`，
  模拟器步骤调 `scripts/test-emulator.nu`，后者跑 `:app:connectedDebugAndroidTest`、
  `:benchmark:connectedBenchmarkReleaseAndroidTest` 与
  `:benchmark:connectedBenchmarkNixAndroidTest`）。
- 与 `docs/specification/TESTING.md` 环境节声明的比对：`aislop`、`jscpd` 与 `rapidocr`
  三项声明了检查手段但未进门禁，已在上表「落地方案（未实施）」逐项登记差异位置与接入
  方式，并各带当次实测基线；其余声明（lavapipe 提供 Vulkan 环境、nix 提供工具链与
  字体）由构建与运行环境提供，不是独立检查项，无需另立门禁条目。
- semgrep 的覆盖面：全仓扫描（`semgrep scan --config .semgrep/rules.yml --json`）的
  `errors` MUST NOT 含 `PartialParsing`。2026-11-16 实测为 0：改动前为 11 个文件 /
  104 个区域被跳过扫描（cucumber 步骤定义文件用了 CJK 标识符
  `io.cucumber.java.zh_cn.*`，semgrep 的 Kotlin 解析器解析不了），这些区域里的规则命中
  一律不会被报出；步骤注解已改为 `io.cucumber.java.en.*`（步骤正则、Gherkin 特性文件与
  匹配语义不变——cucumber 按正则匹配步骤文本，不校验注解的关键词类别，仓内已有反例：
  特性文件里的 `而且 …` 由 `@那么` 的步骤定义接住）。

**静态形状扫描**（`footgun-scan` 的 32 个模式，覆盖 Kotlin/Compose 源码；Rust 侧由
`cargo clippy --all-targets -- --deny warnings` 与 `aislop` 覆盖，本工具不适用）：

- 参数：`python3 <footgun-scan>/scripts/scan.py --all <目录>`，逐行扫描受版本控制的文件；
  目录取 `android/app/src/main`、`android/app/src/test` 与 `android/app/src/androidTest`。
- 当日实测（2026-11-21）：主源码集 119 个文件 / 6 处命中，单测源码集 145 个文件 /
  3 处命中，仪器测试源码集 91 个文件 / 0 处命中；全仓 355 个文件 / 9 处命中，全部为
  `look` 级形状提示，无 `likely` 级命中，与下表的逐处判定一一对应。

| 位置 | 命中形状 | 判定 | 依据 |
| --- | --- | --- | --- |
| `NativeBridge.kt` 的类初始化块（`System.loadLibrary("native")`） | `native-load` | 驳回 | 陷阱讲的是 JNA：同名库可能被系统副本抢先加载，故须打印实际打开的路径。本仓走 Android 的 JNI `System.loadLibrary`，装载路径由应用自身的 native 库目录决定，不存在同名系统副本兜底装载；失败路径把平台链接器给出的 `UnsatisfiedLinkError` 消息全文记入日志；构建侧保证 `.so` 打进 APK 的 `jniLibs`（`BUILD.md` 明确不设计运行时兜底） |
| `NativeBridgeSmokeTest.kt` 的 `loadNativeLibrary` | `native-load` | 驳回 | 走 `System.load(绝对路径)`：路径是常量（宿主编译产物），不经名字解析，缺失时 `System.load` 自身抛错，不存在「同名副本被抢先加载」的场景 |
| `BootstrapInstallService.kt` 的 `install` | `run-blocking` | 驳回 | `runBlocking` 在 `install()` 内，而 `install()` 由 `onStartCommand` 里显式 `Thread { … }.start()` 调用，全程在后台线程，不在主线程 |
| `RenderSupervisor.kt` 的 `startRenderThread` | `run-blocking` | 驳回 | `runBlocking` 在渲染线程的 `Thread { … }` 体（专用渲染线程）内，不在主线程 |
| `ModifierBar.kt` 的按键手势 `pointerInput(Unit)` | `pointer-input-unit` | 驳回 | 陷阱指认的是「以 Unit 为键时块内捕获的普通值不再更新」。此处块内读到的可变值全部经 `rememberUpdatedState`（按键几何、手势区、点击、重复、次要动作），没有裸捕获；以 Unit 为键是刻意选择：改用 lambda 实例作键会在长按期间的任何重组上取消并重启手势协程，静默杀死自动重复 |
| `SettingsAppearanceSection.kt` 的 `FontSizeSlider` | `slider-value-range` | 驳回 | 传入滑块的值是 `sliderFontSize`，初值经 `SettingsRepository.clampFontSize` 收敛，区间收紧时由 `LaunchedEffect(fontSizeUpperBound)` 再次收敛；区间上界与收敛入口同源（`SettingsRepository.fontSizeMaxSp`），故值恒在区间内 |
| `SettingsScreen.kt` 的设置页遮罩层 `pointerInput(Unit)` | `pointer-input-unit` | 驳回 | 块体是 `detectTapGestures(onTap = {})`，不读任何状态，无可失效的捕获值 |
| `SettingsSliderRowRobolectricTest.kt` 的两处 `valueRange =` | `slider-value-range` | 驳回 | 测试调用点的具名实参：传入值恒在区间内（区间 8f..24f 或 8f..16f，值 12f），不构成「存量值落在区间外」；该用例正是要验证区间与步数变化时重建状态，区间取自用例自身而非设置数据 |

表内不含已消失的命中：`FontManager.refreshFontList` 与 `RenderWatchDog.start` 补上
`cancel()` 后 `job-relaunched-without-cancel` 形状消失；`SettingsSliderRow` 改用
`key(valueRange, steps)` 承载区间与步数后，组件内的 `valueRange =` 实参移到调用点，
形状命中随之从该文件移到调用点（即上表的 `SettingsAppearanceSection.FontSizeSlider`
与 `SettingsSliderRowRobolectricTest` 两行）。

**显示节拍的帧率声明与省电模式分支**（`code-health`「显示节拍禁止使用固定帧率和省电模式
分支」要求的审查留档，2026-11-15 实测）：

- 固定帧率声明：`android/` 与 `native/src` 全仓无 `setFrameRate()` 调用，`Window.setFrameRate`
  与等价声明均未出现。渲染节拍仍以显示帧回调为唯一帧源，定时闭锁只作安全网（口径同
  `display-adaptation`「刷新率与省电只影响节拍不影响正确性」）。
- 省电模式分支：`PowerManager.isPowerSaveMode` 在生产代码中仅一处——`TerminalApp` 的
  `currentPowerPreference()`（应用启动期、native 装载前读一次）。其唯一去向是 **GPU
  适配器的排序偏好**：`hostPowerPreference` → `NativeBridge.setPowerPreference` →
  `wgpu_backend::set_power_preference` → `initialize_wgpu` 中 `request_adapter` 的
  `PowerPreference`。它只影响适配器选取顺序，不决定帧率，也不切换渲染路径；渲染路径
  内无任何省电模式分支。取值只影响排序，取不到适配器的风险为零，故不影响正确性。
- 前台服务的省电保活（`TerminalForegroundService` 的 `PARTIAL_WAKE_LOCK` 与配对的
  Wi-Fi 锁）属规范明列的豁免项，不在此列。

**依赖活跃度**（直接依赖的下载量与最近发布）：

- crates.io 侧（口径同 `code-health`：累计下载量 ≥ 100 万、近 90 天下载量 ≥ 10 万、
  最近发布在 12 个月内；事实取 crates.io crate 元数据接口的 `downloads`、
  `recent_downloads` 与最新非撤回版本的 `created_at`），采样于 2026-11-14（`nix` 行随当日版本复查更新）：

| crate | 累计下载 | 近 90 天 | 最近发布 | 判定 |
| --- | --- | --- | --- | --- |
| base64 | 1,966,914,514 | 525,319,336 | 2026-08-04 | 保留 |
| bytemuck | 475,531,146 | 142,627,622 | 2026-07-19 | 保留 |
| cosmic-text | 12,311,205 | 5,040,766 | 2026-04-22 | 保留（上游阻断：需 fontdb 0.23） |
| flume | 292,039,169 | 81,786,289 | 2025-12-08 | 保留 |
| foldhash | 826,077,404 | 340,488,435 | 2025-08-23 | 保留 |
| fontdb | 46,003,933 | 17,173,480 | 2026-07-29 | 保留（与 cosmic-text 同版本要求） |
| pollster | 48,575,613 | 21,836,244 | 2026-07-10 | 保留 |
| etagere | 10,167,806 | 4,450,841 | 2026-03-18 | 保留 |
| jni | 246,541,292 | 85,150,843 | 2026-03-16 | 保留 |
| libc | 1,976,289,422 | 486,586,000 | 2026-10-16 | 保留 |
| log | 1,531,065,096 | 392,713,342 | 2026-10-30 | 保留 |
| lru | 442,790,173 | 131,595,588 | 2026-11-21 | 保留（随例行维护升到 0.19.1，本行于当日复采） |
| nix | 981,417,548 | 250,769,308 | 2026-11-14 | 保留 |
| parking_lot | 1,276,525,387 | 283,846,814 | 2026-10-21 | 保留 |
| raw-window-handle | 135,190,490 | 44,606,428 | 2024-05-17 | 保留（生态基线类型，见下） |
| roxmltree | 105,796,268 | 40,792,343 | 2026-10-24 | 保留 |
| serde | 1,705,161,054 | 430,394,306 | 2026-07-18 | 保留 |
| serde_json | 1,611,830,375 | 432,551,167 | 2026-10-11 | 保留 |
| swash | 18,893,111 | 7,889,958 | 2026-07-17 | 保留 |
| thiserror | 1,849,817,042 | 528,582,996 | 2026-09-23 | 保留 |
| toml | 1,138,979,644 | 317,041,580 | 2026-11-06 | 保留 |
| unicode-script | 63,172,017 | 23,214,424 | 2026-11-02 | 保留 |
| wgpu | 50,340,268 | 18,915,738 | 2026-11-12 | 保留 |
| wgpu-types | 52,488,031 | 19,846,342 | 2026-11-12 | 保留 |
| criterion | 339,537,495 | 80,151,633 | 2026-02-04 | 保留（dev） |
| proptest | 245,654,555 | 76,495,907 | 2026-10-10 | 保留（dev） |
| shuttle | 7,517,082 | 2,498,430 | 2026-11-05 | 保留（dev） |
| tempfile | 995,301,049 | 254,563,200 | 2026-03-11 | 保留（dev） |

- 结论：28 个 crates.io 直接依赖全部通过门槛（最低者为 `shuttle` 累计 751 万、近 90 天
  249 万）。唯一的豁免是 `raw-window-handle`：最近发布停在 2024-05-17，它只出现在类型
  边界上（0.6.x 是 wgpu/窗口句柄的事实标准），按「生态基线类型」豁免「最近发布」一项。
  `lru` 一行是全表唯一的复采样行：2026-11-21 随例行维护升到 0.19.1（补丁版，锁文件
  升级，未改依赖声明文件），故「最近发布」按当日 crates.io 元数据复采，其余各行仍取
  2026-11-14 的采样值。
- 上游 git 依赖 `libghostty-vt`（`Uzaaft/libghostty-rs`，钉 rev）不在 crates.io，按本规范
  首条 Requirement 的「跟踪上游 master rev」口径核对。
- Maven 侧（Google Maven 与 Maven Central）：两仓库的公开接口只发布版本与时间戳，不发布
  下载量（`repo1.maven.org` 与 `dl.google.com` 的 `maven-metadata.xml` 只含 `lastUpdated`；
  `search.maven.org` 的 solr 接口在本环境不可达，Central Portal 站点受 Cloudflare 拦截），
  故采信口径的「累计下载量」「近 90 天下载量」两项对 Maven 侧不可测，MUST NOT 以估算或
  记忆补写。Maven 侧以「最近发布」核对，事实取各构件 POM 响应头的 `Last-Modified`：
  47 个直接依赖中 40 个落在 12 个月内；落在 12 个月之外的 7 个按「生态基线类型」豁免
  「最近发布」一项，理由如下：

| 构件 | 最近发布 | 豁免理由 |
| --- | --- | --- |
| `junit:junit:4.13.2` | 2021-02-13 | 生态基线类型：JUnit 4 是事实标准，仓内只用它的断言与运行器接口 |
| `androidx.profileinstaller:profileinstaller:1.4.1` | 2024-10-02 | 生态基线类型：Android Gradle 插件配套的安装期产物，不承担运行期逻辑 |
| `androidx.compose.material:material-icons-extended:1.7.8` | 2025-02-12 | 生态基线类型：图标资源包，版本由 Compose BOM 统一管理，不由本仓直接选型 |
| `androidx.concurrent:concurrent-futures:1.3.0` | 2025-07-16 | 生态基线类型：为对齐 AGP 传递依赖而由 Gradle 强制固定的版本 |
| `app.cash.turbine:turbine:1.2.1` | 2025-06-11 | 生态基线类型：仅测试用的流断言夹具 |
| `com.slack.lint:slack-lint-checks:0.11.1` | 2025-10-10 | 生态基线类型：仅 lint 工具链 |
| `androidx.core:core-splashscreen:1.2.0` | 2025-11-05 | 生态基线类型：启动屏 API 的稳定版，接口面已冻结 |

#### Scenario: 禁改区项不被静默实施

- **WHEN** 本轮或后续复查接触到上述四项
- **THEN** MUST 维持登记状态并按维护者裁定执行，MUST NOT 以「规范要求」为由擅改
      禁改区文件

#### Scenario: 门禁的覆盖面限制被如实记录

- **WHEN** 引用 aislop 的结果作为本仓代码质量结论
- **THEN** MUST 同时写明它不覆盖本仓 Kotlin 代码（`dominantUnsupported` 为
      `Kotlin`），MUST NOT 据其 0 error 声称全仓无问题
- **AND** 全仓扫描因覆盖面闸门而 `score` 为 `null`、label 为 `not scored` 时，
      MUST 写明该事实并以 Rust 子目录扫描的分数作基线，
      MUST NOT 把未评分当作满分或据「not scored」断言质量已达标

#### Scenario: 接入门禁时以基线而非无输出判合格

- **WHEN** 日后获授权把 aislop 或 jscpd 接入 `scripts/check.nu` 或 CI
- **THEN** 判定 MUST 以上表的实测基线为阈值（重复率不升、`confirmed-defect` 维持 0），
      MUST NOT 以「命令无输出」或「退出码 0」当作已通过

### Requirement: 研究阶段残留项与覆盖缺口登记

退役研究报告（编号集 `28`／`29`／`30` 三篇）中**未闭合**的条目 MUST 在本表留有可读去向，
与已落地项分开登记：已落地者 MUST 给出代码或能力落点，未查清者 MUST 写明「未查清」并说明
为何不凭猜测处理，覆盖缺口 MUST 写明缺的是哪半边与替代判据。MUST NOT 把未查清或不可测的
条目写成已解决，也 MUST NOT 因「做不到」而删除条目。

#### Scenario: 未闭合条目的去向

- **WHEN** 审计遗留未闭合条目
- **THEN** MUST 按下表登记维持不处理的理由；覆盖缺口 MUST 显式记为缺口，MUST NOT 冒充已覆盖

| 项 | 判定与依据 |
| --- | --- |
| `get_capabilities` 在模拟器上失败的真因 | **未查清，维持不处理**：无模拟器反馈闭环，加重试属凭猜测（编号集 `28`）。代码侧已落地的是同根因的可见缺陷修复——`attach_surface` 只在重建**确实成功**后才清失效位与连续失败计数（否则失败路径替成功路径还愿，自愈阈值永远攒不满，渲染停摆，`native/src/render/context/mod.rs`）；该修复带 `#[cfg(target_os = "android")]`、宿主无 seam，判别力用例会被迫把被测逻辑抄进测试体（第一版恒通过），故**据实登记为覆盖缺口**，不冒充已覆盖 |
| 焦点上报写出的那半边 | **覆盖缺口，维持不冒充**：`focus_event_bytes` 写出需在宿主单测里等一次 VT 线程往返，并发跑全量测试时该耗时无上界，写成断言即不稳定用例（编号集 `28`）。可确定覆盖的半边已钉在 `key-input-encoding`「查询窗口宽于 VT 空闲 tick」；写出半边只保留结构性保证（`Session::focus_event_bytes` 直写子 PTY） |
| 37 项过度复杂特性嫌疑 | **仅记录，不立即简化**（编号集 `28`）：含高风险六项（上游对标语料裁定流程、备用屏输入位移状态机、表面失效自愈限流、亚毫秒计时、渲染帧调度锁存群、字号上下限换算体系）。简化须单独立项并复现 `DESIGN.md` 保证，本轮不立项 |
| L2–L6 减码建议 | **有界重构建议（P1/P2），不扩大为变更**（编号集 `29`）：L1 批量合并已执行；红线不变——不减测试、不降覆盖、不弱化断言、不删 `DESIGN.md` 功能；想大幅减码只能改 `DESIGN.md` 收窄规格，属产品决策 |
| `sugaloaf`／`libsugaloaf` 承接 render／font 的 spike | **维持不做**（编号集 `29`）：须先有 lavapipe 基准且能复现 `DESIGN.md` 字体保证才动；`librio` 的 render-state pull API 与 staticlib 打包仅作设计参照 |
| esctest／vttest 真机精选子集 | **维持「不扩张成穷举 VT 一致」**（编号集 `30`）：全量一致套件不搬入本仓（违「不测试依赖行为」）；若引入只放真机 `connectedDebugAndroidTest` 且取精选子集。`esctest2` 另阻塞于绑定层缺 `DECRQCRA`（「已复查结论台账」esctest2 行） |
| 真实字体／bootstrap 数据扩面（原优先级 4） | **已由既有基准承担**：字体侧真度量由 `native/src/render/font/rasterization.rs` 的字体表基准（`cell_metrics_match_the_selected_face_own_tables` 按所选面自身表换算并核对格宽行高）覆盖，不再另立 hmtx 扩面；bootstrap 侧真实 zip 与装后可用性由设备侧用例覆盖；金图 OCR 属禁改区待维护者裁定（见「禁改区待维护者裁定项台账」） |
| debug 档 `cargo test` 链的是未优化 Zig 解析器 | **口径登记**（编号集 `30`）：上游 `libghostty-vt-sys` 构建脚本按 cargo 的 `DEBUG` 选 Zig `-Doptimize`，故 debug 档耗时结论 MUST NOT 外推 release；以大量组合符回放为前提的用例在 debug 档会顶穿 `flush` 上限（承载该上限的组合符回放用例已随外部基准删除，口径继续有效） |
| kitty 键盘协议 16 处上游与规范不一致 | **已逐条钉在用例内**（编号集 `30`）：方向键／Home／End／F1/F2/F4 在 kitty 模式下仍发 legacy 形式，媒体与音量键不发字节，只开消除歧义时 CapsLock/NumLock 不发字节；按「不把上游小瑕疵转为本仓测试失败」钉实测值并注明分歧，MUST NOT 按规范断言（承载用例已随外部基准删除，16 处分歧事实以本行为准） |
| CI connected 用例失败的归因口径 | **已更正为「一次一个确定根因」**（编号集 `28`）：初稿判「整体性随机不稳定」是错的——「每次用例都不同」不等于随机，逐次归因后每次都有一个确定根因，修好一个露出下一个。教训登记：判定「是否已修」MUST 核对修复提交时间与 CI 运行时间，MUST NOT 拿过期失败继续追查 |
| Kotlin 侧格式门禁的漏跑 | **教训登记**（编号集 `28`）：`scripts/check.nu` 含 `detekt spotlessCheck` 等 Kotlin 门禁，只跑 Rust 侧门禁会漏掉 Kotlin 侧格式违规；跨语言改动 MUST 跑整套门禁 |
| 报道指出但本仓已自行解决的项 | **已核销，不留待办**（编号集 `28`）：PTY 互斥量与无界写入争锁（现无互斥量）、`clipboard_text` 队列无上界（已封顶丢最旧）、VT 线程 resize 失败永久分歧（已有跨线程失败标记）、每次安装新建 `OkHttpClient`（已进程内缓存）、注释「500ms 防抖」漂移（现为具名常量 300ms）、`closeNotifyThread` 从不 `quitSafely`（复核为设计而非缺陷）；`InputBatchBuffer.close()` 已同批摘回调并复位排程标记 |
| 无可解析码位的负缓存推理 | **口径已更正**（编号集 `28`）：报告原话成立——本仓 `Pipeline::load_font_file`（`native/src/render/font/pipeline.rs`）的全部调用点都在测试模块内，生产无调用；易混的是 `font_db.rs` 中的 `font_database.load_font_file(...)`，那是 fontdb crate 的 `Database` 同名方法，非同一物。故负缓存放进 `clear_identity_caches` 后，其失效面与既有的 `cjk_glyph_cache` 完全相同，不存在「漏一处即字形消失」。用例钉两半（失败后入缓存、换字号后清空），各经一次变异验证 |
| kitty 图形上游测试载荷的退场 | **已退场，非遗漏**：20×15 与 128×96 上游载荷随「删除未发现本项目可解问题的上游测试资产」清理；现行 kitty 用例只保留 1×1 与内联 PNG。日后复查 MUST NOT 因「载荷不见了」重建它们 |
| 外部基准与 BDD 套件的退场 | **已退场，非遗漏**（维护者口径「不测上游，不写重复代码」）：以 UCD 宽度表／kitty 规范字节／ghostty fuzz 语料为期望的 `native/tests/external_benchmarks/`（9 例）与逐条复述既有单测的 `native/tests/bdd/`＋`native/tests/features/`（7 特性 12 场景）整体删除，`ureq`／`cucumber`／`tokio` 退出 dev-dependencies。前者期望由上游决定（判红只说明上游 rev 变动，实测分歧全落上游与终端惯例），后者零独占覆盖（唯一断言在 `native/src` 单测里已有且更强）。日后复查 MUST NOT 因「用例数变少」「Gherkin 叙述更可读」重建它们 |
| 外部基准与原计划的口径差异 | **登记在案**（编号集 `30`）：优先级 1 原写「进 BDD」，改为 Rust 集成测试目标（接缝相同，逐条代码化期望塞进 Gherkin 只多一层无收益的间接）；优先级 2 的「采样」二字作废（可打印码位全量比对）；优先级 3 改为「规范表当真值 + 分歧登记」（字节断言基本落在上游，本仓判据是增强请求是否到达编码器）；优先级 4／5 见上两行。三项落地后整体撤回，见「外部基准与 BDD 套件的退场」行 |

#### Scenario: 未查清项不被写成已解决

- **WHEN** 复查上表任一条目
- **THEN** MUST 读到它是「未查清」「覆盖缺口」还是「已核销」，MUST NOT 出现
      「无反馈闭环却声称已修」的表述

#### Scenario: 口径更正的旧结论被废弃

- **WHEN** 读到 CI 稳定性或 debug 档耗时的旧口径
- **THEN** MUST 以更正后的归因为准（一次一个确定根因、debug 结论不外推 release）

### Requirement: 研究报告逐份处置台账

`docs/research` 两份文档集（编号集 `00`–`30` 中 `26` 缺失、另含非编号的生态汇总报告，
共 31 份；现行报告 74 份；合计 105 份）
MUST 按该变更目录（归档后为 `openspec/changes/archive/*-research-per-doc-resolution/`）
`tasks.md` 的固定顺序**逐份**处置：30 份编号报告（`00`–`30`，其中 `26` 缺号）→
编号集内那份非编号的生态汇总报告 → 74 份现行报告（`LC_ALL=C` 目录序）。
每一份的处置 MUST 由四步构成并留下可核对的证据：读全该份、把该份每个可执行句判入
五类去向（已实现／已裁定采纳／已裁定不采纳并写明理由／`PROHIBITED`／未声明不立项）、
把缺失的去向补进 `research-ledger` 或代码、删除该份文件。
单份文档的删除 MUST 与其结论落点同批提交；MUST NOT 一次删除两份及以上报告。
指出本仓代码时 MUST 用可 `grep` 复核的符号名，MUST NOT 以行号定位。

#### Scenario: 单份处置可脱离台账总体叙述独立复核

- **WHEN** 取任一份曾被处置的报告文件名
- **THEN** `git log` 内能定位到该份的删除提交，且该提交或其所引用的规范行能把该份的
      每个可执行句的去向读全，MUST NOT 需要读台账的总体叙述来补全

#### Scenario: 结论无去向时不许删除

- **WHEN** 某份报告的某个可执行句无法判入五类去向
- **THEN** MUST 先补 `research-ledger` 台账条目或落地代码，再删除该份；MUST NOT 以
      「其余各份已核对」或群体概括代替该句的去向

#### Scenario: 逐份删除不互相牵连

- **WHEN** 复核任一删除提交
- **THEN** 该提交内 MUST 只含一份报告的删除，其它报告的删除 MUST 在各自提交内，
      MUST NOT 出现一次提交删除多份的批量处置

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
