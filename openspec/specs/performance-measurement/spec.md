# performance-measurement Specification

## Purpose

本仓的性能结论来自两条互不替代的路径：CI 内的微基准与测试（`cargo bench --quick`、
Rust 用例、JVM 用例）与真机上的帧时序探针（渲染循环的 avg/p95/max 报告、慢帧分段
诊断）。两者的能力边界必须写明：合成负载可以证明回归，但不能替代物理设备结论。
本能力记录判据口径、环境可比性要求、负载口径，以及已评估后不采纳的测量手段。

## Requirements

### Requirement: 真机性能结论的判据与环境口径

真机性能结论 MUST 以 p95 / p99 / missed-deadline 百分比表达，MUST NOT 以中位数或
单次帧时长下结论。两个 revision 的对比 MUST 同设备、同刷新率、同字体、同尺寸，
并交替顺序执行；每次测量 MUST 记录设备、构建与热状态。模拟器数据 MUST NOT 用于
建立物理设备性能结论，只能用于数量级回归。

#### Scenario: 同一 harness 对比两个 revision

- **WHEN** 在同一台真机上交替运行两个 revision 的同一测量
- **THEN** 两者同刷新率、同字体、同网格尺寸，且每次运行的设备/构建/热状态被记录

#### Scenario: 模拟器数据只作数量级参考

- **WHEN** 在软件 Vulkan 模拟器上测得帧率
- **THEN** 该结果 MUST NOT 作为物理设备结论，只能用于发现数量级回归

#### Scenario: 帧时序报告保留分布而非均值

- **WHEN** 渲染循环结算一个计时窗口
- **THEN** 报告包含 avg、p95、max 与 fps，慢帧诊断按 500ms 限频输出

### Requirement: 基准负载口径固定为合成负载

`cargo bench` 的基准负载 MUST 为可复现的合成负载，MUST NOT 引入需现场采集的
真机 PTY 语料——语料进仓库违反「不得 vendor 可自动获取的测试数据」，现场采集则
每次对比的设备与时机都不同，丧失可比性。键入路径的正确性由 Rust / JVM 用例承担，
基准只测同一路径的性能。

#### Scenario: 基准不依赖设备侧资源

- **WHEN** 在任意开发机上运行 `cargo bench --bench cell_builder --bench vt_typing -- --quick`
- **THEN** 基准只读仓库内构造的负载，不需要连接设备或外部语料

#### Scenario: 突发输出有无损用例兜底

- **WHEN** 评估是否需要真实 PTY 重放负载
- **THEN** 以「键入路径已被用例覆盖 + 突发无损用例在位」为准，不新增采集型负载

### Requirement: CI 下限断言先预热再采样

CI 性能门禁的下限断言（`native/benches/support.rs` 的 `Floor::assert_within`）MUST 在
正式采样前先执行一次被测操作并**不计入**统计：被测路径的首次调用包含一次性初始化
（字形光栅化与图集上传、查询缓存填充），其成本比稳态高两个数量级，摊进平均会把
「初始化成本」误报成「路径退化」。阈值 MUST 相对**预热后**的稳态设定；不设预热的
断言在慢机上永远误报，等于没有门禁。上限 MUST 按 CI 实测均值校准（每轮采样后打印
`[floor]` 实测行，取其实测的约 1.5 倍），与 `docs/specification/TESTING.md` 的
「基准需要和实际测试结果高度相近以发现之后的性能降低」一致。

#### Scenario: 首调成本不判红

- **WHEN** CI runner 上首次运行 `build_instances_from_cell_data` 下限断言
- **THEN** 初始化被预热吸收，采样均值反映稳态转换成本，不因首调光栅化失败

#### Scenario: 数量级退化仍然判红

- **WHEN** 转换路径被改成逐格重建字体或 O(n²) 查表
- **THEN** 稳态均值超出上限，断言失败并报出实测均值与采样数

下限断言的阈值 MUST 按**编译档位**缩放：debug 档位不优化，同一操作比 `bench` 档位
（继承 release，含 LTO）慢约一个数量级，用优化档位的阈值去判 debug 档位会把编译档位
差异误报成性能退化。缩放 MUST 只作用于 debug 档位，优化档位（`cargo bench`）的阈值
MUST NOT 缩放——真正的性能门禁在优化档位。

#### Scenario: debug 档位不误报为性能退化

- **WHEN** 在 debug 档位运行 `cargo test --all-targets`（基准目标被一并执行）
- **THEN** 下限断言的阈值按 debug 档位缩放，基准不因档位差异判红

#### Scenario: 优化档位的门禁不被放宽

- **WHEN** 在优化档位运行 `cargo bench`
- **THEN** 下限断言的阈值与 CI 校准值一致，不因档位缩放而放宽

增量路径 MUST 有独立的基准：生产上每来一次输出就走「脏掩码 + 行级实例缓存」这条路，
只测全量构建会漏掉「增量帧退化成整屏遍历」这类固定开销（基准
`build_instances_cached_one_dirty_row`）。

#### Scenario: 增量帧退化被用例钉住

- **WHEN** 增量帧的预热范围退化成整屏遍历
- **THEN** 用例 `incremental_frame_warms_only_dirty_rows` MUST 因预热访问的单元数超过
      脏行单元数而失败——下限断言留了两个数量级余量，捕不到这种量级的退化

### Requirement: 不采纳的测量手段须记录理由

已评估后决定不采纳的测量手段 MUST 在本规范中记录理由，使后续复查不必重新调研。

- atrace counter 与 FrameTimeline 关联（input-to-present 诊断）：需要输入侧计数器、
  帧侧计数器与关联脚本三件套且只能在真机运行；帧与循环计时报告已能定位帧内慢段，
  作为真机排障可选项记录，不入规。
- 用测试进程内的定长 CPU 循环作宿主速度参照来归一化启动耗时：实测宿主超额占用时
  该参照只慢 28%–70%（21ms → 28–37ms）且同一轮次内自身抖动近一倍，而冷启动中位数
  慢了 78%–84%（2011ms → 3468–3694ms）——启动是多线程且含 I/O 的延迟敏感过程，参照
  是单线程吞吐敏感的，二者不成比例，按参照折算会把宿主慢误判成应用慢。
- 跨应用性能对比的测量手段取舍 MUST 另立一条要求记录，不并入本条。

#### Scenario: 复查不采纳项

- **WHEN** 再次评估「是否需要 input-to-present 关联诊断」
- **THEN** 以本条目给出的边界（现有探针已定位帧内慢段）为起点，MUST NOT 从零开始调研

### Requirement: 性能改动的取舍须留档（含否决项）

已评估过的性能改动 MUST 留下「基线 → 结果 → 取舍」的记录，使后续复查不必把同一个
实验重做一遍：采纳的改动记下实测数字；否决的改动 MUST 也记下否决依据（实测无收益 /
收益在噪声内 / 复杂度不划算），否则死想法会被反复提出。基线与台账见下两条。

#### Scenario: 复查不采纳项

- **WHEN** 再次评估一个已留档的性能改动
- **THEN** MUST 以台账给出的实测数字为起点，MUST NOT 从零开始重做同一实验

### Requirement: 原生渲染路径的实测基线

原生端到端 CPU 管线（终端写入 + CellData 抽取 + 实例构建）在 4 核宿主 release、
24×80 网格下实测约 0.20–0.31ms/帧（约 3200–4900 fps）；整屏实例构建约 96µs，
增量帧（一行脏）约 43µs。旧记录 0.1ms/帧（6019 fps）来自不同宿主与内容，
不再作为当前基线。故原生 CPU 路径不是帧预算的瓶颈：模拟器上的帧时长由软件光栅化
主导，真实设备上由 GPU 主导。这条基线 MUST 作为「不再优化原生 CPU 路径」的判据依据。
基线数字随机器负载浮动：整屏 `[floor]` 复算落在 0.096–0.102ms，增量帧落在
0.033–0.043ms；复算值低于上界不构成回归，判红只认「慢于基线上界」。

#### Scenario: 原生 CPU 路径不再被当作瓶颈

- **WHEN** 出现新的帧时长劣化报告
- **THEN** MUST 先用端到端 CPU 管线基线（当前亚毫秒级，约 0.2–0.3ms/帧）区分「原生 CPU 路径退化」与
      「GPU／光栅化侧劣化」，MUST NOT 直接改原生热路径

### Requirement: 已评估性能改动的取舍台账

取舍台账 MUST 保持完整，逐项记录采纳与否决的决定及其实测依据（实测数字见上条），
MUST NOT 只留结论而丢掉数字。十六条条目的具体内容由下方场景钉住。

#### Scenario: 台账条目逐项列全

- **WHEN** 复查性能改动的取舍台账
- **THEN** 台账 MUST 同时列出十六条：① 采纳 release profile `opt-level = 3`（同机实测键入
  92µs，体积取向 `opt-level = "z"` 为 357µs；单元格实例构建 129µs → 267µs；CJK
  解析 232ns → 510ns，见 build-and-validation）；② 采纳预热遍按（码位, 粗, 斜）去重
  增量帧只预热脏行（整屏 129µs → 98µs，增量帧 72µs → 32.5µs）；③ 否决帧发布去
  克隆（邮箱改 `Arc`）——**该否决已被第⑨项推翻**：当初的估算「每帧克隆 184KB 需
  1.6µs」只计了拷贝本身，漏掉了新分配一块 184KB 缓冲的分配与缺页成本；实测两次
  （帧邮箱克隆 + 行缓存拷贝）整帧拷贝合计约 13.6µs/帧，占原生帧路径约 19%，不再是
  噪声级。教训：拷贝类收益的估算须按「分配 + 拷贝」合计计，不得只量 memcpy；④ 否决
  滚动感知的实例缓存平移——未实施，收益上限是滚动帧整屏重建的 98µs（16ms 帧预算的
  0.6%），代价却是把滚动位移从 VT 层传到渲染层并重写缓存行的四边形原点；⑤ 采纳
  抽取路径的逐单元缓冲复用（组合记号走 `cell.graphemes_buf` 写调用方缓冲，行内单元缓冲
  按帧复用）——整屏改写帧的堆分配由 1029 次/帧降至 26 次/帧（-97.5%），分配字节由
  564KB/帧降至 384KB/帧（该组数字取自「内容不变」窗口：只走抽取与逐字节去重比较，
  帧不外发）；内容逐帧变化时另加帧邮箱克隆的一份整帧拷贝，实测 30 次/帧、
  569KB/帧（3.1 份整帧缓冲，与第③项的「每帧克隆 184KB」同源；该克隆已由第⑨项
  去掉，两窗口现均 26 次/帧、1.1 份），分配预算护栏
  （`native/tests/alloc_budget.rs`）两个窗口都覆盖，字节预算按整帧份数表达而非写死
  字节数；抽取路径 CPU 由约 685µs/帧降至约 650µs/帧（共享宿主上
  噪声约 ±10%，分配次数为确定性判据）；分配来源是上游 `graphemes()` 的逐单元
  `Vec<char>`，其 `OutOfSpace` 保留前缀语义与旧的「取前 7 个附加码位」一致；⑥ 否决
  行缓存与上一帧比较缓冲的合并或回收通道——行缓存整帧拷贝与帧邮箱克隆同量级，
  与第③项同源；该否决同样被第⑨项推翻（当时的 1.6µs 估算只计了 memcpy，见第③项）；
  ⑦ 采纳实例构建路径的组合簇
  零分配——整形缓存键的文本改为内联 `ShapeText`（簇文本上限 32 字节，键查找不再
  `text.to_string()`）、缓存值改为共享切片 `Arc<[ShapedGlyphInfo]>`（命中只增引用计数，
  不再复制 `Vec`）、簇文本写入跨单元复用缓冲——24×80 网格中八分之一单元为组合簇时，
  整屏实例构建的堆分配由 730 次/帧降至 8 次/帧（分配字节 15451 → 999 字节/帧），
  无组合簇的基线为 6 次/帧，逐簇三次分配一旦回归即以每帧约 720 次的量级判红，护栏见
  `native/tests/instance_alloc_budget.rs`（每帧 100 次、 64KB 双预算）；⑧ 否决整屏抽取的
  逐单元样式缓存——按内容形态实测（24×80，每帧全量改写，含 VT 写入、抽取与帧邮箱
  克隆）：纯 ASCII 92µs/帧、逐行换色 102µs/帧、逐单元 SGR 296µs/帧、相邻单元交替两种
  颜色 383µs/帧、逐单元组合记号 368µs/帧；逐单元成本来自上游 `libghostty-vt` 的逐单元
  取值往返（第⑪项后为每单元四次：`style_id`/`codepoint`/`wide`/`content_tag`；此前另含
  `next`/`raw_cell`/`graphemes_buf`），而上游每条 SGR 序列分配一个新样式 id，故按样式 id
  缓存对「逐单元 SGR」无效，按样式内容缓存又必须先读出样式（即那次昂贵的调用）——真实
  输出按样式段而非逐单元发 SGR，实测逐行换色仅 102µs/帧，缓存扩容的收益只落在病态输入
  上，不值得引入逐单元缓存；⑨ 采纳帧
  网格的单一所有权（`Arc` 共享）——VT 线程构建的帧网格由行缓存（下一帧复用干净行）、
  去重基线（逐字节比较）与帧邮箱/渲染侧共用同一份，不再各存一份整网格拷贝：整网格
  184KB（24×80），每帧去掉两次「分配 + 拷贝」。确定性判据（分配预算护栏
  `native/tests/alloc_budget.rs`）：整帧缓冲份数由 2.1 份（内容不变窗口）与 3.1 份
  （内容变化窗口）降到两窗口均 1.1 份，分配字节由 384KB/帧与 569KB/帧降到 200KB/帧。
  CPU 判据（`vt_typing` 基准，同机交替 A/B 四对）：基线 64.1/70.3/71.8/69.6µs，
  改造后 55.5/57.2/56.2/56.4µs，中位数 69.9µs → 56.3µs（约 -19%）；另一次独立复算
  为 58.1µs → 47.6µs（约 -18%），两次比值一致。回归钉：帧内存身份比对（`Arc` 地址）
  覆盖「发布到渲染侧不复制」与「行缓存不复制」两条路径，重新引入拷贝时两条均判红。
  行缓存交出内容后需先放回一份空网格，一并改为共享同一份不可变空网格占位，
  每帧再去掉一次空 `Arc` 的堆分配（两窗口均 27 次/帧 → 26 次/帧）。
  代价：帧类型由 `Vec<CellData>` 改为 `Arc<Vec<CellData>>`，波及 7 个文件的签名
  （帧邮箱、行缓存、去重基线、公开取帧 API、渲染侧帧状态及两处测试辅助）；⑩ 否决实例
  缓存的整屏回写消除——增量帧构建完把整屏实例（1920 × 92B = 176,640 字节）从构建
  缓冲复制回缓存，实测该整屏拷贝 2.24–2.38µs/次（热缓冲）；另一处「干净行从缓存
  复制到构建缓冲」同量级（2.1µs），两处合计约 4.4µs/帧，而增量帧构建全程约 26µs
  （实测 25–28µs）、原生端到端 0.2–0.31ms/帧——消除回写只值 2.3µs/帧，占 16.7ms
  帧预算的 0.014%，却要改 `CachedInstances` 与 GPU 上传路径（上传源从构建缓冲改为
  双缓冲的当前帧），弄错就是画面错乱；与第④项同类，收益比其上限（98µs）还小两个
  数量级仍被否决，故同样否决；⑪ 采纳整行批量取单元与按需取组合簇——抽取路径的逐单元
  取值往返由每单元六次（`next` + `raw_cell` + `style_id` + `codepoint` + `wide` +
  `graphemes_buf`）降到每单元四次（`style_id` + `codepoint` + `wide` + `content_tag`）：
  整行原始单元改由上游 `cells_raw` 一次取齐（每行一次），单元迭代器只在样式解析与组合簇
  附加码位处按需定位到目标列。实测（同进程交替 A/B，24×80，每轮一整屏，300 轮，release）：
  现状每单元六次取值中位 36.8µs/帧（19.1ns/单元）→ 批量取行 + 四次单取值 + 按需取簇中位
  23.5µs/帧（12.3ns/单元，-36%）；端到端公开 API 复算（每帧全量改写，`vt_write` →
  `flush` → 取帧，含命令往返，三轮交替采样一致）：24×80 的 p10 由 37.7µs 降到 32.4µs
  （-14%；min 34.2µs → 27.8µs，-19%），100×200（20000 单元）的 min/p10/中位由
  330.5/340.4/355.3µs 降到 259.3/270.5/280.0µs（-21.5%/-20.5%/-21.2%）。按需取簇的
  等价性依据：上游 `row_cells_get` 的 `graphemes_buf` 对非组合簇成员的单元只回基础码位
  本身（`cell.hasGrapheme()` 为假时附加码位为空），故按 `content_tag` 判据跳过这次取值
  与输出完全等价，组合簇单元仍走原路径。回归钉：同一行内相邻不同样式必须各取自己的颜色
  （`sgr_run_boundary_in_one_row_reads_the_right_cell`）、宽字符 spacer 之后的样式仍须
  落在本格（`sgr_run_boundary_after_a_wide_char_reads_the_right_cell`）、组合簇附加码位
  不得外溢到后一格（`a_plain_cell_after_a_combining_cluster_has_no_grapheme_extra`）——
  三条均已在「定位恒取第 0 列」「不复位附加码位」两种变异下判红；⑫ 否决逐单元取值改用
  上游 `get_multi` 批量取值——上游 `ghostty_cell_get_multi` 在 Zig 侧只是循环调用
  `ghostty_cell_get`（同进程内同代价），Rust 侧却要多组装两组栈数组并多一次指针间接写：
  实测（同进程交替 A/B，24×80，300 轮，release）三次单取值中位 36.8µs/帧 →
  `get_multi` 三键 39.9µs/帧（+14.8%）；把 `content_tag` 也并入批量后 `get_multi` 四键
  27.4µs/帧，仍比同样键数的四次单取值（23.5µs/帧）慢 17%。上游文档亦写明「同进程内与
  逐键调用同价，按可读性择一」，故保留逐键单取值；⑬ 采纳剪贴板读应答的写回线程
  合并——原实现为每条 OSC 52 读请求 spawn 一个应答线程，线程创建实测 24.6–26.5µs/次
  （release，同机）；单会话每帧的未作答上限为 8，故洪泛时每帧要付 198–212µs 渲染
  线程时间，且每秒新建 480 个线程。改为进程内唯一的应答写线程消费应答队列后，同一
  路径（8 条读请求的登记、事件产出与槽位登记）实测 1.24µs/帧，线程数与请求数脱钩。
  代价：写回由「每请求一线程并行」变为「单线程串行」，故单条写回仍以
  `pty::WRITE_BACK_TIMEOUT` 为界（子进程不读 stdin 时最多等满该期限）；槽位的超期
  清理由写线程按 250ms 节拍清扫承担（原为每请求线程各自的 2s 等待；清扫与队列
   是否空闲无关——宿主只答部分请求时队列常非空，只在空闲时清扫会让其余槽位永不
   超期），远端等的是应
  答本身而非槽位消失的时刻；⑭ 否决轮询路径的改造——`pump_sessions` 的逐帧开销与
  逐事件 JSON 往返实测都在亚微秒量级，不构成帧预算的一部分（release，同机，
  20000 轮）：单活跃会话 `pump_sessions` 232.7–236.7ns/帧，1 活跃 + 4 后台会话
  1047.9–1061.0ns/帧；`serde_json::to_string`（Bell 事件）28.8–38.3ns/次，一帧排空
  32 个事件（含推进、取事件与序列化）1059.4–1131.0ns。逐帧分配方面，稳态下只有
  收集会话句柄的那个 `Vec` 真正分配（其余收集 `Vec` 为空、不分配），即每帧一次
  malloc/free，代价在百纳秒量级。故不改轮询路径：事件逐条经 JNI 传输与逐事件 JSON
  往返的收益上限（一帧 32 事件时原生侧约 1µs）远低于改动 JNI 契约与 Kotlin 解码
  路径的复杂度；⑮ 采纳逐格搜索高亮查表按行免判——无高亮是常见情形，而发射遍原本
  每格都要按行号哈希查一次 `highlights_by_row`，整屏重建即 1920 次哈希。改为行级
  一判空表后，实测（release，同机，24×80，4000 轮，中位）：整屏重建 73.12µs →
  71.44µs（-2.3%），全空白屏 28.68µs → 28.40µs（-1.0%）。同轮把整屏重建成本归因
  完毕：全空白屏（逐格骨架 + 空四边形发射 + 逐调用准备）28.4µs、预热遍 10.2µs
  （临时跳过预热实测 71.6µs → 61.4µs，占 14%）、其余约 33µs 为非空格的逐格字形
  查表与实例发射。整屏重建只在图集代际变化或网格尺寸变化时发生，稳态走增量路径
  （单脏行重建实测 7.5µs 量级），故不再对这条路径做进一步优化；⑯ 否决 OSC 52 扫描器的
  「按段批量拷贝」改造——扫描器逐字节过一遍 PTY 输出，代价实测 1.184ns/字节（release，
  同机，1MiB 纯文本，30 轮中位），约为上游 VT 解析（1.970ns/字节）的 60%。但 PTY 读取块
  为 8KiB，渲染线程每帧最多收割 2 块（`PTY_POLL_CHUNKS_PER_FRAME`），即每帧扫描 16KiB
  约 19µs，占 16ms 帧预算的 0.12%。改写为「memchr 找 ESC → 整段 memcpy」可把扫描降到
  0.2ns/字节量级（`memchr` 2.8.3 已在依赖树内），收益上限是每帧约 15µs，而代价是重写
  一条安全敏感的状态机（OSC 52 读取请求的拦截路径，误判即让远端读到或读不到剪贴板）与
  新增一个直接依赖。故不改，保持逐字节状态机

#### Scenario: 否决项不得被反复重提

- **WHEN** 再次评估「是否要把帧邮箱改成 `Arc` 以免每帧克隆」、「是否把逐单元取值改用
  上游 `get_multi` 批量取值」或台账中其余否决项
- **THEN** MUST 以本条目给出的实测数字为起点，MUST NOT 重新实现同一实验；台账中的否决
      只有在给出「原数字被证伪」的新证据时才可重开（如第⑨项：原估算只量 memcpy、
      漏了分配与缺页成本），且重开时 MUST 同时更新原条目与新增条目，MUST NOT 只留
      新结论而让原条目继续写着已失效的否决依据

#### Scenario: 实例构建路径的分配回归可被判定

- **WHEN** 实例构建重新引入逐簇堆分配（拼簇文本的 `String`、缓存键文本的 `String`、
  命中时克隆的 `Vec`）
- **THEN** 含组合簇的整屏实例构建分配次数 MUST 回到数百次/帧，与台账第⑦项给出的
      8 次/帧形成可判别的差距；护栏 MUST 按每帧分配次数与字节双预算判定

#### Scenario: 抽取路径的分配回归可被判定

- **WHEN** 抽取路径重新引入逐单元堆分配（如上游 `graphemes()`），或帧发布/行缓存重新
      引入整帧拷贝
- **THEN** 整屏改写帧的分配次数 MUST 回到千次量级（当前两窗口均 26 次/帧）；
      整帧缓冲份数 MUST 回到台账第⑨项推翻前的 2.1 份（内容不变窗口）与 3.1 份
      （内容变化窗口），当前两窗口均 1.1 份；分配字节 MUST NOT 超过每帧两份整帧缓冲
      （护栏按整帧份数表达，不写死字节数）

### Requirement: 跨应用性能对比的宏基准与 HWUI 帧面类测量手段取舍须记录理由

跨应用性能对比的宏基准与 HWUI 帧面类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- 宏基准的 `FrameTimingMetric` 与 `StartupTimingMetric` 作跨应用主判据：官方明确不推荐在
  模拟器上跑；目标应用须非 debuggable；两者量的是各目标自身管线的帧，跨应用语义不同；
  既有规约已把它定位为「模拟器数量级回归」用途。
- `dumpsys gfxinfo` 的帧数作终端内容帧判据：该入口只覆盖经 ViewRootImpl/RenderThread
  绘制的帧，SurfaceView 直绘不在其中。
- `screenrecord` 作判据：编码与分辨率引入偏差，只作人工复核制品。
- 引用对照应用的自述性能数字：对照应用既无基准模块又无可核实的官方性能数字时不得引用。

#### Scenario: 复查跨应用判据类不采纳项

- **WHEN** 再次评估「能否用某家自报的帧率或宏基准指标作跨应用结论」
- **THEN** MUST 以本条目给出的边界为起点：自报帧率与宏基准指标不作跨应用主判据，
      跨应用帧面 MUST 取系统侧帧面

### Requirement: 跨应用性能对比的系统侧帧面类测量手段取舍须记录理由

跨应用性能对比的系统侧帧面类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- Perfetto `android.surfaceflinger.frametimeline` 作终端内容帧的跨应用判据：官方文档明确
  SurfaceView 不受 FrameTimeline 支持；本项目终端内容在 SurfaceView 内不在其覆盖内，对照对象
  走 HWUI 管线在其覆盖内，两者不是同一数据面。
- Perfetto FrameTimeline 的 SurfaceFlinger 侧轨道作终端内容判据：该轨道的切片是 display frame
  级合成轨迹，没有图层归属，无法归属到某个应用的终端内容层。

#### Scenario: 复查系统侧帧面类不采纳项

- **WHEN** 再次评估「能否用 Perfetto FrameTimeline 作终端内容判据」
- **THEN** MUST 以本条目给出的边界为起点：终端内容帧 MUST 取系统侧逐图层通道，
      MUST NOT 用 Perfetto FrameTimeline 覆盖终端内容

### Requirement: 跨应用性能对比的 trace 与能耗类测量手段取舍须记录理由

跨应用性能对比的 trace 与能耗类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- Perfetto `android.log` 数据源：仅 userdebug 构建可用，正式版真机改用 logcat 全量落盘。
- `gpu.counters` 跨设备对比：counter ID 由设备描述符给定，跨设备不可比。
- `PowerMetric` 跨设备对比：依赖具体硬件与充电状态。

#### Scenario: 复查 trace 与能耗类不采纳项

- **WHEN** 再次评估「能否用 Perfetto 日志数据源或 GPU 计数器作跨设备对比」
- **THEN** MUST 以本条目给出的边界为起点：正式版真机用 logcat 全量落盘替代
      `android.log`，GPU 计数器与功耗指标只在同设备内可比

### Requirement: Android 宏基准在模拟器上运行并断言

Android 宏基准（`android/benchmark`）MUST 在模拟器上以连接测试方式运行
（`:benchmark:connectedBenchmarkReleaseAndroidTest`），MUST NOT 只写在文档里而不真跑。

每个用例 MUST 至少断言两件事：交互窗口内**至少出一帧**（帧数为 0 说明被测路径没有
产生任何绘制，测量无意义）、操作**总耗时在预算内**（拦住「慢到不可用」级回归）。
滚动、按键等状态类交互还 MUST 断言功能状态真的发生（粘滞态置起、抽屉节点出现），
MUST NOT 用「没报错」代替「发生了」。

#### Scenario: 滚动不产生帧判红

- **WHEN** 回滚区填满后滚动手势没有产生任何帧
- **THEN** 用例失败并报出呈现帧数为 0，而不是留下一个测了个寂寞的空窗口

#### Scenario: 启动超预算判红

- **WHEN** 冷启动到终端可用的各轮耗时中位数超过预算
- **THEN** 用例失败并报出各轮实测耗时、中位数与预算值

### Requirement: 宏基准的阈值校准与日志等级

阈值校准与回归判定 MUST 以执行环境里模拟器的实测为准（当前执行环境为本机 AVD，
API 35 / SwiftShader），MUST NOT 因某条流水线缺席而停摆或降低覆盖。

宏基准 MUST 以启动 extra 把被测进程的日志等级提到 DEBUG：原生呈现汇总是 debug 级，
而 release 构建的缺省等级是 WARN——等级不够时基准永远读不到帧，只能以「没出帧」
判红，测的是日志等级而不是性能。extra MUST NOT 写回用户的日志等级设置。

模拟器宏基准数据 MUST NOT 用于建立物理设备性能结论，只用于数量级回归与功能级
断言（本能力「真机性能结论的判据与环境口径」不变）。

#### Scenario: 日志等级不够则读不到帧

- **WHEN** 被测进程以 release 构建的缺省日志等级启动
- **THEN** 基准读不到 debug 级的呈现汇总，判红的是日志等级而不是性能

#### Scenario: 系统侧拉起的日志等级同样要传

- **WHEN** 场景以系统侧 `am start` 拉起被测应用
- **THEN** 日志等级 extra MUST 经 `--es` 传入，MUST NOT 只对宏基准的启动 Intent 生效

#### Scenario: 流水线缺席不停摆

- **WHEN** 某条流水线缺席宏基准的执行环境
- **THEN** 阈值仍以执行环境里的模拟器实测为准，不停摆也不降低覆盖

### Requirement: 宏基准的启动耗时判定与宿主负载口径

宏基准的启动耗时 MUST 按各轮中位数对预算、最差一轮对硬上限判定：预算是按实测中位数
校准的，而宿主被并发任务抢占时整轮都会变慢（实测同一构建在超额占用的宿主上冷启动
十轮中位数从 2.0s 涨到 3.5s），拿最差一轮当判据会把宿主抖动算成应用回归。预算 MUST
留出覆盖该情形的余量，MUST NOT 因宿主抖动判红。

启动耗时 MUST 分两段记录：首帧（`startActivityAndWait` 走 `am start -W` 的系统侧口径）
与终端界面可见（UiAutomator 检测口径）。分两段才分得清「应用慢」与「检测慢」——实测
宿主超额占用时首帧段从约 1.0s 涨到约 2.9s，检测段基本不变。

#### Scenario: 宿主被抢占时启动判定不误报

- **WHEN** 宿主被并发任务抢占导致各轮启动耗时整体变慢
- **THEN** 用例 MUST 以中位数对预算判定并通过，MUST NOT 因宿主抖动判红

#### Scenario: 启动耗时分两段记录

- **WHEN** 宏基准记录启动耗时
- **THEN** MUST 同时给出首帧段与终端界面可见段的实测值，MUST NOT 只给合并后的总数

### Requirement: 热启动的系统侧口径与内容层取证

热启动 MUST 用系统侧 `am start -W` 取切前台耗时，并校验启动态是 `HOT`；MUST NOT 用宏基准
自带的启动等待的墙钟耗时作热启动耗时——热启动不新建 Activity，系统报「Activity not started」
时该等待退化成固定两秒（实测十一轮每次都睡满两秒，测得中位 2838ms，而同一次启动的系统侧
总耗时只有 113ms），量到的是固定等待而不是应用。

热启动期间终端内容层的 Surface 随界面回到前台重新创建：内容层帧 MUST 取启动之后拿到的图层，
MUST NOT 用窗口开始前的图层名——按 home 之后内容层图层已从图层清单消失，旧图层名在重建后的
图层上也不再命中。

#### Scenario: 热启动按启动态判红

- **WHEN** 热启动场景测得的启动态不是 `HOT`
- **THEN** 该次测量 MUST 判红，MUST NOT 把它记成一次热启动

#### Scenario: 热启动不用固定等待当耗时

- **WHEN** 热启动场景取切前台耗时
- **THEN** MUST 取系统侧 `am start -W` 的总耗时，MUST NOT 取框架启动等待的墙钟耗时

#### Scenario: 热启动的内容帧从重建后的图层取

- **WHEN** 热启动场景读终端内容层的帧
- **THEN** MUST 取启动之后重建的图层，MUST NOT 用窗口开始前的图层名

### Requirement: 空载场景的判据面

空载场景 MUST 同时取三个互相独立的面判定「没有持续绘制」：系统侧逐图层的终端内容帧、
应用进程的 `gfxinfo` 帧数、被测进程的 CPU 时间（`/proc/<pid>/stat`）；MUST NOT 只用
`gfxinfo` 帧数——终端内容直绘 SurfaceView，`gfxinfo` 记不到那些帧，只看它会把「终端内容
还在刷」误判成空载。

空载场景 MUST 以「终端内容层真的画出了第一帧」为就绪信号，MUST NOT 用固定等待替代就绪
信号；进静置窗口前 MUST 让启动余波散尽，量到的才是稳态。

空载场景 MUST 核对被测应用在前台，MUST NOT 把被系统弹窗盖住时的零帧当成应用的空闲表现；
测量前 MUST 预授权限，避免通知权限弹窗盖住应用。前台核对 MUST 覆盖测量窗口的两端
（进入窗口前与窗口结束后各一次）：只在窗口前核对会漏掉「弹窗在窗口中途抢走前台」，
那正是零帧被误记成空载的形状。

#### Scenario: 空载只看得见主线程帧时判红

- **WHEN** 被测应用在静置窗口内持续绘制终端内容
- **THEN** 该次测量 MUST 判红，MUST NOT 因 `gfxinfo` 帧数为零而判通过

#### Scenario: 空载的进程 CPU 占用超限时判红

- **WHEN** 静置窗口内被测进程的 CPU 时间超过上限
- **THEN** 该次测量 MUST 判红，MUST NOT 只看帧数而放过持续占用

#### Scenario: 应用被弹窗盖住时判红

- **WHEN** 静置窗口前台焦点不在被测应用上
- **THEN** 该次测量 MUST 判红，MUST NOT 把界面停画记成空载

### Requirement: 持续输出场景的负载完成与呈现判据

持续输出场景 MUST 由设备侧 shell 自己产出字节流（固定批数、每批固定行数、批间固定
间隔），MUST NOT 依赖宿主注入速度；每批 MUST 在批末写一条完成标记，批次是否真的跑完
以完成标记计数判定，MUST NOT 用「呈现帧数不为零」代替。

判据 MUST 同时取三个互相独立的面：呈现汇总的窗口数与最长停摆、系统侧逐图层的终端内容
层呈现间隔 p95 与刷新槽位占用、被测进程的 CPU 时间与 PSS 增长；显示级帧时间线的画面
MUST 按属主归属到被测应用并与逐图层通道的呈现时刻交叉校验。

内存峰值 MUST 在负载进行中采样，MUST NOT 在负载结束后补采。

测量前 MUST 预授权限：通知权限弹窗会盖住应用，被盖住时输入法不弹出、终端拿不到输入
焦点，场景 MUST 在输入前置条件上判红，MUST NOT 量到被遮挡的应用。

#### Scenario: 负载未跑完即判红

- **WHEN** 完成标记数少于批数
- **THEN** 该次测量 MUST 判红并强停被测应用，MUST NOT 因呈现帧数不为零而判通过

#### Scenario: 呈现停摆即判红

- **WHEN** 相邻呈现窗口的间隔超过停摆上限
- **THEN** 该次测量 MUST 判红，MUST NOT 只看总呈现帧数

#### Scenario: 两条系统侧通道不一致即判红

- **WHEN** 显示级帧时间线归属到被测应用的终端内容帧与逐图层通道的呈现时刻对不上
- **THEN** 该次测量 MUST 判红

#### Scenario: 内存峰值在负载进行中采

- **WHEN** 负载结束且内存已回落
- **THEN** PSS 峰值 MUST 取负载进行中的采样值，MUST NOT 用负载结束后的读数

#### Scenario: 权限弹窗盖住应用时判红

- **WHEN** 拉起被测应用时系统权限弹窗接管了前台
- **THEN** 该次测量 MUST 在输入前置条件上判红，MUST NOT 把弹窗状态当成终端的行为

### Requirement: 突发输出场景的无损与呈现判据

突发输出场景 MUST 由设备侧 shell 一次产出整段字节流（无节流），MUST NOT 依赖宿主注入
速度；送达字节数 MUST 按行内容加 PTY 把每个换行展开成的 CRLF 计入。

无损 MUST 由两个哨兵证明，MUST NOT 用「呈现帧数不为零」代替：完成标记证明生产者把全部
字节写进了 PTY，流末尾的会话标题哨兵证明终端把整段流解析到了末尾。MUST NOT 以被提示符
覆盖后的标题读数作判据。

判据 MUST 同时取四面：无损、呈现（呈现汇总的窗口数与最长停摆、系统侧逐图层的呈现间隔
p95 与刷新槽位占用、显示级帧时间线按属主归属并与逐图层通道交叉校验）、资源（负载进行中
采的 PSS/RSS 峰值与进程 CPU 时间）、归因（窗口内进程退出即判红）。负载超时 MUST 强停被
测应用，MUST NOT 让失败场景继续耗设备。

#### Scenario: 流被截断即判红

- **WHEN** 流末尾的会话标题哨兵没有出现在会话标题里
- **THEN** 该次测量 MUST 判红，MUST NOT 因呈现帧数不为零而判通过

#### Scenario: 生产者没写完即判红

- **WHEN** 超时内没有收到完成标记
- **THEN** 该次测量 MUST 判红并强停被测应用

#### Scenario: 整段字节流把终端压到不画即判红

- **WHEN** 出帧的呈现窗口数低于下限或呈现最长停摆超过上限
- **THEN** 该次测量 MUST 判红

#### Scenario: 内存峰值在负载进行中采

- **WHEN** 负载结束且内存已回落
- **THEN** PSS 峰值 MUST 取负载进行中的采样值，MUST NOT 用负载结束后的读数

#### Scenario: 回滚区封顶后突发不再涨内存

- **WHEN** 突发行数远超回滚区上限
- **THEN** 内存增长 MUST 在上限内，MUST NOT 随突发字节数线性增长

### Requirement: 宿主负载与进程 CPU 时间的记录口径

每轮测量 MUST 记录宿主负载与被测进程的 CPU 时间。宿主负载取 `/proc/stat` 的 steal 计数占
CPU 总时间的比例（设备读不到宿主负载，但读得到「自己没拿到多少 CPU」）；进程 CPU 时间取
`/proc/<pid>/stat` 的 utime 与 stime 合计，配合 starttime 与 `/proc/uptime` 得进程年龄。
判定 MUST 用负载无关的相对量，MUST NOT 按负载占比折算耗时——steal 记的是全部 vCPU 的被抢
时间，空闲的 vCPU 被抢走并不拖慢被测进程，按占比缩放会把被测进程算得比实际更快。

场景失败时 MUST 把该轮宿主负载附进抛出的失败消息（steal/busy 占比、窗口长度与
contended），原始失败 MUST 保留为 cause：「本轮慢是应用慢还是宿主被抢占」必须在失败
消息里就能归因，不必翻 logcat 的 `[bench-load]` 行或复跑。附加证据 MUST NOT 改变判定
——负载是证据，不是免责。

#### Scenario: 宿主被抢占时两个口径同时可见

- **WHEN** 宿主被并发任务抢占，墙钟耗时整体变慢
- **THEN** 该轮 MUST 同时给出墙钟耗时与进程 CPU 时间，MUST NOT 只给墙钟

#### Scenario: 负载不折算进耗时

- **WHEN** 该轮的 steal 占比高于抢占阈值
- **THEN** 该轮 MUST 标为被抢占并保留原值，MUST NOT 按占比缩放耗时

#### Scenario: 负载采集失败不覆盖原始失败

- **WHEN** 场景本身失败且负载采集同时失败
- **THEN** 原始失败 MUST 保留，负载采集的失败 MUST 作为附加信息附带

#### Scenario: 失败消息自带负载证据

- **WHEN** 场景失败且该轮采到了宿主负载
- **THEN** 抛出的失败消息 MUST 含 steal/busy 占比与窗口长度，原始失败 MUST 作为 cause 保留

### Requirement: 宏基准的帧级数据面按绘制主体选源

帧级数据面 MUST 按「谁画了这帧」选源，MUST NOT 用单一数据面覆盖所有被测路径：

- Compose / HWUI 绘制的界面（修饰键栏、抽屉、输入法动画）→ `dumpsys gfxinfo` 的
  帧数、janky 占比与 p50/p90/p95/p99。
- 终端内容由原生渲染线程直绘 SurfaceView 的 Surface，不经过主线程 `ViewRootImpl`
  遍历，HWUI 根本记不到这些帧 → MUST 取原生渲染器的呈现汇总
  （`render_frame: presented N frames`），MUST NOT 用 gfxinfo 的帧数判「终端没出帧」。

呈现帧数是**宿主吞吐**的函数，不是被测逻辑的函数：同一台模拟器在空闲宿主与被并发
任务抢占的宿主上，同一构建、同一手势的实测帧数可差三倍。故帧数下限 MUST 按**最差
实测**校准，MUST NOT 按空闲宿主的峰值设定；同时 MUST 另设「有帧的呈现窗口数」下界
——该判据与宿主速度无关，拦的是滚动中途停摆。

#### Scenario: 抢占归因经复跑确认

- **WHEN** 宿主被并发构建抢占时（steal 0.55、busy 0.90、窗口 39.9s）连续输出、宽字符
      与滚动回滚区三例失败
- **THEN** 同一构建在宿主空闲时 MUST 全部通过，且失败消息 MUST 自带该轮负载证据

#### Scenario: 终端滚动按原生呈现汇总判帧

- **WHEN** 断言终端滚动的帧数
- **THEN** 帧数取自原生呈现汇总，gfxinfo 只用于 Compose / HWUI 绘制的界面

#### Scenario: 满载宿主不误判为回归

- **WHEN** 宿主被并发任务抢占，滚动实测帧数降到空闲时的约三分之一
- **THEN** 用例仍通过（帧数下限按最差实测校准），且呈现窗口数下界仍成立

#### Scenario: 终端内容帧另有系统侧逐图层证据

- **WHEN** 宏基准断言终端内容帧
- **THEN** 除原生呈现汇总外 MUST 另有系统侧逐图层通道的独立证据（终端内容层有帧记录），
      MUST NOT 只依赖应用自己的计数

### Requirement: 宏基准实测值可观测且阈值按实测校准

宏基准的实测值 MUST 可观测：每轮迭代把耗时与帧统计写进设备 logcat（`println` 进
instrumentation 输出流、`Log.i("Benchmark")` 进 logcat，两条通道各留一份），连接测试
任务按用例把设备 logcat 落进构建产物
（`androidTest-results/connected/<variant>/<device>/logcat-<类名>-<方法名>.txt`），
MUST NOT 让实测值只留在 IDE 可见的 instrumentation bundle 里。

帧级阈值与耗时预算 MUST 以实测值校准（实测的约 1.5 倍口径，与 Rust 微基准
上限断言同口径），并在常量注释里引用实测出处；实测出处取执行环境里的模拟器
（当前为本机 AVD）；MUST NOT 凭空设定，也不得宽到只拦「卡死级」。
`FrameTimingMetric` 的百分位只进 IDE 结果 bundle，模拟器上可脚本采集
的帧级数据按上一条 Requirement 的分工取（gfxinfo 或原生呈现汇总）。

#### Scenario: 实测值落进构建产物

- **WHEN** 在模拟器上运行宏基准
- **THEN** 连接测试产出的用例 logcat 里出现每轮迭代的实测耗时与帧统计
      （`[bench-measure]` 行），断言预算可据此校准

#### Scenario: 断言预算有实测出处

- **WHEN** 给帧级阈值或耗时设断言阈值
- **THEN** 常量注释引用实测值与运行出处，阈值约为实测的 1.5 倍

### Requirement: 宏基准的设备侧采集通道按执行环境的真实语义构造

宏基准通过 `UiDevice.executeShellCommand` 采集设备侧数据。该入口 MUST 按其真实
语义构造命令：`UiAutomation` 以 `Runtime.exec(String)` 执行，**不经 shell**——
命令按空白切词、引号原样传进参数、管道不生效。故：

- 命令 MUST NOT 含引号、管道、重定向等 shell 语法；需要过滤时 MUST 取回整份输出
  后在本地筛选。
- logcat 的 `-e` 过滤词 MUST 是 POSIX ERE 且不含空白（空白用 `.` 代替）：Java 正则
  的 `\d`/`\w` 在 ERE 里不是字符类，设备侧过滤会把真正的记录全滤掉。

MUST NOT 以「命令没报错」当作「采集到了数据」：采集失败 MUST 表现为空结果，并由
断言判红。

#### Scenario: 过滤词按 ERE 构造

- **WHEN** 采集 logcat 里形如 `session 1 first output received` 的记录
- **THEN** 过滤词写作不含空白的 ERE（如 `first.output.received`），设备侧命中该行

#### Scenario: 采集为空即判红

- **WHEN** 采集通道返回空结果（如命令被 shell 语义毁掉）
- **THEN** 用例失败并指出采集为空，MUST NOT 把空结果当成「被测路径没有发生」

### Requirement: 宏基准的输入前置条件必须可判定

宏基准通过真实输入链路（`input text` / `input keyevent`）驱动终端时，MUST 先判定
「终端已经能吃键」再继续：shell 起来前写入的输入会被 tty 初始化冲掉，命令静默
消失，测量到的却是「什么都没发生」。

终端内的 shell 没有提示符可等（实测 `/system/bin/sh` 在 PTY 里首启无任何输出），
故 MUST 用探针命令诱发一次输出作为就绪信号（幂等、可重发），MUST NOT 用固定
等待时长替代。

就绪上限 MUST 覆盖实测的启动耗时：模拟器上 shell 的首个 PTY 输出实测可达 44s
（2026-11-12 真机日志：19:28:55 spawn，19:29:40 首输出），上限过短会把「还没
开口」误判成「shell 不工作」。上限 MUST NOT 用固定等待时长替代就绪信号本身。

#### Scenario: shell 未就绪判红

- **WHEN** 探针在上限内始终没有回显
- **THEN** 用例失败并报出「shell 未就绪」，MUST NOT 继续测量一个空终端

### Requirement: 跨应用性能对比的对照面与统一帧面

跨终端性能对比 MUST 以系统侧可观测面为准，MUST NOT 以各家自报的帧率作跨应用结论。帧时序
MUST 取系统侧帧面，分层与覆盖面见后续两条。对照对象 MUST 取现实可得的同类终端应用，且 MUST 先
核实其是否提供基准资产与可核实的官方性能数字；两者皆无时对比 MUST 自建 harness，MUST NOT 引用
其自述性能数字。

`dumpsys gfxinfo` 的帧统计 MUST 只用于 HWUI 绘制的界面，MUST NOT 作终端内容帧的跨应用判据：
该入口只统计经 ViewRootImpl/RenderThread 绘制的帧，SurfaceView 直绘不经该路径。

#### Scenario: 终端内容帧按系统侧帧面比对

- **WHEN** 同一场景在多个终端应用上采集帧时序
- **THEN** 每个应用的帧时序 MUST 取自系统侧帧面，MUST NOT 取自各家自报帧率，
      MUST NOT 用 `gfxinfo` 帧数作终端内容帧的跨应用判据

#### Scenario: 对照对象无基准资产时不引用自述数字

- **WHEN** 对照应用既无基准模块又无可核实的官方性能数字
- **THEN** 对比 MUST 由自建 harness 产出，MUST NOT 引用该应用的自述性能数字作为结论

#### Scenario: 对照应用与被测应用撞包名

- **WHEN** 对照应用的包名与被测应用相同（实测上游 Termux 与本项目同为 `com.termux`）
- **THEN** 编排 MUST 按应用轮换安装或给对照构建改包名，MUST NOT 让安装动作替换被测应用，
      且 MUST 记录 APK 校验和与安装顺序

### Requirement: 跨应用性能对比的帧面分层与覆盖面

系统侧帧面 MUST 分三层使用，并按各层的覆盖面取数：

- 显示级 `dumpsys SurfaceFlinger --frametimeline -all`（或 `-jank`）：逐 display frame 的期望与
  实际起止时刻、呈现时刻与 jank 分类；每个 display frame 下挂着该帧涉及的逐图层条目，含
  `Owner Pid`、`Layer ID`、`Is Buffer?`、呈现状态与呈现时刻，故可按属主归属到被测应用的内容图层。
  该面不需要预知图层名，对所有应用同一口径。
- 图层级 `dumpsys SurfaceFlinger --latency <layer>`：逐帧 `desired/actual/ready`（纳秒），覆盖
  承载内容帧的图层，含 BLAST 类型的 SurfaceView 内容层。
- 应用内帧统计只作该应用内部的判据，MUST NOT 跨应用比较。

显示级只保留最近的 display frame 窗口，MUST 按场景时间窗筛选，MUST NOT 假设它能被清空。

#### Scenario: 显示级帧时间线按时间窗筛选

- **WHEN** 用 `--frametimeline` 取某一场景的呈现时序
- **THEN** MUST 按场景时间窗筛选帧，MUST NOT 依赖「先清空」——该面没有清空入口；筛选前的时刻
      MUST 先按下一条 Requirement 换算基准

### Requirement: 显示级帧时间线的时刻基准与跨通道一致性

显示级帧时间线的时刻以保留窗口里最旧的一帧为基准（AOSP `FrameTimeline::dumpAll` 取
`mDisplayFrames[0]->getBaseTime()`），窗口滑动时基准前移，故它不是稳定时钟：实测相隔 8 秒的两次
dump，最新一帧的相对时刻都是约 1.05 秒（保留窗口的跨度），而不是 8 秒后。

取场景时间窗前 MUST 把它的相对时刻换算到稳定时钟：逐图层 `--latency` 通道给的是绝对纳秒时刻
（CLOCK_MONOTONIC，与 `elapsedRealtime` 同域），两条通道记录的是同一批物理呈现，按呈现时刻序列
对齐即可换算出基准。换算后 MUST 用同一图层在两条通道里的呈现时刻逐个校验（逐个对得上，容差
1ms）；对不上 MUST 判取数失败，MUST NOT 拿错位的时刻做统计。

#### Scenario: 帧时间线的时刻先换算基准再筛时间窗

- **WHEN** 用 `--frametimeline` 的时刻按场景时间窗筛选帧
- **THEN** MUST 先把相对时刻换算到稳定时钟（与逐图层通道的绝对时刻对齐），MUST NOT 直接拿 dump
      里的相对时刻与场景起止比较

#### Scenario: 两条系统侧通道的呈现时刻必须一致

- **WHEN** 同一图层在显示级帧时间线与逐图层通道里都有呈现时刻
- **THEN** 换算后的时刻 MUST 逐个对得上（容差 1ms），对不上 MUST 判取数失败

### Requirement: 跨应用性能对比的帧面保留窗口与帧数口径

两个系统侧通道都只保留最近一段历史（`--latency` 每图层 127 行、`--frametimeline` 最近 64 个
display frame），MUST 只用它们取时序，MUST NOT 把被保留窗口截断的行数当作场景总帧数。帧数类指标
MUST 取各应用自己的计数口径。

#### Scenario: 保留窗口饱和不作总帧数

- **WHEN** 持续输出使系统侧帧行数达到保留窗口上限（每图层 127 行或 64 个 display frame）
- **THEN** 该次采集 MUST 只用于时序分析，帧数 MUST 取应用自己的计数，MUST NOT 把行数当作场景
      总帧数

### Requirement: 跨应用性能对比的图层名与图层覆盖判据

图层级通道的取数 MUST 有明确的图层名与覆盖判据：

- 图层名 MUST 是 `dumpsys SurfaceFlinger --list` 给出的精确 `<base>#<id>`；Android 15 起
  `--list` 的每行是 `RequestedLayerState{<名字> <键>=<值> ...}`，名字 MUST 从该包装里取出
  （名字本身可能含空格）。
- 只有首行表示图层未匹配；全零行表示图层匹配但没有内容帧记录；两者 MUST 判为采集失败。
- 场景开始前 MUST 先清空该图层的帧历史，否则读到的 MUST NOT 含上一个场景的残留帧。

#### Scenario: 采集前清空图层帧历史

- **WHEN** 一个场景开始前采集图层级时序
- **THEN** MUST 先用 `dumpsys SurfaceFlinger --latency-clear <layer>` 清空该图层的帧历史，
      否则读到的 MUST NOT 含上一个场景的残留帧

#### Scenario: 图层未匹配或无帧记录即判采集失败

- **WHEN** `--latency` 只返回首行、命令无输出，或返回的帧行全部为零
- **THEN** 该次采集 MUST 判为采集失败并中止该场景，MUST NOT 把空结果当作「被测路径没有
      出帧」，MUST NOT 静默跳过该场景

#### Scenario: 内容图层按属主与图层名选取

- **WHEN** 为某个被测应用选取承载终端内容的图层
- **THEN** MUST 取该应用属主下承载内容帧的那条图层——属主可由显示级帧时间线的 `Owner Pid`
      归属，也可由图层名里的应用标识给出（本项目为 BLAST SurfaceView 层，HWUI 对照对象为应用
      窗口层），MUST NOT 以「帧行全为零」判定图层不可寻址

#### Scenario: 父层不带内容帧时取子层

- **WHEN** 同一 SurfaceView 的父层与 BLAST 子层都在图层清单里（实测父层帧行全为零）
- **THEN** MUST 取承载内容帧的那条，MUST NOT 按图层类型（父子、名字前缀）直接认定

### Requirement: 跨应用性能对比的图层名在两类命令通道下的转义判据

图层名含 `[` `]` `(` `)`，两类命令通道的转义判据 MUST 分开：

- 命令经真实 shell（`adb shell`）时名称 MUST 加引号，不加引号会被 shell 当作元字符，
  命令出错且无输出。
- 命令不经 shell（`UiAutomation` 走 `Runtime.exec(String)`，按空白切词）时 MUST NOT 加
  引号——引号会原样进入参数使图层名匹配不上，且此时图层名 MUST NOT 含空白。

#### Scenario: 经真实 shell 的图层名加引号

- **WHEN** 用 `adb shell` 采集图层帧时序
- **THEN** 图层名 MUST 加引号传入，否则 shell 把 `(` `)` 当作元字符，命令出错且无输出

#### Scenario: 不经 shell 的图层名原样传入

- **WHEN** 用 `UiDevice.executeShellCommand` 采集图层帧时序
- **THEN** 图层名 MUST 原样传入，MUST NOT 加引号，且 MUST NOT 含空白

### Requirement: 终端内容直绘 SurfaceView 时的帧面取数

终端内容直绘 SurfaceView 的应用，其终端内容帧的跨应用判据 MUST 取系统侧逐图层通道：
`--latency` 给逐帧 `desired/actual/ready`，`--frametimeline` 的图层条目给属主归属与呈现时刻。
Perfetto FrameTimeline MUST NOT 作终端内容的跨应用判据——官方文档明确 SurfaceView 不受其支持，
而对照对象走 HWUI 管线在其覆盖内，用它比较等于比较两套不同的数据面。

#### Scenario: 终端内容帧按系统侧逐图层通道判

- **WHEN** 被测应用的终端内容由原生渲染线程直绘 SurfaceView
- **THEN** 其终端内容帧 MUST 取系统侧逐图层通道（`--latency` 的逐帧三列与 `--frametimeline` 的
      图层条目），MUST NOT 只依赖应用自己的呈现汇总

#### Scenario: FrameTimeline 不作终端内容判据

- **WHEN** 需要比较各应用终端内容的呈现时序
- **THEN** MUST 以系统侧逐图层通道为准，MUST NOT 用 Perfetto FrameTimeline 作终端内容判据

### Requirement: SurfaceView 内容帧的 jank 判定与 N/A 兜底

SurfaceView 内容帧不参与 Choreographer 调度预测：其图层条目的 `Token` 为 `-1`、
`Prediction State` 为 `None`、`Jank Type` 为 `None`、`Present Metadata` 为 `Unknown Present`，
故 jank 判定 MUST 由呈现间隔与刷新周期比较得出，MUST NOT 读调度器 jank 分类。该类帧的
`Start time` 恒为 `N/A`，`Present time` 偶为 `N/A`（此时 MUST 以 `Last latch time` 兜底），
MUST NOT 把 `N/A` 当作 0。

#### Scenario: SurfaceView 帧的 jank 由间隔比较得出

- **WHEN** 判定终端内容帧是否丢帧
- **THEN** MUST 用呈现间隔与刷新周期比较得出，MUST NOT 用图层条目的 jank 分类（对 SurfaceView
      内容帧恒为 `None`）

#### Scenario: N/A 呈现时刻不当作 0

- **WHEN** 图层条目的 `Present time` 为 `N/A`
- **THEN** MUST 以 `Last latch time` 兜底，MUST NOT 把 `N/A` 当作 0 参与间隔计算

### Requirement: BLAST 内容帧的期望呈现时刻不作迟到判据

BLAST 内容帧的「实际呈现减期望呈现」恒为一个刷新周期左右（实测 p50 16.2ms，刷新周期
16.67ms），逐帧比较会把每一帧都判成迟到。该差值 MUST NOT 作迟到或 jank 判据；判据 MUST
取呈现间隔跨过的刷新槽位数（`round(间隔 ÷ 刷新周期) - 1`），间隔在半周期以内的抖动 MUST
NOT 记为丢帧。

#### Scenario: 期望呈现时刻不作迟到判据

- **WHEN** 判定 BLAST 内容帧是否迟到
- **THEN** MUST NOT 用「实际呈现减期望呈现」判迟到，MUST 用呈现间隔跨过的刷新槽位数

### Requirement: 帧面阈值用相对量而非绝对丢帧数

系统侧逐图层通道给出的丢帧数是保留窗口内的绝对量，会随宿主负载与窗口内帧数一起变大：
同一构建、同一手势、同一 AVD 实测，宿主轻到中载时窗口内丢帧 6–32 帧，并发编译任务抢占宿主
时丢 41 帧，宿主被 8 个忙循环超额占用时丢 89–113 帧。作阈值 MUST 用相对量——呈现频率除以
设备刷新率得到的槽位比例（无量纲），MUST NOT 用绝对丢帧数作阈值。

#### Scenario: 阈值取槽位比例而非丢帧数

- **WHEN** 为宏基准或跨应用对比设系统侧逐图层通道的帧面阈值
- **THEN** MUST 用槽位比例（呈现频率 ÷ 设备刷新率）这类相对量，MUST NOT 用保留窗口内的绝对
      丢帧数

### Requirement: 全场景矩阵与环境固定化

场景矩阵 MUST 覆盖启动三态（冷／温／热）、稳态空载、持续输出、突发输出、长滚动、宽字符行、
交互延迟、输入洪水、前后台切换与资源极端；每个场景 MUST 有固定负载、统一指标、采集面、判据
与硬上限。矩阵 MUST 以可执行数据落地：每项 MUST 带上述字段，测量行 MUST 能引用矩阵编号。

同一场景 MUST 给各被测应用完全相同的字节流与网格尺寸；负载不等价时该场景的对比结论无效。
同一场景 MUST 重复多次并以分位数（p95／p99）表达，MUST NOT 以均值或单次结果下结论；被测
应用的执行顺序 MUST 交替以抵消宿主漂移。

每次测量 MUST 记录设备型号、ROM 构建号、API、刷新率、电池状态、热状态与构建标识。刷新率、
性能档位、动画缩放与温度 MUST 在测量前固定并记录实际生效值。

#### Scenario: 负载不等价的对比无效

- **WHEN** 同一场景给各被测应用的字节流或网格尺寸不同
- **THEN** 该场景的对比结论无效，MUST NOT 以该结果作性能结论

#### Scenario: 环境漂移被记录

- **WHEN** 测量在刷新率、电池状态或热状态发生变化后进行
- **THEN** 该次测量 MUST 记录变化后的实际值，MUST NOT 与变化前的数据直接对比

#### Scenario: 同一场景重复多次取分位数

- **WHEN** 一个场景在同一被测应用上重复多次
- **THEN** 结论 MUST 以分位数表达，MUST NOT 以均值或单次结果下结论

#### Scenario: 测量行引用矩阵项

- **WHEN** 一次测量对应矩阵中的场景
- **THEN** 该测量行 MUST 带该场景的矩阵编号，MUST NOT 只留一个自由文本的场景名

#### Scenario: 环境固定不到即判红

- **WHEN** 刷新率、动画缩放或热状态的固定后实际生效值与目标不符
- **THEN** 该次测量 MUST 判红，MUST NOT 记一份在漂移环境下测出的数字

#### Scenario: 平台不提供核对通道时只记不可核对

- **WHEN** 平台接受固定命令但不暴露该项的生效状态
- **THEN** 测量行 MUST 记「不可核对」，MUST NOT 记成已固定

#### Scenario: 测量结束后复原设备原值

- **WHEN** 一条用例结束（无论通过与否）
- **THEN** 测量 MUST 复原被固定的环境项，MUST NOT 把设备留在被改过的状态里

### Requirement: 测量环境快照的采集与判红

每次测量 MUST 采集并记录一份环境快照：设备型号、ROM 构建号、API、活动显示的刷新率、电池
电量与状态、电池温度、热状态与被测应用的版本标识。快照 MUST 随该次测量的实测值一起落盘，
MUST NOT 只在整轮结束时记一次。

环境项读不到时该次测量 MUST 判红，MUST NOT 记一份缺项的环境充数。刷新率、电池状态或热
状态发生变化后测得的数值 MUST 以变化后的快照为准，MUST NOT 与变化前的数据直接对比。

#### Scenario: 每次测量带环境快照

- **WHEN** 一次测量完成
- **THEN** 该次测量 MUST 附带环境快照（型号、ROM 构建号、API、刷新率、电池状态、热状态与构建标识）

#### Scenario: 环境项缺失即判红

- **WHEN** 环境快照的任一项读不到
- **THEN** 该次测量 MUST 判红，MUST NOT 以缺项的环境继续出数

#### Scenario: 环境漂移以当轮快照为准

- **WHEN** 测量在刷新率、电池状态或热状态变化后进行
- **THEN** 该次测量 MUST 以变化后的快照作对比依据，MUST NOT 与变化前的数据直接比较

### Requirement: 失败归因与采集失败

失败 MUST 按四类归因：应用归因、系统归因、人为与编排归因、采集失败。归因依据 MUST 取自
`ApplicationExitInfo` 的 reason、FrameTimeline 的 jank 类型、logcat 与 dropbox 四源交集；证据
不足时 MUST 记「未定」并保留全部制品，MUST NOT 强行归因，MUST NOT 把归因不清记为无问题。

`REASON_OTHER` MUST 归人为与编排归因——其官方口径是既不是应用的问题也不是应用能操作的。
`REASON_CRASH`、`REASON_CRASH_NATIVE` 与 `REASON_ANR` MUST 归应用归因；
`REASON_LOW_MEMORY` 与 `REASON_MEMORY_LIMITER` MUST 归系统归因。

采集为空 MUST 表现为采集失败并判红，MUST NOT 当作「被测路径没有发生」。

#### Scenario: exit-info reason 归因

- **WHEN** 被测进程在场景中退出
- **THEN** 归因 MUST 由 exit-info 的 reason 给出，应用归因与系统归因 MUST 分列，
      MUST NOT 以「跑挂了」一类概括收尾

#### Scenario: 采集失败不当作没有发生

- **WHEN** 采集命令返回空结果
- **THEN** 该次采集 MUST 判为采集失败并判红，MUST NOT 当作「被测路径没有发生」

#### Scenario: 证据不足记未定

- **WHEN** 四源证据不足以区分应用归因与系统归因
- **THEN** 结论 MUST 记「未定」并保留全部制品，MUST NOT 强行归因、亦 MUST NOT 记为无问题

### Requirement: 进程退出归因的采集与判红

进程退出归因 MUST 取自 `dumpsys activity exit-info <包名>`：MUST 区分「无退出记录」与「采集失败」，
MUST NOT 把两者混为一谈。退出记录的时刻是墙钟时刻，场景起止 MUST 同取墙钟再比较。场景执行窗口内
出现进程退出 MUST 判红并附归因；reason 码不在已公开常量范围内时 MUST 归「未知」并判红，MUST NOT
猜测语义。退出归因采集自身失败时 MUST 判红并标为采集失败，且 MUST NOT 掩盖场景本身的失败。

#### Scenario: 场景内进程退出即判红并附归因

- **WHEN** 被测进程在场景执行窗口内退出
- **THEN** 该次测量 MUST 判红并附退出归因（reason、子原因与描述），MUST NOT 以「跑挂了」一类概括收尾

#### Scenario: 无退出记录与采集失败分开判

- **WHEN** exit-info 输出只有 dump 头而无退出记录块
- **THEN** 该次采集 MUST 记为无退出记录，MUST NOT 记为采集失败；MUST NOT 把空输出当作「被测路径没有发生」

#### Scenario: 未公开的原因码归未知并判红

- **WHEN** exit-info 的 reason 码不在已公开常量范围内
- **THEN** 该退出 MUST 归「未知」并判红，MUST NOT 猜测其语义

#### Scenario: 退出归因采集失败不掩盖原始失败

- **WHEN** 场景本身失败且退出归因采集同时失败
- **THEN** 该次测量 MUST 保留原始失败，退出归因采集的失败 MUST 作为附加信息附带，MUST NOT 替换或吞掉原始失败

#### Scenario: 退出归因按可 actionable 程度取

- **WHEN** 同一窗口内既有准备动作留下的强停记录又有应用崩溃记录
- **THEN** 归因 MUST 取应用故障优先的记录，MUST NOT 让强停记录盖掉崩溃原因

### Requirement: 测量结果的落盘格式与配额

每轮测量 MUST 产出一份 JSONL 行文件与一个制品目录。行 MUST 只放摘要与制品指针，字段 MUST
含 `run_id`、设备环境、应用标识、场景标识（含迭代序号与起止时刻）、宿主负载、退出归因、
制品指针与判定。

制品 MUST 附字节数与校验和，并 MUST 有配额：腾不出空间时 MUST 轮转清理最旧的制品，单件超配额
MUST 丢弃该件，MUST NOT 让设备存储被制品占满。

行数 MUST 记实际发生的测量；框架的准备轮次（温／热启动前的那一轮）MUST 与测量轮次可区分，
MUST NOT 让行数与框架迭代数的差异无从判读。

#### Scenario: 每轮一份行文件与制品目录

- **WHEN** 一轮测量完成
- **THEN** 该轮 MUST 产出一份 JSONL 行文件与一个制品目录，制品目录 MUST 按 `run_id / 场景 / 迭代` 分目录

#### Scenario: 制品附字节数与校验和

- **WHEN** 一件制品落盘
- **THEN** 该制品的行内指针 MUST 含字节数与校验和，MUST NOT 只有路径

#### Scenario: 超配额轮转清理

- **WHEN** 制品目录达到配额
- **THEN** 旧制品 MUST 被轮转清理；单件超配额时该件 MUST 被丢弃，MUST NOT 让设备存储被制品占满

#### Scenario: 准备轮次与测量轮次可区分

- **WHEN** 宏基准框架在测量前多跑了准备轮次
- **THEN** 该轮的行 MUST 标为非测量轮次，MUST NOT 与测量轮次混为一谈

### Requirement: 跨应用启动对比的系统侧口径与准备动作

跨应用启动对比 MUST 用系统侧 `am start -W` 的启动耗时作口径，MUST NOT 用各应用自报的启动
指标（那些指标量的是各目标自身管线内的耗时，跨应用不可比）。启动入口 MUST 由
`cmd package resolve-activity --brief` 解析。

冷启动的准备动作（强停）MUST 落在测量窗口之外：强停自身的退出记录不是被测行为。权限弹窗
会抢走启动（系统侧报的是弹窗自己的 Activity），测量前 MUST 预授权限；拉起的不是被测应用或
启动态不是冷启动时 MUST 判红。各被测包的数据 MUST 能在落盘行里按包名分组。

#### Scenario: 启动口径取系统侧耗时

- **WHEN** 同一台设备上测两个应用的冷启动
- **THEN** 两者 MUST 取同一 `am start -W` 的 TotalTime 口径，MUST NOT 混用各应用自报的启动指标

#### Scenario: 准备动作不落在测量窗口内

- **WHEN** 冷启动前强停被测应用
- **THEN** 强停 MUST 在测量窗口外完成，MUST NOT 让强停自身的退出记录作废该轮测量

#### Scenario: 权限弹窗抢走启动即判红

- **WHEN** 启动被系统权限弹窗接管
- **THEN** 该轮 MUST 判红并报出接管启动的 Activity，MUST NOT 把弹窗的启动耗时记成被测应用的

#### Scenario: 启动态不是冷启动即判红

- **WHEN** 拉起后的启动态不是 `COLD`
- **THEN** 该轮 MUST 判红，MUST NOT 记成冷启动数据

#### Scenario: 各包数据按包名分组

- **WHEN** 一轮测量覆盖多个被测包
- **THEN** 落盘行 MUST 带被测包的标识，MUST NOT 只能按场景名区分

#### Scenario: 被测包参数解析失败即退回默认包

- **WHEN** 仪器参数给出的被测包名列表为空、只有分隔符或含空白项
- **THEN** 解析 MUST 退回默认被测包，MUST NOT 静默地只测一部分包

#### Scenario: 系统没测到启动时按缺失总耗时判红

- **WHEN** `am start -W` 的输出没有总耗时行（系统只在测到启动时才打印该行）
- **THEN** 该轮 MUST 判红，MUST NOT 把缺失的总耗时读成 0

#### Scenario: 启动态未知即判红

- **WHEN** 拉起后的启动态是 `UNKNOWN`
- **THEN** 该轮 MUST 判红，MUST NOT 记成冷启动数据

### Requirement: 极端场景的资源上限

每个场景 MUST 有 wall-clock 上限、输出字节上限与采样间隔下限；超限 MUST 终止该场景并记录
「超限终止」，MUST NOT 继续等待，MUST NOT 继续追加输出。超限终止后 MUST 强停被测应用，
MUST NOT 让被测进程继续跑在失败场景上。制品落盘 MUST 有配额并轮转清理，超限制品 MUST
丢弃而非占满设备。

采集 MUST 单被测应用串行执行：同一时刻只跑一个被测应用，以免宿主抢占污染结论。

#### Scenario: 超限即终止

- **WHEN** 场景达到 wall-clock 上限或输出字节上限
- **THEN** 该场景 MUST 被终止并记录「超限终止」，MUST NOT 继续等待或继续追加输出

#### Scenario: 超限终止落进测量行

- **WHEN** 场景因超出 wall-clock 或输出字节上限而终止
- **THEN** 该测量行的判定 MUST 是「超限终止」，且被测应用 MUST 被强停

#### Scenario: 制品配额轮转

- **WHEN** 制品目录达到配额
- **THEN** 旧制品 MUST 被轮转清理，MUST NOT 让设备存储被制品占满

### Requirement: 场景 wall-clock 预算在执行期生效

场景的 wall-clock 上限 MUST 在场景执行期间持续生效：超限即终止该场景（强停被测应用）
并记「超限终止」，MUST NOT 等场景跑完后再判定。预算取自场景矩阵，不在执行期临时
决定。极端场景 MUST 快速失败并及时终止，不得继续消耗设备资源。

#### Scenario: 超限即终止

- **WHEN** 场景执行期间 wall-clock 超过矩阵上限
- **THEN** 该场景 MUST 立即终止并记「超限终止」
- **AND** 被测应用 MUST 被强停，不得继续追加输出

#### Scenario: 预算取自矩阵

- **WHEN** 场景开始执行
- **THEN** 其 wall-clock 上限与输出字节上限 MUST 来自场景矩阵
- **AND** MUST NOT 在执行期临时决定

### Requirement: S8–S12 场景执行器

矩阵中的 S8（宽字符行）、S9（交互延迟）、S10（输入洪水）、S11（后台前台切换）、
S12（资源极端）MUST 各自有可执行的场景执行器，覆盖宽字符排版、交互延迟双口径
（回显产出与按键到首个呈现）、输入洪水丢键与 ANR 归因、切后台切回与旋转恢复首帧、
资源极端下的存活/快速死亡与退出归因。同一场景给各被测应用完全相同的负载与网格尺寸。

#### Scenario: 宽字符无损与呈现

- **WHEN** S8 执行宽字符负载（CJK、组合字符、emoji、ZWJ 家庭、全角拉丁）
- **THEN** 标题哨兵 MUST 留在会话标题里
- **AND** 呈现面 MUST 满足出帧、窗口数、停摆、间隔 p95、槽位占比、两通道一致性六项判据

#### Scenario: 输入洪水不丢键

- **WHEN** S10 注入连续按键事件
- **THEN** 终端 MUST 收到全部按键，且活性探针 MUST 回应（无 ANR）

#### Scenario: 资源极端归因明确

- **WHEN** S12 施加低内存、强压、灭屏、Doze
- **THEN** 进程活着或死得有归因均合格
- **AND** `ProcessExitCause.UNKNOWN` 归因 MUST 判红

### Requirement: 恢复与旋转的首帧取数口径

界面重建（切回前台、旋转）之后，内容层的帧历史里可能还留着重建之前的帧，把它们算进首帧
延迟会让恢复看起来是零延迟，故首帧 MUST 取「重建确认之后」的首个呈现，MUST NOT 取图层上
最早的那一帧。

旋转的计时起点 MUST 取「系统确认转到目标方向」的时刻，MUST NOT 取「发起旋转」的时刻：
旋转动画期间显示方向与用户方向设置不同步，起算点取早了会把旋转动画的时间算进首帧延迟。

等待出帧的上限与首帧判据 MUST 分开：等待上限判「没出帧」，判据上限判「慢」，后者 MUST
小于前者。

#### Scenario: 旧图层的帧不算恢复首帧

- **WHEN** 切回前台或旋转之后取内容层的首帧
- **THEN** 首帧 MUST 取重建确认之后的呈现，MUST NOT 取图层帧历史里重建之前的帧

#### Scenario: 旋转计时起点取系统确认转向的时刻

- **WHEN** 旋转场景测内容层首帧延迟
- **THEN** 计时起点 MUST 取显示方向确认转到目标方向的时刻，MUST NOT 取发起旋转的时刻

#### Scenario: 等待上限与判据上限分开

- **WHEN** 内容层在等待上限内没有呈现
- **THEN** 该轮 MUST 以「没出帧」判红，MUST NOT 与「首帧超过判据上限」混为一谈

### Requirement: 资源极端的屏幕状态复原

灭屏压力的复原 MUST 按当前屏幕状态判断：电源键是切换语义，施加压力时按过一次后屏幕状态
与原值相反，当前状态与原值不同才可再按一次。MUST NOT 按原值盲按，否则会把设备留在被改过
的状态里，后续测量在不同的屏幕状态下进行。

#### Scenario: 屏幕状态按当前状态复原

- **WHEN** 施加灭屏压力后复原
- **THEN** 复原 MUST 以当前屏幕状态与原值比较决定是否再按电源键，MUST NOT 按原值盲按
