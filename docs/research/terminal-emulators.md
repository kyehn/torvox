# 终端模拟器横向研究报告

对 ghostty、alacritty、rio、kitty、wezterm、xterm、foot 七个终端模拟器的深度调研结论，目的是回答：本项目有什么可参考的。调研方式为克隆全部仓库（xterm 为官方 release tarball）并通读渲染、解析、线程模型与输入路径的核心源码。其中 ghostty 是本项目的引擎来源，rio 是唯一的 wgpu 同源项目，xterm 是 VT 语义的权威基准。

## 七项目一览

| 项目 | 语言 | 渲染 | 架构特征 |
| --- | --- | --- | --- |
| ghostty | Zig | Metal/OpenGL | 独立渲染线程，回滚 lz4 压缩，10 万行 |
| alacritty | Rust | OpenGL | polling 事件循环，damage 驱动渲染，引擎/UI 分 crate |
| rio | Rust | wgpu/Metal/Vulkan/CPU | 16 crate workspace，sugarloaf 渲染层，主线程渲染 |
| kitty | Python + C | OpenGL | vt-parser 输入背压，SIMD 扫描，Kitty 图像协议 |
| wezterm | Rust | OpenGL/Metal | 40+ crate，term 引擎零 GUI/PTY 依赖 |
| xterm | C | Xlib | 表驱动解析鼻祖，charproc.c 一万五千行 |
| foot | C | pixman/Wayland | 扁平单层，每功能一文件，sixel 完整支持 |

## 各项目要点

### ghostty（本项目引擎源）

渲染线程 `Thread.zig`：独立线程跑 xev 事件循环；`wakeup`（合帧唤醒）与 `draw_now`（不合帧立即绘制）分离；动画 timer（custom shaders、Kitty graphics）与光标闪烁 timer（600ms）各自独立；Mailbox 为容量 64 的阻塞队列。`State.zig` 的 `yieldToDemand` 在渲染帧快照这一关键锁上让 IO 线程优先。渲染侧 `cell.zig` 按内容类型分 key（bg/text/underline/strikethrough/overline）维护各自 GPU 缓冲并做行级脏跟踪，注释明言目标是消除每帧重建 GPU buffer。回滚走 `terminal/compress/`（Page + lz4 + lz4_differential 增量压缩）。

### alacritty

事件循环用 `polling` crate（mio 后继）Level 触发。damage 是显式数据结构：`TermDamage` 枚举 Full/Partial，`LineDamageBounds` 按行记录左右列边界（列级粒度），`TermDamageState` 含 full 提示与 `last_cursor`（光标移动即产生损伤），`display_offset` 在迭代时过滤不可见损伤。`alacritty_terminal` 引擎 crate 与 `alacritty` UI 层分离。

### rio（wgpu 同源）

Cargo workspace 16 成员：`sugarloaf`（渲染）、`teletypewriter`（PTY）、`rio-vt`、`rio-grid`、`rio-fonts`、`frontends/rioterm` 等。sugarloaf 多后端（wgpu feature + Metal + Vulkan + CPU 回退，shaders 含 wgsl/metal/glsl 三套）。`QuadInstance` 96 字节实例化渲染，vertex shader 由 `vertex_id` 生成四角（batch.rs 源自 dfrg/swash_demo）。渲染在主线程，由 scheduler 的 Timer deadline 队列驱动。

### kitty

vt-parser 的 `ParserInputWake` 三态输入背压：`small_pending`（不足 1KB 的新鲜字节立即解析，但 IO 线程仍合帧唤醒）、`large_ready`（大块下次唤醒才接收）、`large_held_until`（大块按时间拒收至期限）。`simd-string` 提供 128/256/512 位多宽度 SIMD 扫描。`graphics.c` 3109 行实现 Kitty 图像协议，GPU glyph-cache 做字形缓存。

### wezterm

40+ crate。`term` crate 是完整 VT 引擎却零 GUI、零 PTY 依赖——文档写明「你提供一个 `Write` 实现，经 `advance_bytes` 喂字节即可」。`vtparse` 为表驱动解析（`transitions.rs` 状态转移表）。`termwiz` 抽象终端接口层。

### xterm（VT 语义基准）

表驱动解析的鼻祖：`VTparse.h` 声明 `ansi_table`、`csi_amp_table`、`csi_dec_dollar_table` 等多张 `PARSE_T` 状态表；`charproc.c` 一万五千行承载全部字符处理。是 VT 行为歧义时的权威参照系。

### foot

纯 C 扁平结构，每功能一文件（vt/csi/osc/dcs/grid/selection/sixel 各自独立）。`async_write` 非阻塞写：EAGAIN 返回 REMAIN 并保留 `*idx` 部分写进度，下次续写。box-drawing 以几何绘制（LIGHT/HEAVY 厚度、pixman 图像）。`quirks.c` 集中放合成器兼容（如 weston 的 subsurface workaround）。带 pgo 构建选项。

## 与本项目的对照

### 同构印证（本项目已是同一形状，无需改动）

1. **渲染线程模型**：ghostty 的独立渲染线程 + 合帧唤醒与本项目渲染循环（17ms 活跃 / 500ms idle latch）同构；本项目 latch 是其在移动端的省电变体，ghostty 无 idle 停摆概念是因为桌面无此功耗约束。
2. **实例化渲染**：rio 的 `QuadInstance`（96 字节 per quad、vertex_id 生成四角）与本项目 cell instance 同族；rio 是少数与本项目同为 wgpu 的终端，其 quad 批处理思路与本项目实例切片一致。
3. **部分写退回**：foot 的 `async_write` 保留 `*idx` 进度、下次续写，与本项目 `write_with_timeout_counted`「返回已写出条数、只把未写出部分退回队列」完全同构——独立实现的两个项目殊途同归，印证该背压形状正确。
4. **引擎解耦**：wezterm `term` crate 零 GUI/PTY 依赖，与本项目 `libghostty-vt` 引擎/宿主分离同思路；本项目把引擎放进 Rust 进程内则更进一步，无跨语言边界。
5. **表驱动解析**：xterm 与 wezterm `vtparse` 均为表驱动，本项目经 libghostty-vt（其解析器同族）继承该路线。

### 可参考点

1. **锁让渡**（ghostty `yieldToDemand`）：渲染持帧快照锁时若 IO 线程在等，主动让渡——值得对照本项目渲染线程与 VT 线程的锁次序自查：是否存在渲染长帧期间 VT 输入线程被帧快照锁挡住的路径。
2. **damage 语义清单**（alacritty）：光标移动即损伤、`display_offset` 过滤视口外损伤——本项目脏带渲染应对照确认「光标移出/移入视口」「滚动后旧脏行偏移」两类场景正确进入脏集。
3. **解析与渲染的饥饿防护**（kitty）：大块输入按时间拒收、小输入合帧唤醒——本项目读线程直读 + `pollEvent` 有界排空 + 续写队列已是等效背压；其价值在于命名了三个明确的输入档位，可作本项目输入路径压测用例设计的参照（突发 1KB 内 / 突发大块 / 持续中速流三档）。
4. **quirks 集中层**（foot）：把平台兼容 workaround 集中到一个文件并配日志——本项目 Android 机型差异（OEM 内核、SurfaceFlinger 行为）若有新增 workaround，照此集中。
5. **xterm 作为仲裁基准**：VT 行为歧义（DEC private mode 细节、回滚边界行为）以 xterm 实测为准，terminal-state-regression 用例可引用其行为作预期。

### 不采用项（记录理由）

- **alacritty 列级 damage**：本项目 GPU 实例按行组织、脏带以行为最小单位，列级粒度收益有限且复杂度翻倍。
- **rio 多后端回退**：本项目 DESIGN 明确 GPU Vulkan 渲染、无 CPU/OpenGL 回退，rio 的四后端维护面不适用。
- **kitty 时间性拒收**：本项目已有有界排空 + 计数退回双背压，时间性拒收变体不再引入。
- **foot box-drawing 几何绘制**：本项目规格未要求，字体路径承担。
- **kitty SIMD 多宽度扫描**：解析在 libghostty-vt 上游，本项目不重复实现 VT 解析。
- **rio/foot 主线程渲染**：两项目渲染在主线程正是本项目独立渲染线程要规避的形状。

## 结论

七个项目中，与本项目架构同族的只有 ghostty（独立渲染线程）与 rio（wgpu）；本项目在这两点上与最优实践同构，且在移动端特有的省电 latch、FFI 无重数据两点上更进一层。背压形状与 foot 殊途同归，引擎解耦与 wezterm 同思路、与 libghostty-vt 的集成同源。真正可落地的参考集中在三处：锁让渡自查（ghostty）、damage 边界用例（alacritty）、输入三档压测用例（kitty）；xterm 作为行为仲裁基准入库引用即可，无需引入任何代码。
