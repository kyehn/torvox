# Design

## 上下文

上一轮变更 `reduce-native-complexity` 已经把 `render_inner`、`pump_sessions` 按
既有的阶段注释拆分过一轮，`aislop` 的函数过长告警从 4 条降到 1 条（`run_inner`）。
文件维度的告警则完全没有动过：`ffi.rs` 6073 行、`render/font/mod.rs` 2239 行、
`terminal/session.rs` 2168 行等 11 个文件都在 1100 行阈值之上。

## 决策

### 1. 拆分依据是职责，不是行数

每个新模块承担一个能独立用一句话说清职责的范围，模块名用领域词而非数字：

- `native/src/android/ffi/`：`session`（会话生命周期）、`render`（渲染帧）、
  `input`（键鼠编码）、`query`（文本/选区/搜索查询）、`font`（字体与字号）、
  `clipboard`（剪贴板请求登记）、`events`（事件泵）。
- `native/src/render/`：按已有子模块边界继续切，不新增层级。
- `native/src/terminal/`：按 `pty` / `session` / `ghostty_terminal` 既有边界切。

行数是拆分的结果，不是目标：只有当某个职责内部仍然过长时才继续往下切。

### 2. JNI 导出符号保持不变

`Java_terminal_emulator_bridge_NativeBridge_*` 由 `#[no_mangle]` 决定导出名，
与所在模块无关。拆到子模块后符号不变，但 `android::ffi` 的公开面要保持一致：
子模块声明 `pub(crate)`，由 `ffi.rs` 门面 `pub use` 转出，`android/mod.rs` 的
`pub mod ffi;` 不动。

### 3. 跨职责共享的全局状态单独成模块

`SESSION_REGISTRY`、`RENDER_STATE`、`ATTACHED_SESSION_ID` 等静态同时被会话、
渲染、查询三类入口读写。它们放进 `ffi/state.rs`，由各子模块经 `super::state::`
访问；渲染状态相关的类型（`RenderState`、`KittyPlacementKeys` 等）随之同迁。

### 4. `run_inner` 的借用约束决定了拆分边界

`Terminal<'term, '_>`、`RenderExtractors<'term>`、`Search<'_, 'term>` 三者都借用
终端分配器（`Search` 内部是 `PhantomData<&'term ()>`），放进同一个结构体就构成
自引用，Rust 不允许。因此：

- 终端、行提取器、搜索对象仍作为 `run_inner` 的局部变量。
- 不受借用约束的量收进 `CommandState`：主题默认前景/背景色、网格快照缓存、
  全量文本缓存、帧推送去重基线、批内脏标记与「本批是否已有内容变更」。
- 命令分派拆成 `apply_command`（分发）+ 每族一个独立函数（落地），参数固定为
  `config` / `terminal` / `extractors` / `state` 四件套。

### 5. 测试代码沿用既有惯例搬到独立文件

crate 已用 `mod tests;` 把测试放在同名文件里（`render/tests.rs`、
`ghostty_terminal/tests.rs`）。内联的 `#[cfg(test)] mod xxx_tests` 按同一惯例
搬到同目录的独立文件，测试内容一字不改。

### 6. 不做的事

- 不改任何公开函数签名、返回值与错误语义。
- 不改日志级别与文案。
- 不借机重构逻辑：任何行为差异都属本变更范围之外。
- 不动 `docs/specification/`、`docs/STATUS.md`、`Cargo.toml` 等受保护文件。

## 风险与对策

| 风险 | 对策 |
| --- | --- |
| 移动代码时漏改可见性 | 以 `cargo clippy --all-targets -- -D warnings` 零告警为准 |
| 拆分后命令处理顺序变化 | 每个命令族仍按原 `match` 顺序落地；`cargo test --lib` 全绿 |
| `run_inner` 拆分引入借用错误 | 编译器为准；状态结构体只收不受借用约束的量 |
| JNI 导出名变化导致运行时 `UnsatisfiedLinkError` | 导出函数名以 `#[no_mangle]` 字面量为准，拆分不改名字；`cargo test --lib` 覆盖符号导出 |
| 新文件触发 aislop 新告警 | 每拆一个文件即跑一次 `aislop scan`，告警数只许下降 |

## 验证

- `cargo fmt --all --check` 通过。
- `cargo clippy --all-targets -- -D warnings` 零告警。
- `cargo test --lib` 全绿。
- `npx --yes aislop@latest scan` 的 Code Quality 告警归零。
- `openspec validate --all --strict` 全绿。
