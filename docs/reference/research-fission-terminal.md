# 深度研究：fission 终端 widget — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：https://github.com/fission-ui/fission
> 前置：`research-fission.md`（框架整体，子代理）；本文补充 **examples/terminal/src/lib.rs 296 行全 + crates/authoring/fission-widgets/src/terminal.rs 1444 行核心**

## 1. 终端示例：`examples/terminal/src/lib.rs`（296 行，完整阅读）

### 1.1 架构（:34-52）

```rust
pub struct TerminalExampleState {
    cwd: PathBuf,
    session: Option<Arc<TerminalSession>>,
    redraw_epoch: u64,
}
```

- `TerminalSession::spawn(TerminalLaunchConfig { cwd, program: SHELL, .. })`（:48-51）——**portable_pty 启动真实 shell**
- **wasm32 降级**（:38-46）：`struct TerminalSession` 桩（take_dirty 返回 false）——浏览器无法跑本地进程

### 1.2 Action/Reducer 模式（:60-110）

- `StartTerminal` / `PollTerminal` / `PollTerminalTick` 三个 Action
- `poll_terminal`（:96-110）：**16ms 定时器轮询 `session.take_dirty()`** → dirty 时 `redraw_epoch += 1`（触发重绘）——**定时轮询脏标志驱动重绘**（fission 无 push 机制）
- `ctx.with_resources(|resources| resources.timer(TimerResource::new("terminal-session-poll", 16ms, ...)))`（:127-137）

**torvox 对比**：torvox 用 notifyRender/unpark（push 式唤醒渲染线程），比 fission 的 16ms 轮询更高效（无轮询空转）。fission 的模式适合其声明式框架。

### 1.3 标题格式化（:263-296）

`format_terminal_title`：trim → 路径取 `.../父/名` → 超 28 字符尾部截断（`...` + 后 25 字符）。

### 1.4 run_desktop（:298-306）

`DesktopApp::<TerminalExampleState, _>::new(...).with_startup_action(StartTerminal)`——桌面入口。

## 2. TerminalView 实现：`crates/authoring/fission-widgets/src/terminal.rs`（1444 行，核心精读）

### 2.1 技术栈（:14）

```rust
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use wezterm_term::{...};  // wezterm 的终端状态机
```

**wezterm_term + portable_pty**——与 torvox 的 libghostty-vt 同类（完整 VT 状态机），torvox 的 ghostty 更现代（kitty graphics）。

### 2.2 TerminalSessionInner（:65-79）

```rust
struct TerminalSessionInner {
    id: u64,
    terminal: Mutex<WezTerminal>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    dirty: AtomicBool,
    focused: AtomicBool,
    scrollback_offset: AtomicUsize,
    cols: AtomicUsize, rows: AtomicUsize,
    exited: AtomicBool,
    selection: Mutex<Option<TerminalSelection>>,
    selection_drag_active: AtomicBool,
}
```

**原子状态标志集合**（dirty/focused/scrollback_offset/cols/rows/exited）——与 torvox SessionEntry 的 @Volatile 标志等价。Drop 杀 child（:81-87）。

### 2.3 TerminalView 构建器（:136-174）

`new(session, w, h)` + `font_size`/`line_height`/`padding` builder——**声明式配置**。

### 2.4 渲染节点（:233-288）

- `TerminalSelection::normalized`（:251-262）：端点规范化（start<=end）
- `range_for_row`（:269-287）：行级选区范围
- `TerminalRenderNode::new`（:288）：渲染节点
- `cursor_rect`（:312）：光标矩形
- `point_to_cell`（:337）：像素→单元格
- `selection_range_for_row`（:353）：行级选区
- `style_for_cell`（:359）：**wezterm CellAttributes → TerminalRunStyle** 映射

**torvox 对比**：torvox 的 cell_builder.rs 有等价（SelectionRange::contains、像素→网格、样式映射）。结构相似，实现不同（torvox GPU 实例化 vs fission DisplayList）。

## 3. 与 torvox 功能对比总表

| 功能 | fission | torvox | 结论 |
|------|---------|--------|------|
| VT 状态机 | wezterm_term | libghostty-vt | torvox 更现代 |
| PTY | portable_pty | 自研 fork/exec + linker64 | torvox 更可控（SELinux） |
| 渲染 | DisplayList → vello/软件 | wgpu 实例化 | 架构不同 |
| 脏检测 | 16ms 定时轮询 take_dirty | notifyRender/unpark push | **torvox 更优** |
| 选区 | TerminalSelection 规范化 | SelectionRange | 等价 |
| 原子标志 | AtomicBool 集合 | @Volatile + Atomic 集合 | 等价 |
| 配置 | builder 模式 | DataStore 响应式 | 等价 |

## 4. 结论

fission 的终端 widget 是 wezterm_term + portable_pty 的封装（1444 行），架构与 torvox 的 libghostty-vt + 自研 PTY 同类但技术选型不同。**无直接移植价值**（torvox 选型更优），但其**声明式 Action/Reducer 模式**（StartTerminal/PollTerminal + 16ms 定时器）和 **builder 配置**是框架级参考。16ms 轮询 vs push 唤醒的对比确认 torvox 渲染唤醒架构更高效。
