# Design

## 研究坐标

| 对象 | 坐标 | 用途 |
| --- | --- | --- |
| `shulhi/rexling` | master 快照（无发布标签） | 协议实现（`rexling-core`）、协议手册正文（`rexling-core/probe/evidence/server-wire-guide-r8.md`，API revision 8）、端到端探针（`rexling-core/probe/src/main.rs`） |
| `leizhiyuan/rexpocket` | `v0.1.3` | 反例参照：协议状态放 Kotlin 侧（`RexClient.kt`、`StreamDecoder.kt`、`TerminalStream.kt`、`PocketSession.kt`） |
| 本仓 | 工作树 | 接缝现状（`native/src/terminal/pty.rs`、`native/src/terminal/session/**`）与上游快照面（本仓所钉 `libghostty-vt` 的 `crates/libghostty-vt/src/snapshot.rs`） |

本仓约定文档不写 git 提交号，故坐标用标签或快照标识。

## 取舍记录

- **只做客户端，不做服务端**：Rex 服务端运行终端并让它们持续运行，客户端只是连接上去显示与控制。本仓是
  Android 终端模拟器，做服务端不现实；报告的方法步骤按「只做客户端」设计。
- **走字节流路线，不走服务端渲染 surface 路线**：本仓终端状态单一来源在本机，若接服务端渲染 surface 则与本仓
  自带的 Ghostty 模拟器重复。Rex 数据面本身送的就是终端字节，与本仓分层相容。
- **快照走上游解码面，不自研**：服务端快照是 Ghostty 自己的 `GHOSTSNP` 格式，本仓所钉 `libghostty-vt` 已有
  `Decoder::new_buf`／`ready`／`IncrementalDecoder::next`。自研快照解析会形成第二份真相，违单一来源。
- **协议真值放 Rust 侧**：与本仓「终端状态、PTY、VT 线程与 GPU 绘制归属 Rust」一致；`rexling-core/jni` 的少数
  JNI 入口是同向先例，`rexpocket` 把协议状态放 Kotlin 侧是反向先例。
- **传输依赖暂不引入**：新增 WebSocket／TLS 依赖须改 `native/Cargo.toml`，属受保护文件，未获明确同意前不引入。
- **维持「不立项」**：能力面未在 `docs/specification/` 声明，`STYLE.md` 禁止实现未声明功能；
  不立项」。本变更只交付评估与方法步骤，不交付实现。

## 风险与边界

- 与 `PROHIBITED.md`「会话数据持久化 / 恢复」的边界：Rex 数据面 resume 是「服务端保留 60 秒 + 客户端按已收
  字节计数续传」，不属该明禁项；但客户端若保存 reconnect token 与远端会话标识以支持应用被杀后接回同一会话，
  则撞明禁，须由维护者在声明阶段划清。
- 服务端与客户端各有一套终端实现，颜色、尺寸、模式（alternate screen、kitty 图形）若不对齐会出现两套状态打架；
  本仓固定主题列表与远端服务端配色的优先级亦须裁定。
- 快照解码的上游约束：上游解码器是同步读取器，把暂时读不到数据当致命 EOF，故网络流必须先在客户端攒成完整
  记录再喂；PAGE 记录后还要预读下一条记录头，否则解码器缺字节报错。

## 方法步骤概览

报告正文的六个阶段：规范声明 → 会话传输面接缝 → Rex 协议层（控制面／数据面）→ 快照恢复 → 输入与尺寸与所有权
→ 退出、错误与重连 → 测试与门禁。每一阶段给出改动面、判据与阻塞点，未获维护者同意前不进入实现。
