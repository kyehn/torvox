# 粘贴取用上游编码并发出 bracketed 序列

## Why

本仓粘贴从不发 bracketed paste：多行剪贴板内容被 `pasteChunks` 一律换行归一为 CR，
写进 PTY 后每一行都是独立的行编辑提交。

后果有两处，都可实测：

- 在 helix/vim 等支持 bracketed paste 的 TUI 里粘贴多行文本，每行立即被当作
  提交执行——`pasteChunks` 把 LF 换成 CR 正是「立即提交」的编码，不是缺陷。
- 在不支持 bracketed paste 的传统 shell 里，裸换行本就该归一为 CR，否则多行
  粘成一行。两种应用要的行为相反，本仓只实现了后者。

`modifier-bar-sticky-encoding` 已写明该缺口：「真正实现 bracketed paste；
本 MUST NOT 被解读为多行提交已修复」。本变更闭合粘贴路径这一半——输入法提交
路径不经粘贴编码，行为不变，该声明据此精确化而非删除。

## What Changes

- 新增 `Query::EncodePaste`：在 VT 线程（与按键、鼠标编码同属终端状态感知路径）
  按 `mode_get(2004, 0)` 取 bracketed 标志，经上游 `paste::encode` 产出完整编码流。
- `PasteChunks.kt` 删换行归一（`\r\n`→`\r`、`\n`→`\r` 两条替换）：归一职责移入
  上游编码器，且归一与否由 bracketed 决定。保留截断与切块，切块点仍须避开
  多字节字符边界。
- 包裹序列只出现在整体首尾，故切块 MUST NOT 把 `ESC [ 200~` 与 `ESC [ 201~`
  拆开：包裹由上游一次性产出，Kotlin 侧只在首块之前与末块之后保持原位切分。
- 缓冲区不足时按上游返回的 `OutOfSpace { required }` 精确扩容重试，不截断。

### 裁定：取 `paste::encode`，不取 `Terminal::paste`

上游提供两个入口，本变更只取前者：

- `Terminal::paste` 是与终端状态耦合的高阶入口。含换行或 bracketed 终止序列的
  文本会被判为 `Error::Rejected`，要求嵌入方弹确认框后以
  `with_allow_unsafe(true)` 重试。`PROHIBITED.md` 明列「粘贴确认对话框」为禁止
  实现，故该流程在本仓不可用；而绕过它的唯一途径是恒置 `allow_unsafe`，
  届时安全检查只剩开销、不再有作用。其 kitty 粘贴事件能力另需
  `on_clipboard_read` 回调，本仓零消费者。
- `paste::encode` 是无终端依赖的纯函数：给定数据与 bracketed 标志产出编码字节。
  安全控制字节中化（控制字节换空格、bracketed 终止序列换空格）全部由上游承担，
  且不需要任何确认流程。

这不是妥协：`Terminal::paste` 的额外能力属「按需取用」，`upstream-alignment`
已登记该模式。

### 上游编码实测（本轮探针）

| 输入 | `bracketed=false` | `bracketed=true` |
| --- | --- | --- |
| `hello` | `hello` | `ESC [ 200~hello ESC [ 201~` |
| `a` LF `b` | `a` CR `b` | `ESC [ 200~a` LF `b` ESC [ 201~` |
| `a` NUL `b` | `a b`（空格） | 同左 |
| `a` ESC `b` | `a b`（空格） | 同左 |
| `a` DEL `b` | `a b`（空格） | 同左 |
| `a` `ESC[201~` `b` | `a b` | `ESC [ 200~a [201~b` ESC [ 201~` |
| `中文` | `中文` | `ESC [ 200~中文 ESC [ 201~` |

`is_safe`：`hello` 真，`a` LF `b` 假，`a` `ESC[201~` `b` 假，`a` CR `b` 真。

## Capabilities

### New Capabilities

- `paste-encoding`: 粘贴编码的归属、bracketed 判定、控制字节安全与切块边界

### Modified Capabilities

- `upstream-alignment`: `Terminal::paste` 由「待办」改为「不采用」（`PROHIBITED`
  依据），新增 `paste::encode` 采用与 `paste::is_safe` 不采用两行
- `modifier-bar-sticky-encoding`: bracketed 缺口声明精确化——粘贴路径已实现，
  输入法提交路径不受影响

## Non-goals

- 不实现粘贴确认对话框（`PROHIBITED` 明禁）。
- 不接 kitty 粘贴事件与 `on_clipboard_read`：无消费者。
- 不改输入法提交路径的 CR 归一：它不经粘贴编码。

## Impact

- native：`terminal/ghostty_terminal/{commands,internal,public_api}.rs`、
  `android/ffi.rs`
- Kotlin：`runtime/PasteChunks.kt`（删归一，留截断与切块）、`TerminalViewModel.kt`
  的 `executePaste`
- 测试：`PasteChunks` 单测的归一期望改由 native 侧承载，Kotlin 只留截断与切块断言
