# Rex 协议支持评估与方法步骤

## 摘要

本报告回答两个问题：本项目支持 Rex 协议是否必要；若要支持，按什么步骤做。

结论先行：

- **当前不立项**。`docs/specification/DESIGN.md` 未声明远程／网络终端形态，`STYLE.md` 规定「不实现未在
  `docs/specification/` 声明的功能／逻辑」；`openspec/specs/upstream-alignment/spec.md` 的「能力裁定补齐行
  台账」已把「远程会话」与「WebSocket／网络终端形态」两行判为「未声明，不立项」。Rex 支持落在同一裁定面，
  其技术可行性不改变裁定结果。
- **技术上可行且成本可控**，前提是走「字节流 + 本地 VT」路线而不是「服务端渲染 surface」路线：Rex 数据面
  送的是**终端字节**，服务端快照用的是 Ghostty 自己的 `GHOSTSNP` 格式，与本仓上游 `libghostty-vt` 的快照
  解码器同源，因此本仓可以「把快照解成本地终端状态 + 后续字节继续喂本地 VT」，不引入第二个 VT 核。
- **唯一不可回避的前置是本仓接缝**：`native/src/terminal/pty.rs` 的 `Pty` trait 把字节读写与本地 OS 能力
  （`child_pid`、`get_winsize`、`master_borrowed_fd`、`try_clone_reader_fd`、`set_nonblocking`）混在一起；
  `Session::spawn_with_pty` 与 `PtyMaster::new` 的可见性是 `pub(super)`；`Session::request_exit` 走本地进程组
  信号路径。Rex 会话没有本机 pid，也没有 ioctl 可调，必须先把「字节源」与「生命周期」从「本地进程」里剥出来。
- **方法步骤按六个阶段推进**：规范声明 → 会话传输面接缝 → Rex 协议层（控制面／数据面）→ 快照恢复 →
  输入与尺寸与所有权 → 退出、错误与重连 → 测试与门禁。协议真值必须落在 Rust 侧（对照 rexling 的
  `rexling-core/jni` 与 rexling-android 的 `NativeBridge` 形态），不得放到 Kotlin 侧。

## 研究方法与坐标

原则：只用一手资料。对外部项目直接克隆仓库并阅读源码、协议手册与探针代码，不引用项目自述以外的二手描述；
对本仓结论回到本仓源码与 `docs/specification/` 规范核对。

| 外部仓库 | 研究坐标 | 形态 |
| --- | --- | --- |
| `shulhi/rexling` | master 快照（无发布标签） | Rust 协议核心（`rexling-core`）+ Android 壳（`rexling-android`）+ 端到端探针（`rexling-core/probe`） |
| `leizhiyuan/rexpocket` | `v0.1.3` | Android 远程 shell 客户端（`core`／`app` 分层 + Ghostty JNI） |

本仓约定文档不写 git 提交号，故上表用标签或快照标识研究基线。

一手资料的三处关键落点：

- 协议手册：`rexling-core/probe/evidence/server-wire-guide-r8.md`，即服务端自带的 `/llms.txt` 正文，描述
  **API revision 8**。
- 协议实现：`rexling-core/src/transport.rs`、`control.rs`、`data.rs`、`snapshot.rs`、`ghostty.rs`。
- 端到端行为：`rexling-core/probe/src/main.rs` 的探针流程（建会话 → attach → own_block → resize →
  写字节 → 数据面断点续传 → 控制面换连接续用）。

## Rex 协议一手事实

### 服务端与端点

- Rex 服务端运行终端并让它们持续运行，客户端只是连接上去显示与控制。四类对象：**session**（一组相关工作）、
  **window**（session 内的标签页）、**block**（窗口内的一个应用，今天就是终端）、**client**（连接进来的程序）。
- 端点：`GET /api/control`（控制连接）、`GET /api/data`（数据连接，每个连接流式送一个 block 的输出）、
  `GET /api/help`（全部方法的 JSON Schema）、`GET /llms.txt`（协议手册 Markdown）。
- 传输统一是 WebSocket，子协议头分别是 `rex.control.v1` 与 `rex.data.terminal.v1`。
  `rexling-core/src/transport.rs` 把这两个常量命名为 `CONTROL` 与 `TERMINAL`，并把单消息上限固定为
  1 MiB（`MAX_MESSAGE`）；服务端地址支持 `unix`／`http`／`https`／`ws`／`wss`／`ssh` 六种 scheme。
- 服务端每 120 秒 ping 一次客户端，每次等 60 秒应答，连续 3 次无应答即以 1001 `keepalive timeout` 关闭连接。

### 控制面

- 首帧必须是 `hello`，服务端回 `welcome`。`hello` 携带 `api_revision`（客户端能说的最新修订）与
  `api_min_revision`（客户端能用的最旧修订），服务端取双方交集里最新的一个，并在 `welcome.api_revision`
  里公布；此后该连接上的所有消息都按这个修订的形状走。协议手册描述 revision 8，服务端支持 0–8 并为旧客户端
  做翻译。`rexling-core/src/control.rs` 的 `Control::connect_revision` 实际发送 `api_revision: 8`、
  `api_min_revision: 7`。
- `hello` 整体须小于 512 字节，且不得压缩（它可能带 `reconnect_token`）。服务端 5 秒内没收到 `hello` 就直接
  断连（客户端看到 1006）。
- 客户端发 `request`（`id`／`method`／`payload`），服务端回 `response`（同一 `id`，带 `payload` 或 `error`）。
  响应里的 `seq` 是「服务端开始处理该请求前，该连接已排队的最后一个事件序号」；序号不大于该 `seq` 的事件其
  结果里已经包含，客户端要把结果与事件合并时据此去重。
- 事件从 1 开始连续编号、无空洞。客户端落后超过 8 MiB 事件积压时服务端以 1008 `event backlog exceeded`
  关闭连接，要求重连后重新读状态。
- 有一组方法是**保序执行**的（`block.close`、`block.event`、`client.key`、`session.focus_block` 等），同一连接
  上按到达顺序一个个跑完；其余方法（含 `com.superlogical.terminal.write`）可并发执行。协议手册明确要求：
  要连打两次输入，必须等第一次的响应再发第二次，或把文本合成一次发。
- 错误按 `code` 分支，不按 `message`：`bad_request`(400)、`not_found`(404)、`method_not_found`(404)、
  `session_not_found`(404)、`conflict`(409)、`not_supported`(400)、`refused`(403)、`client_unavailable`(409)、
  `too_large`(413)、`revision_too_low`(400)、`timeout`(504)、`internal`(500)。未知 `code` 按 `status` 处理。
- 连接断开且没有 close 帧时，服务端为该客户端保留 60 秒。客户端用上次 `welcome.client.reconnect_token` 在新
  连接的 `hello` 里续用；服务端接受则新 `welcome.client.resumed` 为 true 并换发新 token，旧 token 立即失效。
  **断线期间发生的事件会丢失，`seq` 从 1 重新开始**，客户端必须重新读状态（如 `session.view`）再合并事件；
  断线时仍在跑的请求其结果也丢失，但服务端可能已经执行过，客户端必须先查状态再决定是否重发。
- 挂接：`session.attach`（挂到一个 session 并取回当前视图）、`session.attach_all`（挂到全服并取 session 列表）、
  `session.bootstrap`（一次调用完成 attach_all + attach + 取服务端状态）。**只有 `session.attach` 或
  `session.bootstrap` 能开数据连接、claim／释放 block**；只 `attach_all` 不行。

### 数据面

- 一个数据连接承载一条**流**：一个 block 产出的字节。流可以比连接活得久；连接掉了可以换连接续上。
- 开连接的顺序是：先在控制面挂接 session → 调 `data.connect` 取一次性票据 → 用同一子协议开
  `GET /api/data` → 5 秒内发二进制 `HELLO` → 服务端回 `WELCOME`（或 `REJECT` 后关闭）。
- 票据是 16 字节，写成 22 个 base64url 字符（无填充）；一次性、60 秒有效、只能由与控制连接同一用户的连接使用。
- 握手是定长二进制（整数一律小端）：

  | 消息 | 长度 | 布局 |
  | --- | --- | --- |
  | HELLO | 17 字节 | `0x01` + 16 字节票据 |
  | RESUME | 25 字节 | `0x02` + 16 字节 reconnect token + 已收流字节数（u64） |
  | WELCOME | 32 字节 | `0x81` + 16 字节 reconnect token + 3 字节保留 + 保留时长毫秒（u32）+ channel nonce（u64，非零） |
  | REJECT | 2 字节 | `0x82` + 原因码 |

- REJECT 原因码：`0x01` 票据或 token 无效／过期／已用／属他人／session 已不存在；`0x02` 该客户端已有一条同
  数据源的连接（旧连接掉了要发 RESUME 顶替）；`0x03` 流不可用（block 已关，或 RESUME 计数少于服务端已发字节）；
  `0x04` 会越过服务端限额；`0x05` 握手畸形或超时，或 RESUME 计数超过服务端已发字节；`0x06` 服务端故障；
  `0x07` 票据与子协议不匹配。`HELLO` 即使被拒也消耗票据，重试前必须重新取票；RESUME 被拒后只能退回 HELLO 重来。
- WELCOME 之后，服务端每条二进制消息带 1–65536 字节流内容，**逐条拼接即为字节流，消息边界无语义**。
  服务端忽略 WELCOME 之后客户端发来的任何消息——**数据面是单向的**。
- 续传：连接掉了，服务端为该流保留 60 秒并持有其输出。客户端在新连接上发 RESUME（token + 已收字节数），
  服务端回 WELCOME（新 token、同一个 nonce），并从客户端尚未收到的第一个字节续发。**计数必须精确**：要数
  客户端代码真正用掉的每一个流字节，跨所有连接累计；WELCOME 不计入，WebSocket 库读超前但代码未用的字节也
  不计入。服务端保留时长由 WELCOME 的 `resume_ms` 给出。
- 关闭语义：1000 表示流结束；1013 `data reconnect required` 要换新票据重新开流；4000 `block closed` 不得重连；
  1001 `keepalive timeout` 要续传；1008 `data provider unavailable` 与 1011 `data provider failed` 是服务端侧
  问题；握手被拒时以 1002／1008／1011／1013 加 `data handshake rejected` 关闭。

### 终端数据流内容

- 终端流**以服务端终端的 Ghostty 快照开头**（`GHOSTSNP` 格式），之后是终端程序写出的全部字节，其中夹着 Rex
  自己的消息。Rex 消息是 APC 转义序列：`ESC _ rex1;<nonce>;<json> ESC \`，`<nonce>` 是 WELCOME 给的
  channel nonce 的十进制。**nonce 不匹配的 APC 是程序自己打印的，必须原样喂给终端**；nonce 匹配的三类消息是
  客户端自己的业务，要摘出来：`resize`（`cols`／`rows`，另有 `cell_width_px`／`cell_height_px`，`owner` 为置尺寸的
  客户端 id 或 null）、`theme`（`foreground`／`background`／`cursor` 为 `RRGGBB` 或 `#RRGGBB`，`palette` 为
  `index=RRGGBB` 列表，`scheme` 为 `light`／`dark`）、`child_exited`（程序退出，`exit_code` 为退出码；之后不再有
  程序输出，但流仍然开着）。未知 `type` 跳过。
- 通道宽度与高度的协商在控制面：`com.superlogical.terminal.resize` 的参数名是 `columns` 与 `rows`（注意与 APC
  负载里的 `cols`／`rows` 不同名），返回时用 `com.superlogical.terminal.size` 读回。**尺寸与配色归「持有者」**：
  `session.own_block` 让本客户端成为 block 的 owner，「终端在被持有期间采用该客户端给的尺寸与配色」；协议手册
  写明 resize 在「调用方持有该 block 或无人持有」时立即生效，否则要等调用方成为持有者时才生效。探针
  `rexling-core/probe/src/main.rs` 的实测顺序正是 `session.own_block`（`mode: "take"`）→ `resize` → `size` 读回。
- 写入终端用 `com.superlogical.terminal.write`（参数 `data`，服务端入队即返回，不等程序读），键与事件另有
  `client.key`（保序，且先给 Lua 键绑定机会）与 `block.event`。

### 快照格式

- 服务端快照用的是 Ghostty 自己的快照格式，magic 是 `GHOSTSNP`，其后是 2 字节版本号（当前为 1），再其后是
  记录流：每条记录 10 字节头（tag u16 + 长度 u32 + 4 字节保留）加负载。tag 取值里 5 是 READY、6 是 FINISH、
  3 是历史页、4 是历史清单、10 是 Kitty 图形放置。
- `rexling-core/src/snapshot.rs` 的 `SnapshotBuffer` 把这条记录流按「完整记录」切分：先等 READY 出现，再逐条
  记录推进，直到 FINISH；PAGE 之后还要预读下一条记录头，因为 Ghostty 读超前以便把 Kitty 放置挂到该页上。该
  缓冲有 128 MiB 上限。
- 关键约束写在 `rexling-core/src/snapshot.rs` 的模块注释里：**Ghostty 会自己校验记录顺序、内容与 CRC，而它的
  同步读取器把「临时读不到数据」当成致命 EOF**。所以网络流不能直接喂给 Ghostty 的解码器，必须先在客户端把
  字节攒成完整记录再喂。

### 退出与所有权

- 程序退出由数据流的 `child_exited` APC 消息告知，带退出码；`com.superlogical.terminal.process` 可读进程列表
  与程序退出方式；`block.close` 关掉 block 并结束其中程序。
- `session.own_block`／`session.disown_block` 管理 block 持有者，`session.describe_block` 可读 block 详情。

## 与本仓现状的差异

### 已有能力

- 本仓 VT、渲染、JNI 三层与传输无关：`native/src/terminal/ghostty_terminal/` 负责把字节喂给上游 Ghostty，
  `native/src/render/` 负责 wgpu 绘制，`native/src/android/ffi/` 只做轻量事件与查询。字节进入 VT 的路径是
  「PTY 读取线程 → 有界通道 → `Session::process_output` → VT」，背压单槽的契约见
  `openspec/specs/pty-output-backpressure/spec.md`。
- 本仓上游 `libghostty-vt` 已经带快照解码面：`crates/libghostty-vt/src/snapshot.rs` 暴露
  `Terminal::encode_snapshot`、`Decoder::new_buf`／`Decoder::ready`／`IncrementalDecoder::next`，
  `Decoder::source_offset` 用于定位 FINISH 之后的字节。**本仓目前未消费这套快照接口**（`upstream-alignment`
  的「上游 API 取用判定登记」表未列该族）。
- Kotlin 侧写入与渲染本来就是两条独立通道：`TerminalViewModel.writeToPty` → `TerminalRuntime.writeToPty` →
  `Bridge.writeToPty` → `NativeBridge.feedPty`；输出侧是 `RenderSupervisor` → `Bridge.waitOutput` →
  `Bridge.renderWithNewOutput`。远程后端不要求改 Kotlin 的数据通路。

### 缺口

| 缺口 | 本仓现状 | Rex 侧要求 |
| --- | --- | --- |
| 字节源形态 | `Pty` trait 同时管字节读写与本地 OS 能力（`child_pid`、`get_winsize`、`master_borrowed_fd`、`try_clone_reader_fd`、`set_nonblocking`） | 远程会话没有本机 pid、没有 ioctl、没有 fd |
| 注入可见性 | `Session::spawn_with_pty` 与 `PtyMaster::new` 均为 `pub(super)` | 远程后端要能在仓内挂上，须提到 `pub(crate)` |
| 生命周期 | `Session::request_exit` 走本机进程组的挂断→唤醒→宽限→强杀；退出码由 `take_reported_exit` 取出 | 远程退出是 `block.close` 调用 + 数据流 `child_exited` 事件 |
| 尺寸 | `PtyMaster::resize` 走 `TIOCSWINSZ` | 远程尺寸是控制面 `com.superlogical.terminal.resize`，且受 block 持有者规则约束 |
| 输入字节序 | 本仓 `PtyMaster::write_with_timeout` 用写入锁保序 | Rex 的 `com.superlogical.terminal.write` 不保序，客户端必须自己串行化写入 |
| 快照解码 | 未接入上游快照解码面 | 流首是 `GHOSTSNP` 快照，必须先解成本地终端状态再继续喂活字节 |
| 协议真值位置 | 本仓会话状态归 Rust | 与之相容：协议状态也应归 Rust，Kotlin 只做展示与平台适配 |

## 必要性评估

### 裁定

**当前不立项**，理由按证据强度排序：

1. `docs/specification/DESIGN.md` 的终端节只声明「本机 Shell + 本机 PTY」形态，没有任何远程／网络终端条目；
   `STYLE.md` 的通用节写明「不实现未在 `docs/specification/` 声明的功能／逻辑」。
2. `openspec/specs/upstream-alignment/spec.md` 的「能力裁定补齐行台账」已有两行直接命中本议题：
   「远程会话（设备配对、隧道、移动端控制）」判为「未声明，不立项」；「WebSocket／网络终端形态（服务端拆分、
   DOM 单元布局、Electron/Qt 集成、WebView 远程终端）」同样判为「未声明，不立项」。Rex 客户端正是 WebSocket
   远程终端形态。
3. `docs/specification/PROHIBITED.md` 未点名 Rex，但「会话数据持久化 / 恢复」是明禁项。Rex 数据面的 resume 是
   「服务端保留 60 秒 + 客户端按已收字节计数续传」，本身不是客户端侧的会话持久化；但只要产品要求「应用被杀后
   重开仍能接回同一个远端会话」，就需要在客户端保存 reconnect token 与会话标识，那才撞明禁。这条边界必须由
   维护者在声明阶段划清，不能由实现阶段默认。

因此结论是「技术可行，但须先有产品声明」，而不是「不可行」。

### 价值

若产品决定立项，Rex 对本项目形态的增量价值是明确的：

- **终端跑在服务端**：本仓当前的能力（bootstrap、Termux 兼容 shell、Termux 风格的会话与修饰键栏）仍然是本地
  形态；Rex 补的是「用手机连到自己机器上的长驻终端」这一形态，这是本仓现有形态完全覆盖不到的场景。
- **快照格式同源**：服务端送来的 `GHOSTSNP` 快照正是本仓上游 Ghostty 的格式，本仓可以用上游解码器把屏幕直接
  恢复成本地终端状态，不需要自研屏幕同步，也不需要服务端渲染 surface。
- **会话在服务端持续**：与本仓「进程被回收则 Rust 状态与 PTY 进程一起消失」的现状形成互补——远端会话不随手机
  应用生命周期消失。

同时要承认三项成本与风险：

- **两层 VT**：服务端也有一个终端实现，客户端再解析一遍字节。本仓作为客户端不冲突，但服务端与客户端的
  颜色、尺寸、模式（如 alternate screen、kitty 图形）必须对齐，否则屏幕上会出现两套状态打架。
- **协议面不小**：控制面有几十个方法与事件分类，Rex 手册本身就在服务端 `/llms.txt` 上逐条列出；只取本仓需要
  的子集也需要持续跟随 API revision。
- **网络依赖引入新的失败面**：断线、重连、事件丢失、字节计数失配、票据过期，这些在本仓现有的本地 PTY 形态下
  完全不存在。

### 前提

立项必须同时满足：

- 维护者在 `docs/specification/DESIGN.md` 声明远程会话形态（该文件属受保护区，须明确同意后才能改）。
- 维护者划清与 `PROHIBITED.md`「会话数据持久化 / 恢复」的边界：明确客户端是否允许保存 reconnect token 与
  会话标识。
- 维护者选定形态：只做「Rex 客户端」，还是同时做「Rex 服务端」。本仓是 Android 终端模拟器，做服务端不现实，
  因此本报告按「只做客户端」设计方法步骤。

## 方法步骤

### 声明远程会话形态

- 改动面：`docs/specification/DESIGN.md`（受保护文件，须维护者明确同意）。
- 内容：声明「远程会话」形态、协议范围（只做 Rex 客户端）、会话标识的来源与生命周期、断线后的用户可见行为、
  以及与 `PROHIBITED.md` 的边界。
- 判据：声明写完后，`STYLE.md` 的「不实现未声明功能」不再是阻塞；`upstream-alignment` 的「能力裁定补齐行
  台账」中「远程会话」与「WebSocket／网络终端形态」两行须同步改判并写明依据。
- 阻塞点：未获同意前，本阶段之后的所有阶段都不成立。

### 抽象会话传输面

- 改动面：`native/src/terminal/pty.rs`（拆字节读写与本地 OS 能力）、`native/src/terminal/session/spawn.rs`
  （读取线程泛化为「字节源」）、`native/src/terminal/session/pty_master.rs`（注入点提到 `pub(crate)`）、
  `native/src/terminal/session/lifecycle.rs`（生命周期改为「请求退出 + 取退出码」两组契约）。
- 形态：字节源契约只保留「写字节、读字节、协商尺寸」三件事；本地进程组信号、`TIOCSWINSZ`、fd 克隆、
  `child_pid` 留在本地实现侧，不进契约。生命周期契约只保留「请求退出」与「取退出码」，本地实现侧继续做挂断→
  唤醒→宽限→强杀，远程实现侧改成控制面调用。
- 判据：抽象完成后，本地 PTY 仍走同一条字节→VT 管线，背压语义逐字节不变
  （`openspec/specs/pty-output-backpressure/spec.md` 不受影响）；远程字节源能以同一个 `Session` 构造路径注入。
- 注意：`PtyMaster` 的写入锁是为「同一条 PTY 的字节序」设计的，远程字节源同样需要它——输入顺序由协议自己
  不保证，串行化必须留在客户端。

### 建立 Rex 协议层

- 改动面：新增 `native/src/terminal/rex/`（协议层）与 `native/src/terminal/rex/transport.rs`（连接实现），
  依赖方向为「协议层 → 传输实现 → 字节源契约」。
- 控制面：实现 `hello`／`welcome` 协商（送 `api_revision` 与 `api_min_revision`，按 `welcome.api_revision`
  校准）、`request`／`response` 配对、`event` 的 `seq` 连续性检查（出现空洞即按协议手册要求「重读状态」）、
  错误按 `code` 分支、`pending` 请求表有上限（`rexling-core/src/control.rs` 的上限是 32，事件队列上限 4096）。
- 数据面：实现 `data.connect` 取票 → `HELLO` → `WELCOME` → 逐条拼接字节流 → 断线后按已收字节计数 `RESUME`。
- 判据：控制面与数据面互相独立——控制面断开不影响已在跑的数据流，数据面断开不影响控制面请求
  （协议手册对此有明确描述：续传不需要票据也不需要控制连接）。
- 依赖：传输实现需要 WebSocket 客户端与 TLS。当前本仓无 WebSocket 依赖，`Cargo.toml` 属受保护文件，须取得
  明确同意后新增；这是本阶段唯一的受保护文件改动点。

### 接入快照恢复

- 改动面：`native/src/terminal/rex/` 内的快照缓冲，以及 `native/src/terminal/ghostty_terminal/` 接入上游快照
  解码面。
- 顺序：数据流的第一段字节是 `GHOSTSNP` 快照 → 客户端按完整记录切分（magic + 版本 + 10 字节记录头）→
  完整记录喂给上游 `Decoder::new_buf` → `Decoder::ready` 取回可立即渲染的终端 → `IncrementalDecoder::next`
  逐条推进历史页与 Kitty 图形像素 → FINISH 后用 `Decoder::source_offset` 定位快照之后的活字节。
- 关键约束：上游解码器是**同步读取器**，把「暂时读不到数据」当致命 EOF，所以不能把网络流直接交给它；必须
  先在客户端把字节攒成完整记录。这一约束写在 `rexling-core/src/snapshot.rs` 的模块注释里，是本阶段最容易
  踩的坑。
- 另一条约束：PAGE 记录之后 Ghostty 会读超前，以便把 Kitty 放置挂到该页上；切分逻辑必须连带预读下一条记录
  头，否则解码器会因缺字节而报错。
- 判据：把一份真实服务端快照（含历史页与 Kitty 放置）灌进本地终端后，本地屏幕内容与服务端一致，且随后的活
  字节能无缝继续解析（快照恢复了 VT 解析器的续接状态）。
- 规范登记：采用上游快照解码面必须在 `upstream-alignment` 的「上游 API 取用判定登记」表补一行，写明取用与
  不取用的 API 面与依据。

### 输入、尺寸与所有权

- 输入：本仓输入字节经 `Bridge.ptyWriteQueue` 与 `InputBatchBuffer` 汇集后进 `NativeBridge.feedPty`。远程形态
  下这些字节不再写 PTY，而是作为 `com.superlogical.terminal.write` 的 `data` 参数发到控制面。
- 顺序：协议手册明确 `com.superlogical.terminal.write` 不保序，「连打两次输入须等第一次的响应再发第二次，或
  合成一次发」。客户端必须自己串行化并做批量合并；照搬 `rexling-core/src/control.rs` 的 `start_call`
  （只发不等）会丢序。
- 尺寸：本仓 `TerminalRuntime.resize` 与 `setPixelSize` 的目标从 ioctl 改为控制面 `resize`（参数名 `columns` 与
  `rows`）。服务端返回的是「谁要求的尺寸」与「当前生效尺寸」，两者不一定相同（见持有者规则），本地显示必须以
  生效尺寸为准。
- 所有权：进场先 `session.own_block`（`mode: "take"`），否则本客户端的尺寸请求在服务端可能只是排队；离开时
  `session.disown_block`，否则该 block 会被本客户端的旧尺寸与配色钉住。
- 配色：服务端的 `theme` APC 消息与本仓的固定主题列表（`DESIGN.md` 规定主题列表固定、构建期读取）冲突。
  必须裁定：远程会话用服务端配色，还是用本仓配色覆盖服务端。裁定前不得实现。

### 退出、错误与重连

- 退出：用户关闭会话时不再杀进程组，改为控制面 `block.close`；服务端确认由数据流 `child_exited` APC 消息给出
  退出码。`Session::take_reported_exit` 的语义保持「取一次」，远程实现侧把 APC 里的 `exit_code` 填进去。
- 错误：控制面错误按 `code` 分支（`not_found` 与 `session_not_found` 是资源失效，`conflict` 是状态冲突可重试，
  `revision_too_low` 要降修订，`too_large` 是负载过大），未知 `code` 按 `status` 处理。数据面错误按 REJECT 原因码
  与关闭状态码分别处理，`0x02`（已有连接）与 `0x03`（流不可用）语义完全不同，不可合并成「重连失败」。
- 重连：控制面用 `reconnect_token` 换连接；服务端保留 60 秒；**断线期间的事件会丢失且 `seq` 从 1 重来**，
  因此重连后必须重读状态再合并事件。数据面用 RESUME + 精确字节计数；计数必须只数客户端真正用掉的字节。
- 保活：客户端应按服务端同样的节奏（120 秒）发 ping，否则服务端会以 1001 关闭连接。
- 判据：断线重连后屏幕内容不丢、输入不重复、事件不重复应用；字节计数失配时按协议要求走「换新票据重开」，
  不做静默重试。

### 测试与门禁

- 本仓只测本项目功能，不测依赖行为（`docs/specification/TESTING.md`）。因此测试对象是**接缝与契约**：
  控制面请求配对与错误分支、`seq` 空洞的处理、快照记录切分在任意切分点下的正确性、APC 摘取与 nonce 不匹配时
  原样透传、RESUME 字节计数的精确性。
- 快照切分的期望值可来自上游的快照格式与协议手册正文（外部真相），不得以本仓实现自身产出作期望值。
- 网络侧用本地假服务器或协议探针做端到端验证；不得依赖真实外网。
- 门禁：沿用现有 `scripts/check.nu` 与 CI 流程，新增依赖须进 `native/Cargo.toml`（受保护文件，须同意），
  并通过 `cargo clippy --deny warnings`、`semgrep`、`markdownlint-cli2` 与 `openspec validate --all --strict`。

## 协议真值归属

协议真值必须落在 Rust 侧，理由与先例：

- 本仓既有分层是「终端状态、PTY、VT 线程与 GPU 绘制归属 Rust；Kotlin 持有会话表、设置与渲染线程调度」
  （`docs/specification/DESIGN.md`）。协议真值与终端状态同类，放 Kotlin 会造成第二次分层。
- `rexling-core/jni` 的 JNI 面只有 `create`／`command`／`poll`／`destroy`／`awaitUpdate`／`readUpdate` 等少数入口，
  `rexling-android` 的 Kotlin 侧只做展示与平台适配——这与本仓要求的方向一致。
- 反例是 `rexpocket`：它把协议与流状态放在 Kotlin 侧（`RexClient` 用 OkHttp WebSocket、`StreamDecoder` 自行解析
  `GHOSTSNP` 与 APC、`PocketSession` 维持会话状态）。本仓若照搬，等于在 Kotlin 侧再造一套协议状态机，且与
  「会话归 Rust」冲突。

分工建议：Rust 侧持有连接、请求配对、事件序号、快照缓冲与字节计数；Kotlin 侧只拿「输入字节」与「显示什么」
两类接口，不感知 `seq`、nonce、票据与 REJECT 原因码。

## 事实、推论与未知

### 事实

- Rex 的控制面与数据面是两条独立的 WebSocket 连接，子协议分别是 `rex.control.v1` 与 `rex.data.terminal.v1`。
- 数据面是单向的：字节只从服务端流向客户端；输入走控制面的 `com.superlogical.terminal.write`。
- 终端流以 Ghostty 的 `GHOSTSNP` 快照开头，之后是程序字节，其中夹带 `ESC _ rex1;<nonce>;<json> ESC \` 的
  `resize`／`theme`／`child_exited` 三类消息。
- 控制面事件 `seq` 连续无空洞；断线重连后 `seq` 从 1 重来，断线期间的事件丢失。
- 数据面续传按客户端已收字节计数，服务端保留 60 秒；计数不精确即被拒。
- `com.superlogical.terminal.write` 不保序；协议手册要求客户端自己串行或合并。
- 本仓 `Pty` trait 混合字节读写与本地 OS 能力；`Session::spawn_with_pty` 与 `PtyMaster::new` 是 `pub(super)`；
  `Session::request_exit` 走本地进程组信号路径。
- 本仓上游 `libghostty-vt` 已带快照解码面（`Decoder::new_buf`／`ready`／`IncrementalDecoder::next`），但本仓未消费。
- 上游快照解码器是同步读取器，把暂时读不到数据当致命 EOF，故网络流必须先攒成完整记录再喂。
- `DESIGN.md` 未声明远程／网络终端形态；`PROHIBITED.md` 明禁「会话数据持久化 / 恢复」；
  `upstream-alignment` 已把「远程会话」与「WebSocket／网络终端形态」判为「未声明，不立项」。

### 推论

- 支持 Rex 的必要前置是本仓的会话传输接缝，不是 WebSocket 依赖，也不是快照解码——后两者只在本仓先有远程会话
  形态之后才有意义。
- 若走「服务端渲染 surface」路线则与本仓冲突（Rex 本身不走这条路，但其姐妹项目 Herdr 走），Rex 路线可用
  「字节流 + 本地 VT」与本仓单一 VT 源并存。
- 快照恢复是本仓独有的低成本点：服务端快照格式与本仓上游同源，恢复屏幕不需要自研状态同步。
- 协议面真正的工作量在控制面（方法、事件、错误、修订协商），数据面反而是最规整的一段。
- 远程会话会引入本仓当前完全没有的失败面（断线、事件丢失、字节计数失配、票据过期），这些必须进测试与错误
  分级，不能静默重试。

### 未知

- 产品是否真的要「用手机连远端终端」这一形态，以及它与本仓现有本地形态是并存还是二选一。
- 本仓是否允许在客户端保存 reconnect token 与会话标识（直接决定是否撞 `PROHIBITED.md` 的会话持久化明禁）。
- 服务端配色与本仓固定主题列表的优先级裁定。
- 需要接入的 Rex 方法与事件子集（手册列了数十个方法与多类事件），以及目标服务端的 API revision 分布。
- 网络传输依赖的选型（WebSocket 客户端、TLS、SSH 隧道是否需要）在 `Cargo.toml` 受保护约束下的批准与否。
- 设备形态：手机侧只做「查看与输入」，还是也要支持多 block 布局切换（后者意味着把 Rex 的 session/window/block
  三层模型搬进本仓的会话列表，代价远大于只做单 block 客户端）。

## 不做清单

本报告不主张实现以下任何一项：

- Rex 服务端、Rex 协议扩展、协议 crate 发布或库化交付。
- 插件运行时、脚本加载、事件总线、多路复用（此前已判为未声明不立项）。
- 服务端权威渲染 surface 模式。
- 自研 VT 核、自研屏幕同步、自研快照格式解析（快照解析必须走上游 `libghostty-vt` 的解码面）。
- 把协议真值放到 Kotlin 侧。
- 在客户端持久化远端会话状态（除非维护者先划清 `PROHIBITED.md` 的边界）。
- 修改 `docs/specification/`、`Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`（本报告不触碰受保护文件）。

## 复现方式

- 本仓证据：`native/src/terminal/pty.rs` 的 `Pty` trait；`native/src/terminal/session/spawn.rs` 的 `Session::spawn`、
  `Session::spawn_with_pty` 与 `send_pty_output`；`native/src/terminal/session/pty_master.rs` 的 `PtyMaster::new`、
  `write_with_timeout`、`write_with_timeout_counted`；`native/src/terminal/session/lifecycle.rs` 的
  `Session::request_exit` 与 `take_reported_exit`；`native/src/terminal/session/mod.rs` 的 `Session::process_output`；
  `native/src/android/ffi/render/pump.rs`；`openspec/specs/pty-output-backpressure/spec.md`。
- 上游证据：本仓所钉 `libghostty-vt` 的 `crates/libghostty-vt/src/snapshot.rs`（`Terminal::encode_snapshot`、
  `Decoder::new_buf`、`Decoder::ready`、`IncrementalDecoder::next`、`Decoder::source_offset`）。
- 外部证据：`rexling-core/src/transport.rs`（`CONTROL`／`TERMINAL`／`MAX_MESSAGE` 与六种 scheme）、
  `rexling-core/src/control.rs`（hello 协商、`pending` 上限 32、事件队列上限 4096、`seq` 连续性检查、错误
  `code` 透传）、`rexling-core/src/data.rs`（票据、`parse_welcome`、REJECT 原因码、流块 1–65536 字节、
  `Resume`）、`rexling-core/src/snapshot.rs`（`SnapshotBuffer` 与「同步读取器视饥饿为致命 EOF」的模块注释）、
  `rexling-core/probe/evidence/server-wire-guide-r8.md`（协议手册正文）、`rexling-core/probe/src/main.rs`
  （端到端探针）、`rexling-core/jni/src/lib.rs`（JNI 面）、`rexpocket/core/src/main/kotlin/me/leaver/rexpocket/core/`
  下的 `RexClient.kt`、`StreamDecoder.kt`、`TerminalStream.kt`、`PocketSession.kt`。
