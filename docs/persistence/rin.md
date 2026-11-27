# rin 研究留档

## 定位

Android 终端模拟器，Kotlin/Compose 外壳加 Rust JNI 核心，内置兼容 Termux 仓库生态的包管理器 `rpkg`。Rust 侧用 `vte` 解析字节，自己实现 `TerminalBuffer`/`Grid`/`Cell`，不是 Ghostty 也不是 alacritty。

## 架构

`TerminalEngine::write` 依次做 `parser.parse`、`buffer.execute_command`、`renderer.mark_dirty`。`AnsiParser` 用 `vte::Parser` 驱动 `Perform` 实现，产出 `Command` 列表；OSC 0/2 标题、OSC 8 超链接（解析 `id=`）、OSC 52 剪贴板会被识别，CSI 覆盖光标移动、擦除、滚动区、模式等。

`TerminalBuffer` 持有 `Grid`、光标、样式、2000 行默认回滚（`VecDeque<Vec<Cell>>`）、备用屏状态、括号粘贴、鼠标模式与待回包。`write_char` 用 `unicode_width` 判宽，零宽字符附加到前一 cell，宽字符写一个 spacer；到达右边界时按自动换行模式回卷或停在最后一列。`Grid` 有 `is_row_dirty`/`mark_row_dirty`/`has_dirty_rows` 等脏行 API。

`rpkg` 是独立 workspace 成员，负责索引解析、依赖解析、安装、查询、删除与升级，只面向 Termux 包仓库布局。

## Android 侧

`TerminalSession` 在 Rust 里创建 `Pty::spawn`，开一个线程循环读 16 KiB 缓冲喂给 engine，读完后 `drain_responses` 把设备查询应答写回 PTY。EOF 或读错误时写入 `[Process completed (EOF) - Press Enter to close]` 并保留会话。`RinLib` 暴露 `createEngine`/`createRootEngine`/`write`/`render`/`resize`/`getLine`/`getCellDataOptimized`/`isRowDirty` 等 JNI。`SessionManager` 在 Kotlin 侧保存会话，`TerminalSessionService` 用前台通知保活，退出按钮走 `destroyAll`。

绘制发生在 Kotlin：`TerminalCanvasView.onDraw` 逐行取 `getCellDataOptimized`，从打包的 `charFlags` 解出 bold/italic/dim 与前景背景色；捏合缩放由 `ScaleGestureDetector` 处理，长按菜单提供复制/粘贴/全选，`InputConnection` 把 `commitText`、`sendKeyEvent`、`deleteSurroundingText` 编码后写入 PTY。

## 已核实局限

- `Command::SetTitle(_title)`、`Command::Bell`、`ShowCursor`/`HideCursor` 都是空分支：解析器产生了命令但 buffer 不执行任何动作，标题与响铃不会到 UI。
- `AndroidRenderer::render` 遍历整屏 cell 并逐个调用 `render_cell`，而 `render_cell` 的函数体是 `let _ = (x, y, cell, is_cursor); Ok(())`：Rust 侧没有真正绘制，遍历本身是空转；真正画面来自 Kotlin 的 JNI 查询。
- 行绘制译码只有 11 个字符的线描映射，`Grid` 的脏行状态未在 Kotlin 绘制路径里被使用，`hasDirtyRows`/`isRowDirty` 接口暴露了但 `onDraw` 仍逐行取全量数据。
- 源码里留有 `// path updated`、`// moved from: src/core/buffer.rs` 等迁移痕迹，说明文件被搬动过，结构与注释尚未完全收敛。

## 与本仓对照

- 本仓规范明确“不重复实现 Ghostty 已有功能”，rin 的自研 VT 正好是反例：为了齐备终端行为需要维护整套 parser/buffer，而标题与响铃已经出现空实现。
- 若参考其会话保活与 PTY 读线程，要同时带上 `drain_responses` 的应答回写路径；本仓的设备查询应答由 Ghostty 回调与查询 API 负责，不应再建一份解析器。
- 脏行 API 暴露但绘制端不使用会造成“看起来有增量、实际全量取数”，本仓 `cell_builder` 的脏带必须与 `CachedInstances` 的复用绑在一起验证。
