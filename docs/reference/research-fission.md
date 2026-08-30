# Fission（fission-ui/fission）深度研究

> 研究对象：`/home/runner/work/kudzu/kudzu/repositories/refs/fission`（fission-ui/fission，Rust 跨端 GPU 应用框架）
> 研究目的：为 torvox（Android 终端，Kotlin Compose + Rust native + wgpu 30 + libghostty-vt）提炼可吸收的架构、渲染管线、测试方法与 Android 平台模式
> 说明：本文档基于对仓库源码（1340 个文件）核心 crate 的完整阅读。所有行号以研究时仓库快照为准；超大文件（>100KB）以符号索引 + 关键段落精读为准，已在文中注明。

---

## 1. 项目定位

Fission 是一个**面向生产环境的 Rust 应用框架**，用一套代码覆盖 macOS / Windows / Linux / Web / Android / iOS / Terminal / 静态站点 / SSR 九类目标（`README.md:8`）。它不是单纯的 widget 库，而是覆盖「创建 → 运行 → 测试 → 打包 → 发布」完整生命周期的工具链：

| 阶段 | 提供内容（README.md:23-29） |
| --- | --- |
| Setup | `fission init`、目标脚手架、`fission add-target`、设备枚举 |
| Learn | 自带文档站点（fission.rs，由框架自身生成，dogfooding） |
| Build | 声明式 widget、类型化 action/reducer、设计系统、图表、3D、Terminal、静态站点、SSR |
| Test | 单元测试、widget 测试、live app 测试、设备冒烟、诊断、路由检查 |
| Publish | 打包输出、就绪检查、GitHub Pages/Releases、商店分发流 |

核心卖点：**确定性（determinism）是一等公民**。整个框架围绕「同一份 UI 描述在所有平台产生相同结果、可离线测试、可回放」设计（`docs/01-2-determinism-as-a-first-class-requirement.md`）。

---

## 2. 完整架构

### 2.1 Workspace / crate 划分

`Cargo.toml`（workspace，resolver = "2"）成员按职责分五组（`Cargo.toml:2-43`）：

```
crates/core/        fission-ir         Core IR 定义（Op/Semantics/WidgetId）
                    fission-semantics  仅 re-export fission_ir 的语义类型（占位壳）
                    fission-layout     自研约束布局引擎（单文件 208KB）
                    fission-core       运行时、widget 系统、action/reducer、输入、平台能力
                    fission-3d         3D 场景 embed 原语
                    fission-text-engine rope 文本缓冲区（代码编辑器用）
                    fission-theme      设计系统（DesignSystem trait + 主题）
                    fission-i18n       i18n 注册表
crates/authoring/   fission-macros     #[fission_component]/#[fission_reducer]/derive Action
                    fission-widgets    authoring 层 widget（Canvas/Flyout/Portal/Modal/Popover…）
                    fission-icons      Material 图标
                    fission-charts     ECharts 风格图表库（chart.rs 146KB）
                    fission            主 crate：re-export 所有子 crate（对外唯一依赖）
crates/rendering/   fission-render     渲染 IR（DisplayOp/DisplayList/RenderLayer/RenderScene）
                    fission-render-vello   vello 后端实现（146KB）
                    fission-render-wgpu2d  wgpu 2D 原型（已停放在 tree 内，不在 workspace：Cargo.toml:17-18 注释）
crates/shell/       fission-shell      平台抽象 trait（VideoBackend/NativeSurfaceHandler/AsyncHost）
                    fission-shell-winit    主 shell：winit + wgpu + vello + 软件渲染 + 合成器（lib.rs 427KB）
                    fission-shell-desktop   DesktopApp（winit shell 薄封装）
                    fission-shell-mobile    MobileApp（Android/iOS，winit shell 薄封装）
                    fission-shell-web      WebAssembly shell
                    fission-shell-terminal Terminal shell（crossterm 渲染 Core IR）
                    fission-shell-site     静态站点渲染
                    fission-shell-server   SSR（axum）
crates/tools/       cargo-fission      `fission` CLI（init/run/test/package/site/devices）
                    fission-test       TestHarness 无头测试框架
                    fission-test-driver LiveTestClient / TestCommand 协议
                    fission-diagnostics  诊断事件（diag）
                    fission-templates   项目模板
```

特殊点：
- `[patch.crates-io] android-activity = { path = "third_party/android-activity/android-activity" }`、`fission-winit = { git = worka-ai/winit fork, rev 固定 }`（`Cargo.toml:59-61`）——**fork 了 winit 和 android-activity**，说明 Android 路径是自维护的。
- release profile：`lto=true, opt-level="z", codegen-units=1, panic="abort", strip=true`（`Cargo.toml:52-57`）。

### 2.2 分层模型

框架是严格的**五层数据流**（`docs/02-2-data-flow-authoring-to-renderer.md:11-25`）：

```
Authoring 节点（开放世界，纯数据结构，每帧可重建）
   │  lowering（确定性 desugar、稳定节点身份、canonical 化）
   ▼
Core IR（封闭世界，fission-ir 的 Op 枚举，平台无关，可序列化）
   │  layout（自研约束引擎，逻辑单位，显式舍入规则）
   ▼
Layout Snapshot（按 WidgetId 索引的几何结果，测试主产物）
   │  display list 生成（Core IR + Snapshot → 有序绘制命令 + PaintMap）
   ▼
DisplayList（渲染器无关的绘制命令）
   │  render（vello / 软件 / 平台表面）
   ▼
像素输出（测试中可选）
```

设计原则（docs/ 目录逐条成文）：
- **open-world authoring / closed-world core**（`docs/03-authoring-layer-open-world.md`、`docs/04-core-ir-closed-world.md`）：应用层可以无限扩展 widget，但 Core IR 只有 ~20 种 op，保证渲染器实现成本可控。
- **widget 树中无闭包**（`docs/03-5-no-closures-in-the-widget-tree.md`）：action 是描述符（数据），不是闭包；保证可序列化、可回放、跨平台稳定。
- 每阶段可独立检查：IR 可打印、Snapshot 可断言、DisplayList 可遍历、像素可截图（`docs/02-2:165-175`）。

### 2.3 帧管线（winit shell 实测代码路径）

主循环位于 `fission-shell-winit/src/lib.rs`（427KB，最大的单文件）。关键路径（`lib.rs:7334-7417`）：

1. **build**：`build::enter` 推入 thread-local `BuildScope`（`fission-core/src/build.rs:63-116`），widget struct 转 `Widget` 树，包 `Overlay { content, portals }` 根（`lib.rs:7334-7343`）。
2. **lower**：`InternalLoweringCx::new` + `lower_widget`（`lib.rs:7345-7358`；实现 `fission-core/src/lowering.rs:10-101`），产出 `CoreIR`，同时 `reconcile_focus`（`fission-core/src/runtime.rs:197`）。
3. **diff**：`pipeline.replace_ir(ir, &env)`（`pipeline.rs:246`）产出 `InvalidationSet`（build/layout/paint/composite 四类失效，`pipeline.rs:33-88`）。
4. **layout**：`pipeline.ensure_layout(...)`（`pipeline.rs:301`）→ `LayoutEngine::compute_layout`（`fission-layout/src/lib.rs:2277`，增量版 `compute_layout_incremental` 在 `lib.rs:2476`）。
5. **后处理**：`post_layout_hook`、`accessibility_bridge.update_tree`（`lib.rs:7391-7407`）。
6. **prepare**：`pipeline.prepare_current(...)`（`pipeline.rs:382`）生成 **retained RenderScene**（`lib.rs:7409-7417`）。
7. **present**：桌面/移动走 `MainRenderer`（Vello 或软件，`lib.rs:398`）；Web 走 `WebRenderer`（Canvas2d 软件光栅 / WebGpu，`lib.rs:472`）。帧率受 `FISSION_MAX_FPS`（默认 60）、`FISSION_REPEAT_ANIMATION_FPS`（默认 10）、`FISSION_RESIZE_FPS` 控制（`lib.rs:4842-4868`）。

### 2.4 终端 shell 的特殊地位

`fission-shell-terminal` 是**同一个管线的第四个渲染后端**：build → lower → layout 完全复用，只是 render 阶段把 Core IR 画到终端单元格（`fission-shell-terminal/src/lib.rs:1-6`）。它「故意不是独立的终端 UI 框架」——不支持的能力从 Core IR 和 semantics 检测，而不是从 widget 名字检测。这对 torvox 有直接借鉴意义（见 §4.2、§6）。

## 3. 核心文件功能说明（文件:行号）

### 3.1 fission-ir（Core IR 定义）

**`crates/core/fission-ir/src/lib.rs`（3.3KB）**
- `IR_VERSION: u32 = 1`（lib.rs:17）——IR 版本号，跨版本兼容的显式契约。
- `CoreNode { id: WidgetId, op: Op, composite: CompositeStyle, children: Vec<WidgetId>, parent: Option<WidgetId>, hash: u64 }`（lib.rs:20-27）——IR 节点 = 节点身份 + 单个 op + 组合样式 + 子节点列表。**树用 HashMap 存、children 显式挂边**（arena 风格）。
- `CoreIR { nodes: HashMap<WidgetId, CoreNode>, root: Option<WidgetId>, custom_render_objects: HashMap<WidgetId, AnyRenderObject> }`（lib.rs:37-45）——自定义渲染对象（`Arc<dyn Any + Send + Sync>`）与 IR 并存但 `#[serde(skip)]`，保证 IR 本身可序列化。
- `CoreIR::add_node` / `add_node_with_composite` / `set_root`（lib.rs:83-113）——构建 API；`add_node_with_composite` 自动回填 `parent` 指针。

**`crates/core/fission-ir/src/op.rs`（58KB，完整精读 + 符号索引）**
- `enum Op { Structural(StructuralOp), Layout(LayoutOp), Paint(PaintOp), Semantics(Semantics) }`（op.rs:8-13）——四类 op 是 IR 的全部。手动实现 `Hash`（op.rs:16-36）以 bit 级确定性（`LayoutUnit` 用 `to_bits` 哈希，op.rs:1047-1056）。
- `StructuralOp::Group { stable_hash: u64 }`（op.rs:39-41）——唯一的结构 op，用于稳定分组。
- `CompositeStyle` / `CompositeScalar { base: f32 }`（op.rs:44-84）——组合（opacity/transform）用「base + motion 目标 id」表达：`CompositeScalar::motion(target: WidgetId)`（op.rs:57）把动画值绑定到另一个 widget 的 motion 属性，实现声明式动画。
- `enum Length { Points(LayoutUnit), Percent(f32), ViewportWidth(f32), ViewportHeight(f32), Calc{...}, Clamp{...}, Min(Vec<Length>), Max(...), FitContent(Option<Length>), Intrinsic{...} }`（op.rs:85-122）——**声明式长度表达式**：`points/percent/vw/vh/clamp/min/max/fit_content`（op.rs:123-166），`resolve()` 在布局时求值（op.rs:180）。
- `LayoutOp`（op.rs:941-1045）：`Box`、`StyledBox{style: BoxStyle}`、`Flex{direction, wrap, gap, align_items, justify_content}`、`Grid{columns, rows, gap}`、`GridItem`、`Responsive{query, cases}`（容器查询）、`Scroll{direction, show_scrollbar}`、`AbsoluteFill`、`Positioned`、`PositionedLengths`、`ZStack`、`Align`、`Flyout{anchor, content}`、`Spotlight{anchor, padding}`（教程高亮遮罩）、`Transform([f32;16])`、`Clip{path}`。
- `enum GridTrack { Fixed, MinMax, Repeat, AutoFit, AutoFill }`（op.rs:745-769，`minmax/repeat/auto_fit/auto_fill` 构造器 op.rs:811-834）。
- `BoxStyle`（op.rs:371-513）：`width/height/min/max/padding/margin/overflow/align/aspect_ratio/positioned/grid/flex` 全套 builder。
- `enum PaintOp`（op.rs:1689-1750）：`BackdropFilter`、`DrawRect`、`DrawText`（含 caret 参数）、`DrawRichText{runs, annotations}`、`DrawImage`、`DrawPath`、`DrawSvg`——**绘制 op 都自带 caret 配置**，说明文本输入是 IR 级一等能力。
- `TextParagraphStyle` 用 `LayoutUnit` 位打包编码（`encode_text_paragraph_style` op.rs:669，`decode` op.rs:693），行数上限 `TEXT_PARAGRAPH_MAX_ENCODED_LINES`——为了确定性哈希与紧凑序列化做的奇技。
- 大量内联测试（op.rs:1861-1966）：段落样式 round-trip、图片缓存 key 稳定性、inline widget marker 等。

**`crates/core/fission-ir/src/semantics.rs`（19.6KB，完整精读）**
- `enum Role`（semantics.rs:18-49）：Button/Link/MenuItem/Text/TextInput/Image/Checkbox/Radio/Switch/Dialog/Slider/Input/List/ListItem/Generic。
- `FocusPolicy::{FocusOnPointer, PreserveCurrentOnPointer}`（semantics.rs:72-80）。
- `enum ActionTrigger`（semantics.rs:87-121）：`Default/DragStart/DragUpdate/DragEnd/HoverEnter/HoverExit/HoverCursor/Focus/Blur/TapOutside/Change/NumberChange/EditingComplete/Submit` 等——**输入系统完全由语义驱动**。
- `Semantics` struct + `ActionSet`（语义动作集合）+ `InputMask::{Numeric, Alphanumeric}`（semantics.rs:546-556，`is_valid_char` 做键盘过滤）。

**`crates/core/fission-ir/src/widget_id.rs`（3.2KB，完整精读）**
- `WidgetId(u128)`（widget_id.rs:30）——**全框架唯一身份类型**（authoring/lowering/layout/渲染/hit-test/runtime 共用），BLAKE3 派生：
  - `WidgetId::explicit(key)`：用户稳定 key 哈希（widget_id.rs:49-62）。
  - `WidgetId::derived(parent, path)`：父 id + 子索引路径哈希（widget_id.rs:67-78），前缀 `b"derived:"` 防碰撞域分离。

### 3.2 fission-layout（自研约束布局引擎，单文件 208KB）

**`crates/core/fission-layout/src/lib.rs`（208KB；符号索引 + 关键段落精读）**
- 基础几何：`LayoutPoint/LayoutSize/LayoutRect/LayoutUnit`（lib.rs:267-334、1620-1660）。
- `BoxConstraints`（lib.rs:335-490）：Flutter 风格双边界约束——`tight/loose/constrain/smallest/deflate/tighten/apply_min_max/loosen`；`is_width_bounded/is_height_bounded`。
- `LayoutInputNode`（lib.rs:830-863）：布局输入 = op + id + children；`layout_input_fingerprint`（lib.rs:1583）对输入做指纹，供增量复用。
- 增量布局：`IncrementalLayoutReuseState`（lib.rs:575-583）、`matches_input_nodes`（lib.rs:589）、`update_nodes`（lib.rs:640）、`rebuild_topology`（lib.rs:664）、`copy_cached_subtree`（lib.rs:2742）——**按指纹复用未变子树的几何**；`compute_layout_incremental`（lib.rs:2476）。
- 图验证：`LayoutGraphValidationState`（lib.rs:512）、`detect_cycle_nodes`（lib.rs:727）DFS 环检测、深度上限 `layout_depth_overflow`（lib.rs:2722）。
- 主入口 `LayoutEngine::compute_layout`（lib.rs:2277-2295）：`ensure_graph_state → validate_graph_state → compute_layout_constraints → emit_scroll_diagnostics → emit_overflow_diagnostics`。
- `compute_layout_constraints`（lib.rs:2301-2474）核心细节：
  - 根约束默认 tight 到 viewport，显式尺寸才 loosen（lib.rs:2312-2325）。
  - `visual_location` 闭包沿祖先链减去 Scroll 偏移（lib.rs:2341-2359）。
  - `Spotlight` 后处理：把 5 个子节点（上/下/左/右/焦点环）摆到 anchor 周围（lib.rs:2361-2384）。
  - `Flyout` 后处理：内容在 viewport 内 clamp、优先放锚点下方（lib.rs:2386-2440）。
- `ScrollDataSource` trait（lib.rs:63）——滚动偏移由外部（runtime）提供，布局引擎保持纯函数。
- `TextMeasurer` trait（lib.rs:814 附近）：`measure/measure_rich_text/get_caret_position/hit_test`；`RecordingMeasurer`（lib.rs:814）记录测量供测试断言。
- 布局 op 实现：`layout_node_constraints`（lib.rs:2873，分派所有 op）、flex 算法 `FlexChildEntry`（lib.rs:3646）、grid 算法 `GridCell`（lib.rs:4151，含 repeat/span/auto 占位推进，测试 lib.rs:1144-1229）。
- 内联测试约 50+ 个（lib.rs:920-1580）：reorder 拒绝、容器查询、grid repeat、fit_content、flyout clamp、spotlight 等——**布局引擎本身就是无 GPU 可测的纯函数**。

### 3.3 fission-core（运行时 + widget 系统 + 输入 + 平台能力）

**`lib.rs`（32KB，完整精读）**：模块清单 lib.rs:45-82；`pub mod internal`（lib.rs:91-165）是 shell/测试/宏的集成边界（`BuildCtx`、`InternalLoweringCx`、`InternalIrBuilder`、`lower_widget`、`lower_widget_to_ir`、`custom_render_widget`）；`pub mod public`（lib.rs:167-372）是应用可见 API 的集中 re-export。

**`action/mod.rs`（10.6KB，完整精读）**
- `ActionId(u128)`（action/mod.rs:103）——`ActionId::from_name` = BLAKE3(全限定类型名) 前 16 字节（action/mod.rs:119），跨编译/平台稳定。
- `ActionEnvelope`（类型擦除传输格式）、`Action` trait（`static_id()`）、`GlobalState` trait（downcast_rs，action/mod.rs:322）。
- 内置 action：`ShellRouteChanged`（路由，action/mod.rs:33）、`Undo/Redo`（action/mod.rs:66-88）。
- `Reducer<S>` 旧式 3 参签名（action/mod.rs:330），新式推荐 `ctx.bind`。

**`runtime.rs`（89KB；符号索引 + dispatch 精读）**
- `Runtime`（runtime.rs:111）：持有 `app_states`（全局状态）、`reducers/persistent_reducers`、`effect_callbacks`、`pending_effects`、滚动/焦点/文本编辑/motion/video 状态。
- builder：`with_measurer/with_clipboard/with_ime_handler`（runtime.rs:178-195）。
- `reconcile_focus`（runtime.rs:197）：IR 变化后按语义重建焦点。
- `dispatch_node_with_input`（runtime.rs:485-591）：dispatch 顺序 = 视频 action 委托（runtime.rs:502）→ scoped handler（runtime.rs:508）→ callback reducers（runtime.rs:516）→ persistent reducers（runtime.rs:528）→ per-frame reducers（runtime.rs:553）→ 效果入队（runtime.rs:578）。全程发 `diag` 事件（`dispatch_start/end`）。
- `tick(dt)`（runtime.rs:594）：推进 motion、资源定时器、动画；`sync_motion_declarations`（runtime.rs:647）；`reconcile_resources`（runtime.rs:1988）——资源（定时器/数据流）按 key + generation 管理，`is_resource_current`（runtime.rs:2035）判定过期。
- `handle_input`/`hit_test_recursive`（runtime.rs:1859）——输入先 hit-test 到 widget，再按 Semantics 的 ActionTrigger 映射为 action dispatch。

**`build.rs`（21.9KB，完整精读）**
- `BuildScope`（build.rs:10-30）：thread-local 构建上下文（build.rs:32-34），用裸指针存 ctx/view/resources（性能考量）。
- `enter()`（build.rs:63-116）：push scope + PopGuard 自动 retain 活跃 local state。
- 隐式 widget 序号 `implicit_widget_seq`、`local_state_ordinals`、`providers`（build.rs:25-29）——构建期依赖注入。

**`build_context.rs`（5.1KB）**：`BuildCtx<S>` 是宏生成组件代码的调用面。

**`lowering.rs`（19KB，完整精读）**
- `InternalLoweringCx`（lowering.rs:10-101）：`next_node_id`（lowering.rs:38）用 `WidgetId::derived` 从 id 栈派生；`insert_node_with_composite`（lowering.rs:68-100）**计算节点 hash = hash(op) ⊕ hash(composite) ⊕ Σ hash(child)**（lowering.rs:80-91），重复 id 会 panic（lowering.rs:76-79）——结构指纹是 diff/缓存的基础。
- `InternalIrBuilder`（lowering.rs:103-120）；`build_layout_tree`（lowering.rs:573 附近）产出 `LayoutInputNode` 列表供布局引擎。

**`view.rs`（8.3KB，完整精读）**
- `View<'a, S>`（view.rs:32-115）：**只读**访问 `state/runtime/env/layout(上一帧)/theme/i18n`；`get_rect/get_constraints`（view.rs:81-89）用上一帧布局做「本帧定位」；`motion_value`（view.rs:99）。
- `Selector` trait（view.rs:320-325）+ `ValueView/ComputedView`——宏生成的选择器模式。

**`env.rs`（26.4KB；符号索引）**：`Env`（env.rs:102，含 theme/i18n/viewport/window/route）；`RuntimeState`（env.rs:176）聚合 `HeroState/GestureState/DragSessionState/ScrollStateMap/ContextMenuState/SelectableTextStateMap/video/web/motion`——**运行时交互状态全部显式存储在 RuntimeState，便于快照与回放**。`Clipboard`/`ImeHandler` trait（env.rs:164-174）：IME 光标区域由 shell 实现。

**`input/text.rs`（117.9KB；符号索引）**——TextInput 控制器（最大单模块）：
- `TextInputController::handle_event`（text.rs:25）主状态机；`handle_key`（text.rs:568）处理全部按键（移动/选择/删除/剪贴板/IME 组合）；
- 移动端专属：`drag_start_behavior`（text.rs:1081）、`selection_handle_hit`（text.rs:1188）、`execute_toolbar_action`（text.rs:1209，剪切/复制/粘贴/全选）、`sync_text_input_affordances`（text.rs:1338）——**selection handles + 工具栏是跨平台文本编辑的一部分**；
- 掩码输入：`mask_text_for_metrics`（text.rs:1908）；字形/词边界：`prev/next_grapheme_boundary`、`prev/next_word_boundary`（text.rs:1952-2000）。

**`motion.rs`（60.6KB；符号索引）**：`MotionValue/MotionPropertyId/MotionDeclaration/MotionController`——声明式动画系统（spring/tween，绑定 `CompositeScalar::motion`）；`RuntimeState.motion` 存每 (widget, property) 当前值。

**`context.rs`（52KB；符号索引）**：`ReducerContext`（context.rs:110）+ `Effects`（context.rs:136）：`bind`（context.rs:179，绑定 action→handler 并生成 envelope）、`add`（context.rs:206，效果入队）、`capability`（context.rs:221，平台能力调用）、平台效果入口（notifications/nfc/biometrics/passkeys/bluetooth/barcode/camera/clipboard/geolocation/haptics/microphone/wifi/volume，context.rs:259-373）、`scroll_into_view`（context.rs:497）。

**`ui/widgets/`（built-in widget 集）**：`button.rs`（23.6KB）、`checkbox.rs`、`column.rs`/`row`（Flex 封装）、`container.rs`（16KB）、`context_menu.rs`（17KB）、`focus_scope.rs`、`gesture_detector.rs`（9.2KB）、`grid.rs`、`icon.rs`、`image.rs`、`lazy_column.rs`（10.7KB）、`stack.rs`、`switch.rs`、`text.rs`（54KB）、`text_input.rs`（73.4KB）、`video.rs`、`action_scope.rs`、`align.rs`、`clip.rs`、`composite.rs`、`transform.rs`、`builder.rs`。widget 是**struct + `impl From<W> for Widget` + 可选的 `InternalLower`**（`ui/node.rs` 23.4KB：`Widget` 枚举/`InternalRenderNode`）。

**`hit_test.rs`（13.9KB）**、**`diff.rs`（8.7KB）**（IR 前后 diff）、**`effect.rs`（18.3KB）**（`Effect/RuntimeEffect/EffectEnvelope` + effect 回调注册）、**`event.rs`（7KB）**（`InputEvent/KeyEvent/PointerEvent/ImeEvent`）、**`data_stream.rs`（9.2KB）**、**`registry.rs`（19.9KB）**（`ActionRegistry/ResourceRegistry/PortalEntry`）、**`capability.rs`（7.2KB）**（`OperationCapability` trait）、**`async_runtime.rs`（7.3KB）**、**`platform*.rs`**（12 个平台能力模块：clipboard/geolocation/haptics/microphone/nfc/passkey/bluetooth/camera/barcode/wifi/volume/biometric，每个 ~4-15KB，定义 host trait + Memory/Unsupported 双实现——**测试用 Memory 实现，未接线用 Unsupported 实现**）。

**`tests/`（fission-core 内联集成测试）**：`focus_traversal_test.rs`（12.4KB）、`focus_scope_test.rs`、`layout_interaction_tests.rs`、`hero_layout_test.rs`、`text_layout_test.rs`、`effect_test.rs`、`safe_area_test.rs`、`clip_test.rs`、`custom_render_focus_test.rs`、`transform_test.rs`、`layout_repro.rs`——都是无 GPU 的纯 Rust 测试。

### 3.4 fission-text-engine（文本引擎，代码编辑器用）

**`lib.rs`（941B）**：re-export `TextBuffer/CoordinateMapper/LspPosition/EditHistory/EditTransaction/TextEdit/LineCol/LineIndex`。
**`buffer.rs`（5.3KB，完整精读）**：`TextBuffer { rope: ropey::Rope, revision: u64 }`（buffer.rs:13-16）——rope 存储 + **revision 计数器**（每次 insert/delete/replace 自增，buffer.rs:111-150），下游布局/高亮/诊断可廉价检测脏数据。
**`edit.rs`（8.3KB，完整精读）**：`TextEdit{range, new_text, old_text}`（edit.rs:10-18）带 inverse（edit.rs:38）；`EditTransaction`（edit.rs:54）；`EditHistory`（edit.rs:95，默认 1000 条上限、FIFO 淘汰）。
**`line_index.rs`（5.4KB）**：`LineIndex` 字节偏移 ↔ (line,col) ↔ UTF-16 列（LSP 协议用）。
**`coordinate.rs`（2.8KB）**：`CoordinateMapper`/`LspPosition`。
**`tests.rs`（17KB）**：rope 边界、undo/redo 事务、UTF-16 映射等。

### 3.5 渲染层：fission-render / fission-render-vello / fission-render-wgpu2d

**`crates/rendering/fission-render/src/lib.rs`（10KB，完整精读）**
- 渲染数据模型：`Color`（lib.rs:9）、`Fill::{Solid, LinearGradient, RadialGradient}`（lib.rs:17-33，渐变坐标**归一化到绘制 bounds**）、`Stroke`（lib.rs:50）、`BoxShadow`（lib.rs:59）、`ImageFit`（lib.rs:73）、`TextStyle/TextRun`（lib.rs:81-99）。
- `DisplayOp`（lib.rs:102-197）：`Save/Restore/ClipRect/ClipRoundedRect/OpacityLayer/Translate/Transform/CachedScene{key, bounds, list}` + 绘制 op（`DrawRect/DrawText/DrawRichText/DrawImage/DrawPath/DrawSvg/DrawSurface{surface_id, position}`）——每个 op 带 `bounds` 和 `node_id`（调试/命中溯源）。
- `DisplayList { ops, bounds }`（lib.rs:219-235）；`LayerClip`（lib.rs:238）、`LayerStyle`（clip/transform/opacity/cache_key/content_cache_key，lib.rs:249 附近）；`RenderNode::{Paint(DisplayList), Layer(RenderLayer)}`（lib.rs:272）；`RenderScene { roots }`（lib.rs:283 附近）。
- `embed_surface_id(kind, widget_id)`（lib.rs:199-207）：Video/Web/Custom 三类 embed surface 的稳定 id 派生（`0xF151_...` 命名空间）。
- `image_cache_store.rs`：通用 LRU 图片缓存 store（ImageCacheStore）。

**`crates/rendering/fission-render-vello/src/lib.rs`（146KB；符号索引 + render 核心精读）**
- `VelloRenderer<'a> { scene, measurer, scene_cache, transform_stack, current_transform, layer_count_stack, clip_stack }`（lib.rs:1995-2004）——渲染时维护变换/裁剪/图层栈。
- `RetainedSceneCache`（lib.rs:2006-2069）：`HashMap<u64, Scene>` + FIFO，默认 256 项；`get_or_insert_with`（lib.rs:2056）按 cache_key 复用已编码 vello Scene。
- `impl Renderer for VelloRenderer`：`render_scene` 遍历 `scene.roots`（lib.rs:3978-3984）；`render_node`（lib.rs:3845）分派 Paint/Layer；`render_layer`（lib.rs:3852）：**场景缓存策略**——仅当 layer 无 clip/transform/opacity 且 `FISSION_ENABLE_VELLO_SCENE_CACHE=1` 时缓存（lib.rs:3853-3883），缓存 scene 用 `scene.append(cached, transform)` 零成本复用；`render_layer_uncached`（lib.rs:3888）按 clip→opacity→transform 顺序 push_layer/pop_layer。
- 文本：`prepare_paragraph_layout`（lib.rs:558，Parley 排版）、`render_paragraph_text`（lib.rs:2659）、`render_text`（lib.rs:2860）、`draw_caret`（lib.rs:3209）、行淡出 `paragraph_fade`（lib.rs:700）、**可见性裁剪**（`paragraph_line_visual_bounds` lib.rs:674、`TextClip` lib.rs:457——只编码可见行，测试 lib.rs:1746）。
- 图片：异步解码管线 `spawn_image_load`（lib.rs:950/969）、`fetch_network_image`（lib.rs:1030）、LRU `ImageCacheStore<ImageCacheEntry>`（lib.rs:787）、`configured_image_cache_bytes`（lib.rs:798，环境变量配置预算）。
- SVG：`parse_svg_entry`（lib.rs:1184）自研 SVG 子集解析器（形状/路径/渐变），`SVG_CACHE`（lib.rs:1983）。
- 工作量估算：`workload_profile_for_scene`（lib.rs:124）→ vello `RenderWorkloadProfile`（瓦片覆盖统计，供 renderer 调度/诊断）。
- 测试：全部无 GPU——`test_renderer`（lib.rs:1385）构造内存 Scene 断言编码结果（fade/裁剪/对齐/strut/背景段等）。

**`fission-render-wgpu2d/src/lib.rs`（原型，已停用；头部精读）**：一个 WGSL shader（uniform viewport + 顶点色），把 DisplayList 的 DrawRect 画成纯色矩形（lib.rs:8-59），依赖 `wgpu = 26`。**结论性注释**：项目最终选择 vello 而非自研 wgpu 2D（见 Cargo.toml:17-18「Prototype parked for later」）。其思路（DisplayList 直译 wgpu draw calls）与 torvox 的实例化渲染本质相同，但 fission 认为通用 2D 场景不值得自研。

### 3.6 shell 层

**`crates/shell/fission-shell/src/lib.rs`（基础抽象）**：`Platform` 枚举（lib.rs:10）；`VideoBackend/VideoPlayer` trait（lib.rs:34-47，视频播放器抽象，shell 注入）；`NativeSurfaceHandler` trait（lib.rs:132-154：`handles_payload/attach_host/detach_host/present_surfaces`，让应用把原生 surface 嵌进 UI）；`async_host::AsyncRegistry`（异步能力注册）。

**`crates/shell/fission-shell-winit/src/lib.rs`（427KB，主 shell；符号索引 + 关键段落精读）**
- `MainRenderer`（lib.rs:398）：`Vello` / `Native`（软件渲染）/ `WebGpu` 三选一；`create_native_main_renderer`（lib.rs:844）、`create_vello_main_renderer`（lib.rs:952）。
- **渲染器自动选择**：`should_auto_select_native_software`（lib.rs:978）——Windows CPU/WARP adapter 自动落软件渲染；`preferred_native_present_mode`（lib.rs:677）；surface 获取失败恢复策略 `surface_acquire_recovery`（lib.rs:1030-1037，`Lost/Outdated/OutOfMemory/Timeout` 分类处理）。
- `WinitApp<S, W>`（lib.rs:4141-4170）：聚合 `Runtime/LayoutEngine/root_widget/Env/Pipeline/VelloTextMeasurer/AsyncRegistry/test_control_port/effect 通道/deep_link/notification` 等；`run()`（lib.rs:4666）、`run_with_android_app(AndroidApp)`（lib.rs:4674，仅 Android）。
- `run_inner`（lib.rs:4678-4920）：`EventLoop::<TestEvent>::with_user_event`（lib.rs:4692，**测试事件作为用户事件类型**）；Android 强制 `WGPU_BACKEND=gl`（lib.rs:4773-4777）；渲染状态**懒创建**（等 Android resume 后才有 native surface，lib.rs:4771-4772）；帧率/动画帧率/光标闪烁周期全部环境变量可调（lib.rs:4842-4886）。
- `present_frame_with_winit_coordination`（lib.rs:750）：与 winit 协调呈现（Wayland 启动清屏帧 `should_present_startup_clear_frame` lib.rs:767）。
- 帧管线（lib.rs:7334-7417，见 §2.3）。
- 辅助：`FrameTraceState`（lib.rs:1511，重绘原因追踪）、`ResizeSettle`（lib.rs:1716，resize 稳定延迟）、`focused_text_input_id/config`（lib.rs:2263-2289，IME 联动）、`platform_window` trait（lib.rs:1451）。

**`fission-shell-winit/src/compositor.rs`（54KB；符号索引）**——`TextureLayerCompositor`（compositor.rs:121）：
- **GPU 纹理图层合成器**：把 pipeline 提取的 `CompositorTexturePlan`（pipeline.rs:153，滚动/变换/视频等动态层）渲染为缓存纹理，再用 WGSL 全屏三角形合成到目标（`compositor.wgsl` 2.4KB）。
- `render_layers`（compositor.rs:257）、`sync_plans`（compositor.rs:359，计划增删）、`build_layer_draw_batches`（compositor.rs:673）、`draw_batches_to_view`（compositor.rs:775，damage 区域 scissor）、`enforce_texture_budget`（compositor.rs:835，`compositor_cache_budget_bytes` compositor.rs:885 预算）、`render_or_seed_layer_base`（compositor.rs:1406，**首帧播种 base 纹理，后续只画 delta**）、`plan_with_clip`（compositor.rs:1546，圆角裁剪蒙版）。
- 这就是 fission 的「滚动/变换内容 GPU 缓存」方案：静态层每帧重绘，动态层纹理化。

**`fission-shell-winit/src/pipeline.rs`（140KB；符号索引）**——渲染管线核心：
- `InvalidationSet`（pipeline.rs:33-88）：build/layout/paint/composite 四级失效（`mark_build/mark_layout/mark_paint/mark_composite`）。
- `Pipeline`（pipeline.rs:169）：`replace_ir`（pipeline.rs:246，diff 出新 IR 与旧 IR）、`classify_animation_updates`（pipeline.rs:286）、`ensure_layout`（pipeline.rs:301）、`prepare_current`（pipeline.rs:382，**产出 retained RenderScene**）、`render_current`（pipeline.rs:523）、`refresh_retained_metadata`（pipeline.rs:579）、`compute_runtime_dynamic_subtree`（pipeline.rs:666，动态子树 = 引用 motion/scroll/video/web 的节点）、`patch_retained_scene`（pipeline.rs:738，**只 patch 动态子树到 retained scene**）。
- 图层提取：`build_descending_wrapper_plans`（pipeline.rs:1108）、`layer_should_extract_as_plan`（pipeline.rs:1261）、`texture_plan_key_for_layer`（pipeline.rs:1325）、`scene_cache_key`（pipeline.rs:1342）——哪些层值得纹理化由启发式决定。

**`fission-shell-winit/src/software_renderer.rs`（78KB；符号索引）**：`SoftwareRenderer`——纯 CPU 光栅化 DisplayList/RenderScene（多边形填充、渐变、文本用 swash 位图、圆角裁剪、阴影），用于 Web Canvas2D 回退与 Windows 软件 adapter；`render_with_text_measurer` 供 web 路径调用（lib.rs:7435）。

**`fission-shell-winit/src/accessibility.rs`（43.7KB）**：`AccessibilityBridge`——把 IR Semantics 映射到平台（Windows UIA/macOS NSAccessibility/Web ARIA），`update_tree`（lib.rs:7401 调用）。

**`fission-shell-winit/src/android_capabilities.rs`（83KB）**：Android 平台能力实现（`register_android_operation_capabilities`，lib.rs:4695）：通知、剪贴板、生物识别、NFC、相机、蓝牙、地理位置、触觉、WIFI、音量、passkey 等——全部通过 `ndk`/`jni` 与 Android framework 交互。**torvox 可参考其 JNI 面组织方式**（见 §6）。
- 另有 `ios_capabilities.rs`（32KB）、`macos_capabilities.rs`（43.5KB）、`web_capabilities.rs`（74.5KB）——**同一套 OperationCapability trait 每平台一个实现文件**，`Memory*` 实现用于测试、`Unsupported*` 用于未接线平台。

**`fission-shell-winit/src/ime.rs`（15.4KB）**：`DesktopImeHandler`（winit IME + `set_ime_cursor_area`）；`test_control.rs`（22.2KB）：LiveTest TCP server（见 §3.9）；`video_backend.rs`（98.7KB）：视频播放（gstreamer/系统播放器）；`web_backend.rs`：WebView embed；`notifications.rs`（61KB）：桌面通知。

**`fission-shell-terminal/`（完整精读全部 7 个文件）**——见 §4.2 详细分析。

**`fission-shell-mobile/src/lib.rs`（3.4KB，完整精读）**：`MobileApp<S, W>` = `WinitApp` 薄封装（lib.rs:21-26），`run()`（lib.rs:295）/`run_with_android_app`（lib.rs:300）；host trait 全部 re-export 自 winit shell（lib.rs:6-16）。Android 下 `pub use winit::platform::android::activity::AndroidApp`（lib.rs:18-19）。结论：**移动端没有独立渲染栈，就是 winit shell + wgpu/vello + android-activity**。

**`fission-shell-desktop/`**：`DesktopApp` = WinitApp 封装 + 桌面特有（tray 等）。
**`fission-shell-web/`**：wasm32 目标封装（mount selector、canvas 样式）。
**`fission-shell-site/`**：静态站点渲染（把 widget 树渲染为 HTML/静态资源，dogfood 文档站）。
**`fission-shell-server/`**：SSR（axum）——同一组件树服务端渲染。

### 3.7 authoring 层

**`crates/authoring/fission/src/lib.rs`（32KB，完整精读）**：主 crate 的模块 re-export 地图（lib.rs:28-150）：`core/layout/theme/i18n/text_engine/widgets/charts/three_d/macros/icons/shell/site/server/test_driver`；`use fission::prelude::*` 为入口。

**`fission-macros/src/lib.rs`（符号索引）**：`#[derive(Action)]`（lib.rs:29）、`#[fission_action]`（lib.rs:65）、`#[fission_reducer]`（lib.rs:119）、`#[fission_component]`（lib.rs:136，展开为 struct + Widget From 实现 + state view，`expand_fission_component` lib.rs:234）、`#[derive(FissionGlobalState)]`（lib.rs:165，生成 `view()` 只读视图 + 字段选择器）、`FissionStateView`（lib.rs:151）。

**`fission-widgets/src/lib.rs`（符号索引）**：authoring 层 widget：`Canvas`（lib.rs:243-296，命令式绘制闭包 → DisplayList）、`AbsoluteFill`（lib.rs:313）、`Flyout`（lib.rs:379）、`Portal`（lib.rs:397，跨子树叠加）等。

**`fission-charts/`（146KB chart.rs）**：ECharts 风格声明式图表（series 30+ 种、layout 12 种、components），与 torvox 无关，略。

**`fission-icons/`**：Material 图标（静态路径数据）。

### 3.8 其他 core crate

- **`fission-3d/src/lib.rs`（符号索引）**：`Point3D/Primitive3D/Scene3D`（lib.rs:10-74）——3D 场景作为 embed 节点 lower 进 IR（lib.rs:102-107），实际渲染交给外部 three-d 集成。
- **`fission-theme/src/lib.rs`（符号索引）**：`DesignSystem` trait（lib.rs:34-53：`tokens()/components()/patterns()/assets()/theme_ref(mode)`）；`DesignTokenSet/DesignToken/DesignValue`（lib.rs:66-78）；`PackagedFont`（lib.rs:210，内嵌字体带变体轴）；`ComponentSize/ComponentState`（lib.rs:226-235，组件规范）。静态 &'static 数据，零运行时开销。
- **`fission-i18n/`**：`I18nRegistry`（locale 注册表，字符串查找）。
- **`fission-semantics/`**：占位壳，`pub use fission_ir::{ActionSet, FocusPolicy, Role, Semantics}`（lib.rs:1）——语义类型实体在 fission-ir。

### 3.9 tools（测试与工程化）

**`crates/tools/fission-test/src/lib.rs`（37.6KB；符号索引）**
- `TestHarness<S>`（lib.rs:227）：**无头测试框架**——用 `MockRenderer`（lib.rs:30，接收 RenderScene 计数）+ `MockTextMeasurer`（lib.rs:42，宽高=字符数）或真 vello measurer（lib.rs:142，`should_use_mock_measurer` lib.rs:129 按环境切换）；`lint()`（lib.rs:241）跑布局违例检查。
- **`generate_display_list`（lib.rs:626）**：**纯软件生成 DisplayList**（不需要 GPU），`resolve_composite_scalar`（lib.rs:981）、`composite_transform_matrix`（lib.rs:995）——测试里直接断言绘制命令序列。
- `layout_input_nodes`（lib.rs:24）：IR → 布局输入，供布局单测。

**`crates/tools/fission-test/src/driver.rs`（9.8KB）**：driver 层（连接 live app）。
**`linter.rs`（4.4KB）**：`LayoutViolation`（布局 lint 规则）。

**`crates/tools/fission-test-driver/src/lib.rs`（30.7KB；符号索引）**——**LiveTest 协议**：
- `TestCommand`（lib.rs:37）：`Tap/Drag/Scroll/TapText/InputText/KeyPress/...`（坐标 + 文本两种寻址）；
- `TestEvent`（lib.rs:197）、`Selector`（lib.rs:350：`semantic_identifier/widget_id/test_id/accessibility_identifier/role_label/label`）、`SelectorQuery`（lib.rs:406：`scoped/index/include_hidden`）、`SemanticNode/TextItem/Bounds`（lib.rs:468-526）；
- `TestResponse`（lib.rs:557：`Ok/Error/Value/...`）；
- `LiveTestClient`（lib.rs:609）：`connect(port)`（lib.rs:615）、`wait_for_ready`（lib.rs:621）、`tap/drag/scroll/type_text/wait_for_text/wait_for_value/wait_for_gone/assert*`（lib.rs:654-862）。

**`fission-shell-winit/src/test_control.rs`（22.2KB；符号索引）**——server 端：`EventInjector::{Proxy, Queue}`（test_control.rs:28，Proxy 走 EventLoopProxy，Queue 用于 Android 无事件循环注入）、`spawn_server(port, injector)`（test_control.rs:43）TCP server、`dispatch_command`（test_control.rs:135）、`wait_for_selector_state`（test_control.rs:453）、`wait_for_motion_idle`（test_control.rs:381）、`auto_scroll_then_query`（test_control.rs:414）。

**`fission-shell-terminal/src/test_control.rs` → `TerminalLiveTest`**（见 §4.2）：in-process 无 TTY 测试适配。

**`cargo-fission/`（CLI）**：`fission init/run/add-target/devices/test/site/package` 全生命周期命令。

## 4. 与 torvox 的功能对比

torvox 现状（据仓库快照）：Kotlin Compose UI + Rust native 库（`native/`，wgpu 30 + cosmic-text + swash + guillotiere + fontdb + libghostty-vt），JNI FFI 桥（`native/src/android/ffi.rs` 125KB），渲染管线 `native/src/render/`（pipeline/pass/context/cell_builder/font/），终端层 `native/src/terminal/`（ghostty_terminal 封装、pty、session、vt_conformance），测试含 proptest/quickcheck/screenshot_tests/vt_conformance/maestro。

### 4.1 渲染管线对比

| 维度 | fission | torvox | 结论 |
| --- | --- | --- | --- |
| GPU 后端 | vello（wgpu 之上，默认）；自研 wgpu2d 原型已停用；软件渲染器保底 | wgpu 30 直接实例化渲染（cell/background/kgp 3 个 WGSL shader） | **fission 通用 2D 用 vello，专用场景（终端单元格）torvox 自研 wgpu 反而更优** |
| 场景模型 | 五层确定性管线：Widget → CoreIR → LayoutSnapshot → DisplayList → RenderScene（`fission-render/src/lib.rs:102-283`） | GridSnapshot/CellIterator → CellData 实例 → GPU（`docs/adr/0008-rendering-pipeline.md:107-123`） | torvox 是 2 层直通；fission 每层可测可缓存 |
| 增量/缓存 | 节点 hash 指纹 diff（`lowering.rs:80-91`）→ 四类失效（`pipeline.rs:33-88`）→ retained scene 只 patch 动态子树（`pipeline.rs:738`）+ vello Scene 缓存（`vello lib.rs:2006-2069`）→ 动态层纹理化（`compositor.rs:121`，首帧播种 base 纹理 `compositor.rs:1406`） | 无场景级缓存；全帧重建实例 buffer（ADR 0008 后简化） | **fission 的「静态层重绘 + 动态子树 patch + 纹理层合成」三级缓存是 torvox 可借鉴的最大单项** |
| 文本 | Parley 排版 + 可见行裁剪（`vello lib.rs:674`）+ 文本背景段（`vello lib.rs:475-532`）+ caret 全参数化（IR `DrawText` caret_* 字段） | cosmic-text shaping + swash 光栅 + guillotiere atlas | 终端场景 torvox 的等宽网格渲染更快更简单；富文本/选区背景渲染 fission 的思路可参考 |
| 确定性 | IR 手动位级 Hash（`op.rs:1047-1056`）、文本度量可注入（`RecordingMeasurer`）、舍入规则成文（docs/05-6） | 无全局确定性保证；截图测试靠设备快照 | fission 为测试而生的确定性设计远超 torvox |
| 软件回退 | 完整 `SoftwareRenderer`（78KB）用于 Web Canvas2D + Windows CPU adapter 自动选择（`lib.rs:978`） | 无（wgpu 失败即失败） | torvox 目前只跑 Android GPU，低优先 |
| 帧率控制 | 环境变量 FISSION_MAX_FPS/REPEAT_ANIMATION_FPS/RESIZE_FPS + resize settle（`lib.rs:4842-4868,1716`） | ATrace 标记 + 手工节流 | fission 的模式值得抄（环境变量可测） |

**torvox 有、fission 没有的**：终端单元格专用 GPU 管线（cell.wgsl 实例化 + KGP）、Kitty Graphics Protocol 支持、网格对齐渲染、等宽字体专用路径。

### 4.2 终端支持对比（重点）

fission 的终端能力 = `fission-shell-terminal`（完整精读 7 个文件）：

| 文件 | 功能 | torvox 对应 |
| --- | --- | --- |
| `lib.rs`（1-22） | 模块组织；定位声明「不是独立终端框架，从 Core IR 检测能力」 | torvox 终端核心在 native（ghostty-vt） |
| `app.rs`（24.9KB） | `TerminalApp<S, W>`（app.rs:55-73）：**同一个 Runtime/LayoutEngine/Env**，只是 `TerminalTextMeasurer` 替换文本度量（app.rs:91）；`TerminalRunOptions`（app.rs:34-41：尺寸/截图路径/退出条件/poll 间隔）；`render_frame(width, height)`（app.rs:200 附近）输出 `TerminalFrame`；事件循环 crossterm raw mode + alternate screen（app.rs:620-629）；**key_handler 回调 + state_update 回调**（app.rs:64-68）——应用可完全接管按键 | torvox 无（终端协议处理在 Rust，UI 在 Kotlin；没有「把任意 Compose 式 UI 渲染到终端」的需求） |
| `render.rs`（24.2KB） | `TerminalRenderer`（render.rs:13-19）：把 CoreIR+LayoutSnapshot 渲染成 `TerminalFrame`；**递归 render_node 按 Op 分派**（render.rs:49-78）；Scroll 偏移→视觉偏移 + clip 求交（render.rs:88-116）；`render_paint`（render.rs:118）把 DrawRect/DrawText/DrawRichText 映射为单元格；滚动条绘制（render.rs:74-76，`scrollbar_geometry_for_node`）；宽度按 `unicode-width`（render.rs:11） | torvox 的 `render/cell_builder.rs`（30.7KB）做 CellIterator→CellData；无「UI 框架渲染到终端」概念 |
| `frame.rs`（5.4KB） | `TerminalCell{ch, style}` / `TerminalFrame{width, height, cells}`（frame.rs:68-85）；`TerminalColor::blend_over`（frame.rs:26，alpha 合成） | torvox 无独立 frame 类型（直接进 GPU 实例） |
| `text.rs`（4.4KB） | `TerminalTextMeasurer`（text.rs:7）：按 unicode-width 折行/测量/`get_caret_position`（text.rs:84） | torvox 用 cosmic-text 度量（富文本场景） |
| `verify.rs`（4.8KB） | **`verify_terminal_ir(ir)`（verify.rs:23-32）**：遍历 IR 检查每个 op 是否终端可表达（Clip path/Embed/Flyout/Spotlight/渐变/图片/路径/SVG 返回 `TerminalSupportError`，verify.rs:49-109）——**能力检测基于 IR 而非 widget 名** | torvox 无此概念（架构不同） |
| `screenshot.rs`（4.9KB） | `write_frame_png`（screenshot.rs:23）：TerminalFrame → PNG（ab_glyph 渲染字形），**无 TTY 也能产出像素截图** | torvox 有 `render/screenshot_tests.rs`（35KB）截图测试（真 GPU 表面） |
| `test_control.rs`（14.2KB） | **`TerminalLiveTest`（test_control.rs:19-29）**：in-process 驱动 `TestCommand`（Tap/Drag/Scroll/TapText/Text 等，test_control.rs:55-120），`text_items()` 语义查询（test_control.rs:105-113），**确定性终端截图** | torvox 用 maestro（真机/模拟器 UI 自动化） |

**对比结论**：
1. torvox 的终端是「真实 PTY 终端模拟器」（ghostty-vt + pty + session），fission 的 terminal shell 只是「把 UI 树画进终端」的展示层——**两者目标不同，fission 的 shell 无法替代 torvox 的终端核心**。
2. fission 可吸收的是：**IR 级能力验证（verify_terminal_ir 模式）**、**无 TTY 确定性帧 + PNG 截图**、**in-process live test 驱动**、**unicode-width 度量与 caret 定位的简洁实现**。
3. fission 终端 shell 用 `RenderCtx{offset, clip}` 传播滚动/裁剪（render.rs:80-116）——torvox 的滚动视口裁剪可参考其「祖先链偏移累加」写法（fission-layout/lib.rs:2341-2359 同款）。

### 4.3 Android 支持对比

| 维度 | fission | torvox |
| --- | --- | --- |
| 入口 | winit + android-activity（**自 patch fork**，Cargo.toml:59-61）；`run_with_android_app(AndroidApp)`（lib.rs:4674）；`EventLoop::with_android_app`（lib.rs:4702）；渲染状态等 resume 后懒建（lib.rs:4771） | Kotlin Activity/Compose 宿主 + `jni` 桥（`android/ffi.rs` 125KB） |
| GPU | wgpu 后端强制 `WGPU_BACKEND=gl`（lib.rs:4773-4777，GLES 兜底） | wgpu 30 Vulkan（ADR 0008 明确） |
| 平台能力 | `android_capabilities.rs`（83KB）：通知/剪贴板/生物识别/NFC/相机/蓝牙/地理位置/触觉/WIFI/音量/passkey，全部 `OperationCapability` trait + Memory/Unsupported 实现 | 仅剪贴板/输入法/文件等少量 JNI |
| 输入法 | `ImeHandler` trait（env.rs:169）+ `set_ime_cursor_area`（env.rs:171）——**光标区域上报驱动 IME 定位**；文本输入控制器内建 selection handles/工具栏（input/text.rs:1188-1209） | IME 在 Kotlin 侧（Compose TextField 或自绘 + IME 回调），Rust 侧 `ime.rs` 处理像素稳定（docs/lessons/07-ime-pixel-stable.md） |
| 测试 | LiveTest TCP server + EventInjector::Queue（无事件循环也能注入，test_control.rs:28-43） | maestro（YAML 场景，`maestro/` 目录） |

**对比结论**：fission 的 Android =「winit 事件循环 + wgpu + 平台能力 trait」，torvox 的 Android =「Compose 宿主 + JNI」。torvox 的 Kotlin 宿主对终端 UX（手势选择、键盘）更合适；可吸收的是 fission 的**平台能力 trait 模式**（host trait + Memory/Unsupported 实现）与 **IME 光标区域上报**契约。

### 4.4 测试工具对比

| 能力 | fission | torvox |
| --- | --- | --- |
| 无头 widget 测试 | `TestHarness` + MockRenderer/MockTextMeasurer（fission-test/lib.rs:227-275）；**generate_display_list 纯软件断言绘制命令**（lib.rs:626） | 无 widget 层；`render/tests.rs`（85KB）直接测渲染状态 |
| 布局单测 | 布局引擎纯函数 + 50+ 内联测试（fission-layout/lib.rs:920-1580） | proptest（`prop_tests.rs` 6.5KB）+ 布局辅助测试 |
| 快照测试 | 布局 Snapshot 断言（LayoutSnapshot 是一等测试产物）；终端 PNG 截图（screenshot.rs） | `snapshot_test.rs`（14.6KB）+ `screenshot_tests.rs`（35KB，GPU 表面读回） |
| 协议一致性 | 无（不模拟终端协议） | **`vt_conformance.rs`（98.8KB）**——ghostty-vt 行为一致性测试，torvox 独有 |
| Live/设备测试 | LiveTest TCP 协议：`TestCommand`（语义选择器 + 坐标）↔ `TestResponse`（fission-test-driver/lib.rs:37-609）；桌面 Proxy 注入 / Android Queue 注入（test_control.rs:28）；终端 in-process（TerminalLiveTest） | maestro（设备级 YAML 场景，含 OCR 断言，media/selection/* 截图） |
| 确定性时钟 | `Runtime::tick(dt)` + 测试时钟推进（test_animations_paused/pending_test_clock_advance_ms，lib.rs:4874-4875） | 无全局时钟抽象 |
| 模糊/属性 | 少量 | proptest + quickcheck 大量使用 |

**对比结论**：torvox 的 vt_conformance + maestro + GPU 截图是终端领域更强的组合；fission 的 **LiveTest 协议（TCP + 语义选择器 + 注入器抽象）** 是 torvox 缺少且高价值的一层——它让「同一套测试命令」在桌面（Proxy）、Android（Queue）、终端（in-process）三种宿主上跑。

### 4.5 功能总表（该功能 torvox 有没有）

| fission 能力 | torvox | 备注 |
| --- | --- | --- |
| 声明式 widget 树 + 宏组件 | ❌（Compose 承担） | 架构不同，不迁移 |
| Core IR + 确定性管线 | ❌ | 对终端网格场景过度设计 |
| 自研约束布局引擎 | ❌（Compose 布局） | 不迁移 |
| 增量渲染缓存（指纹 diff + retained scene + 纹理层） | ❌ | **可吸收思路**（§6） |
| 文本选区/光标渲染细节（背景段、caret 参数化、可见行裁剪） | ⚠️ 部分（选区 inverse video、光标闪烁） | fission 的背景段融合算法可参考 |
| IME 光标区域上报 | ⚠️（有 pixel-stable 教训） | 契约可对比 |
| 平台能力 trait + Memory/Unsupported | ❌ | **可吸收**（§6） |
| LiveTest 协议 + 注入器 | ❌（maestro） | **可吸收**（§6） |
| 终端 IR 能力验证 | ❌ | 思路可吸收 |
| 无 TTY 帧 + PNG 截图 | ⚠️（GPU 截图） | 终端 shell 模式可吸收 |
| 五层管线文档体系（docs/） | ⚠️（ADR + lessons） | **可吸收组织方式**（§7） |
| 渲染器自动降级（软件回退） | ❌ | 低优先 |
| 图表/3D/图标/i18n/主题系统 | ❌ | 与终端无关 |

## 5. 依赖分析（是否适用于 torvox？是否先进激进？）

### 5.1 fission 的依赖全景（按 Cargo.toml 与源码）

| 依赖 | 用途 | 先进度 |
| --- | --- | --- |
| vello（+ parley + swash + kurbo） | GPU 2D 场景渲染/文本排版 | **先进**：linebender 系最前沿，但迭代快、API 不稳（146KB 后端里大量适配代码可见） |
| wgpu 26（wgpu2d 原型）/ 主 shell wgpu | 平台表面 + vello 底层 | 成熟 |
| winit（**worka-ai fork，rev 固定**） | 事件循环/窗口 | 激进：fork 上游，自维护成本高 |
| android-activity（**本地 patch**） | Android 活动集成 | 激进：本地 patch 说明上游不稳 |
| crossterm + unicode-width/segmentation | 终端 shell | 成熟保守 |
| ropey | 文本缓冲区 | 成熟 |
| blake3 | WidgetId/ActionId 哈希 | 成熟快速 |
| serde/serde_json | IR/action 序列化 | 成熟 |
| anyhow/thiserror/lazy_static/downcast_rs | 基础设施 | 成熟 |
| taffy/yoga | **无**——布局自研 | 激进：208KB 自研布局引擎（flex+grid+响应式+飞出行）而非复用 taffy，换取确定性/可测性 |
| 图片解码（image）、SVG（自研子集解析） | 媒体 | 自研 SVG 解析器较激进 |

### 5.2 对 torvox 的适用性判定

- **直接可用**：crossterm 模式（无）、unicode-width 度量（torvox 已用 cosmic-text，不需要）、blake3（torvox 无身份需求）、serde（已有）。
- **可借鉴但需评估**：vello（torvox 的单元格渲染是确定性网格，vello 的曲线/图层开销反而拖慢；**不推荐引入**）；winit 路线（torvox 已有 Kotlin 宿主，不需要）；自研布局引擎（终端无此需求）。
- **不建议引入**：fission 全家桶（widget 树/IR/布局/宏）——终端应用是单一场景，五层管线的复杂度与收益不成比例；fission 自己也承认 terminal shell 是「同一个管线的第四个后端」，其价值前提是已有完整管线。
- **激进程度评价**：fission 在「确定性 + 可测性」上激进（位级 hash、编码段落样式、自研布局、fork winit），在渲染技术上保守（选 vello 而非自研 shader）。torvox 相反：渲染激进（自研 wgpu 实例化管线）、测试策略务实。**两边互补而非竞争**。

## 6. 可吸收到 torvox 的具体内容（含代码注释建议）

按 ROI 排序。所有建议均标注 fission 出处，可直接对照源码移植。

### 6.1 （高价值）LiveTest 协议：TCP + 语义选择器 + 注入器抽象

torvox 现状：maestro（YAML + OCR）做设备级测试，无进程内/宿主内驱动协议。
fission 出处：`fission-test-driver/src/lib.rs:37-609`（协议类型）、`fission-shell-winit/src/test_control.rs:28-135`（server + 注入器）、`fission-shell-terminal/src/test_control.rs:19-120`（in-process 适配）。

```rust
// 建议 torvox 在 native/src/android/ 下新增 live_test.rs：
// 1) 定义与 fission-test-driver 同构的 TestCommand/TestResponse（serde JSON）。
// 2) 在 Rust 侧 spawn TCP server（fission 用 EventInjector::Queue 把命令排进
//    事件队列再 wake 主循环 —— torvox 可把命令经 channel 送给 render/session 线程）。
// 3) 选择器复用 fission 的语义思路：torvox 没有 Semantics 层，可用
//    (grid_x, grid_y) / 文本搜索 / widget id 三类寻址（对应 fission 的
//    Selector::widget_id / label / role_label，fission-test-driver/lib.rs:350-397）。
// 4) 响应含 frame 文本快照 + 光标位置 + 尺寸，供断言（对应 TestResponse::Value）。
// 收益：同一套命令驱动 emulator 与 host 侧 headless 会话，减少对 maestro 的依赖。
```

### 6.2 （高价值）无 TTY 确定性帧 + PNG 截图（终端 shell 模式）

fission 出处：`fission-shell-terminal/src/frame.rs:80-173`（TerminalFrame 纯数据）、`screenshot.rs:23-81`（ab_glyph 画 PNG，`cell_width=10, cell_height=18` 默认）、`TerminalLiveTest::new` 首帧即产出（test_control.rs:36-45）。
torvox 已有 GPU 截图测试；建议补一层 **CPU 侧网格帧快照**（不必走 wgpu）：

```rust
// 建议 torvox 在 native/src/render/ 下新增 frame_snapshot.rs：
// pub struct CellSnapshot { cols: u16, rows: u16, cells: Vec<CellView> } // 纯数据
// 在 render/pass.rs 生成实例 buffer 前，把 CellIterator 映射为 CellSnapshot
// （与 GPU 路径共用 cell_builder 的样式计算，仅跳过顶点编码）。
// 断言 + 存 PNG 均不依赖设备，CI 可跑；与 fission TerminalFrame 同构。
// 参考 fission-frame.rs 的 index(x,y)=y*width+x 与 blend_over alpha 合成实现。
```

### 6.3 （中价值）「动态子树 patch + 静态层重绘」的失效分类

fission 出处：`pipeline.rs:33-88`（InvalidationSet 四级）、`pipeline.rs:666`（compute_runtime_dynamic_subtree：依赖 motion/scroll/video/web 的节点 = 动态）、`pipeline.rs:738`（patch_retained_scene）。
torvox 对应：当前全帧重建实例 buffer。建议：

```rust
// 建议 torvox 在 render/pipeline.rs 增加：
// enum Invalidations { Cells, Geometry, CursorOnly, Full }  // 参考 fission InvalidationSet
// 理由：终端帧 95% 的帧只有光标/选区变化；ghostty CellIterator 已提供 dirty 行
// 信息，可映射为 CellOnly 失效，跳过顶点重建。
// fission 的做法：mark_build/mark_layout/mark_paint/mark_composite 各自独立
// （fission-shell-winit/src/pipeline.rs:41-59），torvox 只需 Cells/Cursor 两级。
```

### 6.4 （中价值）滚动/裁剪的「祖先链偏移累加」模式

fission 出处：`fission-layout/src/lib.rs:2341-2359`（visual_location 沿祖先减 Scroll offset）、`fission-shell-terminal/src/render.rs:88-116`（RenderCtx.offset/clip 传播）。
torvox 的场景：滚动视口内渲染、selection 绘制坐标换算。建议对照 fission 写法统一 scroll offset 计算（当前 torvox 各模块各自算）。

### 6.5 （中价值）IME 光标区域上报契约

fission 出处：`fission-core/src/env.rs:169-174`（`ImeHandler::set_ime_allowed/set_ime_cursor_area`）、shell 侧 `DesktopImeHandler`（ime.rs）。
torvox 已有 pixel-stable 教训（docs/lessons/07-ime-pixel-stable.md）。建议把「光标像素矩形上报」抽象为 trait，由 Kotlin 侧实现，与 fission 的契约对齐（fission 把 IME 处理做成 trait 注入 Runtime，而不是散落在 shell 事件循环里）。

### 6.6 （中价值）平台能力 trait + Memory/Unsupported 双实现

fission 出处：`fission-core/src/capability.rs`（OperationCapability）、`platform_clipboard.rs` 等 12 个模块（host trait + `Memory*` 测试实现 + `Unsupported*` 占位）、`fission-shell-winit/src/android_capabilities.rs`（JNI 面）。
torvox 现状：剪贴板等 JNI 直接调用，无 trait 抽象。建议：

```rust
// 建议 torvox 在 native/src/android/ 下抽取：
// pub trait ClipboardHost: Send + Sync { fn get_text(&self) -> Option<String>; ... }
// 实现：JniClipboardHost（现有 ffi.rs 逻辑）| MemoryClipboardHost（测试用）
// 参考 fission platform_clipboard.rs 的组织：trait 在 core，实现按平台一个文件
// （fission-shell-winit/src/clipboard.rs），Memory 实现直接进 crate 供单测。
```

### 6.7 （中价值）渲染器/后端可切换 + 环境变量控制

fission 出处：`lib.rs:978`（Windows CPU adapter 自动切软件）、`lib.rs:4842-4886`（FISSION_MAX_FPS 等 8 个环境变量）、`FISSION_ENABLE_VELLO_SCENE_CACHE`（vello lib.rs:3853）、`FISSION_BACKGROUND_TEST`（lib.rs:4691）。
建议 torvox：`TORVOX_MAX_FPS`、`TORVOX_FORCE_SOFTWARE`、`TORVOX_TRACE_FRAME=1`（对照 FrameTraceState lib.rs:1511）三个环境变量 + `ATrace` 已有。

### 6.8 （低价值，思路）终端 IR 能力验证

fission 出处：`verify_terminal_ir`（verify.rs:23-32）。torvox 没有「把任意 UI 渲染进终端」的需求，但该模式可泛化为**「渲染前校验网格可表达性」**：在 cell_builder 入口断言 CellIterator 提供的数据可被当前 shader 表达（颜色空间/连字/宽字符），失败走降级路径而非黑屏——对应 torvox 的 black-screen-investigation 教训。

### 6.9 （低价值）文本背景段融合

fission 出处：`text_background_segments_for_cluster_ranges`（vello lib.rs:480-532，相邻 cluster 合并 + clip，测试 lib.rs:1416-1465）。torvox 的 selection/IME 高亮背景绘制可对照其「合并相邻段减少 draw call」的算法。

### 6.10 不建议吸收

- vello/parley 文本栈（终端网格渲染不需要曲线排版，cosmic-text 已够）。
- widget 树 + 宏 + Core IR 五层管线（架构不匹配，成本高）。
- 自研布局引擎（无通用布局需求）。
- fork winit/android-activity（torvox 用 Kotlin 宿主，无此需求）。

## 7. 项目文档吸收价值

fission 的 `docs/` 目录（~100 篇、每篇 3-6KB 的设计文档）是**按主题编号的系统性设计文档体系**，对 torvox 的 docs/（ADR + lessons + research）是很好的补充范本：

| fission 文档 | 内容 | 对 torvox 的借鉴 |
| --- | --- | --- |
| `docs/01-2-determinism-as-a-first-class-requirement.md` | 确定性作为一等需求 | torvox 可写「终端渲染确定性」篇（脏行/光标/截图可复现性） |
| `docs/02-2-data-flow-authoring-to-renderer.md` | 五层数据流逐层定义「输入/输出/性质」 | torvox 可仿写「CellIterator → CellData → 实例 buffer → 像素」数据流文档（ADR 0008 已有雏形，可补充每层可测性声明） |
| `docs/02-4-headless-execution-and-ci.md` | 无头执行与 CI | 直接对应 torvox 的 headless 测试目标 |
| `docs/04-2-core-ir-design-constraints.md` + `04-3-*` | 封闭 IR 的设计约束与 op 参考 | torvox 可给 cell/background/kgp 三 shader 写「封闭绘制契约」 |
| `docs/05-*`（lowering/canonical 化） | 身份、keyed 节点、舍入规则 | torvox 的网格对齐（font_grid_fixed 教训）可成文为舍入规则文档 |
| `docs/06-*`（semantics） | 无障碍/焦点遍历 | torvox 的 TalkBack/无障碍（如有）可借鉴 |
| `docs/08-*`（layout） | 布局快照格式 = 测试产物 | torvox 的 CellSnapshot（§6.2）可照此定义「快照是一等测试产物」 |
| `docs/09-*`（painting/display list） | 绘制确定性、PaintMap、headless 光栅化 | 对应 torvox screenshot_tests 的定位文档 |
| `docs/10-*`（scrolling） | 滚动物理与确定性 | torvox 的滚动回退/选择滚动可对照 |
| `rfc-terminal-shell.md`（15.9KB） | 终端 shell 的 RFC（动机/取舍） | 值得一读：它对「终端作为渲染后端」的论证，可帮 torvox 明确自己的边界 |

fission 文档风格的三个优点值得 torvox 吸收：**（1）每篇聚焦单一主题且短**（3-6KB，易维护）；**（2）「输入/输出/性质」固定模板**，可测性声明写在设计期；**（3）RFC 与实现分离**（rfc-*.md 与编号文档并存，标注采纳状态）。torvox 现有 docs/ 的组织（adr/ + lessons/ + reference/ + progress/）已经很好，可增加「每篇带验收断言」的习惯（fission 的文档常以可断言性质收尾）。

## 8. 附录：核心文件速查索引

```
crates/core/fission-ir/src/lib.rs        CoreIR/CoreNode/IR_VERSION（3.3KB）
crates/core/fission-ir/src/op.rs         Op 四类 op 全定义 + 位级 Hash（58KB）
crates/core/fission-ir/src/semantics.rs  Role/ActionTrigger/Semantics（19.6KB）
crates/core/fission-ir/src/widget_id.rs  WidgetId BLAKE3 身份（3.2KB）
crates/core/fission-layout/src/lib.rs    自研约束布局引擎 + 增量布局（208KB）
crates/core/fission-core/src/lib.rs      模块地图 + internal/public 边界（32KB）
crates/core/fission-core/src/runtime.rs  Runtime/dispatch/tick/focus/资源（89KB）
crates/core/fission-core/src/build.rs    BuildScope/thread-local 构建（21.9KB）
crates/core/fission-core/src/lowering.rs InternalLoweringCx/节点 hash（19KB）
crates/core/fission-core/src/view.rs     View 只读视图/Selector（8.3KB）
crates/core/fission-core/src/input/text.rs TextInput 控制器状态机（117.9KB）
crates/core/fission-core/src/motion.rs   声明式动画（60.6KB）
crates/core/fission-core/src/context.rs  ReducerContext/Effects/平台效果（52KB）
crates/core/fission-text-engine/src/     rope 缓冲/LineIndex/EditHistory（~40KB）
crates/rendering/fission-render/src/lib.rs DisplayOp/DisplayList/RenderScene（10KB）
crates/rendering/fission-render-vello/src/lib.rs VelloRenderer/SceneCache/文本（146KB）
crates/rendering/fission-render-wgpu2d/src/lib.rs wgpu2d 原型（已停用）
crates/shell/fission-shell-winit/src/lib.rs 主 shell：WinitApp/run_inner/帧管线（427KB）
crates/shell/fission-shell-winit/src/pipeline.rs 失效分类/retained scene/纹理层计划（140KB）
crates/shell/fission-shell-winit/src/compositor.rs 纹理图层 GPU 合成器（54KB）
crates/shell/fission-shell-winit/src/software_renderer.rs 纯 CPU 光栅化（78KB）
crates/shell/fission-shell-winit/src/android_capabilities.rs Android 平台能力（83KB）
crates/shell/fission-shell-winit/src/test_control.rs LiveTest TCP server（22.2KB）
crates/shell/fission-shell-winit/src/ime.rs DesktopImeHandler（15.4KB）
crates/shell/fission-shell-terminal/src/   终端 shell 全部 7 文件（~76KB）
crates/shell/fission-shell-mobile/src/lib.rs MobileApp = WinitApp 封装（3.4KB）
crates/authoring/fission/src/lib.rs      主 crate re-export 地图（32KB）
crates/authoring/fission-macros/src/lib.rs 组件/action/reducer 宏（~50KB）
crates/tools/fission-test/src/lib.rs     TestHarness/软 display list（37.6KB）
crates/tools/fission-test-driver/src/lib.rs TestCommand/Selector/LiveTestClient（30.7KB）
docs/                                    设计文档体系（~100 篇）+ RFC
```

**一句话总结**：fission 是「为确定性可测性而生的通用 UI 框架」，其价值不在 widget 或渲染技术本身，而在**把每一层都做成可断言产物**（IR 可打印、布局可快照、绘制命令可遍历、像素可截图、真机可注入命令）的工程方法论；torvox 是「为终端像素而生的专用渲染器」，两者在 LiveTest 协议、无头帧快照、失效分类、平台能力 trait 四个点上存在清晰的吸收窗口。




