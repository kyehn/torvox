# 深度研究：rin（Rust 终端）

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/rin`（depth 1）
> 语言：Rust（1321 行）+ Android（Kotlin）；edition 2024
> 定位：简洁 Rust 终端引擎（vte crate + 自研 grid/renderer + JNI 直接导出）——小型化参考

## 1. 架构

```
src/
├── lib.rs              # TerminalEngine（buffer + parser + renderer 组合）
├── core/               # grid.rs / cell.rs / buffer/（screen+alternate+cursor）
├── parser/             # ansi.rs（vte::Perform 实现）
├── renderer/           # screen.rs（RenderContext + Renderer trait）
├── input/              # handler.rs（Key/KeyEvent/Modifiers）
├── pty.rs              # Pty 封装（portable-pty）
└── platform/android/   # jni.rs + session.rs
```

## 2. TerminalEngine（lib.rs:15-60）

```rust
pub struct TerminalEngine {
    buffer: TerminalBuffer,
    parser: AnsiParser,
    renderer: Box<dyn Renderer + Send>,
    width: usize, height: usize,
}
// write: parse → buffer.execute_command → renderer.mark_dirty()
// render: renderer.render(RenderContext { buffer, width, height })
// resize: buffer.resize
```

- `Renderer` trait（renderer/mod.rs:5-13）：`render(&mut self, context)` + `mark_dirty()`——渲染器抽象
- 用 `vte` crate（0.15）解析，自研 `AnsiPerformer`（print/execute/hook/put/unhook）

**对比 torvox**：torvox 用 libghostty-vt（比 vte 完整得多）。rin 的 `TerminalEngine` 组合模式（buffer+parser+renderer）与 torvox 的 `GhosttyTerminal` 类似但简单一个数量级。**Renderer trait + RenderContext** 模式值得注意（torvox 的 render 直接操作 RenderState，无 trait 抽象——但 torvox 只有 1 个渲染器，YAGNI）。

## 3. JNI（platform/android/jni.rs）

```rust
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rin_RinLib_createEngine(...)
// createRootEngine / destroyEngine / write / writeToEngine
// render / resize / getLine / getCursorX / getCursorY
```

**对比 torvox**：torvox 的 `ffi.rs` JNI 导出（17 个函数）结构相同。rin 用 `jni 0.22`，torvox 用 `jni 0.21`（版本差异无实质）。

## 4. PTY（pty.rs + platform/android/session.rs）

- `Pty::spawn`（portable-pty）：`Arc<Mutex<Pty>>` 共享
- `TerminalSession::new`：spawn 线程读取 PTY → 回调（responses 收集 → write 回写）
- 注意：`lock().unwrap()` 在 session.rs 使用（poison 直接 panic）——torvox 用 `lock_or_recover` 更稳健

**对比 torvox**：torvox 的 PTY（自研 fork/exec + linker64）比 portable-pty 更可控（SELinux 方案）。rin 的 reader-thread + response-write 模式与 torvox 的 PTY Reader 线程相同。

## 5. 渲染（renderer/screen.rs）

`ScreenRenderer` 实现 `Renderer`：从 buffer 遍历 cell → 绘制（具体绘制在 Android 侧 Java/Kotlin）。

## 6. 依赖

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| vte 0.15 | VT 解析 | 不适用（libghostty-vt） |
| portable-pty | PTY | 不适用（自研 + linker64 方案） |
| unicode-width | 宽度 | torvox 用 ghostty 的 is_wide |
| jni 0.22 | JNI | 已用（0.21） |
| android_logger | 日志 | torvox 自研 logging.rs |

## 7. 结论

rin 是最小化参考：证明自研终端引擎 + JNI 的可行结构，但 torvox 的 libghostty-vt 在功能完整性上全面超越。参考价值限于 `TerminalEngine` 组合模式和 Renderer trait 设计（可选）。

## 4. deep-v2 增量（亲自复核 platform/android/）

### 4.1 jni.rs（525 行，结构亲读）

- 会话注册表：`OnceLock<Arc<RwLock<HashMap<EngineHandle, TerminalSession>>>>`（:44-49）＋ `NEXT_HANDLE: AtomicI64`（:46）——与 torvox `ffi.rs` SessionRegistry 同构；torvox 更强（不要求全 JNI 走 EnvUnowned）。
- `get_jstring`（:61-68）：`env.with_env(...).resolve::<ThrowRuntimeExAndDefault>()` —— **异常规范化模式**（所有 JNI 错误统一抛 `java/lang/RuntimeException`，Kotlin 侧检查）。torvox ffi.rs 手动 throw——rin 的 `.resolve` 链更简洁，可参考。
- `create_banner`（:86-136）：启动横幅（root/非 root 区分 + `ROOT SESSION` 红字）——torvox 无启动横幅（可 P3）。

### 4.2 session.rs（129 行，全读）

- 单 reader 线程 + `drain_responses()`（:88）：**读线程把引擎产生的响应（OSC/回显）写回 PTY**——torvox 的 `process_output` 等已覆盖同功能，但 drain 语义清晰。
- `[Process completed (EOF) - Press Enter to close]`（:59/:78）——**终端 UX 提示**（TerminalSession.java:297-315 同类）；torvx 会话退出目前只发 Exit 事件，无屏上提示。

### 4.3 对 torvox 的落点

- 无新代码注释需求（架构同构、价值已在既有文档 §3 记录）；本条记录"Process completed 提示"为 P2 UX 候选。

## 5. 完成状态

- [x] lib.rs TerminalEngine（既有 §2）
- [x] jni.rs（深读 1-120 + 结构）
- [x] session.rs（全读 129）
- [ ] pty.rs（portable-pty，torvox 自研歧义——低价值可跳过）

## 6. deep-v3 增量（复核第 1 轮）

### 6.1 pty.rs（87 行全读）

- `portable_pty` crate 封装（`native_pty_system().openpty(size)`）；`PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }`——**像素尺寸传 0**（与 zelland CLAUDE_NOTES 指出的同款问题：Ghostty 拿不到像素尺寸则鼠标坐标/行高不准）。torvox 的 PtyPair 用 nix 自研 + 真实像素尺寸（resize 链传 cell 尺寸），**更优，不吸收 portable-pty**。
- `cmd.env("ENV", home/.mkshrc)`（:35-38）：**mksh 启动时 source 用户 rc 的机制**（ENV 变量）。torvox ShellEnv 未设 ENV——若支持 mksh 用户需此（P2 记录）。
- `take_reader`/`take_writer` 分离（:50-55）：与 torvox PtyPair reader/writer 同构。

### 6.2 复核结论

- rin pty.rs 无新代码落点；ENV/mkshrc 记为 P2 UX 候选。
