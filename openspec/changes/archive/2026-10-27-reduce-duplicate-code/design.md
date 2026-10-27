# Design

## 背景

jscpd 全量扫描（`native/src` + `android/app/src/main`，阈值 50 token）测得 53 处克隆。
逐一复核后，生产代码里的克隆分三类：

1. **同一份判定被抄写两遍**——改一处即静默分叉，必须收敛；
2. **同一段拷贝/拼装逻辑被抄写两遍**——必须收敛；
3. **只是长得像**（导入列表、资源文件骨架、函数签名）——收敛收益低于引入新抽象的代价，记录后不改。

## 判定

### 收敛（第 1 类、第 2 类）

| 位置 | 重复内容 | 处理 |
| --- | --- | --- |
| `render/font/mod.rs`、`render/cell_builder.rs`、`render/font/pipeline.rs` | 图集 UV 换算 | `GlyphInfo::atlas_uv` 单一入口 |
| `android/ffi/input.rs` | JNI 字节数组读取 | `read_byte_array` 单一入口 |
| `android/ffi/input.rs`、`android/ffi/state.rs` | 原子标志 → JNI 布尔 | `read_terminal_flag_jboolean` 单一入口 |
| `terminal/ghostty_terminal/internal/kitty.rs` | RGB → RGBA 展开 | `rgb_to_rgba` 单一入口 |
| `terminal/output_processor.rs` | ESC 原样透传 | `ReadScan::pass_through` 单一入口 |
| `android/ffi/font.rs` | 渲染状态守卫 + 解锁时机 | `with_render_state` 单一入口 |
| `android/ffi/query.rs` | 选区查询导出包装 | `select_bounds_export` 单一入口 |
| `android/ffi/render/frame.rs` | 渲染状态锁获取、单元绘制调用 | `render_state_guard`、`draw_cells` 单一入口 |
| `render/font/atlas.rs` | 掩码/彩色两条分支的像素拷贝 | 统一为一条拷贝路径 |
| `render/context/atlas.rs`、`render/pipeline.rs` | wgpu 绑定组创建 | `create_atlas_bind_group` 单一入口 |
| `terminal/ghostty_terminal/internal/dispatch.rs` | 查询资源参数逐个传递 | 改传既有的 `QueryContext` |
| `terminal/ghostty_terminal/internal/run.rs` | 空闲节拍与批尾的「应答查询→置脏→发帧」 | 合并为 `respond_queries_and_publish` |
| `terminal/ghostty_terminal/public_api.rs` | 单发查询包装 | 内联到唯一调用点 |
| `android/.../ui/TerminalSurface.kt` | 修饰键状态读取 | 单一读取点 |
| `render/font/shaping.rs` | 测试里的推进宽度求和 | 测试内 helper |

### 不收敛（第 3 类，记录理由）

| 位置 | 理由 |
| --- | --- |
| `ui/ModifierBar.kt`、`ui/SessionDrawer.kt`、`ui/SettingsComponents.kt` | 命中的是 `import` 列表；Kotlin 的导入必须逐文件声明，抽取反而要新建文件或改构建。 |
| `res/drawable/launcher_monochrome.xml`、`res/drawable/logo_art.xml` | 命中的是 `<vector>` 骨架；Android 资源不支持 XML 片段复用。 |
| `res/mipmap/ic_launcher.xml`、`res/mipmap/ic_launcher_round.xml` | 同上；两者内容本就该一致（同一自适应图标）。 |
| `res/xml/data_extraction_rules.xml` | 命中的是备份排除清单；备份规则由系统按固定 schema 解析，不走资源 include。 |
| `render/font/shaping.rs` 的两处测试断言 | 已用测试内 helper 收敛（见上表），剩余差异是断言文案本身。 |

## 关键决策

### 渲染状态锁纪律不变

`with_render_state` 只包住「读渲染状态」这一段，取值后立刻释放锁，再调 JNI。
JNI 调用可能回调宿主，回调回头取同一把锁即死锁——与改动前的 `drop(state)` 位置一致。

### 单元绘制调用收敛为 `draw_cells`

阶段 1（新帧）与阶段 3（空闲帧）的 `render_cell_data` 调用参数完全相同，只有
「单元数据来自本帧还是上一帧」不同。收敛后由 `draw_cells` 统一持有渲染器与
字体管道的可变借用，两个阶段各自只负责准备数据。

### 查询资源改传 `QueryContext`

`QueryContext` 已经持有 `Encoders`、`Search`、`MatchBuffer`、`KittyPlacementCache`
四项查询资源，`process_query` / `drain_queries` 却把它们拆成四个参数逐层传递。
改传 `&mut QueryContext` 是复用既有结构，不引入新类型。

### 空闲节拍与批尾合并

两条路径的差异只有「什么算脏」：空闲节拍看 `state.cell_data_dirty`，批尾看
`batch.dirty`。两者语义相同（都需要重推帧），故合并为 `state.cell_data_dirty`
一个标志：

- 命令改网格 → `state.cell_data_dirty = true`（原 `batch.dirty = true`）；
- 查询安装选区 → `state.cell_data_dirty = true`；
- 发帧成功后由 `publish_cell_data` 清零。

行为差异只有一处且是修正：同步输出保持（ESU）期间被推迟的帧，原先要等到下一次
空闲节拍才补发，合并后在批尾即可补发。

## 验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- --deny warnings`
- `cargo ndk -t arm64-v8a clippy --all-targets`
- `cargo test --no-fail-fast`
- `semgrep scan --error --dataflow-traces --config .semgrep/rules.yml`
- jscpd 全量扫描（生产克隆数逐轮下降）
