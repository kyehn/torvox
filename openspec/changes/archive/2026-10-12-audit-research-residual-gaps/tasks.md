# Tasks

## 1. 行级抽取（`grid-extraction-incremental`）

- [x] 1.0 前提已实测复核（探针跑完即删）：同一 `RenderState` 跨帧复用并逐帧标记快照已消费时，24 行中 22 行判干净；去掉该标记后全行判脏；四个对象跨函数传递需同一具名生命周期。结论见 `design.md` §1.2、§1.3
- [x] 1.1 把 `RenderState`、`RowIterator`、`CellIterator` 从 `create_render_iterators`（每次调用新建）移入 VT 线程循环，与 `Terminal`、`search`、`match_buffer` 同生命周期；删除该函数，逐帧路径直接复用循环内持有的三者
- [x] 1.2 抽取函数改用具名生命周期 `Terminal<'a,'a>`、`RenderState<'a>`、`RowIterator<'a>`、`CellIterator<'a>`，使 `build_cell_data` 仍可作为独立函数存在（省略生命周期会因 `'alloc` 不变而编译失败）
- [x] 1.3 三者的创建失败在 VT 线程启动阶段返回错误并出声，不得以「本帧返回空网格」的形式延后到第一次渲染；确认删除逐帧失败分支后 `build_cell_data` 的 `None` 返回路径仍只由抽取错误触发
- [x] 1.4 每次抽取在该帧行数据全部读出后调用上游快照 clean 标记；不调用则上游累积脏状态不重置，行级判定退化为全量重建
- [x] 1.5 行缓存结构：保留上一帧 `Vec<CellData>` 与按 `build_row_ranges` 划分的行区间，随每次成功抽取后就地更新
- [x] 1.6 `build_cell_data` 改为按上游 `RowIteration::dirty`/`next_dirty` 判定：脏行走原有逐格抽取（CellRun 样式缓存、宽字符占位、选区反白烘焙全部不动），干净行整段复制行缓存
- [x] 1.7 行缓存整体失效并退化为全量重建的触发点：`Command::Resize`、`Command::Reset`、`Command::SetTheme`、`SetSelection`、`ClearSelection`、`RenderState::update` 报错、`CellIterator::update` 中断；失效点与既有的 `mark_grid_dirty`／`last_cell_data_push = None` 同处，避免两套时机
- [x] 1.8 `Command::SetCellPixelSize` 不列入失效条件：它只改单元格像素几何、网格内容不变，`CellData` 不承载度量，列入会使每次字号变化退化为全量重建
- [x] 1.9 更新 `build_cell_data` 与 `build_snapshot` 的文档注释：删去「实测命中率 0%」与「以避开 invariant-param 生命周期问题」的结论，改为说明缓存的来源（上游行级脏标记 + 快照已消费标记）与失效条件；`build_snapshot` 若不接入跨帧缓存则删除该注释而非保留错误依据
- [x] 1.10 确认网格级 `snapshot_needs_rebuild` 语义与渲染侧 `CachedInstances` 均未改动，行级判定不与二者合并

## 2. 行级抽取的测试

- [x] 2.1 断言「跨帧复用后未变行判干净」：首帧写满网格，次帧只写一行，脏行数小于总行数且包含该行
- [x] 2.2 断言「未标记 clean 则脏状态不重置」：连续两帧均不调用快照 clean 标记，第二帧仍全行判脏
- [x] 2.3 断言「网格未变时不进入行级路径」：`grid_dirty` 为假且已有缓存时，逐格抽取不被调用
- [x] 2.4 断言「未变化行不产生跨语言读取」：仅改一行的网格状态，干净行不逐格回到终端读取（以可观测的读取计数或等价探针判定，不靠时间阈值）
- [x] 2.5 断言「增量与全量结果一致」：同一段 PTY 输出分别走行级增量与强制全量重建，两次 `CellData` 逐字节相同
- [x] 2.6 断言「尺寸变化后退化为全量重建」与「选区安装后反白正确」
- [x] 2.7 断言「行抽取中断不以缓存冒充」：注入单元迭代失败，该帧不输出由缓存拼出的残缺网格并出声
- [x] 2.8 全部用例进入现有 `cargo test` 体系，不新增测试框架、不加 `#[ignore]`
- [x] 2.9 以现有 `native/benches/` 体系测量持续输出场景下单帧抽取耗时（改动前/改动后），确认收益成立；若实测无显著收益，MUST 如实记录并重新评估本变更是否值得保留，不得以「规范要求」代替效果证据

## 3. 失真注释清理（`upstream-alignment`）

- [x] 3.1 删除描述 BEL 回调通道的悬空注释：`ghostty_terminal/commands.rs` 的 `RunConfig`（`cell_size_px` 保留自身关于单元格像素几何与 XTWINOPS 应答的文档）
- [x] 3.2 `ghostty_terminal/types.rs` 的 `EVENT_CHANNEL_CAPACITY` 说明只保留剪贴板写入，删除「振铃」；`session.rs:525`、`:533`、`:539`、`:575`、`:1185` 的收割路径注释同样删除「振铃」，保留 OSC 52 应答终止符相关的 `:612`、`:1028`、`:1040`
- [x] 3.3 确认 `on_bell` 仍未接线、无新增事件变体、无新增消费者；确认无其他文件描述该通道
- [x] 3.4 在 `upstream-alignment` 的「上游 API 取用判定」表中补记振铃通道的两处注释残留与处理方式
- [x] 3.5 把台账中 `selection/gesture` 一行的不采用理由由「DESIGN 禁多击选择」改为拖动路径的往返成本（见 `design.md` §4），并逐项登记四项优势（吸附策略、词粒度拖拽、边缘自动滚动、单一来源）的已覆盖情况：已走上游或已由本仓覆盖者不构成采纳理由
- [x] 3.6 按 5.6 的裁定（收窄声明）处理 `render/font/shaping.rs` 模块文档：删去「以支持连字」，改为单码位整形与复杂文种叠加的表述

## 4. 网格状态断言口径（`terminal-state-regression`）

- [x] 4.1 核对 `ghostty_terminal::tests` 中回滚区旧行顺序、输入回显与光标、简体中文显示宽度、字重与颜色四类断言确实直接读取网格输出；缺项补断言
- [x] 4.2 更正 `openspec/specs/terminal-state-regression/spec.md` 的 Purpose 与实现细节：删除「`TESTING.md` 明确不进行回归测试」及据此的推论，改为引用 `upstream-alignment` 的自验证门槛
- [x] 4.3 确认 `native/src/terminal/testdata/`、`snapshot_test.rs` 与相关依赖不在仓库内，不以任何形式恢复语料装置

## 5. 验证

- [x] 5.1 `openspec validate 2026-10-12-audit-research-residual-gaps --type change` 通过（已跑，Change is valid）
- [x] 5.2 `cargo test` 全绿（572 单测 + 2 并发 + 12 BDD 场景，0 失败 0 忽略）；新增 6 例行级抽取用例，未改动无关用例
- [x] 5.3 `clippy --all-targets` 零告警；期间修掉一处自引入的多余 `drop(row_iter_impl)`（`RowIteration` 不实现 `Drop`，显式 drop 只延长借用）；未新增任何抑制
- [ ] 5.4 **仪器测试需 Android 设备与 Vulkan（Mesa lavapipe），本会话未执行**：渲染无卡顿/撕裂一项仍须设备侧确认，不以主机数字替代。主机侧以 60 帧持续输出对照实测：稳态增量 0.180ms/帧（60 帧复用 1379 行）vs 冷启动首帧全量 2.918ms。复跑复核（2026-10-12 换机）：稳态增量 0.242ms/帧对全量 2.509ms，**复用行数 1379 与上次逐帧一致**——绝对耗时随机器而变，可比的判别力在复用行数

      设备侧阻塞点已探明（`research-residual-audit-closure`，2026-10-16 复跑）：
      本宿主具备 `/dev/kvm`（`crw-rw-rw-`，非只读占位）与 `platform-tools`，
      但模拟器组件为**部分下载**——`system-images/android-35/google_apis/x86_64/`
      仅存 146 字节 `.installer/.installData`、无 `package.xml`，故 `avdmanager`
      报 `Package path is not valid. Valid system image paths are: null`；
      `emulator` 目录虽有 `lib64/` 与 `package.xml`（`sdkmanager --list_installed`
      列为 37.2.12）却缺可执行二进制。修复尝试 `sdkmanager "emulator"
      "system-images;android-35;google_apis;x86_64"` 解压镜像时失败于
      `Error on ZipFile unknown archive`（下载包损坏）。
      故本项从「环境未探索」升级为「环境已探明、阻塞点为镜像包损坏」，维持未勾选；
      缺口性质不变，仍须设备侧确认，不得以主机数字替代。
- [x] 5.5 本变更未新增任何 `docs/specification/` 改动：两处声明收窄来自 5.6 裁定，`DESIGN.md:118` 的「跳过逐行复制」已是依据；`REFERENCE.md` 的采用状态补注属上一轮经批准的独立改动（不在本变更范围，本变更未触及该文件）
- [x] 5.6 `DESIGN.md:176` 的「清除滚动条」与 `DESIGN.md:138` 的「支持连字」两处声明与实现的不一致已提交维护者裁定（见 `design.md` §5）：裁定为重置按钮清除回滚与滚动状态、不引入可见滚动条；连字声明收窄为单码位整形与复杂文种叠加，不做跨单元连字。`DESIGN.md` 措辞已按裁定更新，本变更不实现这两项
