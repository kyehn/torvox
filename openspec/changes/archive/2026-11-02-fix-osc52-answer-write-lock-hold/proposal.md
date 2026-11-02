# Proposal

## Why

OSC 52 剪贴板**读取应答**的写入路径（`Session::answer_clipboard_read`）与逐帧收割
（`Session::drain_pty_write_back`）共享同一把主端写入锁 `pty_write_order`，也与用户输入
（`feedPty`）共享；但它走的写入入口与逐帧收割**不同**：

| 路径 | 写入入口 | 取主端写入锁 | 排空期限 |
| --- | --- | --- | --- |
| 逐帧收割 VT 应答（DECRPM/DSR/DA） | `write_with_timeout_counted` | `try_lock_for`，**有界** | 绝对预算，**不续期** |
| 用户输入（`feedPty`） | `write_with_timeout` | 无期限 `lock()` | 5s，且每有部分写入即续期 → 无上界 |
| **OSC 52 读取应答** | **`write_with_timeout`** | **无期限 `lock()`** | **50ms，但按进度续期 → 无上界** |

`pty::WRITE_BACK_TIMEOUT` 的文档明确写着「VT 应答回写（DECRPM/DSR/DA、**OSC 52
应答**）的排空期限……这两条在渲染线程逐帧收割，且全程持有会话锁」，即该期限本就把
OSC 52 应答算作「持会话锁的帧级路径」。而这条路径实际上取锁无期限、排空按进度续额，
与常量自身的约定相反。

后果是会话锁可被无上界地按住：用户输入写入（5s 期限 + 按进度续期）持住主端写入锁
时，一条 OSC 52 应答会**持着会话锁**等它释放。持有者：

- 洪泛削减分支在**渲染线程**上（`android/ffi/render/clipboard.rs`），
- 正常应答分支在应答线程上，但应答线程持的是**同一个会话锁**。

会话锁每帧都被渲染线程与看门狗取得，故一次应答就能把逐帧渲染按住数秒；渲染停滞
会让看门狗误判渲染线程已死并关掉会话。用户侧的表现是「远端程序一读剪贴板，终端就
卡住并被杀掉」。

同一形状的缺陷在逐帧收割那条路上**已经修过**（`write_back_yields_the_frame_when_a_user_write_holds_the_master_lock`
钉住），本次修复把同一条纪律补到 OSC 52 应答这条遗漏的路上。

## What Changes

- `Session::answer_clipboard_read`：写入入口由 `PtyMaster::write_with_timeout` 改为
  `PtyMaster::write_with_timeout_counted`——取锁与排空都受
  `pty::WRITE_BACK_TIMEOUT` 约束，且期限为绝对预算（不按进度续期）。
- 未写完的部分退回 `pty_write_back_queue`（与逐帧收割同一续写队列），由后续帧补齐：
  应答是请求-响应语义，截断在 base64 中间会让子应用永久等不到一个完整回答。
- 取锁失败（用户输入正占着主端）时整条退回队列并记调试日志：应答不丢，且写入只是
  被推迟而非失败。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `pty-lifecycle`：把「持会话锁的写入路径必须以墙钟为界」这条既有纪律的适用范围由
  逐帧收割扩到 OSC 52 应答——此前该纪律只落在逐帧收割这一条路上。

## 不做什么

- 不改 OSC 52 应答的协议形态（`ESC ] 52 ; <selection> ; <base64> BEL`）与应答线程的
  生命周期：应答仍由宿主答复后即时写出，只是不再无界等待。
- 不改洪泛削减每帧回写条数上限（`MAX_FLOOD_SHED_ANSWERS_PER_FRAME`）：那条上限针对
  的是「病态远端塞满读请求」，与本次的取锁无界是不同问题。
- 不把 OSC 52 应答挪出会话锁：应答需要会话的退出态与续写队列，写在锁外要另建一套
  所有权，收益（省下至多一个帧级期限的持锁）不抵复杂度。
